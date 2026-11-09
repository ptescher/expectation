//! Code coverage: how much of the instrumented program's source ran while a test did.
//!
//! This is a different thing from the contract coverage in [`crate::coverage`], which asks which
//! outcomes of a contract were seen. Here the question is which lines, branches and functions of
//! the application's own source were executed. Expectation doesn't instrument the app or read
//! LCOV or Istanbul data itself: the application or its driver collects it, hands it over as
//! step [attachments](crate::Attachment), and an extension (`extensions/code-coverage`) works out
//! the numbers and writes a file next to the results. This module reads it, and holds it to
//! these rules:
//!
//! - It is observed execution. It never changes a test's result or an expectation's verdict, and
//!   it says nothing about what a test asserted.
//! - A capture counts only for the run it came from, and for the test as it was fingerprinted
//!   then. Captures from another run, or from a test that has changed since, are not shown.
//! - A capture that is missing or unusable is shown as such, never as zero: no data is not
//!   uncovered code.
//! - What a step added is only claimed when the counters were cumulative and never went down.
//!   After a restart, or when captures skipped steps, it says so instead of guessing, and it
//!   can't know what unrelated background activity ran in the same window.
//!
//! The file is `results/<name>[@platform].code.tsv`, tab-separated, `-` for nothing, and lines of
//! a kind not listed here are skipped:
//!
//! ```text
//! expectation-code-coverage 1
//! source   <name>  <revision>  <scope>  <report url>
//! capture  <run>  <test slug>  <fingerprint>  <step>  <state>  <basis>  <note>
//!          <lines hit>  <lines found>  <branches hit>  <branches found>
//!          <functions hit>  <functions found>  <new lines>  <new branches>  <new functions>
//! ```
//!
//! (`capture` is one line.) `<step>` is 0 for the test's start. `<state>` is `captured`,
//! `missing` (nothing was captured there) or `incomplete` (something was, but it can't be
//! used). `<basis>` says what the `<new …>` numbers are: `baseline` (the first capture of the
//! test: the program's state before it), `step` (since the capture of the step before),
//! `since:<n>` (since the capture of step `n`, which is earlier than the one before), `restart`
//! (the counters went down, so the program restarted and nothing is attributed) or `none`.

use crate::coverage::Credit;
use crate::plan::Plan;
use crate::report::Report;
use crate::sidecar::{MAX_ID, MAX_TEXT, clean};

/// Hit and found counts of one kind of thing in the source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub hit: u64,
    pub found: u64,
}

/// What a capture holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureState {
    Captured,
    /// Nothing was captured at this step.
    Missing,
    /// A capture was made but can't be read or is empty.
    Incomplete,
}

impl CaptureState {
    pub fn name(self) -> &'static str {
        match self {
            CaptureState::Captured => "captured",
            CaptureState::Missing => "missing",
            CaptureState::Incomplete => "incomplete",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [
            CaptureState::Captured,
            CaptureState::Missing,
            CaptureState::Incomplete,
        ]
        .into_iter()
        .find(|x| x.name() == s)
    }
}

/// What a capture's "new" numbers are relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    /// The first capture of the test: what the program had run before it.
    Baseline,
    /// Since the capture of the step before.
    Step,
    /// Since the capture of an earlier step, as captures in between are missing.
    Since(usize),
    /// The counters went down: the program restarted, so nothing is attributed.
    Restart,
    None,
}

impl Basis {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "baseline" => Some(Basis::Baseline),
            "step" => Some(Basis::Step),
            "restart" => Some(Basis::Restart),
            "none" => Some(Basis::None),
            _ => s
                .strip_prefix("since:")
                .and_then(|n| n.parse().ok())
                .map(Basis::Since),
        }
    }
}

/// The capture taken at one step of one test of one run.
#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
    pub run: String,
    pub plan: String,
    pub fingerprint: String,
    /// 0 for the test's start.
    pub step: usize,
    pub state: CaptureState,
    pub basis: Basis,
    pub note: String,
    /// The program's totals when the capture was taken (cumulative since it started).
    pub lines: Count,
    pub branches: Count,
    pub functions: Count,
    /// Lines, branches and functions first run since the capture [`Basis`] names, if it
    /// attributes any.
    pub new: Option<[u64; 3]>,
}

