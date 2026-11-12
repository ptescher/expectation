//! Contract coverage: which outcomes a contract describes (an API's responses, an event
//! catalogue, a list of error codes) were seen while the tests ran, which were not, and which
//! nobody can tell.
//!
//! Expectation doesn't read network captures or API specifications itself. An extension does:
//! it reads the run's results (for each step's [`Window`](crate::Window) and the run's id), its
//! evidence (a HAR, a log) and the contract, and writes a coverage file next to the results.
//! This module reads those files and holds them to the same rules whatever wrote them:
//!
//! - Evidence counts only for the run it came from: its run id and the test's fingerprint must
//!   match the result loaded for that test. Evidence from another run, or from a test that has
//!   changed since, is shown as such and covers nothing.
//! - Only evidence tied to a test and a step counts. Traffic nobody can place covers nothing.
//! - "Not observed" stands only if the capture of every run that ran tests is complete.
//!   Otherwise nobody can say, and the outcome is unknown.
//! - None of it changes whether a test passed, or any expectation's verdict.
//!
//! The format is tab-separated, one record per line, `-` for nothing. Unknown kinds of line
//! are skipped, so later versions can add to it:
//!
//! ```text
//! expectation-coverage 1
//! contract      <name>  <kind>  <revision>
//! evidence      <id>  <run>  <test slug>  <fingerprint>  <step>  <attribution>  <source>  <label>
//! item          <id>  <group>  <label>  <location>  <observed|not-observed|unknown>  <reason>
//! seen          <item id>  <evidence id>
//! complete      <run>  <yes|partial|no>  <note>
//! undocumented  <group>  <label>  <evidence id>
//! unmatched     <label>  <evidence id>  <reason>
//! excluded      <group>  <reason>
//! ```
//!
//! `<step>` is 0 for the test's start (resetting and arranging it), 1 for its first step;
//! `<attribution>` is how the evidence was tied to it: `declared` (the driver said so),
//! `timed` (inside the step's window), `spans` (began in that step and ended in a later one),
//! `setup` (while the test was being set up) or `unassigned`. `<source>` is `live`, `replay`,
//! `stand-in`, `fault` (an injected failure) or `unknown`.
//!
//! There is nowhere to put a header, a body or a token, on purpose. Every field is cut to a few
//! hundred characters with control characters removed, and a file over 16 MiB is refused.

use crate::plan::Plan;
use crate::report::Report;
use crate::sidecar::{MAX_ID, MAX_TEXT, clean};

/// How a piece of evidence was tied to a test step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribution {
    /// The driver captured it for that step and said so.
    Declared,
    /// It began and ended inside the step's window.
    Timed,
    /// It began in that step and ended in a later one, so the step only started it.
    Spans,
    /// It happened while the test was being reset and arranged.
    Setup,
    /// Nothing ties it to a test or a step.
    Unassigned,
}

impl Attribution {
    pub fn name(self) -> &'static str {
        match self {
            Attribution::Declared => "declared",
            Attribution::Timed => "timed",
            Attribution::Spans => "spans",
            Attribution::Setup => "setup",
            Attribution::Unassigned => "unassigned",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [
            Attribution::Declared,
            Attribution::Timed,
            Attribution::Spans,
            Attribution::Setup,
            Attribution::Unassigned,
        ]
        .into_iter()
        .find(|a| a.name() == s)
    }

    /// In words, for a reader.
    pub fn describe(self) -> &'static str {
        match self {
            Attribution::Declared => "captured during this step",
            Attribution::Timed => "during this step, by time",
            Attribution::Spans => "started in this step and finished later",
            Attribution::Setup => "while the test was set up",
            Attribution::Unassigned => "not tied to any step",
        }
    }
}

/// Where the evidence came from, as the extension was told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Live,
    Replay,
    StandIn,
    /// An injected failure: it shows the app's handling of the outcome, not that the real
    /// service produces it.
    Fault,
    Unknown,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Live => "live",
            Source::Replay => "replay",
            Source::StandIn => "stand-in",
            Source::Fault => "fault",
            Source::Unknown => "unknown",
        }
    }

    fn parse(s: &str) -> Self {
        [Source::Live, Source::Replay, Source::StandIn, Source::Fault]
            .into_iter()
            .find(|x| x.name() == s)
            .unwrap_or(Source::Unknown)
    }
}

