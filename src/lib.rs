#![forbid(unsafe_code)]

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Qualifier {
    pub path: String,
    pub value: Value,
    pub shape: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearMiss {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub qualifiers: Vec<Qualifier>,
    pub near_misses: Vec<NearMiss>,
}

const ALLOWED: &str = "revision rev commit commit_sha sha etag version corpus_version \
    index_version snapshot snapshot_id generation seq sequence as_of checkpoint tree_size log_index";
const TIMES: &str =
    "timestamp time date datetime updated_at created_at modified_at last_modified mtime";

/// Maximum envelope depth walked. Guards against hostile or pathological nesting;
/// no realistic response envelope is anywhere near this deep.
const MAX_DEPTH: usize = 64;

pub fn analyze(value: &Value) -> Report {
    let mut report = Report::default();
    if !value.is_object() {
        report.near_misses.push(NearMiss {
            path: "$".into(),
            reason: "response is not a JSON object, so it carries no envelope".into(),
        });
        return report;
    }
    walk(value, "$", false, 0, &mut report);
    report
}

/// Walks the whole payload. Anything reached through an array is a member of a
/// results collection and is therefore per-item scope, which can never satisfy R1;
/// everything else is envelope scope, at any depth.
fn walk(value: &Value, path: &str, per_item: bool, depth: usize, report: &mut Report) {
    if depth >= MAX_DEPTH {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = child_path(path, key);
                inspect(key, child, &child_path, per_item, report);
                walk(
                    child,
                    &child_path,
                    per_item || child.is_array(),
                    depth + 1,
                    report,
                );
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                walk(item, &format!("{path}[{index}]"), true, depth + 1, report);
            }
        }
        _ => {}
    }
}

fn inspect(key: &str, value: &Value, path: &str, per_item: bool, report: &mut Report) {
    let name = normalize(key);
    if TIMES.split_ascii_whitespace().any(|item| item == name) {
        report.near_misses.push(NearMiss {
            path: path.into(),
            reason: "rejected by name (wall-clock field)".into(),
        });
        return;
    }
    let allowed = ALLOWED.split_ascii_whitespace().any(|item| item == name);
    if per_item && (allowed || name == "hash") {
        report.near_misses.push(NearMiss {
            path: path.into(),
            reason: "per-item scope".into(),
        });
        return;
    }
    if !allowed {
        return;
    }
    let shape = match value {
        Value::String(text) if text.is_empty() => return wrong_type(report, path, "empty string"),
        Value::String(text) if is_iso8601(text) => {
            report.near_misses.push(NearMiss {
                path: path.into(),
                reason: "ISO-8601 date/time value".into(),
            });
            return;
        }
        Value::String(text) if is_hex(text, 40) => "hex40",
        Value::String(text) if is_hex(text, 64) => "hex64",
        Value::String(_) => "opaque",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Null => return wrong_type(report, path, "null"),
        Value::Bool(_) => return wrong_type(report, path, "boolean"),
        Value::Number(_) => return wrong_type(report, path, "float"),
        Value::Array(_) => return wrong_type(report, path, "array"),
        Value::Object(_) => return wrong_type(report, path, "object"),
    };
    report.qualifiers.push(Qualifier {
        path: path.into(),
        value: value.clone(),
        shape,
    });
}

fn wrong_type(report: &mut Report, path: &str, kind: &str) {
    report.near_misses.push(NearMiss {
        path: path.into(),
        reason: format!("wrong type: {kind}"),
    });
}

fn child_path(parent: &str, key: &str) -> String {
    if key.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic())
        && key.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
    {
        format!("{parent}.{key}")
    } else {
        format!(
            "{parent}[{}]",
            serde_json::to_string(key).unwrap_or_default()
        )
    }
}

fn normalize(name: &str) -> String {
    let chars: Vec<_> = name.chars().collect();
    let mut out = String::new();
    for (index, &ch) in chars.iter().enumerate() {
        if ch == '_' || ch == '-' {
            if !out.ends_with('_') && !out.is_empty() {
                out.push('_');
            }
        } else {
            let previous = index.checked_sub(1).and_then(|i| chars.get(i));
            let next = chars.get(index + 1);
            if ch.is_ascii_uppercase()
                && ((!out.ends_with('_')
                    && previous.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
                    || (previous.is_some_and(|c| c.is_ascii_uppercase())
                        && next.is_some_and(|c| c.is_ascii_lowercase())))
            {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        }
    }
    out.trim_matches('_').into()
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_iso8601(value: &str) -> bool {
    let bytes = value.as_bytes();
    let date_len = if valid_date(bytes, true) {
        10
    } else if valid_date(bytes, false) {
        8
    } else {
        return false;
    };
    if bytes.len() == date_len {
        return true;
    }
    bytes
        .get(date_len)
        .is_some_and(|c| *c == b'T' || *c == b't')
        && valid_time(&value[date_len + 1..])
}

fn valid_date(value: &[u8], extended: bool) -> bool {
    let (month_at, day_at, len) = if extended { (5, 8, 10) } else { (4, 6, 8) };
    if value.len() < len
        || extended && (value[4] != b'-' || value[7] != b'-')
        || !value[..4].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let number = |at| (value[at] - b'0') * 10 + value[at + 1] - b'0';
    if !value[month_at..month_at + 2].iter().all(u8::is_ascii_digit)
        || !value[day_at..day_at + 2].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let year = value[..4]
        .iter()
        .fold(0_u16, |n, c| n * 10 + u16::from(c - b'0'));
    let month = number(month_at);
    let day = number(day_at);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => 28 + u8::from(leap),
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    day > 0 && day <= days
}

fn valid_time(value: &str) -> bool {
    let value = value.strip_suffix(['Z', 'z']).unwrap_or(value);
    let zone_at = value.find(['+', '-']);
    let (time, zone) = zone_at.map_or((value, None), |at| (&value[..at], Some(&value[at + 1..])));
    if zone.is_some_and(|z| !valid_clock(z, true)) {
        return false;
    }
    let clock = time.split(['.', ',']).next().unwrap_or("");
    let suffix = time.strip_prefix(clock).unwrap_or("");
    let fraction_ok =
        suffix.is_empty() || suffix.len() > 1 && suffix[1..].bytes().all(|b| b.is_ascii_digit());
    fraction_ok && valid_clock(clock, false)
}

fn valid_clock(value: &str, zone: bool) -> bool {
    let digits: String = value.chars().filter(|c| *c != ':').collect();
    let colons_ok = !value.contains(':') || value.as_bytes().get(2) == Some(&b':');
    let length_ok = if zone {
        digits.len() == 4
    } else {
        matches!(digits.len(), 4 | 6)
    };
    if !colons_ok || !length_ok || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let part = |at| digits[at..at + 2].parse::<u8>().unwrap_or(255);
    part(0) <= 23 && part(2) <= 59 && (digits.len() == 4 || part(4) <= 60)
}
