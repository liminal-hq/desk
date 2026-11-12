// steps-junit: converts the step times and log of one `tools/check.sh` run into JUnit XML, and a Markdown table for the job summary.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use steps_junit::convert;

const USAGE: &str =
    "usage: steps-junit --times FILE --log FILE --suite NAME --junit FILE [--summary]\n  --summary writes a Markdown table to standard output";

fn main() -> ExitCode {
    let mut times = None;
    let mut log = None;
    let mut suite = None;
    let mut junit = None;
    let mut summary = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "--times" => &mut times,
            "--log" => &mut log,
            "--suite" => &mut suite,
            "--junit" => &mut junit,
            "--summary" => {
                summary = true;
                continue;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => return usage(&format!("unknown argument '{other}'")),
        };
        match args.next() {
            Some(value) => *slot = Some(value),
            None => return usage(&format!("{arg} needs a value")),
        }
    }
    let (Some(times), Some(log), Some(suite), Some(junit)) = (times, log, suite, junit) else {
        return usage("--times, --log, --suite and --junit are all needed");
    };

    // A missing file is not an error here: the report says so in a test case, and the job's own result comes from the check step.
    let report = convert(
        &suite,
        fs::read(&times)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .as_deref(),
        fs::read(&log).ok().as_deref(),
    );
    if let Some(dir) = Path::new(&junit).parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("steps-junit: cannot create {}: {e}", dir.display());
            return ExitCode::from(1);
        }
    }
    if let Err(e) = fs::write(&junit, report.to_junit()) {
        eprintln!("steps-junit: cannot write {junit}: {e}");
        return ExitCode::from(1);
    }
    if summary {
        print!("{}", report.to_summary());
    }
    ExitCode::SUCCESS
}

fn usage(why: &str) -> ExitCode {
    eprintln!("steps-junit: {why}\n{USAGE}");
    ExitCode::from(2)
}
