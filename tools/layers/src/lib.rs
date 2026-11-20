// layers: the rules file, the edges between files and the check that every edge points down. See docs/LAYERS.md.
//
// The edges are the same ones tools/ci-scope.sh's closure() reads, so the two are kept in step: Cargo path dependencies, the target
// specification and linker script a `.cargo/config.toml` names, files compiled in with include_bytes!, include_str! and include!, and
// modules named by #[path]. Edges into dist/ and vendor/ are build outputs and tools, not source, and are not edges.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

/// The tracked files of a repository: each path, and the text of the files an edge can come from (empty for the others).
pub struct Tree {
    pub files: BTreeMap<String, String>,
}

impl Tree {
    /// Whether the text of `path` is read for edges.
    pub fn is_read(path: &str) -> bool {
        path.ends_with(".rs") || path.ends_with("/Cargo.toml") || path.ends_with("/.cargo/config.toml")
    }
}

struct Allow {
    from: String,
    to: String,
    reason: String,
}

/// The layers, what each may use and the recorded exceptions, from tools/layers/layers.txt.
pub struct Config {
    layers: Vec<(String, String)>,
    may_use: BTreeMap<String, Vec<String>>,
    allows: Vec<Allow>,
}

impl Config {
    /// Parses the rules file. `layer NAME PREFIX...` puts the paths under each prefix in a layer (the longest prefix wins),
    /// `may-use NAME LAYER...` (or `*`) lists what a layer may name besides itself, and `allow FROM TO REASON...` records one
    /// exception for the edges from a path under FROM to a path under TO. Lines starting with `#` and blank lines are skipped.
    pub fn parse(text: &str) -> Result<Config, String> {
        let mut layers: Vec<(String, String)> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut may_use: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut allows = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = |msg: &str| format!("line {}: {msg}", n + 1);
            let words: Vec<&str> = line.split_whitespace().collect();
            match words[0] {
                "layer" if words.len() >= 3 => {
                    if !names.iter().any(|x| x == words[1]) {
                        names.push(words[1].to_string());
                    }
                    for prefix in &words[2..] {
                        let prefix = prefix.trim_end_matches('/');
                        if layers.iter().any(|(p, _)| p == prefix) {
                            return Err(at(&format!("the prefix {prefix} is in two layers")));
                        }
                        layers.push((prefix.to_string(), words[1].to_string()));
                    }
                }
                "may-use" if words.len() >= 3 => {
                    may_use
                        .entry(words[1].to_string())
                        .or_default()
                        .extend(words[2..].iter().map(|w| w.to_string()));
                }
                "allow" if words.len() >= 4 => allows.push(Allow {
                    from: words[1].trim_end_matches('/').to_string(),
                    to: words[2].trim_end_matches('/').to_string(),
                    reason: words[3..].join(" "),
                }),
                "layer" | "may-use" | "allow" => {
                    return Err(at(&format!("`{}` is missing words (allow needs a reason)", words[0])))
                }
                other => return Err(at(&format!("unknown directive `{other}`"))),
            }
        }
        for (name, used) in &may_use {
            if !names.contains(name) {
                return Err(format!("may-use names the undeclared layer {name}"));
            }
            for u in used {
                if u != "*" && !names.contains(u) {
                    return Err(format!("{name} may use the undeclared layer {u}"));
                }
            }
        }
        Ok(Config { layers, may_use, allows })
    }

    /// The layer of `path`: that of the longest layer prefix that is `path` or a directory above it.
    pub fn layer_of(&self, path: &str) -> Option<&str> {
        self.layers
            .iter()
            .filter(|(prefix, _)| under(path, prefix))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, name)| name.as_str())
    }

    fn may(&self, from: &str, to: &str) -> bool {
        from == to || self.may_use.get(from).is_some_and(|u| u.iter().any(|x| x == "*" || x == to))
    }
}

fn under(path: &str, prefix: &str) -> bool {
    path == prefix || (path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/'))
}

/// One file naming another.
#[derive(Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: &'static str,
}

/// `rel` resolved against the directory `dir`, as a path from the repository root; None when it climbs out of the repository.
pub fn resolve(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for p in rel.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// The quoted values that follow `key = ` in `line`, where `key` is a whole word.
fn quoted_values(line: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in line.match_indices(key) {
        let before = line[..i].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            continue;
        }
        let rest = line[i + key.len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
        }
    }
    out
}

