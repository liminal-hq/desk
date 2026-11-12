// Tests of the command line: the exit statuses, the files written and the summary on standard output.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_steps-junit"))
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn out(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli").join(name)
}

#[test]
fn it_writes_the_report_and_the_summary() {
    let junit = out("a/b/steps.xml");
    let _ = std::fs::remove_file(&junit);
    let o = bin()
        .args([
            "--times",
            &fixture("pass.times"),
            "--log",
            &fixture("pass.log"),
            "--suite",
            "Base",
            "--summary",
            "--junit",
        ])
        .arg(&junit)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    let xml = std::fs::read_to_string(&junit).unwrap();
    assert!(xml.contains("<testsuite name=\"Base\" tests=\"3\""), "{xml}");
    let summary = String::from_utf8(o.stdout).unwrap();
    assert!(summary.starts_with("## Base\n"), "{summary}");
}

#[test]
fn missing_inputs_still_write_a_report_and_exit_zero() {
    let junit = out("missing.xml");
    let _ = std::fs::remove_file(&junit);
    let status = bin()
        .args([
            "--times",
            "/nonexistent/check.times",
            "--log",
            "/nonexistent/check.log",
            "--suite",
            "Base",
            "--junit",
        ])
        .arg(&junit)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    assert!(std::fs::read_to_string(&junit)
        .unwrap()
        .contains("<error message=\"no step times were written"));
}

#[test]
fn a_usage_mistake_exits_two() {
    for args in [
        vec![],
        vec!["--times"],
        vec!["--nope"],
        vec!["--times", "a", "--log", "b", "--suite", "c"],
    ] {
        let o = bin().args(&args).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&o.stderr).contains("usage:"), "{args:?}");
    }
}

#[test]
fn an_unwritable_report_exits_one() {
    let o = bin()
        .args([
            "--times",
            &fixture("pass.times"),
            "--log",
            &fixture("pass.log"),
            "--suite",
            "Base",
            "--junit",
            env!("CARGO_MANIFEST_DIR"),
        ])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
}
