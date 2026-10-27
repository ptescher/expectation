//! The controls of the program under test (its buttons, fields, switches: anything a person can
//! act on), as the driver listed them, and which of them the tests used.
//!
//! The driver supplies the inventory while the tests run, so the framework needs to know nothing
//! about a UI toolkit; this joins it to the plans that used each control.

use crate::plan::{Origin, Plan};
use crate::report::{Report, Status};
use expectation_model::{EventId, ExpId, Suite, text};

/// One thing a person can act on, as the driver found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Control {
    /// Stable between runs and independent of display copy (an accessibility id, a test id).
    pub id: String,
    /// What sort of control it is: "button", "text field", "switch", ...
    pub kind: String,
    /// What the person sees on it, if anything.
    pub label: String,
    /// The screen, dialog or other place it is in; controls with the same id in different views
    /// are different controls.
    pub view: String,
    /// The vocabulary event ("the user presses lock") that using it makes happen, if the
    /// integration knows. Matched to the vocabulary the way steps are, ignoring case and a
    /// leading "the".
    pub event: Option<String>,
}

impl Control {
    pub fn new(
        view: impl Into<String>,
        id: impl Into<String>,
        kind: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Control {
            view: view.into(),
            id: id.into(),
            kind: kind.into(),
            label: label.into(),
            event: None,
        }
    }

    /// Says which vocabulary event using this control makes happen.
    pub fn mapped_to(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// What `used` lists: the view and id, which together say which control it is.
    pub fn key(&self) -> String {
        format!("{}\u{1f}{}", self.view, self.id)
    }
}

/// Why a control is a gap in what is specified, documented or verified. A control can have
/// several; they are listed from the model outwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    /// Nothing says which event using it makes happen, so no expectation can be about it.
    NoEventMapping,
    /// It is mapped to an event that isn't in the vocabulary.
    UnknownEvent,
    /// Its event is in the vocabulary, but no expectation is triggered by it.
    NoExpectation,
    /// Every expectation about it is pending: written, not built.
    OnlyPending,
    /// No story performs its event in its view, and none of the tests that used it was a story.
    NoAuthoredStory,
    /// No test used it.
    NeverUsed,
    /// It was used, but no test that used it passed (they failed, or there is no result).
    NoPassingExecution,
}

impl Gap {
    pub fn text(self) -> &'static str {
        match self {
            Gap::NoEventMapping => "no event mapping",
            Gap::UnknownEvent => "event not in the vocabulary",
            Gap::NoExpectation => "no expectation",
            Gap::OnlyPending => "only pending expectations",
            Gap::NoAuthoredStory => "no authored story",
            Gap::NeverUsed => "never used by a test",
            Gap::NoPassingExecution => "no passing execution",
        }
    }

    /// A stable word for machines.
    pub fn key(self) -> &'static str {
        match self {
            Gap::NoEventMapping => "no-event-mapping",
            Gap::UnknownEvent => "unknown-event",
            Gap::NoExpectation => "no-expectation",
            Gap::OnlyPending => "only-pending",
            Gap::NoAuthoredStory => "no-authored-story",
            Gap::NeverUsed => "never-used",
            Gap::NoPassingExecution => "no-passing-execution",
        }
    }
}

/// One control: what is specified about it, the tests that used it, and what is missing.
#[derive(Clone, Debug)]
pub struct ControlRow {
    pub control: Control,
    /// The vocabulary event it is mapped to, if that is one.
    pub event: Option<EventId>,
    /// The expectations triggered by that event: the behaviour specified for it. The event is
    /// the same in every view, so this is the model's, not the view's.
    pub specified: Vec<ExpId>,
    /// Indices into the plans of the stories that used it, in a run.
    pub stories: Vec<usize>,
    /// Indices of the generated tests that used it. A generated test is evidence that it works,
    /// not documentation of it, so it is counted apart from the stories.
    pub generated: Vec<usize>,
    /// Stories that perform its event in a section named like its view but did not use it in a
    /// run (they could not run, or have not): documented, not verified.
    pub documented: Vec<usize>,
    /// How many of the tests that used it passed, and how many failed.
    pub passed: usize,
    pub failed: usize,
    /// What became of the documented stories that did not run: not run, skipped, unsupported,
    /// pending, with how many plans each.
    pub waiting: Vec<(Status, usize)>,
    pub gaps: Vec<Gap>,
}

impl ControlRow {
    pub fn used(&self) -> bool {
        !self.stories.is_empty() || !self.generated.is_empty()
    }

    /// Used by a test that went on to pass. A control that was only used by tests that failed,
    /// or whose result is unknown, is not shown to work.
    pub fn verified(&self) -> bool {
        self.passed > 0 && self.failed == 0
    }

    /// Every authored story about it, those that used it and those that only document it.
    pub fn authored(&self) -> Vec<usize> {
        let mut all = self.stories.clone();
        all.extend(&self.documented);
        all.sort_unstable();
        all.dedup();
        all
    }
}

