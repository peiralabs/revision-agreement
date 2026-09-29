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
