// Tests of the converter: each edge case of a check run, and the shape of the output against the fixture pr-results holds for it.
//
// tests/fixtures/pr-results-desk-job.xml is `tests/fixtures/steps/desk-job.xml` of liminal-hq-labs/pr-results at commit
// 8afc480363b3c8f09a4db221d1c4814415ab6eb2 (v0.1.0), copied unchanged: the JUnit its `steps` flavour is built and tested to read.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use steps_junit::{clean, convert, Outcome, Report, TAIL_BYTES, TAIL_LINES};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn run(suite: &str, times: &str, log: &str) -> Report {
    convert(suite, Some(times), Some(log.as_bytes()))
}

// A small XML reader for the tests: the start tags with their attributes, the end tags and the text that is not blank.
#[derive(Debug, PartialEq, Clone)]
enum Ev {
    Start(String, Vec<(String, String)>),
    End(String),
    Text(String),
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn parse(xml: &str) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut rest = xml;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("<?") {
            rest = &r[r.find("?>").unwrap() + 2..];
        } else if let Some(r) = rest.strip_prefix("<!--") {
            rest = &r[r.find("-->").unwrap() + 3..];
        } else if let Some(r) = rest.strip_prefix("</") {
            let i = r.find('>').unwrap();
            out.push(Ev::End(r[..i].to_string()));
            rest = &r[i + 1..];
        } else if let Some(r) = rest.strip_prefix('<') {
            let mut i = 0;
            let mut quoted = false;
            for (n, c) in r.char_indices() {
                match c {
                    '"' => quoted = !quoted,
                    '>' if !quoted => {
                        i = n;
                        break;
                    }
                    _ => {}
                }
            }
            let tag = &r[..i];
            let empty = tag.ends_with('/');
            let tag = tag.trim_end_matches('/');
            let name_end = tag.find(char::is_whitespace).unwrap_or(tag.len());
            let mut attrs = Vec::new();
            let mut a = tag[name_end..].trim_start();
            while let Some(eq) = a.find("=\"") {
                let value_end = a[eq + 2..].find('"').unwrap() + eq + 2;
                attrs.push((a[..eq].to_string(), unescape(&a[eq + 2..value_end])));
                a = a[value_end + 1..].trim_start();
            }
            out.push(Ev::Start(tag[..name_end].to_string(), attrs));
            if empty {
                out.push(Ev::End(tag[..name_end].to_string()));
            }
            rest = &r[i + 1..];
        } else {
            let i = rest.find('<').unwrap_or(rest.len());
            if !rest[..i].trim().is_empty() {
                out.push(Ev::Text(unescape(&rest[..i])));
            }
            rest = &rest[i..];
        }
    }
    out
}

fn attr<'a>(e: &'a Ev, key: &str) -> Option<&'a str> {
    match e {
        Ev::Start(_, a) => a.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn starts<'a>(evs: &'a [Ev], tag: &str) -> Vec<&'a Ev> {
    evs.iter().filter(|e| matches!(e, Ev::Start(t, _) if t == tag)).collect()
}