/// Every control any test saw, and every one in `declared` (an inventory an integration
/// supplies, so a control no run ever reached still shows), in view and id order, joined to the
/// behaviour specified for its event, the stories and generated tests, and the results.
///
/// Stories are attached by what they plan, not by their text: a test used a control when the
/// driver says its action did; a story that performs the control's event in a section of its
/// view's name documents it. The same event on another view's control is a different row.
pub fn control_coverage(
    suite: &Suite,
    plans: &[Plan],
    report: &Report,
    declared: &[Control],
) -> Vec<ControlRow> {
    let mut rows: Vec<ControlRow> = vec![];
    let mut index: std::collections::HashMap<String, usize> = Default::default();
    let mut add = |c: &Control, rows: &mut Vec<ControlRow>| {
        index.entry(c.key()).or_insert_with(|| {
            let event = c.event.as_deref().and_then(|name| {
                let wanted = text::key(name);
                suite
                    .vocabulary
                    .events
                    .iter()
                    .find(|e| e.key == wanted)
                    .map(|e| e.id)
            });
            let specified = match event {
                Some(ev) => suite
                    .expectations
                    .iter()
                    .filter(|x| x.trigger.event == Some(ev))
                    .map(|x| x.id)
                    .collect(),
                None => vec![],
            };
            rows.push(ControlRow {
                control: c.clone(),
                event,
                specified,
                stories: vec![],
                generated: vec![],
                documented: vec![],
                passed: 0,
                failed: 0,
                waiting: vec![],
                gaps: vec![],
            });
            rows.len() - 1
        });
    };
    for c in declared {
        add(c, &mut rows);
    }
    for result in &report.results {
        for c in &result.controls {
            add(c, &mut rows);
        }
    }
    for (i, (plan, result)) in plans.iter().zip(&report.results).enumerate() {
        for key in &result.used {
            let Some(&r) = index.get(key) else { continue };
            let row = &mut rows[r];
            match plan.origin {
                Origin::Story { .. } => row.stories.push(i),
                _ => row.generated.push(i),
            }
            match result.status {
                Status::Passed => row.passed += 1,
                Status::Failed => row.failed += 1,
                _ => {}
            }
        }
    }
    // Stories that perform the event in the control's view, whether or not they ran.
    for (i, (plan, result)) in plans.iter().zip(&report.results).enumerate() {
        if !matches!(plan.origin, Origin::Story { .. }) {
            continue;
        }
        let views: Vec<String> = suite
            .chain(plan.section)
            .iter()
            .map(|s| text::key(&s.name))
            .collect();
        for row in &mut rows {
            let Some(ev) = row.event else { continue };
            if row.stories.contains(&i)
                || !views.contains(&text::key(&row.control.view))
                || !plan.steps.iter().any(|s| s.action.event == Some(ev))
            {
                continue;
            }
            row.documented.push(i);
            if let Some(n) = row.waiting.iter_mut().find(|(s, _)| *s == result.status) {
                n.1 += 1;
            } else {
                row.waiting.push((result.status, 1));
            }
        }
    }
    for row in &mut rows {
        let mut gaps = vec![];
        match (&row.control.event, row.event) {
            (None, _) => gaps.push(Gap::NoEventMapping),
            (Some(_), None) => gaps.push(Gap::UnknownEvent),
            (Some(_), Some(_)) if row.specified.is_empty() => gaps.push(Gap::NoExpectation),
            (Some(_), Some(_)) if row.specified.iter().all(|&e| suite.exp(e).pending) => {
                gaps.push(Gap::OnlyPending)
            }
            _ => {}
        }
        if row.authored().is_empty() {
            gaps.push(Gap::NoAuthoredStory);
        }
        if !row.used() {
            gaps.push(Gap::NeverUsed);
        } else if !row.verified() {
            gaps.push(Gap::NoPassingExecution);
        }
        row.gaps = gaps;
    }
    rows.sort_by(|a, b| (&a.control.view, &a.control.id).cmp(&(&b.control.view, &b.control.id)));
    rows
}

/// The coverage as JSON, for CI: totals, then one object per control.
pub fn controls_json(suite: &Suite, rows: &[ControlRow], plans: &[Plan]) -> String {
    let q = |s: &str| {
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
    };
    let list = |items: Vec<String>| format!("[{}]", items.join(","));
    let slugs = |v: &[usize]| {
        let mut v = v.to_vec();
        v.sort_unstable();
        v.dedup();
        list(v.iter().map(|&i| q(&plans[i].slug)).collect())
    };
    let unused = rows.iter().filter(|r| !r.used()).count();
    let gaps = rows.iter().filter(|r| !r.gaps.is_empty()).count();
    let mut o = format!(
        "{{\n  \"total\": {},\n  \"unused\": {unused},\n  \"with_gaps\": {gaps},\n  \"controls\": [",
        rows.len()
    );
    for (n, r) in rows.iter().enumerate() {
        if n > 0 {
            o.push(',');
        }
        let event = match &r.control.event {
            Some(e) => q(e),
            None => "null".into(),
        };
        let specified = list(
            r.specified
                .iter()
                .map(|&e| {
                    let x = suite.exp(e);
                    format!("{{\"name\": {}, \"pending\": {}}}", q(&x.name), x.pending)
                })
                .collect(),
        );
        let waiting = list(
            r.waiting
                .iter()
                .map(|(s, n)| format!("{{\"status\": {}, \"plans\": {n}}}", q(s.name())))
                .collect(),
        );
        o.push_str(&format!(
            "\n    {{\"view\": {}, \"id\": {}, \"kind\": {}, \"label\": {}, \"event\": {event}, \
             \"specified\": {specified}, \"used\": {}, \"verified\": {}, \"stories\": {}, \
             \"documented_stories\": {}, \"generated\": {}, \"passed\": {}, \"failed\": {}, \
             \"not_run\": {waiting}, \"gaps\": {}}}",
            q(&r.control.view),
            q(&r.control.id),
            q(&r.control.kind),
            q(&r.control.label),
            r.used(),
            r.verified(),
            slugs(&r.stories),
            slugs(&r.documented),
            slugs(&r.generated),
            r.passed,
            r.failed,
            list(r.gaps.iter().map(|g| q(g.key())).collect())
        ));
    }
    o.push_str("\n  ]\n}\n");
    o
}