/// What is known about one outcome the contract describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// A test that ran saw it.
    Observed,
    /// Every run's capture was complete and none saw it: a candidate for review, not a defect.
    NotObserved,
    /// Nobody can say: the capture was partial, the body was missing, the evidence was from
    /// another run, or the contract says something the extension can't check.
    Unknown,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Observed => "observed",
            State::NotObserved => "not-observed",
            State::Unknown => "unknown",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [State::Observed, State::NotObserved, State::Unknown]
            .into_iter()
            .find(|x| x.name() == s)
    }
}

/// How much of a run the extension had to look at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness {
    Yes,
    Partial,
    No,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Evidence {
    pub id: String,
    pub run: String,
    /// The test's slug, if anything ties it to one.
    pub plan: Option<String>,
    pub fingerprint: String,
    /// 0 for the test's start, 1 for its first step.
    pub step: Option<usize>,
    pub attribution: Attribution,
    pub source: Source,
    /// What it was, sanitized by the extension: `POST /quote → 429`.
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    /// What it belongs to: `POST /quote`.
    pub group: String,
    /// The outcome: `429 rate limited`.
    pub label: String,
    /// Where the contract says it: `#/paths/~1quote/post/responses/429`.
    pub location: String,
    /// What the extension says.
    pub claimed: State,
    pub reason: String,
    /// The evidence the extension says saw it.
    pub seen: Vec<String>,
}

/// Something seen that the contract doesn't describe, or that matched nothing in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub group: String,
    pub label: String,
    pub evidence: String,
    pub reason: String,
}

/// One coverage file: one contract, and what the extension made of the evidence.
#[derive(Clone, Debug, Default)]
pub struct Contract {
    pub name: String,
    /// What sort of contract: "API contract", "Event catalogue".
    pub kind: String,
    /// Which version of it: a hash or a tag.
    pub revision: String,
    /// The file it was read from, for messages.
    pub file: String,
    pub evidence: Vec<Evidence>,
    pub items: Vec<Item>,
    pub complete: Vec<(String, Completeness, String)>,
    pub undocumented: Vec<Note>,
    pub unmatched: Vec<Note>,
    pub excluded: Vec<(String, String)>,
    /// Lines that were left out, and why.
    pub problems: Vec<String>,
}

/// Field text as it is kept: no control characters, at most `max` characters, `-` for none.
impl Contract {
    /// Its kind, or "Contract coverage" when the file doesn't say.
    pub fn kind_or_default(&self) -> String {
        if self.kind.is_empty() {
            "Contract coverage".into()
        } else {
            self.kind.clone()
        }
    }

