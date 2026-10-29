//! The approved part of a suite: what a manual for readers who need agreed behaviour shows.
//!
//! `Approval: approved | proposed` goes right under a Feature, an Expectation or a Story. An
//! expectation without its own says what its feature says; nothing says neither, and neither
//! counts as approved. Only what is approved is kept, before the suite is resolved, so the
//! suite that comes out is one where the rest was never written: its features, stories,
//! navigation and links all agree, and an approved story or expectation that leans on
//! something that isn't approved is an error to settle, not behaviour published as agreed.
//!
//! A feature stays if any of its expectations does; its description stays only if the
//! feature itself is approved. The vocabulary, terms and backgrounds are not specifications
//! of behaviour and stay. This is only a view for documentation: the full suite is what
//! `check`, `plan` and the tests use, and `Status: pending` is a separate matter.

use crate::parse::{Approval, RawFile};
use expectation_model::{Diagnostic, text};

pub fn only_approved(files: &mut [RawFile], diags: &mut Vec<Diagnostic>) {
    // What is left out, for saying what an approved expectation can't lean on.
    let mut left: Vec<String> = vec![];
    for f in files.iter_mut() {
        for feature in &mut f.features {
            let inherited = feature.approval;
            for e in &mut feature.expectations {
                e.approval = e.approval.or(inherited);
            }
            let (kept, dropped): (Vec<_>, Vec<_>) = std::mem::take(&mut feature.expectations)
                .into_iter()
                .partition(|e| e.approval == Some(Approval::Approved));
            left.extend(dropped.iter().map(|e| text::key(&e.name)));
            feature.expectations = kept;
            if feature.approval != Some(Approval::Approved) {
                feature.description.clear();
            }
        }
        f.features
            .retain(|feature| !feature.expectations.is_empty());
        f.stories.retain(|s| s.approval == Some(Approval::Approved));
    }
    for f in files.iter() {
        for e in f.features.iter().flat_map(|feature| &feature.expectations) {
            for (name, span) in &e.overrides {
                if left.contains(&text::key(name)) {
                    diags.push(
                        Diagnostic::error(
                            "approval-dependency",
                            format!(
                                "«{}» is approved, but it overrides «{name}», which is not",
                                e.name
                            ),
                        )
                        .at(span, "")
                        .help(
                            "approve what it overrides, or leave this out of the approved manual",
                        ),
                    );
                }
            }
        }
    }
}
