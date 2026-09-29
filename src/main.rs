#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use revcheck::{analyze, NearMiss};
use serde_json::{json, Value};
use std::{fs, io::Read, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    version,
    about = "Test a JSON read response for SPEC.md R1 revision attribution",
    long_about = "Tests R1 and revision advance only. R2 and R3 require system-specific write, agreement, and consumer operations and cannot be tested generically from a response body. A pass establishes level 1 only; it does not imply levels 2-4."
)]
struct Cli {
    /// Emit a machine-readable verdict
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
    /// JSON file to inspect; omit or use - for stdin
    file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Assert that two attributed payloads report different revisions at a common path
    Advance { a: PathBuf, b: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Command::Advance { a, b }) => run_advance(a, b, cli.json),
        None => read(cli.file.as_deref()).map(|value| run_check(&value, cli.json)),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("revcheck: {message}");
            ExitCode::from(2)
        }
    }
}

fn read(path: Option<&std::path::Path>) -> Result<Value, String> {
    let mut input = String::new();
    if path.is_none() || path.is_some_and(|path| path == std::path::Path::new("-")) {
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|error| format!("cannot read stdin: {error}"))?;
    } else if let Some(path) = path {
        input = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    }
    serde_json::from_str(&input).map_err(|error| format!("invalid JSON: {error}"))
}

fn run_check(value: &Value, machine: bool) -> u8 {
    let report = analyze(value);
    let qualifier = report.qualifiers.first();
    if machine {
        let near_misses: Vec<_> = report.near_misses.iter().map(near_json).collect();
        println!(
            "{}",
            json!({
                "level": u8::from(qualifier.is_some()),
                "label": if qualifier.is_some() { "Attributed" } else { "Unverified" },
                "satisfied": qualifier.is_some(),
                "path": qualifier.map(|q| &q.path),
                "shape": qualifier.map(|q| q.shape),
                "value": qualifier.map(|q| &q.value),
                "near_misses": near_misses,
            })
        );
    } else if let Some(qualifier) = qualifier {
        println!("level 1 (Attributed)");
        println!("path: {}", qualifier.path);
        println!("shape: {}", qualifier.shape);
        println!("value: {}", qualifier.value);
    } else {
        println!("level 0 (Unverified)");
        print_misses(&report.near_misses);
    }
    u8::from(qualifier.is_none())
}

fn run_advance(a: PathBuf, b: PathBuf, machine: bool) -> Result<u8, String> {
    let a_report = analyze(&read(Some(&a))?);
    let b_report = analyze(&read(Some(&b))?);
    if a_report.qualifiers.is_empty() || b_report.qualifiers.is_empty() {
        if machine {
            println!(
                "{}",
                json!({"satisfied": false, "error": "both payloads must independently satisfy R1"})
            );
        } else {
            println!("advance not satisfied: both payloads must independently satisfy R1");
            if a_report.qualifiers.is_empty() {
                println!("A is level 0 (Unverified)");
                print_misses(&a_report.near_misses);
            }
            if b_report.qualifiers.is_empty() {
                println!("B is level 0 (Unverified)");
                print_misses(&b_report.near_misses);
            }
        }
        return Ok(1);
    }
    let pair = a_report.qualifiers.iter().find_map(|left| {
        b_report
            .qualifiers
            .iter()
            .find(|right| right.path == left.path)
            .map(|right| (left, right))
    });
    let Some((left, right)) = pair else {
        let message = "payloads satisfy R1 but have no qualifying revision at the same JSON path";
        if machine {
            println!("{}", json!({"satisfied": false, "error": message}));
        } else {
            println!("advance not satisfied: {message}");
        }
        return Ok(1);
    };
    let changed = left.value != right.value;
    if machine {
        println!(
            "{}",
            json!({"satisfied": changed, "result": if changed { "changed" } else { "unchanged" }, "path": left.path, "a": left.value, "b": right.value})
        );
    } else {
        println!("{}", if changed { "changed" } else { "unchanged" });
        println!("path: {}", left.path);
    }
    Ok(u8::from(!changed))
}

fn near_json(miss: &NearMiss) -> Value {
    json!({"path": miss.path, "reason": miss.reason})
}

fn print_misses(misses: &[NearMiss]) {
    if misses.is_empty() {
        println!("near-misses: none");
    } else {
        println!("near-misses:");
        for miss in misses {
            println!("- {}: {}", miss.path, miss.reason);
        }
    }
}