    /// A coverage file's text. A file that isn't one (no `expectation-coverage 1` header) is
    /// an error; a line that can't be read is left out, with why in [`Contract::problems`].
    pub fn parse(file: &str, text: &str) -> Result<Contract, String> {
        let mut lines = text.lines();
        match lines.next().map(str::trim_end) {
            Some("expectation-coverage 1") => {}
            Some(h) if h.starts_with("expectation-coverage ") => {
                return Err(format!(
                    "{file}: a coverage file of a version this can't read ({h})"
                ));
            }
            _ => {
                return Err(format!(
                    "{file}: not a coverage file (no `expectation-coverage 1` line)"
                ));
            }
        }
        let mut c = Contract {
            file: file.to_string(),
            ..Default::default()
        };
        let id = |s: &str| clean(s, MAX_ID);
        let text = |s: &str| clean(s, MAX_TEXT);
        let mut problems = vec![];
        for (n, line) in lines.enumerate() {
            let at = n + 2;
            let f: Vec<&str> = line.split('\t').collect();
            let mut problem = |why: &str| problems.push(format!("line {at}: {why}"));
            match f.as_slice() {
                ["contract", name, kind, revision] => {
                    c.name = text(name);
                    c.kind = text(kind);
                    c.revision = text(revision);
                }
                [
                    "evidence",
                    ev,
                    run,
                    plan,
                    print,
                    step,
                    attribution,
                    source,
                    label,
                ] => {
                    let ev = id(ev);
                    if ev.is_empty() || c.evidence.iter().any(|e| e.id == ev) {
                        problem("evidence needs an id of its own");
                        continue;
                    }
                    let Some(attribution) = Attribution::parse(attribution) else {
                        problem("unknown attribution");
                        continue;
                    };
                    let step = match *step {
                        "-" => None,
                        s => match s.parse::<usize>() {
                            Ok(n) => Some(n),
                            Err(_) => {
                                problem("a step is a number or -");
                                continue;
                            }
                        },
                    };
                    let plan = Some(id(plan)).filter(|p| !p.is_empty());
                    // Evidence that names no test, or no step, is tied to nothing, whatever it says.
                    let attribution = if plan.is_none() || step.is_none() {
                        Attribution::Unassigned
                    } else {
                        attribution
                    };
                    c.evidence.push(Evidence {
                        id: ev,
                        run: id(run),
                        plan,
                        fingerprint: id(print),
                        step,
                        attribution,
                        source: Source::parse(source),
                        label: text(label),
                    });
                }
                ["item", item, group, label, location, state, reason] => {
                    let item = id(item);
                    if item.is_empty() || c.items.iter().any(|i| i.id == item) {
                        problem("an item needs an id of its own");
                        continue;
                    }
                    let Some(claimed) = State::parse(state) else {
                        problem("an item is observed, not-observed or unknown");
                        continue;
                    };
                    c.items.push(Item {
                        id: item,
                        group: text(group),
                        label: text(label),
                        location: text(location),
                        claimed,
                        reason: text(reason),
                        seen: vec![],
                    });
                }
                ["seen", item, ev] => {
                    let (item, ev) = (id(item), id(ev));
                    match c.items.iter_mut().find(|i| i.id == item) {
                        Some(i) if c.evidence.iter().any(|e| e.id == ev) => {
                            if !i.seen.contains(&ev) {
                                i.seen.push(ev)
                            }
                        }
                        Some(_) => problem("seen by evidence that isn't listed before it"),
                        None => problem("seen for an item that isn't listed before it"),
                    }
                }
                ["complete", run, how, note] => {
                    let how = match *how {
                        "yes" => Completeness::Yes,
                        "partial" => Completeness::Partial,
                        "no" => Completeness::No,
                        _ => {
                            problem("complete is yes, partial or no");
                            continue;
                        }
                    };
                    c.complete.push((id(run), how, text(note)));
                }
                ["undocumented", group, label, ev] => c.undocumented.push(Note {
                    group: text(group),
                    label: text(label),
                    evidence: id(ev),
                    reason: String::new(),
                }),
                ["unmatched", label, ev, reason] => c.unmatched.push(Note {
                    group: String::new(),
                    label: text(label),
                    evidence: id(ev),
                    reason: text(reason),
                }),
                ["excluded", group, reason] => c.excluded.push((text(group), text(reason))),
                [kind, ..]
                    if [
                        "contract",
                        "evidence",
                        "item",
                        "seen",
                        "complete",
                        "undocumented",
                        "unmatched",
                        "excluded",
                    ]
                    .contains(kind) =>
                {
                    problem(&format!("a `{kind}` line with the wrong number of fields"))
                }
                // A kind of line from a later version, or a blank one.
                _ => {}
            }
        }
        if c.name.is_empty() {
            c.name = file.to_string();
        }
        c.problems = problems;
        Ok(c)
    }
}

/// Whether a piece of evidence counts for the results it is shown with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credit {
    /// It is from the run of the result shown for its test, and the test is as it was.
    Counts { plan: usize, step: usize },
    /// It names no test or step.
    Unassigned,
    /// No such test in the suite.
    NoSuchTest,
    /// The test has no such step.
    NoSuchStep,
    /// The test has changed since: different steps, so a different test.
    TestChanged,
    /// The result shown for the test is from another run, or from none.
    OtherRun,
    /// The result shown for the test says it didn't run: not supported yet, pending, not run.
    DidNotRun,
}

impl Credit {
    pub fn counts(self) -> bool {
        matches!(self, Credit::Counts { .. })
    }

