#![forbid(unsafe_code)]

use revcheck::analyze;
use serde_json::json;

#[test]
fn name_forms_and_shapes_are_classified() {
    let cases = [
        (json!({"commitSHA": "abc"}), "opaque"),
        (json!({"corpus-version": 7}), "integer"),
        (json!({"sha": "a".repeat(64)}), "hex64"),
    ];
    for (value, shape) in cases {
        let report = analyze(&value);
        assert_eq!(report.qualifiers.len(), 1, "{value}");
        assert_eq!(report.qualifiers[0].shape, shape);
    }
}

#[test]
fn all_disallowed_value_shapes_are_near_misses() {
    let value = json!({
        "revision": null,
        "rev": "",
        "commit": true,
        "sha": 1.5,
        "etag": [],
        "version": {}
    });
    let report = analyze(&value);
    assert!(report.qualifiers.is_empty());
    assert_eq!(report.near_misses.len(), 6);
    assert!(report
        .near_misses
        .iter()
        .all(|miss| miss.reason.starts_with("wrong type:")));
}

#[test]
fn result_items_never_supply_envelope_revision() {
    let report = analyze(&json!({"results": [{"revision": "r1"}]}));
    assert!(report.qualifiers.is_empty());
    assert_eq!(report.near_misses[0].reason, "per-item scope");
}

#[test]
fn iso_calendar_dates_are_rejected_but_other_opaque_values_qualify() {
    for date in ["2026-09-29", "20260929", "2026-09-29T11:00:00-05:00"] {
        let report = analyze(&json!({"asOf": date}));
        assert!(report.qualifiers.is_empty(), "{date}");
        assert!(report.near_misses[0].reason.contains("ISO-8601"));
    }
    assert_eq!(analyze(&json!({"as_of": "release-29"})).qualifiers.len(), 1);
}

/// Regression: envelope scanning must be fully recursive. An earlier traversal only
/// looked one level inside a top-level object, so a revision at `$.meta.index.revision`
/// was reported as absent with no near-miss at all. A silent false negative is the worst
/// failure this tool can have: it is what lets someone state in public that a system
/// lacks a property it actually has.
#[test]
fn envelope_revision_is_found_at_any_depth() {
    for value in [
        json!({"revision": "a".repeat(40)}),
        json!({"meta": {"revision": "a".repeat(40)}}),
        json!({"meta": {"index": {"revision": "a".repeat(40)}}}),
        json!({"a": {"b": {"c": {"d": {"revision": "a".repeat(40)}}}}}),
    ] {
        let report = analyze(&value);
        assert_eq!(report.qualifiers.len(), 1, "missed a revision in {value}");
        assert_eq!(report.qualifiers[0].shape, "hex40");
    }
}

/// Per-item scope must survive arbitrary nesting beneath a collection. A revision buried
/// in `results[0].meta.revision` describes one result, never the store.
#[test]
fn per_item_scope_propagates_through_nesting() {
    let value = json!({"results": [{"meta": {"deep": {"revision": "a".repeat(40)}}}]});
    let report = analyze(&value);
    assert!(report.qualifiers.is_empty());
    assert!(
        report
            .near_misses
            .iter()
            .any(|miss| miss.reason == "per-item scope"),
        "expected a per-item near-miss, got {:?}",
        report.near_misses
    );
}

/// A payload that is not an object has no envelope, and the reason must be stated
/// rather than reported as a bare absence.
#[test]
fn non_object_payload_explains_itself() {
    for value in [json!([{"revision": "a".repeat(40)}]), json!("x"), json!(7)] {
        let report = analyze(&value);
        assert!(report.qualifiers.is_empty());
        assert_eq!(report.near_misses.len(), 1, "{value}");
        assert!(report.near_misses[0].reason.contains("no envelope"));
    }
}

