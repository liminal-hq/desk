// Tests of the layering check over small in-memory trees: each kind of edge, what is allowed, and each way the check fails.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use layers::{check, edges, resolve, Config, Tree};

const RULES: &str = "
layer low low
layer high high
may-use high low
layer spikes spikes
may-use spikes *
";

fn tree(files: &[(&str, &str)]) -> Tree {
    Tree {
        files: files.iter().map(|(p, t)| (p.to_string(), t.to_string())).collect(),
    }
}

fn problems(rules: &str, files: &[(&str, &str)]) -> Vec<String> {
    let config = Config::parse(rules).expect("rules parse");
    let tree = tree(files);
    check(&config, &tree, &edges(&tree))
}

#[test]
fn a_higher_layer_may_name_a_lower_one() {
    let p = problems(
        RULES,
        &[
            ("high/Cargo.toml", "[dependencies]\nlow = { path = \"../low\" }\n"),
            ("low/Cargo.toml", "[package]\nname = \"low\"\n"),
        ],
    );
    assert!(p.is_empty(), "{p:?}");
}

#[test]
fn a_lower_layer_may_not_name_a_higher_one() {
    let p = problems(
        RULES,
        &[
            (
                "low/Cargo.toml",
                "[dev-dependencies]\nhigh = { path = \"../high\", version = \"0\" }\n",
            ),
            ("high/Cargo.toml", "[package]\nname = \"high\"\n"),
        ],
    );
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(
        p[0].contains("low/Cargo.toml -> high (low -> high): path dependency is not allowed"),
        "{p:?}"
    );
}

#[test]
fn only_dependency_sections_name_dependencies() {
    let p = problems(
        RULES,
        &[
            (
                "low/Cargo.toml",
                "[lib]\npath = \"../high/src/lib.rs\"\n[[bin]]\npath = \"../high\"\n",
            ),
            ("high/Cargo.toml", "[package]\nname = \"high\"\n"),
        ],
    );
    assert!(p.is_empty(), "{p:?}");
}

#[test]
fn a_target_and_a_linker_script_in_a_cargo_config_are_edges() {
    let files = [
        (
            "low/.cargo/config.toml",
            "[build]\ntarget = \"../high/t.json\"\n[target.t]\nrustflags = [\"-C\", \"link-arg=-T../high/l.ld\"]\n",
        ),
        ("high/t.json", "{}"),
        ("high/l.ld", ""),
    ];
    let p = problems(RULES, &files);
    assert_eq!(p.len(), 2, "{p:?}");
    assert!(
        p.iter().any(|l| l.contains("high/t.json")) && p.iter().any(|l| l.contains("high/l.ld")),
        "{p:?}"
    );
}

#[test]
fn files_compiled_in_and_path_modules_are_edges() {
    let files = [
        (
            "low/src/lib.rs",
            "static A: &[u8] = include_bytes!(\"../../high/a.bin\");\nconst B: &str = include_str!(\"../../high/b.txt\");\n#[path = \"../../high/m.rs\"]\nmod m;\n",
        ),
        ("high/a.bin", ""),
        ("high/b.txt", ""),
        ("high/m.rs", ""),
    ];
    let p = problems(RULES, &files);
    assert_eq!(p.len(), 3, "{p:?}");
    assert!(p.iter().any(|l| l.contains("include_bytes!")), "{p:?}");
    assert!(p.iter().any(|l| l.contains("include_str!")), "{p:?}");
    assert!(p.iter().any(|l| l.contains("#[path]")), "{p:?}");
}

#[test]
fn comments_build_outputs_and_untracked_files_are_not_edges() {
    let files = [
        (
            "low/src/lib.rs",
            "// include_str!(\"../../high/a.txt\")\nstatic B: &[u8] = include_bytes!(\"../../dist/x.bin\");\nconst C: &str = include_str!(\"../../high/missing.txt\");\n",
        ),
        ("high/a.txt", ""),
        ("dist/x.bin", ""),
    ];
    let p = problems(RULES, &files);
    assert!(p.is_empty(), "{p:?}");
}

#[test]
fn a_spike_may_name_anything_and_nothing_may_name_a_spike() {
    let files = [
        ("spikes/s/Cargo.toml", "[dependencies]\nh = { path = \"../../high\" }\n"),
        ("low/src/lib.rs", "const A: &str = include_str!(\"../../spikes/s/a.txt\");\n"),
        ("high/Cargo.toml", "[package]\nname = \"h\"\n"),
        ("spikes/s/a.txt", ""),
    ];
    let p = problems(RULES, &files);
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(p[0].contains("low/src/lib.rs -> spikes/s/a.txt (low -> spikes)"), "{p:?}");
}

#[test]
fn an_allow_excuses_its_edges_and_an_unused_allow_is_an_error() {
    let files = [
        ("low/src/lib.rs", "const A: &str = include_str!(\"../../high/a.txt\");\n"),
        ("high/a.txt", ""),
    ];
    let allowed = format!("{RULES}allow low/src high/a.txt a recorded exception\n");
    assert!(problems(&allowed, &files).is_empty());
    let stale = format!("{RULES}allow low/src high/gone.txt the edge went\n");
    let p = problems(&stale, &files);
    assert_eq!(p.len(), 2, "{p:?}");
    assert!(p.iter().any(|l| l.contains("no edge needs it any more")), "{p:?}");
}

#[test]
fn a_manifest_in_no_layer_is_an_error() {
    let p = problems(RULES, &[("elsewhere/Cargo.toml", "[package]\nname = \"x\"\n")]);
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(p[0].contains("a manifest in no layer"), "{p:?}");
}

#[test]
fn the_longest_prefix_decides_the_layer() {
    let rules = "layer a a\nlayer b a/b\n";
    let config = Config::parse(rules).unwrap();
    assert_eq!(config.layer_of("a/x.rs"), Some("a"));
    assert_eq!(config.layer_of("a/b/x.rs"), Some("b"));
    assert_eq!(config.layer_of("a/b"), Some("b"));
    assert_eq!(
        config.layer_of("ab/x.rs"),
        None,
        "a prefix is a directory, not a string prefix"
    );
}

#[test]
fn bad_rules_are_rejected() {
    for bad in [
        "layer onlyname",
        "frobnicate x y",
        "layer a a\nlayer b a",
        "layer a a\nmay-use a ghost",
        "may-use ghost a",
        "layer a a\nallow a/x b",
    ] {
        assert!(Config::parse(bad).is_err(), "{bad:?} parsed");
    }
}

#[test]
fn paths_resolve_against_their_directory() {
    assert_eq!(resolve("a/b", "../c/d.rs").as_deref(), Some("a/c/d.rs"));
    assert_eq!(resolve("a", "./x").as_deref(), Some("a/x"));
    assert_eq!(resolve("", "x").as_deref(), Some("x"));
    assert_eq!(resolve("a", "../../x"), None);
}
