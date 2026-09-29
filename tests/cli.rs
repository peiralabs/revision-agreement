#![forbid(unsafe_code)]

use serde_json::Value;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_revcheck"))
        .args(args)
        .output()
        .expect("revcheck should run")
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
}

#[test]
fn per_item_hash_is_a_scope_near_miss() {
    let output = run(&["--json", &fixture("memory-results.json")]);
    assert_eq!(output.status.code(), Some(1));
    let verdict = json(&output);
    assert_eq!(verdict["level"], 0);
    assert!(verdict["near_misses"]
        .as_array()
        .unwrap()
        .iter()
        .any(|miss| { miss["path"] == "$.results[0].hash" && miss["reason"] == "per-item scope" }));
}

#[test]
fn timestamp_name_is_reported_but_never_qualifies() {
    let output = run(&["--json", &fixture("timestamped-results.json")]);
    assert_eq!(output.status.code(), Some(1));
    let verdict = json(&output);
    assert_eq!(verdict["level"], 0);
    assert!(verdict["near_misses"]
        .as_array()
        .unwrap()
        .iter()
        .any(|miss| {
            miss["path"] == "$.results[0].updated_at"
                && miss["reason"]
                    .as_str()
                    .unwrap()
                    .contains("rejected by name")
        }));
}

#[test]
fn unrelated_envelope_has_no_near_misses() {
    let output = run(&["--json", &fixture("dataset-result.json")]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json(&output)["near_misses"], serde_json::json!([]));
}

#[test]
fn top_level_hex_and_nested_integer_qualify() {
    let hex = run(&["--json", &fixture("conforming-hex40.json")]);
    assert!(hex.status.success());
    assert_eq!(json(&hex)["shape"], "hex40");
    assert_eq!(json(&hex)["path"], "$.revision");

    let integer = run(&["--json", &fixture("conforming-meta-integer.json")]);
    assert!(integer.status.success());
    assert_eq!(json(&integer)["shape"], "integer");
    assert_eq!(json(&integer)["path"], "$.meta.corpus_version");
}

#[test]
fn iso_value_is_rejected() {
    let output = run(&["--json", &fixture("iso-version.json")]);
    assert_eq!(output.status.code(), Some(1));
    let verdict = json(&output);
    assert_eq!(verdict["level"], 0);
    assert!(verdict["near_misses"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("ISO-8601"));
}

#[test]
fn malformed_json_is_exit_two() {
    let output = run(&[&fixture("malformed.json")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid JSON"));
}

#[test]
fn advance_changed_unchanged_and_nonconforming() {
    let base = fixture("conforming-hex40.json");
    let changed = run(&["advance", &base, &fixture("conforming-hex40-changed.json")]);
    assert!(changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stdout).starts_with("changed\n"));

    let unchanged = run(&["advance", &base, &base]);
    assert_eq!(unchanged.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unchanged.stdout).starts_with("unchanged\n"));

    let rejected = run(&["advance", &base, &fixture("dataset-result.json")]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&rejected.stdout).contains("B is level 0"));
}