fn keys(e: &Ev) -> Vec<&str> {
    match e {
        Ev::Start(_, a) => a.iter().map(|(k, _)| k.as_str()).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn all_steps_pass() {
    let r = run("Licence headers", &fixture("pass.times"), &fixture("pass.log"));
    assert_eq!(r.cases.len(), 3);
    assert!(r.cases.iter().all(|c| c.outcome == Outcome::Passed));
    assert_eq!(r.cases[0].classname, "check.base");
    assert_eq!(r.cases[0].name, "Licence headers");
    assert_eq!(r.cases.iter().map(|c| c.ms).collect::<Vec<_>>(), [500, 2750, 650]);
    let x = r.to_junit();
    assert!(
        x.contains("<testsuites name=\"check\" tests=\"3\" failures=\"0\" time=\"3.900\">"),
        "{x}"
    );
    assert!(
        x.contains("<testcase classname=\"check.base\" name=\"Licence headers\" time=\"0.500\"/>"),
        "{x}"
    );
    assert!(!x.contains("<failure") && !x.contains("<error") && !x.contains("skipped"));
}

#[test]
fn the_second_step_fails_and_lines_that_look_like_steps_stay_in_its_section() {
    // The script stops at the first failure, so the failing step is the last one started; the "== depth 8: ..." lines inside it are not steps.
    let r = run("Progress report", &fixture("fail.times"), &fixture("fail.log"));
    let Outcome::Failure { message, body } = &r.cases[1].outcome else {
        panic!("{:?}", r.cases)
    };
    assert_eq!(message, "exit status 1: FAIL: moved 8 differs from the golden");
    assert_eq!(
        body,
        "== depth 8: this line looks like a step but belongs to the section\n== fontfile: the faces read the same\n\
         tests/shell/first-window.sums: 3 lines\nFAIL: moved 8 differs from the golden\n"
    );
    assert_eq!(r.cases[0].outcome, Outcome::Passed);
    assert_eq!(r.cases[1].ms, 500);
    let x = r.to_junit();
    assert!(
        x.contains("<testsuites name=\"check\" tests=\"2\" failures=\"1\" time=\"1.500\">"),
        "{x}"
    );
    assert!(
        x.contains("<failure message=\"exit status 1: FAIL: moved 8 differs from the golden\" type=\"step\">"),
        "{x}"
    );
}

#[test]
fn the_message_prefers_a_fail_line_then_an_error_line_then_the_last_line() {
    let times = "start\t0\tg\ts\nend\t10\t101\n";
    let m = |log: &str| match run("j", times, log).cases.remove(0).outcome {
        Outcome::Failure { message, .. } => message,
        o => panic!("{o:?}"),
    };
    assert_eq!(
        m("== g: s\n        FAIL [   0.004s] hal::bus::reads\nSummary: 1 failed\nerror: test run failed\n"),
        "exit status 101: FAIL [   0.004s] hal::bus::reads"
    );
    assert_eq!(
        m("== g: s\nwarning: x\nerror: could not compile `a`\nmore\n"),
        "exit status 101: error: could not compile `a`"
    );
    assert_eq!(m("== g: s\nsomething\nthe last line\n\n"), "exit status 101: the last line");
    assert_eq!(m("== g: s\n"), "exit status 101");
}

#[test]
fn a_run_with_no_end_line_ends_in_an_error_on_the_last_step() {
    let times = "start\t1000\tdos\tBuild DESK.EXE\nstart\t5000\tdos\tRun DESK.EXE\n";
    let r = run(
        "DOS",
        times,
        "== dos: Build DESK.EXE\nbuilt\n\n== dos: Run DESK.EXE\nrunning...\n",
    );
    assert_eq!(r.cases[0].outcome, Outcome::Passed);
    assert_eq!(r.cases[0].ms, 4000);
    let Outcome::Error { message, body } = &r.cases[1].outcome else {
        panic!("{:?}", r.cases)
    };
    assert_eq!(message, "the run ended without an exit status (timed out or cancelled)");
    assert_eq!(body, "running...\n");
    assert_eq!(r.cases[1].ms, 0);
    let x = r.to_junit();
    assert!(x.contains("errors=\"1\""), "{x}");
    assert!(x.contains("<error message=\"the run ended"), "{x}");
}

#[test]
fn no_step_and_a_non_zero_end_is_one_error_that_quotes_the_last_line() {
    let r = run("Base", "end\t5\t2\n", "check: unknown group 'nope' (try --list)\n");
    assert_eq!(r.cases.len(), 1);
    assert_eq!(r.cases[0].classname, "check");
    assert_eq!(r.cases[0].name, "tools/check.sh");
    let Outcome::Error { message, body } = &r.cases[0].outcome else {
        panic!("{:?}", r.cases)
    };
    assert!(
        message.starts_with("tools/check.sh exited with status 2 before its first step"),
        "{message}"
    );
    assert!(message.ends_with("check: unknown group 'nope' (try --list)"), "{message}");
    assert_eq!(body, "check: unknown group 'nope' (try --list)\n");
}

#[test]
fn missing_files_make_the_same_single_error_and_never_fail() {
    for r in [
        convert("Base", None, None),
        convert("Base", None, Some(b"setup failed\n")),
        convert("Base", Some(""), Some(b"")),
    ] {
        assert_eq!(r.cases.len(), 1);
        assert!(matches!(r.cases[0].outcome, Outcome::Error { .. }), "{:?}", r.cases);
        assert!(r.to_junit().contains("<error message="));
    }
    let r = convert("Base", None, Some(b"setup failed\n"));
    let Outcome::Error { message, .. } = &r.cases[0].outcome else {
        panic!()
    };
    assert!(
        message.contains("no step times were written") && message.contains("setup failed"),
        "{message}"
    );
}

#[test]
fn a_clean_run_of_no_steps_is_an_empty_suite() {
    let r = run("Base", "end\t5\t0\n", "check: groups: base\n");
    assert!(r.cases.is_empty());
    assert!(r.to_junit().contains("tests=\"0\""));
}

#[test]
fn a_later_run_appended_to_the_times_file_replaces_the_earlier() {
    let times = "start\t0\ta\tOld\nend\t5\t1\nstart\t10\ta\tNew\nend\t30\t0\n";
    let r = run("j", times, "== a: Old\nx\n== a: New\ny\n");
    assert_eq!(r.cases.len(), 1);
    assert_eq!(
        (r.cases[0].name.as_str(), r.cases[0].ms, &r.cases[0].outcome),
        ("New", 20, &Outcome::Passed)
    );
}

#[test]
fn xml_is_escaped_and_forbidden_characters_are_removed() {
    let times = "start\t0\tg\tA & B < C > \"D\" 'E'\nend\t1500\t1\n";
    let log = "== g: A & B < C > \"D\" 'E'\n\u{1b}[31mred\u{1b}[0m <&>\"'\u{0}\u{1}\u{8}\u{b}\u{c}\u{1b}x\u{fffe}\t\u{7f}ok\r\nline two\r\n";
    let x = run("S & \"T\"", times, log).to_junit();
    assert!(x.contains("<testsuite name=\"S &amp; &quot;T&quot;\""), "{x}");
    assert!(
        x.contains("name=\"A &amp; B &lt; C &gt; &quot;D&quot; &apos;E&apos;\""),
        "{x}"
    );
    assert!(x.contains("red &lt;&amp;&gt;&quot;&apos;x\tok\nline two\n</failure>"), "{x}");
    for bad in [
        '\u{0}', '\u{1}', '\u{8}', '\u{b}', '\u{c}', '\u{1b}', '\u{7f}', '\u{fffe}', '\r',
    ] {
        assert!(!x.contains(bad), "{bad:?} in {x}");
    }
    // The document reads back to the same strings.
    let evs = parse(&x);
    let case = starts(&evs, "testcase")[0];
    assert_eq!(attr(case, "name"), Some("A & B < C > \"D\" 'E'"));
    assert_eq!(attr(starts(&evs, "testsuite")[0], "name"), Some("S & \"T\""));
}

#[test]
fn ansi_sequences_and_a_non_utf8_byte_are_handled() {
    assert_eq!(
        clean("\u{1b}[1;32mok\u{1b}[0m \u{1b}]0;title\u{7}done \u{1b}]8;;x\u{1b}\\end"),
        "ok done end"
    );
    let r = convert(
        "j",
        Some("start\t0\tg\tt\nend\t5\t1\n"),
        Some(b"== g: t\nbad \xff\xfe byte\n"),
    );
    let Outcome::Failure { body, .. } = &r.cases[0].outcome else {
        panic!()
    };
    assert_eq!(body, "bad \u{fffd}\u{fffd} byte\n");
    assert!(r.to_junit().contains("bad \u{fffd}\u{fffd} byte"));
}

#[test]
fn a_title_with_a_colon_is_found_by_its_exact_text_in_order() {
    let times = "start\t0\tshelldos\tShell on DOS: the first window's scenario\nstart\t10\tshelldos\tNext\nend\t20\t1\n";
    let log = "== shelldos: Shell on DOS: the first window's scenario\n== fontfile: the faces read the same\n\n== shelldos: Next\nboom\n";
    let r = run("j", times, log);
    assert_eq!(r.cases[0].outcome, Outcome::Passed);
    let Outcome::Failure { body, .. } = &r.cases[1].outcome else {
        panic!()
    };
    assert_eq!(body, "boom\n");
}

#[test]
fn a_step_whose_header_is_missing_has_an_empty_section() {
    let r = run("j", "start\t0\tg\tOne\nstart\t5\tg\tTwo\nend\t9\t1\n", "== g: One\nx\n");
    let Outcome::Failure { message, body } = &r.cases[1].outcome else {
        panic!()
    };
    assert_eq!((message.as_str(), body.as_str()), ("exit status 1", ""));
    assert!(r.to_junit().contains("<failure message=\"exit status 1\" type=\"step\"/>"));
}

#[test]
fn the_tail_is_cut_to_the_last_lines_and_the_last_bytes() {
    let times = "start\t0\tg\tt\nend\t5\t1\n";
    let mut log = String::from("== g: t\n");
    for i in 0..500 {
        log.push_str(&format!("line {i}\n"));
    }
    log.push_str("\n\n");
    let Outcome::Failure { body, .. } = run("j", times, &log).cases.remove(0).outcome else {
        panic!()
    };
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), TAIL_LINES);
    assert_eq!((lines[0], lines[TAIL_LINES - 1]), ("line 420", "line 499"));
    // Long lines: the cut is by bytes, from the front, at a whole line, and keeps the end.
    let mut long = String::from("== g: t\n");
    for i in 0..60 {
        long.push_str(&format!("{i:02} {}\n", "é".repeat(400)));
    }
    long.push_str("FAIL: the end\n");
    let Outcome::Failure { body, .. } = run("j", times, &long).cases.remove(0).outcome else {
        panic!()
    };
    assert!(body.len() <= TAIL_BYTES && body.len() > TAIL_BYTES - 1000, "{}", body.len());
    assert!(body.ends_with("FAIL: the end\n") && body.starts_with(|c: char| c.is_ascii_digit()));
}

