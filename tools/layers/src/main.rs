// layers: reads the tracked files of the repository it is run in and fails when a file names a file in a layer it may not use.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::process::{Command, ExitCode};

use layers::{check, edges, Config, Tree};

const CONFIG: &str = "tools/layers/layers.txt";

fn run() -> Result<Vec<String>, String> {
    let config_text = fs::read_to_string(CONFIG).map_err(|e| format!("{CONFIG}: {e} (run from the repository root)"))?;
    let config = Config::parse(&config_text).map_err(|e| format!("{CONFIG}: {e}"))?;
    let out = Command::new("git")
        .args(["ls-files", "-z"])
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !out.status.success() {
        return Err("git ls-files failed".to_string());
    }
    let mut files = BTreeMap::new();
    for path in String::from_utf8_lossy(&out.stdout).split('\0').filter(|p| !p.is_empty()) {
        // Only the files an edge can come from are read; the others are listed so a target can be found. A file that is not text
        // (or has been removed from the working tree) has no edges.
        let text = if Tree::is_read(path) {
            fs::read_to_string(path).unwrap_or_default()
        } else {
            String::new()
        };
        files.insert(path.to_string(), text);
    }
    let tree = Tree { files };
    let found = edges(&tree);
    let problems = check(&config, &tree, &found);
    if problems.is_empty() {
        println!("layers: ok ({} files, {} edges)", tree.files.len(), found.len());
    }
    Ok(problems)
}

fn main() -> ExitCode {
    match run() {
        Ok(problems) if problems.is_empty() => ExitCode::SUCCESS,
        Ok(problems) => {
            for p in &problems {
                eprintln!("{p}");
            }
            eprintln!("layers: {} problem(s); see docs/LAYERS.md", problems.len());
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("layers: {e}");
            ExitCode::FAILURE
        }
    }
}