/// One code coverage file.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeCoverage {
    /// The file's name, for messages.
    pub file: String,
    /// What was instrumented: a name for the program or package.
    pub name: String,
    /// The source revision that was instrumented (a commit, say).
    pub revision: String,
    /// Which source is counted ("src/**, without tests").
    pub scope: String,
    /// A separately hosted HTML report, as a web address.
    pub report: Option<String>,
    pub captures: Vec<Capture>,
    /// Lines that couldn't be read.
    pub problems: Vec<String>,
}

impl CodeCoverage {
    /// A code coverage file's text. A file without the `expectation-code-coverage 1` header is
    /// an error; a line that can't be read is left out, with why in [`CodeCoverage::problems`].
    pub fn parse(file: &str, text: &str) -> Result<CodeCoverage, String> {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some("expectation-code-coverage 1") {
            return Err(format!(
                "{file}: not a code coverage file (no \"expectation-code-coverage 1\" line)"
            ));
        }
        let mut out = CodeCoverage {
            file: file.to_string(),
            name: String::new(),
            revision: String::new(),
            scope: String::new(),
            report: None,
            captures: vec![],
            problems: vec![],
        };
        for (n, line) in lines.enumerate() {
            let n = n + 2;
            let f: Vec<&str> = line.split('\t').collect();
            match f[0] {
                "source" if f.len() >= 5 => {
                    out.name = clean(f[1], MAX_TEXT);
                    out.revision = clean(f[2], MAX_TEXT);
                    out.scope = clean(f[3], MAX_TEXT);
                    let url = clean(f[4], 2000);
                    // A link on a published page: only a web address, never a script or a file.
                    out.report =
                        (url.starts_with("https://") || url.starts_with("http://")).then_some(url);
                    if out.report.is_none() && f[4] != "-" {
                        out.problems
                            .push(format!("line {n}: the report address isn't http(s)"));
                    }
                }
                "capture" => match capture(&f) {
                    Some(c) => out.captures.push(c),
                    None => out
                        .problems
                        .push(format!("line {n}: can't read this capture")),
                },
                _ => {}
            }
        }
        Ok(out)
    }

    /// The address of the hosted report for one test of one run: the file's, with `{run}` and
    /// `{slug}` in it filled in.
    pub fn report_for(&self, plan: &str, run: &str) -> Option<String> {
        let enc = |s: &str| -> String {
            s.bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                        (b as char).to_string()
                    }
                    b => format!("%{b:02X}"),
                })
                .collect()
        };
        self.report
            .as_ref()
            .map(|u| u.replace("{run}", &enc(run)).replace("{slug}", &enc(plan)))
    }

    /// The captures that count for test `plan` (index `i` in `plans`), whose result is `report`'s:
    /// from the run its result came from, of the test as it is now. In step order.
    pub fn for_plan(&self, plans: &[Plan], report: &Report, i: usize) -> Vec<&Capture> {
        let Some(plan) = plans.get(i) else {
            return vec![];
        };
        let mut found: Vec<&Capture> = self
            .captures
            .iter()
            .filter(|c| c.plan == plan.slug)
            .filter(|c| {
                crate::sidecar::credit(plans, report, &c.plan, &c.fingerprint, &c.run, c.step)
                    == Credit::Counts {
                        plan: i,
                        step: c.step,
                    }
            })
            .collect();
        found.sort_by_key(|c| c.step);
        found
    }
}

