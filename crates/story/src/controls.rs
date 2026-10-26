//! The controls of the program under test (its buttons, fields, switches: anything a person can
//! act on), as the driver listed them, and which of them the tests used.
//!
//! The driver supplies the inventory while the tests run, so the framework needs to know nothing
//! about a UI toolkit; this joins it to the plans that used each control.

use crate::plan::{Origin, Plan};
use crate::report::{Report, Status};

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
        }
    }

    /// What `used` lists: the view and id, which together say which control it is.
    pub fn key(&self) -> String {
        format!("{}\u{1f}{}", self.view, self.id)
    }
}

/// One control, and the tests that used it.
#[derive(Clone, Debug)]
pub struct ControlRow {
    pub control: Control,
    /// Indices into the plans of the stories that used it.
    pub stories: Vec<usize>,
    /// Indices of the generated tests that used it. A generated test is evidence that it works,
    /// not documentation of it, so it is counted apart from the stories.
    pub generated: Vec<usize>,
    /// How many of those tests passed, and how many failed.
    pub passed: usize,
    pub failed: usize,
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
}

/// Every control any test saw, in view and id order, with the tests that used it. A test that
/// couldn't run (unsupported, pending, not run) saw and used nothing.
pub fn control_coverage(plans: &[Plan], report: &Report) -> Vec<ControlRow> {
    let mut rows: Vec<ControlRow> = vec![];
    let mut index: std::collections::HashMap<String, usize> = Default::default();
    for (i, (plan, result)) in plans.iter().zip(&report.results).enumerate() {
        for c in &result.controls {
            index.entry(c.key()).or_insert_with(|| {
                rows.push(ControlRow {
                    control: c.clone(),
                    stories: vec![],
                    generated: vec![],
                    passed: 0,
                    failed: 0,
                });
                rows.len() - 1
            });
        }
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
    rows.sort_by(|a, b| (&a.control.view, &a.control.id).cmp(&(&b.control.view, &b.control.id)));
    rows
}

/// The coverage as JSON, for CI: totals, then one object per control.
pub fn controls_json(rows: &[ControlRow], plans: &[Plan]) -> String {
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
    let slugs = |v: &[usize]| {
        let mut slugs: Vec<String> = v.iter().map(|&i| q(&plans[i].slug)).collect();
        slugs.dedup();
        format!("[{}]", slugs.join(","))
    };
    let unused = rows.iter().filter(|r| !r.used()).count();
    let mut o = format!(
        "{{\n  \"total\": {},\n  \"unused\": {unused},\n  \"controls\": [",
        rows.len()
    );
    for (n, r) in rows.iter().enumerate() {
        if n > 0 {
            o.push(',');
        }
        o.push_str(&format!(
            "\n    {{\"view\": {}, \"id\": {}, \"kind\": {}, \"label\": {}, \"used\": {}, \"verified\": {}, \
             \"stories\": {}, \"generated\": {}, \"passed\": {}, \"failed\": {}}}",
            q(&r.control.view),
            q(&r.control.id),
            q(&r.control.kind),
            q(&r.control.label),
            r.used(),
            r.verified(),
            slugs(&r.stories),
            slugs(&r.generated),
            r.passed,
            r.failed
        ));
    }
    o.push_str("\n  ]\n}\n");
    o
}