    /// Why it doesn't count, for a reader.
    pub fn why_not(self) -> Option<&'static str> {
        match self {
            Credit::Counts { .. } => None,
            Credit::Unassigned => Some("not tied to a test step"),
            Credit::NoSuchTest => Some("from a test that no longer exists"),
            Credit::NoSuchStep => Some("from a step the test doesn't have"),
            Credit::TestChanged => Some("from the test before it changed"),
            Credit::OtherRun => Some("from another run, not the one these results are from"),
            Credit::DidNotRun => Some("from a test these results say didn't run"),
        }
    }
}

/// An item as the results bear it out.
#[derive(Clone, Debug)]
pub struct Judged {
    pub state: State,
    /// Why, when it isn't what the extension claimed, or the extension's reason.
    pub why: String,
    /// The evidence that counts for it, by index into [`Contract::evidence`].
    pub by: Vec<usize>,
}

/// A contract, held to the results: what counts, and what each item really is.
#[derive(Clone, Debug)]
pub struct Coverage {
    pub contract: Contract,
    /// One per piece of evidence, in order.
    pub credit: Vec<Credit>,
    /// One per item, in order.
    pub judged: Vec<Judged>,
    /// Runs that ran tests, by id, with how complete the extension says its capture was.
    pub runs: Vec<(String, Option<Completeness>)>,
}

impl Coverage {
    /// Hold `contract` to `report`, the results of `plans` as they are shown.
    pub fn judge(contract: Contract, plans: &[Plan], report: &Report) -> Coverage {
        let credit: Vec<Credit> = contract
            .evidence
            .iter()
            .map(|e| credit(e, plans, report))
            .collect();
        // The runs these results are from, that ran something that could make traffic.
        let mut runs: Vec<(String, Option<Completeness>)> = vec![];
        for r in report.results.iter().filter(|r| r.status.ran()) {
            if let Some(run) = &r.run
                && !runs.iter().any(|(id, _)| *id == run.id)
            {
                let how = contract
                    .complete
                    .iter()
                    .filter(|(id, _, _)| *id == run.id)
                    .map(|(_, how, _)| *how)
                    // The least complete, if it says more than once.
                    .min_by_key(|h| match h {
                        Completeness::No => 0,
                        Completeness::Partial => 1,
                        Completeness::Yes => 2,
                    });
                runs.push((run.id.clone(), how));
            }
        }
        // Results saved before runs had ids can't be matched to any capture.
        let unnamed = report
            .results
            .iter()
            .any(|r| r.status.ran() && r.run.is_none());
        let judged = contract
            .items
            .iter()
            .map(|item| {
                let by: Vec<usize> = item
                    .seen
                    .iter()
                    .filter_map(|ev| contract.evidence.iter().position(|e| e.id == *ev))
                    .filter(|&i| credit[i].counts())
                    .collect();
                if !by.is_empty() {
                    return Judged {
                        state: State::Observed,
                        why: String::new(),
                        by,
                    };
                }
                let unknown = |why: String| Judged {
                    state: State::Unknown,
                    why,
                    by: vec![],
                };
                match item.claimed {
                    State::Unknown => unknown(item.reason.clone()),
                    State::Observed => {
                        let why = item
                            .seen
                            .iter()
                            .filter_map(|ev| contract.evidence.iter().position(|e| e.id == *ev))
                            .find_map(|i| credit[i].why_not())
                            .unwrap_or("no evidence that counts");
                        unknown(format!("seen only in evidence {why}"))
                    }
                    State::NotObserved => {
                        if unnamed {
                            unknown("some results are from a run with no id, so no capture can be matched to it".into())
                        } else if runs.is_empty() {
                            unknown("no test that ran has results to match a capture to".into())
                        } else if let Some((id, how)) =
                            runs.iter().find(|(_, how)| *how != Some(Completeness::Yes))
                        {
                            unknown(match how {
                                None => format!("nothing says the capture of run {id} is complete"),
                                Some(_) => format!("the capture of run {id} is incomplete"),
                            })
                        } else {
                            Judged {
                                state: State::NotObserved,
                                why: item.reason.clone(),
                                by: vec![],
                            }
                        }
                    }
                }
            })
            .collect();
        Coverage {
            contract,
            credit,
            judged,
            runs,
        }
    }