fn capture(f: &[&str]) -> Option<Capture> {
    if f.len() < 17 {
        return None;
    }
    let num = |i: usize| -> Option<u64> { f[i].parse().ok() };
    let count = |a: usize| -> Option<Count> {
        Some(Count {
            hit: num(a)?,
            found: num(a + 1)?,
        })
    };
    let state = CaptureState::parse(f[5])?;
    let new = match (f[14], f[15], f[16]) {
        ("-", "-", "-") => None,
        (a, b, c) => Some([a.parse().ok()?, b.parse().ok()?, c.parse().ok()?]),
    };
    Some(Capture {
        run: clean(f[1], MAX_ID),
        plan: clean(f[2], MAX_ID),
        fingerprint: clean(f[3], MAX_ID),
        step: f[4].parse().ok()?,
        state,
        basis: Basis::parse(f[6])?,
        note: clean(f[7], MAX_TEXT),
        lines: count(8)?,
        branches: count(10)?,
        functions: count(12)?,
        new,
    })
}

/// The code coverage file for `name` on `platform`: `app@iOS.code.tsv`, or `app.code.tsv`.
pub fn code_coverage_file(name: &str, platform: Option<&str>) -> String {
    match platform {
        Some(p) => format!("{name}@{p}.code.tsv"),
        None => format!("{name}.code.tsv"),
    }
}

/// Is this file in a results directory a code coverage file?
pub fn is_code_coverage_file(path: &std::path::Path) -> bool {
    crate::sidecar::is_sidecar(path, Some("code"))
}

/// Every code coverage file in `dir` for `platform` (and those for every platform), read, with an
/// error for each that couldn't be.
pub fn load_code_coverage_for(
    dir: &std::path::Path,
    platform: Option<&str>,
) -> (Vec<CodeCoverage>, Vec<String>) {
    crate::sidecar::load(dir, "code", platform, CodeCoverage::parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(run: &str, step: usize, state: &str, basis: &str, tail: &str) -> String {
        format!("capture\t{run}\tgo\tfp\t{step}\t{state}\t{basis}\t-\t{tail}\n")
    }

    #[test]
    fn a_file_is_read_and_unreadable_lines_are_listed() {
        let text = format!(
            "expectation-code-coverage 1\nsource\tapp\tabc123\tsrc/**\thttps://cov.example/{{run}}\n{}{}capture\tr\tgo\tfp\tx\n{}future\tkind\n",
            row("r", 0, "captured", "baseline", "5\t10\t1\t4\t2\t3\t-\t-\t-"),
            row("r", 1, "captured", "step", "7\t10\t2\t4\t2\t3\t2\t1\t0"),
            row("r", 2, "missing", "none", "0\t0\t0\t0\t0\t0\t-\t-\t-"),
        );
        let c = CodeCoverage::parse("app.code.tsv", &text).unwrap();
        assert_eq!((c.name.as_str(), c.revision.as_str()), ("app", "abc123"));
        assert_eq!(c.report.as_deref(), Some("https://cov.example/{run}"));
        assert_eq!(c.captures.len(), 3);
        assert_eq!(c.problems.len(), 1);
        assert_eq!(c.captures[1].new, Some([2, 1, 0]));
        assert_eq!(c.captures[0].basis, Basis::Baseline);
        assert_eq!(c.captures[2].state, CaptureState::Missing);
        assert_eq!(c.captures[1].lines, Count { hit: 7, found: 10 });
    }

    #[test]
    fn a_report_address_must_be_http() {
        let text = "expectation-code-coverage 1\nsource\tapp\t-\t-\tjavascript:alert(1)\n";
        let c = CodeCoverage::parse("a.code.tsv", text).unwrap();
        assert_eq!(c.report, None);
        assert_eq!(c.problems.len(), 1);
        assert!(CodeCoverage::parse("a", "nope\n").is_err());
    }

    #[test]
    fn the_report_address_names_the_run_and_test() {
        let text =
            "expectation-code-coverage 1\nsource\tapp\t-\t-\thttps://c.example/{run}/{slug}.html\n";
        let c = CodeCoverage::parse("a", text).unwrap();
        assert_eq!(
            c.report_for("a-b", "r 1").as_deref(),
            Some("https://c.example/r%201/a-b.html")
        );
    }

    #[test]
    fn spans_and_restarts_are_read() {
        assert_eq!(Basis::parse("since:2"), Some(Basis::Since(2)));
        assert_eq!(Basis::parse("restart"), Some(Basis::Restart));
        assert_eq!(Basis::parse("since:x"), None);
    }
}
