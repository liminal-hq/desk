// steps-junit: the conversion of one `tools/check.sh` run (its step times and its log) into JUnit XML in the shape the `steps` flavour of
// pr-results reads (liminal-hq-labs/pr-results, docs/inputs.md): one `<testsuite>` per job, a `<testcase classname="check.GROUP">` per
// step, a `<failure type="step">` with the tail of the step's output on the step that failed, and no skipped steps.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use std::fmt::Write;

/// Lines of a failed step's output kept in its `<failure>`.
pub const TAIL_LINES: usize = 80;
/// Bytes of a failed step's output kept at most. pr-results keeps 16 KiB of a case's detail and drops the rest from the end, so the
/// tail is cut here, from the front, to stay under it with the error line still in.
pub const TAIL_BYTES: usize = 15 * 1024;
/// Characters of the failing line quoted in a message (pr-results keeps 1 KiB).
const MESSAGE_CHARS: usize = 300;

/// How one step ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    /// The step that was running when `tools/check.sh` exited non-zero.
    Failure {
        message: String,
        body: String,
    },
    /// The run itself went wrong: no exit status, no step times, a missing log.
    Error {
        message: String,
        body: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Case {
    pub classname: String,
    pub name: String,
    pub ms: u64,
    pub outcome: Outcome,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub suite: String,
    pub cases: Vec<Case>,
}

/// One top-level step from the times file.
struct Step {
    start: u64,
    group: String,
    title: String,
}

/// The last run in a times file: `start<TAB>ms<TAB>group<TAB>title` per step and `end<TAB>ms<TAB>status` once. A `start` after an `end`
/// begins a new run, since the file is appended to.
fn parse_times(text: &str) -> (Vec<Step>, Option<(u64, i64)>) {
    let mut steps = Vec::new();
    let mut end = None;
    for line in text.lines() {
        let mut f = line.splitn(4, '\t');
        match (
            f.next(),
            f.next().and_then(|v| v.trim().parse::<u64>().ok()),
            f.next(),
            f.next(),
        ) {
            (Some("start"), Some(ms), Some(group), Some(title)) => {
                if end.take().is_some() {
                    steps.clear();
                }
                steps.push(Step {
                    start: ms,
                    group: group.to_string(),
                    title: title.to_string(),
                });
            }
            (Some("end"), Some(ms), Some(status), None) => {
                if let Ok(status) = status.trim().parse::<i64>() {
                    end = Some((ms, status));
                }
            }
            _ => {}
        }
    }
    (steps, end)
}

/// The text without ANSI escape sequences or the control characters XML 1.0 forbids (tab, line feed and carriage return stay), and
/// with every line ending as a line feed.
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.peek() {
                // A control sequence ends at its first byte from @ to ~.
                Some('[') => {
                    chars.next();
                    for d in chars.by_ref() {
                        if ('@'..='~').contains(&d) {
                            break;
                        }
                    }
                }
                // An operating system command ends at BEL or ESC \.
                Some(']') => {
                    chars.next();
                    while let Some(d) = chars.next() {
                        if d == '\u{7}' {
                            break;
                        }
                        if d == '\u{1b}' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    out.push('\n');
                }
            }
            '\t' | '\n' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {}
            // XML 1.0 forbids the surrogates (which a char cannot be) and these two.
            '\u{fffe}' | '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}

/// The lines of each step's section of the log: from the line after `== GROUP: TITLE` up to the next step's header. The headers are found
/// in order and by their exact text, never by pattern, since a sub-script prints lines of the same look (`== depth 8: ...`) inside a step.
fn sections(steps: &[Step], log: &[String]) -> Vec<Vec<String>> {
    let mut at = Vec::with_capacity(steps.len());
    let mut from = 0;
    for s in steps {
        let header = format!("== {}: {}", s.group, s.title);
        match log[from..].iter().position(|l| *l == header) {
            Some(i) => {
                at.push(Some(from + i));
                from += i + 1;
            }
            None => at.push(None),
        }
    }
    (0..steps.len())
        .map(|i| match at[i] {
            None => Vec::new(),
            Some(a) => {
                let stop = at[i + 1..].iter().flatten().next().copied().unwrap_or(log.len());
                log[a + 1..stop].to_vec()
            }
        })
        .collect()
}

/// The line that says why a step failed: the last that starts with FAIL (this script's `FAIL: ...`, nextest's `FAIL [...] test`), else
/// the last that contains FAILED or `error:`, else the last that is not blank.
fn why(lines: &[String]) -> Option<&str> {
    let pick = |f: &dyn Fn(&str) -> bool| lines.iter().rev().map(|l| l.trim()).find(|l| !l.is_empty() && f(l));
    pick(&|l| l.starts_with("FAIL"))
        .or_else(|| pick(&|l| l.contains("FAILED") || l.contains("error:")))
        .or_else(|| pick(&|_| true))
}

fn shorten(line: &str) -> String {
    let mut s: String = line.chars().take(MESSAGE_CHARS).collect();
    if line.chars().count() > MESSAGE_CHARS {
        s.push('…');
    }
    s
}

/// The last lines of a section, at most TAIL_LINES lines and TAIL_BYTES bytes, without the blank lines at the end.
fn tail(lines: &[String]) -> String {
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map_or(0, |i| i + 1);
    let start = end.saturating_sub(TAIL_LINES);
    let mut text = lines[start..end].join("\n");
    if text.len() > TAIL_BYTES {
        let mut cut = text.len() - TAIL_BYTES;
        while !text.is_char_boundary(cut) {
            cut += 1;
        }
        text = text[cut..].to_string();
        // Start at a whole line.
        if let Some(i) = text.find('\n') {
            text = text[i + 1..].to_string();
        }
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

/// One test case for a run that has no steps to report: the check did not start, or wrote no times.
fn run_error(message: &str, log: &[String]) -> Report {
    let last = log.iter().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_string());
    let message = match &last {
        Some(l) => format!("{message}; the last line of the output: {}", shorten(l)),
        None => message.to_string(),
    };
    Report {
        suite: String::new(),
        cases: vec![Case {
            classname: "check".into(),
            name: "tools/check.sh".into(),
            ms: 0,
            outcome: Outcome::Error {
                message,
                body: tail(log),
            },
        }],
    }
}

/// Reads one job's step times and log (None when the file is missing) and reports each step. It never fails: whatever is missing is
/// a test case that says so, because the report must not hide the job's own failure.
pub fn convert(suite: &str, times: Option<&str>, log: Option<&[u8]>) -> Report {
    let log_text = log.map(|b| clean(&String::from_utf8_lossy(b)));
    let log: Vec<String> = log_text
        .as_deref()
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default();
    let (steps, end) = times.map(|t| parse_times(&clean(t))).unwrap_or_default();

    let mut report = if steps.is_empty() {
        match (times, end) {
            (None, _) => run_error(
                "no step times were written (tools/check.sh did not run, or its job failed before it)",
                &log,
            ),
            (Some(_), Some((_, 0))) => Report {
                suite: String::new(),
                cases: Vec::new(),
            },
            (Some(_), Some((_, status))) => run_error(
                &format!("tools/check.sh exited with status {status} before its first step"),
                &log,
            ),
            (Some(_), None) => run_error("the step times hold no steps and no exit status", &log),
        }
    } else {
        let parts = sections(&steps, &log);
        let last = steps.len() - 1;
        let cases = steps
            .iter()
            .zip(&parts)
            .enumerate()
            .map(|(i, (s, lines))| {
                let stop = steps.get(i + 1).map(|n| n.start).or(end.map(|(ms, _)| ms));
                let outcome = match end {
                    _ if i != last => Outcome::Passed,
                    Some((_, 0)) => Outcome::Passed,
                    Some((_, status)) => {
                        let message = match why(lines) {
                            Some(l) => format!("exit status {status}: {}", shorten(l)),
                            None => format!("exit status {status}"),
                        };
                        Outcome::Failure {
                            message,
                            body: tail(lines),
                        }
                    }
                    None => Outcome::Error {
                        message: "the run ended without an exit status (timed out or cancelled)".into(),
                        body: tail(lines),
                    },
                };
                Case {
                    classname: format!("check.{}", s.group),
                    name: s.title.clone(),
                    ms: stop.map_or(0, |t| t.saturating_sub(s.start)),
                    outcome,
                }
            })
            .collect();
        Report {
            suite: String::new(),
            cases,
        }
    };
    report.suite = suite.to_string();
    report
}

fn seconds(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// Text for an element or an attribute: the five characters XML needs escaped, and line breaks in an attribute as spaces.
fn escape(s: &str, attribute: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' | '\t' if attribute => out.push(' '),
            // Legal in XML, but the C1 controls and the Unicode line and paragraph separators can disturb how a comment renders.
            '\u{80}'..='\u{9f}' | '\u{2028}' | '\u{2029}' => {}
            c => out.push(c),
        }
    }
    out
}

impl Report {
    pub fn failures(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| matches!(c.outcome, Outcome::Failure { .. }))
            .count()
    }

    pub fn errors(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| matches!(c.outcome, Outcome::Error { .. }))
            .count()
    }

    pub fn ms(&self) -> u64 {
        self.cases.iter().map(|c| c.ms).sum()
    }

    /// The JUnit document. `errors` appears only when there is one, so a run of steps has the attributes pr-results' fixture shows.
    pub fn to_junit(&self) -> String {
        let counts = |out: &mut String| {
            let _ = write!(out, " tests=\"{}\" failures=\"{}\"", self.cases.len(), self.failures());
            if self.errors() > 0 {
                let _ = write!(out, " errors=\"{}\"", self.errors());
            }
            let _ = write!(out, " time=\"{}\"", seconds(self.ms()));
        };
        let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"check\"");
        counts(&mut x);
        x.push_str(">\n");
        let _ = write!(x, "    <testsuite name=\"{}\"", escape(&self.suite, true));
        counts(&mut x);
        x.push_str(">\n");
        for c in &self.cases {
            let _ = write!(
                x,
                "        <testcase classname=\"{}\" name=\"{}\" time=\"{}\"",
                escape(&c.classname, true),
                escape(&c.name, true),
                seconds(c.ms)
            );
            let (tag, message, body) = match &c.outcome {
                Outcome::Passed => {
                    x.push_str("/>\n");
                    continue;
                }
                Outcome::Failure { message, body } => ("failure", message, body),
                Outcome::Error { message, body } => ("error", message, body),
            };
            let kind = if tag == "failure" { " type=\"step\"" } else { "" };
            if body.is_empty() {
                let _ = write!(
                    x,
                    ">\n            <{tag} message=\"{}\"{kind}/>\n        </testcase>\n",
                    escape(message, true)
                );
            } else {
                let _ = write!(
                    x,
                    ">\n            <{tag} message=\"{}\"{kind}>{}</{tag}>\n        </testcase>\n",
                    escape(message, true),
                    escape(body, false)
                );
            }
        }
        x.push_str("    </testsuite>\n</testsuites>\n");
        x
    }

    /// A Markdown table for the job summary.
    pub fn to_summary(&self) -> String {
        let cell = |s: &str| s.replace('|', "\\|");
        let mut m = format!(
            "## {}\n\n| Step | Group | Time | Result |\n|---|---|---:|---|\n",
            cell(&self.suite)
        );
        for c in &self.cases {
            let result = match c.outcome {
                Outcome::Passed => "passed",
                Outcome::Failure { .. } => "failed",
                Outcome::Error { .. } => "error",
            };
            let group = c.classname.strip_prefix("check.").unwrap_or(&c.classname);
            let _ = writeln!(
                m,
                "| {} | {} | {} s | {} |",
                cell(&c.name),
                cell(group),
                seconds(c.ms),
                result
            );
        }
        let _ = writeln!(
            m,
            "\n{} steps, {} failed, {} errors, {} s.",
            self.cases.len(),
            self.failures(),
            self.errors(),
            seconds(self.ms())
        );
        m
    }
}