/// Deep nesting must not crash the real path. Input always arrives as text, and
/// `serde_json` enforces its own recursion limit there, so a hostile payload is refused
/// at parse time rather than being walked. `analyze`'s own depth cap is the second belt.
#[test]
fn pathological_nesting_is_refused_at_parse_time() {
    let deep = format!(
        "{}{}{}",
        "{\"n\":".repeat(5_000),
        "{\"revision\":\"x\"}",
        "}".repeat(5_000)
    );
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&deep);
    assert!(
        parsed.is_err(),
        "serde_json should refuse recursion this deep rather than overflow"
    );
}

use revcheck::{analyze_headers, split_http};

fn headers(raw: &[(&str, &str)]) -> Vec<(String, String)> {
    raw.iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// Regression: `normalize` splits camelCase, so it turned `ETag` into `e_tag` and missed
/// the most widely deployed revision identifier in existence. HTTP field names are
/// case-insensitive and hyphenated, never camelCase, and need their own normalisation.
#[test]
fn etag_qualifies_in_every_spelling() {
    for spelling in ["ETag", "etag", "Etag", "ETAG"] {
        let report = analyze_headers(&headers(&[(spelling, &format!("\"{}\"", "a".repeat(40)))]));
        assert_eq!(report.qualifiers.len(), 1, "missed {spelling}");
        assert_eq!(report.qualifiers[0].shape, "hex40");
    }
}

/// RFC 9110: a weak validator asserts semantic equivalence, not one immutable state, so
/// it is a perfectly good cache validator and still cannot satisfy R1.
#[test]
fn weak_etag_is_a_near_miss_not_a_pass() {
    let report = analyze_headers(&headers(&[("ETag", "W/\"abc123\"")]));
    assert!(report.qualifiers.is_empty());
    assert!(report.near_misses[0].reason.contains("weak validator"));
}

#[test]
fn header_names_follow_the_same_name_rules() {
    // Wall-clock headers are refused by name, exactly as in a body.
    let report = analyze_headers(&headers(&[(
        "Last-Modified",
        "Tue, 29 Sep 2026 17:00:00 GMT",
    )]));
    assert!(report.qualifiers.is_empty());
    assert!(report.near_misses[0].reason.contains("wall-clock"));

    // The conventional vendor prefix is stripped before matching.
    let report = analyze_headers(&headers(&[("X-Revision", "4711")]));
    assert_eq!(report.qualifiers.len(), 1);

    // An unrelated header is simply ignored, not reported as a near-miss.
    let report = analyze_headers(&headers(&[("Content-Type", "application/json")]));
    assert!(report.qualifiers.is_empty() && report.near_misses.is_empty());
}

#[test]
fn split_http_only_claims_real_responses() {
    assert!(split_http("{\"results\":[]}").is_none());
    let (found, body) = split_http("HTTP/2 200\r\nETag: \"x\"\r\n\r\n{\"a\":1}").unwrap();
    assert_eq!(found, headers(&[("ETag", "\"x\"")]));
    assert_eq!(body, "{\"a\":1}");
    // Bare LF separators, as some tools emit.
    let (found, body) = split_http("HTTP/1.1 200 OK\nETag: \"x\"\n\n{}").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(body, "{}");
}

/// Regression: an object keyed by id is still a collection. Removing the original
/// `results`-as-object special case while making the walk recursive let a per-item
/// revision in `{"results": {"id1": {...}}}` be read as envelope scope and wrongly
/// satisfy R1 — a false positive, which is the tool certifying what it cannot see.
#[test]
fn object_keyed_collections_are_still_per_item() {
    let value = json!({"results": {"id1": {"revision": "a".repeat(40)}}});
    let report = analyze(&value);
    assert!(
        report.qualifiers.is_empty(),
        "certified a per-item revision"
    );

    // The mirror risk: a wrapper object must NOT be mistaken for a collection.
    let wrapped = json!({"data": {"revision": "a".repeat(40)}});
    assert_eq!(
        analyze(&wrapped).qualifiers.len(),
        1,
        "lost a real revision"
    );
}
