//! Where each feature, expectation and story stands, for a board or a CI gate: approved or
//! not, built or pending, and what the tests said. One row per feature, with its expectations,
//! and one per story; everything comes from the suite and the results the runs saved.
//!
//! Approval, being built and being verified are three separate facts and are kept apart. A
//! feature that is approved and failing is a different card from one that is proposed and
//! verified.

use crate::plan::{Origin, Plan};
use crate::report::{Report, Status, Verdict};
use expectation_model::{Approval, Suite};

/// How a group of expectations stands, from the worst of what its tests said. Expectations
/// marked `Status: pending` are left out unless nothing else is there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rollup {
    Failed,
    Unsupported,
    Untested,
    NotRun,
    /// Everything in it is written ahead of the behaviour.
    Pending,
    Verified,
}

impl Rollup {
    pub fn name(self) -> &'static str {
        match self {
            Rollup::Failed => "failed",
            Rollup::Unsupported => "unsupported",
            Rollup::Untested => "untested",
            Rollup::NotRun => "not_run",
            Rollup::Pending => "pending",
            Rollup::Verified => "verified",
        }
    }
}

fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Verified => "verified",
        Verdict::Failed => "failed",
        Verdict::Untested => "untested",
        Verdict::Unsupported => "unsupported",
        Verdict::Pending => "pending",
        Verdict::NotRun => "not_run",
    }
}

pub fn rollup(verdicts: &[Verdict]) -> Rollup {
    let live: Vec<Verdict> = verdicts
        .iter()
        .copied()
        .filter(|v| *v != Verdict::Pending)
        .collect();
    if live.is_empty() {
        return if verdicts.is_empty() {
            Rollup::Untested
        } else {
            Rollup::Pending
        };
    }
    let has = |w: Verdict| live.contains(&w);
    if has(Verdict::Failed) {
        Rollup::Failed
    } else if has(Verdict::Unsupported) {
        Rollup::Unsupported
    } else if has(Verdict::Untested) {
        Rollup::Untested
    } else if has(Verdict::NotRun) {
        Rollup::NotRun
    } else {
        Rollup::Verified
    }
}

/// What the approval of a group comes to: `approved` if every member is, `proposed` if none is
/// approved and some say proposed, `partial` if some are, `unmarked` if nothing says.
fn approval_of(marks: &[Option<Approval>]) -> &'static str {
    let approved = marks
        .iter()
        .filter(|m| **m == Some(Approval::Approved))
        .count();
    let proposed = marks
        .iter()
        .filter(|m| **m == Some(Approval::Proposed))
        .count();
    if !marks.is_empty() && approved == marks.len() {
        "approved"
    } else if approved > 0 {
        "partial"
    } else if proposed > 0 {
        "proposed"
    } else {
        "unmarked"
    }
}

fn approval_name(a: Option<Approval>) -> &'static str {
    match a {
        Some(Approval::Approved) => "approved",
        Some(Approval::Proposed) => "proposed",
        None => "unmarked",
    }
}

fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn status_name(s: Status) -> String {
    s.name().replace('-', "_")
}

/// The standing of every feature and story as JSON, in the suite's order. `plans` and `report`
/// are the ones the run used (stories' plans and the generated ones), so each expectation's
/// verdict is the one the manual shows.
pub fn status_json(suite: &Suite, plans: &[Plan], report: &Report) -> String {
    let verdicts = report.verdicts(suite, plans);
    let mut features = vec![];
    for f in &suite.features {
        let exps: Vec<_> = f
            .expectations
            .iter()
            .map(|id| &suite.expectations[id.0 as usize])
            .collect();
        let vs: Vec<Verdict> = exps.iter().map(|e| verdicts[e.id.0 as usize]).collect();
        let marks: Vec<_> = exps.iter().map(|e| e.approval).collect();
        let pending = exps.iter().filter(|e| e.pending).count();
        let mut rows = vec![];
        for (e, v) in exps.iter().zip(&vs) {
            rows.push(format!(
                "{{\"name\":{},\"slug\":{},\"approval\":{},\"pending\":{},\"verdict\":{}}}",
                q(&e.name),
                q(&e.slug),
                q(approval_name(e.approval)),
                e.pending,
                q(verdict_name(*v))
            ));
        }
        let section = &suite.sections[f.section.0 as usize];
        features.push(format!(
            "    {{\"name\":{},\"slug\":{},\"section\":{},\"file\":{},\"line\":{},\"approval\":{},\"verdict\":{},\"pending\":{},\"expectations\":[{}]}}",
            q(&f.name),
            q(&f.slug),
            q(&section.path),
            q(&f.span.file),
            f.span.line,
            q(approval_of(&marks)),
            q(rollup(&vs).name()),
            pending,
            rows.join(",")
        ));
    }
    let mut stories = vec![];
    for s in &suite.stories {
        // The story's own scenarios, and how each did.
        let mut scenarios = vec![];
        let mut statuses = vec![];
        for (i, p) in plans.iter().enumerate() {
            if let Origin::Story { story, .. } = p.origin
                && story == s.id
            {
                let st = report.results.get(i).map_or(Status::NotRun, |r| r.status);
                statuses.push(st);
                scenarios.push(format!(
                    "{{\"title\":{},\"slug\":{},\"status\":{}}}",
                    q(&p.title),
                    q(&p.slug),
                    q(&status_name(st))
                ));
            }
        }
        let overall = if statuses.contains(&Status::Failed) {
            "failed"
        } else if statuses.is_empty() || statuses.contains(&Status::NotRun) {
            "not_run"
        } else if statuses.contains(&Status::Unsupported) {
            "unsupported"
        } else if statuses.contains(&Status::Pending) {
            "pending"
        } else if statuses.iter().all(|s| *s == Status::Passed) {
            "passed"
        } else {
            "skipped"
        };
        stories.push(format!(
            "    {{\"name\":{},\"slug\":{},\"file\":{},\"line\":{},\"approval\":{},\"status\":{},\"scenarios\":[{}]}}",
            q(&s.name),
            q(&s.slug),
            q(&s.span.file),
            s.span.line,
            q(approval_name(s.approval)),
            q(overall),
            scenarios.join(",")
        ));
    }
    format!(
        "{{\n  \"suite\": {},\n  \"features\": [\n{}\n  ],\n  \"stories\": [\n{}\n  ]\n}}\n",
        q(&suite.name),
        features.join(",\n"),
        stories.join(",\n")
    )
}