    /// Is this item's group excluded from the count?
    pub fn excluded(&self, item: usize) -> bool {
        let group = &self.contract.items[item].group;
        self.contract.excluded.iter().any(|(g, _)| g == group)
    }

    /// How many items are in each state, leaving out excluded groups.
    pub fn count(&self, state: State) -> usize {
        self.judged
            .iter()
            .enumerate()
            .filter(|(i, j)| j.state == state && !self.excluded(*i))
            .count()
    }

    /// The groups, in the order they first appear.
    pub fn groups(&self) -> Vec<&str> {
        let mut out: Vec<&str> = vec![];
        for i in &self.contract.items {
            if !out.contains(&i.group.as_str()) {
                out.push(&i.group);
            }
        }
        out
    }

    /// The evidence with this id, and whether it counts.
    pub fn evidence(&self, id: &str) -> Option<(&Evidence, Credit)> {
        let i = self.contract.evidence.iter().position(|e| e.id == id)?;
        Some((&self.contract.evidence[i], self.credit[i]))
    }
}

fn credit(e: &Evidence, plans: &[Plan], report: &Report) -> Credit {
    let (Some(slug), Some(step)) = (&e.plan, e.step) else {
        return Credit::Unassigned;
    };
    if e.attribution == Attribution::Unassigned {
        return Credit::Unassigned;
    }
    crate::sidecar::credit(plans, report, slug, &e.fingerprint, &e.run, step)
}

impl Coverage {
    /// The contract as the results bear it out, for machines: every item with its state, the
    /// evidence that counts for it (with the test and step), and everything that didn't count
    /// and why. The same facts as the manual's page.
    pub fn json(&self, plans: &[Plan]) -> String {
        let q = |s: &str| {
            let mut o = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => o.push_str("\\\""),
                    '\\' => o.push_str("\\\\"),
                    c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                    c => o.push(c),
                }
            }
            o.push('"');
            o
        };
        let list = |items: Vec<String>| format!("[{}]", items.join(", "));
        let c = &self.contract;
        let evidence = |i: usize| {
            let e = &c.evidence[i];
            let (test, step) = match self.credit[i] {
                Credit::Counts { plan, step } => (q(&plans[plan].slug), step.to_string()),
                _ => ("null".into(), "null".into()),
            };
            format!(
                "{{\"id\": {}, \"label\": {}, \"run\": {}, \"test\": {test}, \"step\": {step}, \"attribution\": {}, \"source\": {}, \"counts\": {}, \"why_not\": {}}}",
                q(&e.id),
                q(&e.label),
                q(&e.run),
                q(e.attribution.name()),
                q(e.source.name()),
                self.credit[i].counts(),
                self.credit[i].why_not().map_or("null".into(), q)
            )
        };
        let items: Vec<String> = c
            .items
            .iter()
            .zip(&self.judged)
            .enumerate()
            .map(|(n, (item, j))| {
                format!(
                    "{{\"id\": {}, \"group\": {}, \"label\": {}, \"location\": {}, \"state\": {}, \"claimed\": {}, \"why\": {}, \"excluded\": {}, \"evidence\": {}}}",
                    q(&item.id),
                    q(&item.group),
                    q(&item.label),
                    q(&item.location),
                    q(j.state.name()),
                    q(item.claimed.name()),
                    q(&j.why),
                    self.excluded(n),
                    list(j.by.iter().map(|&i| q(&c.evidence[i].id)).collect())
                )
            })
            .collect();
        let notes = |v: &[Note]| {
            list(
                v.iter()
                    .map(|n| {
                        format!(
                            "{{\"group\": {}, \"label\": {}, \"evidence\": {}, \"reason\": {}}}",
                            q(&n.group),
                            q(&n.label),
                            q(&n.evidence),
                            q(&n.reason)
                        )
                    })
                    .collect(),
            )
        };
        let runs = list(
            self.runs
                .iter()
                .map(|(id, how)| {
                    let how = match how {
                        Some(Completeness::Yes) => q("yes"),
                        Some(Completeness::Partial) => q("partial"),
                        Some(Completeness::No) => q("no"),
                        None => "null".into(),
                    };
                    format!("{{\"id\": {}, \"complete\": {how}}}", q(id))
                })
                .collect(),
        );
        format!(
            "{{\n  \"contract\": {},\n  \"kind\": {},\n  \"revision\": {},\n  \"counts\": {{\"observed\": {}, \"not_observed\": {}, \"unknown\": {}}},\n  \"runs\": {runs},\n  \"items\": {},\n  \"evidence\": {},\n  \"undocumented\": {},\n  \"unmatched\": {},\n  \"excluded\": {},\n  \"problems\": {}\n}}\n",
            q(&c.name),
            q(&c.kind),
            q(&c.revision),
            self.count(State::Observed),
            self.count(State::NotObserved),
            self.count(State::Unknown),
            list(items),
            list((0..c.evidence.len()).map(evidence).collect()),
            notes(&c.undocumented),
            notes(&c.unmatched),
            list(
                c.excluded
                    .iter()
                    .map(|(g, r)| format!("{{\"group\": {}, \"reason\": {}}}", q(g), q(r)))
                    .collect()
            ),
            list(c.problems.iter().map(|p| q(p)).collect())
        )
    }
}