#[test]
fn the_summary_is_a_markdown_table() {
    let r = run("Progress | report", &fixture("pass.times"), &fixture("pass.log"));
    let s = r.to_summary();
    assert!(
        s.starts_with("## Progress \\| report\n\n| Step | Group | Time | Result |\n|---|---|---:|---|\n"),
        "{s}"
    );
    assert!(s.contains("| Licence headers | base | 0.500 s | passed |\n"), "{s}");
    assert!(s.contains("3 steps, 0 failed, 0 errors, 3.900 s."), "{s}");
}

/// The converter's output for the run behind pr-results' own fixture has that fixture's shape: the same elements, attributes and
/// names, the same times, and the same failure text, with a message that starts with the fixture's.
#[test]
fn the_output_has_the_shape_pr_results_reads() {
    let theirs = parse(&fixture("pr-results-desk-job.xml"));
    let mine = parse(&run("check (host)", &fixture("desk-job.times"), &fixture("desk-job.log")).to_junit());

    // The envelope: the same elements with the same attributes, in the same order.
    for tag in ["testsuites", "testsuite"] {
        assert_eq!(keys(starts(&mine, tag)[0]), keys(starts(&theirs, tag)[0]), "{tag}");
    }
    assert_eq!(
        attr(starts(&mine, "testsuites")[0], "name"),
        attr(starts(&theirs, "testsuites")[0], "name")
    );
    assert_eq!(attr(starts(&mine, "testsuite")[0], "name"), Some("check (host)"));
    assert_eq!(attr(starts(&mine, "testsuite")[0], "time"), Some("7.119"));

    // The first two cases of the fixture, one passing and one failing, tag by tag.
    let cases = |evs: &[Ev]| {
        let first = evs
            .iter()
            .position(|e| matches!(e, Ev::Start(t, _) if t == "testcase"))
            .unwrap();
        let mut out = Vec::new();
        let mut seen = 0;
        for e in &evs[first..] {
            out.push(e.clone());
            if matches!(e, Ev::End(t) if t == "testcase") {
                seen += 1;
                if seen == 2 {
                    break;
                }
            }
        }
        out
    };
    let (theirs, mine) = (cases(&theirs), cases(&mine));
    assert_eq!(theirs.len(), mine.len());
    for (t, m) in theirs.iter().zip(&mine) {
        match (t, m) {
            (Ev::Start(tag, _), Ev::Start(..)) if tag == "failure" => {
                assert_eq!(keys(t), keys(m));
                assert_eq!(attr(t, "type"), attr(m, "type"));
                assert!(attr(m, "message").unwrap().starts_with(attr(t, "message").unwrap()), "{m:?}");
            }
            _ => assert_eq!(t, m),
        }
    }
}