fn cargo_edges(path: &str, text: &str, tree: &Tree, out: &mut Vec<Edge>) {
    let mut in_deps = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            let name = t.trim_matches(|c| c == '[' || c == ']').trim();
            in_deps = name.ends_with("dependencies") || name.contains("dependencies.");
            continue;
        }
        if !in_deps {
            continue;
        }
        for rel in quoted_values(t, "path") {
            if let Some(dep) = resolve(dir_of(path), &rel) {
                if tree.files.contains_key(&format!("{dep}/Cargo.toml")) {
                    out.push(Edge {
                        from: path.to_string(),
                        to: dep,
                        kind: "path dependency",
                    });
                }
            }
        }
    }
}

fn config_edges(path: &str, text: &str, tree: &Tree, out: &mut Vec<Edge>) {
    // The workspace a `.cargo/config.toml` belongs to is the directory above `.cargo`.
    let workspace = dir_of(dir_of(path));
    for line in text.lines() {
        let t = line.trim();
        let mut names: Vec<String> = Vec::new();
        if t.starts_with("target") {
            names.extend(quoted_values(t, "target"));
        }
        for (i, _) in t.match_indices("-T") {
            let name: String = t[i + 2..]
                .chars()
                .take_while(|c| !matches!(c, '"' | ',' | ' ' | '\''))
                .collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
        for rel in names {
            if let Some(dep) = resolve(workspace, &rel) {
                if tree.files.contains_key(&dep) {
                    out.push(Edge {
                        from: path.to_string(),
                        to: dep,
                        kind: "target or linker script",
                    });
                }
            }
        }
    }
}

fn source_edges(path: &str, text: &str, tree: &Tree, out: &mut Vec<Edge>) {
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("//") {
            continue;
        }
        let mut named: Vec<(&'static str, String)> = Vec::new();
        for (kind, key) in [
            ("include_bytes!", "include_bytes!("),
            ("include_str!", "include_str!("),
            ("include!", "include!("),
        ] {
            for (i, _) in t.match_indices(key) {
                if key == "include!(" && t[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                if let Some(rest) = t[i + key.len()..].trim_start().strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        named.push((kind, rest[..end].to_string()));
                    }
                }
            }
        }
        if t.starts_with("#[path") {
            for rel in quoted_values(t, "path") {
                named.push(("#[path]", rel));
            }
        }
        for (kind, rel) in named {
            let Some(dep) = resolve(dir_of(path), &rel) else { continue };
            if dep.starts_with("dist/") || dep.starts_with("vendor/") || !tree.files.contains_key(&dep) {
                continue;
            }
            out.push(Edge {
                from: path.to_string(),
                to: dep,
                kind,
            });
        }
    }
}

/// Every edge in the tree.
pub fn edges(tree: &Tree) -> Vec<Edge> {
    let mut out = Vec::new();
    for (path, text) in &tree.files {
        if path.ends_with("/Cargo.toml") || path == "Cargo.toml" {
            cargo_edges(path, text, tree, &mut out);
        } else if path.ends_with("/.cargo/config.toml") {
            config_edges(path, text, tree, &mut out);
        } else if path.ends_with(".rs") {
            source_edges(path, text, tree, &mut out);
        }
    }
    out
}

/// The problems with `found` under `config`: an edge to a layer the file's layer may not use and that no `allow` covers, a
/// manifest in no layer, and an `allow` no edge needs (so a recorded exception is deleted when the edge it recorded goes).
pub fn check(config: &Config, tree: &Tree, found: &[Edge]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut used = vec![false; config.allows.len()];
    for path in tree.files.keys() {
        if (path.ends_with("/Cargo.toml") || path == "Cargo.toml") && config.layer_of(dir_of(path)).is_none() {
            problems.push(format!(
                "layers: {path}: a manifest in no layer (add its directory to tools/layers/layers.txt)"
            ));
        }
    }
    for e in found {
        let Some(from) = config.layer_of(&e.from) else { continue };
        let to = config.layer_of(&e.to);
        if to.is_some_and(|to| config.may(from, to)) {
            continue;
        }
        let mut excused = false;
        for (i, a) in config.allows.iter().enumerate() {
            if under(&e.from, &a.from) && under(&e.to, &a.to) {
                used[i] = true;
                excused = true;
            }
        }
        if !excused {
            let to_name = to.unwrap_or("no layer");
            problems.push(format!(
                "layers: {} -> {} ({from} -> {to_name}): {} is not allowed",
                e.from, e.to, e.kind
            ));
        }
    }
    for (a, used) in config.allows.iter().zip(used) {
        if !used {
            problems.push(format!(
                "layers: allow {} {} ({}): no edge needs it any more; delete the line",
                a.from, a.to, a.reason
            ));
        }
    }
    problems.sort();
    problems
}