/// The coverage file for `name` on `platform`: `api@iOS.coverage.tsv`, or `api.coverage.tsv`
/// for one that is the same on every platform.
pub fn coverage_file(name: &str, platform: Option<&str>) -> String {
    match platform {
        Some(p) => format!("{name}@{p}.coverage.tsv"),
        None => format!("{name}.coverage.tsv"),
    }
}

/// Is this file in a results directory a contract coverage file?
pub fn is_coverage_file(path: &std::path::Path) -> bool {
    crate::sidecar::is_sidecar(path, Some("coverage"))
}

/// Every coverage file in `dir` for `platform` (and those for every platform), read, with an
/// error for each that couldn't be. In file order.
pub fn load_contracts_for(
    dir: &std::path::Path,
    platform: Option<&str>,
) -> (Vec<Contract>, Vec<String>) {
    crate::sidecar::load(dir, "coverage", platform, Contract::parse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::plan_stories;
    use crate::report::fingerprint;
    use crate::report::{PlanResult, Run, Status};

    const SUITE: &str = "
Vocabulary:
  State: the quote is none | shown | refused
  Event: the user asks for a quote

Background:
  Given the quote is none

Feature: Quotes
  Expectation: Asking shows a quote
    Given the quote is none
    When the user asks for a quote
    Then the quote is shown

Story: Get a quote
  When the user asks for a quote
  Then the quote is shown
";

    fn plans() -> Vec<Plan> {
        let l = expectation_language::load_sources("t", &[("t.expect", SUITE)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let a = expectation_graph::analyze(&l.suite);
        plan_stories(&l.suite, &a).plans
    }

    fn run(id: &str) -> Run {
        Run {
            id: id.into(),
            started_ms: 1,
            provenance: Default::default(),
        }
    }

    /// The plans' results, all `status`, from run `id` (none for `None`).
    fn report(plans: &[Plan], status: Status, id: Option<&str>) -> Report {
        Report {
            results: plans
                .iter()
                .map(|p| {
                    let mut r = PlanResult::with(status, p);
                    r.run = id.map(run);
                    r
                })
                .collect(),
        }
    }

    /// The quote fixture from the issue: one operation, a success and a rate limit, and an enum.
    fn quote(plan: &Plan, run: &str, complete: &str) -> String {
        let print = fingerprint(plan);
        let slug = &plan.slug;
        format!(
            "expectation-coverage 1
contract\tQuote API\tAPI contract\tsha256:abc
evidence\te1\t{run}\t{slug}\t{print}\t1\ttimed\tlive\tPOST /quote → 200
item\tok\tPOST /quote\t200 quote\t#/paths/~1quote/post/responses/200\tobserved\t-
item\tlimited\tPOST /quote\t429 rate limited\t#/paths/~1quote/post/responses/429\tnot-observed\t-
item\tfirm\tPOST /quote\tkind: firm\t#/components/schemas/Quote/properties/kind\tobserved\t-
item\tindicative\tPOST /quote\tkind: indicative\t#/components/schemas/Quote/properties/kind\tnot-observed\t-
seen\tok\te1
seen\tfirm\te1
complete\t{run}\t{complete}\t-
undocumented\tPOST /quote\t418 teapot\te1
"
        )
    }

    fn states(c: &Coverage) -> Vec<(&str, State)> {
        c.contract
            .items
            .iter()
            .zip(&c.judged)
            .map(|(i, j)| (i.id.as_str(), j.state))
            .collect()
    }

    #[test]
    fn the_quote_fixture_reports_what_was_seen_and_what_was_not() {
        let plans = plans();
        let c = Contract::parse("api.coverage.tsv", &quote(&plans[0], "r1", "yes")).unwrap();
        assert!(c.problems.is_empty(), "{:?}", c.problems);
        let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, Some("r1")));
        assert_eq!(
            states(&cov),
            [
                ("ok", State::Observed),
                ("limited", State::NotObserved),
                ("firm", State::Observed),
                ("indicative", State::NotObserved),
            ]
        );
        // The observation links back to the test and the step that produced it.
        assert_eq!(cov.credit[0], Credit::Counts { plan: 0, step: 1 });
        assert_eq!(cov.judged[0].by, [0]);
        assert_eq!(
            (cov.count(State::Observed), cov.count(State::NotObserved)),
            (2, 2)
        );
        assert_eq!(cov.contract.undocumented[0].label, "418 teapot");
        assert_eq!(cov.contract.revision, "sha256:abc");
    }

    #[test]
    fn evidence_from_another_run_covers_nothing() {
        let plans = plans();
        let c = Contract::parse("f", &quote(&plans[0], "r1", "yes")).unwrap();
        let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, Some("r2")));
        assert_eq!(cov.credit[0], Credit::OtherRun);
        // Not observed either: nothing says the capture of r2, the run shown, was complete.
        assert!(cov.judged.iter().all(|j| j.state == State::Unknown));
        assert!(
            cov.judged[0].why.contains("another run"),
            "{}",
            cov.judged[0].why
        );
        assert!(cov.judged[1].why.contains("r2"), "{}", cov.judged[1].why);
    }

    #[test]
    fn evidence_from_a_changed_test_or_a_test_that_did_not_run_covers_nothing() {
        let plans = plans();
        let text = quote(&plans[0], "r1", "yes").replace(&fingerprint(&plans[0]), "0000");
        let c = Contract::parse("f", &text).unwrap();
        let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, Some("r1")));
        assert_eq!(cov.credit[0], Credit::TestChanged);
        assert_eq!(cov.judged[0].state, State::Unknown);

        for status in [Status::Unsupported, Status::Pending, Status::NotRun] {
            let c = Contract::parse("f", &quote(&plans[0], "r1", "yes")).unwrap();
            let cov = Coverage::judge(c, &plans, &report(&plans, status, Some("r1")));
            assert_eq!(cov.credit[0], Credit::DidNotRun, "{status:?}");
            assert_eq!(cov.judged[0].state, State::Unknown);
            // A test that didn't run can't make traffic, so nothing at all ran: no absence either.
            assert_eq!(cov.judged[1].state, State::Unknown);
        }
    }

    #[test]
    fn a_failed_run_still_observes_but_never_passes() {
        let plans = plans();
        let c = Contract::parse("f", &quote(&plans[0], "r1", "yes")).unwrap();
        let shown = report(&plans, Status::Failed, Some("r1"));
        let cov = Coverage::judge(c, &plans, &shown);
        assert_eq!(cov.judged[0].state, State::Observed);
        assert_eq!(shown.results[0].status, Status::Failed);
    }

    #[test]
    fn absence_needs_a_complete_capture_of_every_run() {
        let plans = plans();
        for how in ["partial", "no"] {
            let c = Contract::parse("f", &quote(&plans[0], "r1", how)).unwrap();
            let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, Some("r1")));
            assert_eq!(cov.judged[1].state, State::Unknown, "{how}");
            assert!(cov.judged[1].why.contains("incomplete"));
            // What was seen is still seen.
            assert_eq!(cov.judged[0].state, State::Observed);
        }
        // Nothing about completeness at all.
        let text = quote(&plans[0], "r1", "yes").replace("complete\tr1\tyes\t-\n", "");
        let c = Contract::parse("f", &text).unwrap();
        let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, Some("r1")));
        assert_eq!(cov.judged[1].state, State::Unknown);
        // Results saved before runs had ids.
        let c = Contract::parse("f", &quote(&plans[0], "r1", "yes")).unwrap();
        let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, None));
        assert_eq!(cov.judged[1].state, State::Unknown);
        assert!(cov.judged[1].why.contains("no id"));
    }

    #[test]
    fn unassigned_traffic_covers_nothing_whatever_the_extension_claims() {
        let plans = plans();
        for (step, attribution) in [("1", "unassigned"), ("-", "timed")] {
            let text = quote(&plans[0], "r1", "yes")
                .replace("\t1\ttimed\t", &format!("\t{step}\t{attribution}\t"));
            let c = Contract::parse("f", &text).unwrap();
            assert_eq!(c.evidence[0].attribution, Attribution::Unassigned);
            let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, Some("r1")));
            assert_eq!(cov.credit[0], Credit::Unassigned);
            assert_eq!(cov.judged[0].state, State::Unknown);
        }
    }

    #[test]
    fn unknown_stays_unknown_and_excluded_groups_are_not_counted() {
        let plans = plans();
        let text = quote(&plans[0], "r1", "yes")
            + "item\tstream\tGET /prices\tSSE messages\t#/paths/~1prices/get\tunknown\tstreams are not assessed\n"
            + "excluded\tGET /prices\tnot used by this app\n";
        let c = Contract::parse("f", &text).unwrap();
        let cov = Coverage::judge(c, &plans, &report(&plans, Status::Passed, Some("r1")));
        assert_eq!(cov.judged[4].state, State::Unknown);
        assert_eq!(cov.judged[4].why, "streams are not assessed");
        assert!(cov.excluded(4));
        assert_eq!(cov.count(State::Unknown), 0);
        assert_eq!(cov.groups(), ["POST /quote", "GET /prices"]);
    }

    #[test]
    fn a_coverage_file_is_read_defensively() {
        assert!(Contract::parse("f", "expectation-results 1\n").is_err());
        assert!(Contract::parse("f", "expectation-coverage 2\n").is_err());
        let long = "x".repeat(5000);
        let text = format!(
            "expectation-coverage 1
contract\tA\u{7}PI\t{long}\t-
item\ti\tg\tl\t-\tmaybe\t-
item\ti\tg\tl\t-\tunknown\t-
item\ti\tg\tl\t-\tunknown\t-
seen\ti\tnope
evidence\te\tr\t-\t-\t-\tsomehow\t-\t-
evidence\tonly\tsix\tfields
future\twhatever
"
        );
        let c = Contract::parse("f", &text).unwrap();
        assert_eq!(c.name, "API", "control characters are dropped");
        assert_eq!(c.kind.chars().count(), MAX_TEXT, "long fields are cut");
        assert_eq!(c.revision, "", "- is nothing");
        assert_eq!(c.items.len(), 1);
        assert_eq!(c.evidence.len(), 0);
        assert_eq!(c.problems.len(), 5, "{:?}", c.problems);
        assert!(c.problems[0].starts_with("line 3:"));
    }

    #[test]
    fn coverage_files_are_found_by_platform_and_are_not_results() {
        let dir = std::env::temp_dir().join(format!("expectation-coverage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str, contract: &str| {
            std::fs::write(
                dir.join(name),
                format!("expectation-coverage 1\ncontract\t{contract}\t-\t-\n"),
            )
            .unwrap()
        };
        file("api.coverage.tsv", "everywhere");
        file(&coverage_file("api", Some("iOS")), "iOS");
        file(&coverage_file("api", Some("Android")), "Android");
        std::fs::write(dir.join("bad.coverage.tsv"), "not one").unwrap();
        let (found, errors) = load_contracts_for(&dir, Some("iOS"));
        let names: Vec<&str> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["everywhere", "iOS"]);
        assert_eq!(errors.len(), 1);
        // The results loader leaves them alone.
        let plans = plans();
        let r = crate::report::load_results_for(&dir, &plans, Some("iOS"));
        assert_eq!(r.results[0].status, Status::NotRun);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
