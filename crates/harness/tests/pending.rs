//! `Status: pending`: expectations written ahead of the behaviour. They stay in the model, so
//! conflicts and reachability still see them, but a test that depends on one is reported as
//! pending instead of being run, and nothing pending counts as verified.

#[path = "../examples/kettle/world.rs"]
mod world;

use expectation_harness::{Acceptance, Bindings, RunOptions};
use expectation_story::{Report, Status, Verdict};

const VOCABULARY: &str = "Suite: Pending
Vocabulary:
  State: the kettle is off | starting | heating | dry
  State: the dial is 0..100 %
  State: the preset is WARM | HOT | BOIL
  State: the child lock is on | off
  Event: the user presses start
  Event: the user presses reset
  Event: the user presses preset
  Event: the user presses lock
  Event: the water runs out

Background:
  Given the kettle is off
  And the dial is 0 %
  And the preset is WARM
  And the child lock is off
";

const REQUIRED: &str = "Feature: Heating
  Starting the kettle.

  Expectation: The start button starts the kettle
    Given the kettle is off
    When the user presses start
    Then the kettle is heating

  Expectation: The child lock holds the kettle
    Given the kettle is off
    And the child lock is on
    When the dial is above 55 %
    Then the kettle is off
";

/// Every word is in the vocabulary and the toy world has a binding for each of them, but the
/// world doesn't do this: the preset button doesn't switch a heating kettle off. This is what
/// "unsupported" can't say: the driver is able to run it, and it would fail.
const PENDING: &str = "Feature: Presets
  Pressing the preset button.

  Expectation: The preset button switches a heating kettle off
    Status: pending
    Given the kettle is heating
    When the user presses preset
    Then the kettle is off
";

fn load(files: &[&str]) -> Result<Acceptance, String> {
    let mut src = String::from(VOCABULARY);
    for f in files {
        src.push('\n');
        src.push_str(f);
    }
    let loaded = expectation_language::load_sources("t", &[("t.expect", &src)]);
    Acceptance::from_loaded(loaded).map(Acceptance::with_coverage)
}

fn named<'a>(a: &'a Acceptance, name: &str) -> &'a expectation_model::Expectation {
    a.suite
        .expectations
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no «{name}»"))
}

const PENDING_NAME: &str = "The preset button switches a heating kettle off";

#[test]
fn pending_behaviour_that_is_bound_but_not_built_is_reported_not_failed() {
    let a = load(&[REQUIRED, PENDING]).unwrap_or_else(|e| panic!("{e}"));
    assert!(named(&a, PENDING_NAME).pending);

    let report = a.run(&mut world::driver(false), &RunOptions::default());
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    assert!(report.count(Status::Pending) > 0);
    assert!(report.count(Status::Passed) > 0);
    assert_eq!(report.count(Status::Unsupported), 0);
    assert_eq!(report.count(Status::Failed), 0);

    // Only the plans that depend on the pending expectation are pending, and they were not run.
    for (plan, result) in a.plans.iter().zip(&report.results) {
        assert_eq!(plan.pending, result.status == Status::Pending);
        if plan.pending {
            assert!(result.steps.iter().all(|s| s.checks.is_empty()));
            let why = result.start.error.as_deref().unwrap();
            assert!(why.contains(PENDING_NAME), "{why}");
        }
    }

    let verdicts = report.verdicts(&a.suite, &a.plans);
    assert_eq!(
        verdicts[named(&a, PENDING_NAME).id.0 as usize],
        Verdict::Pending
    );
    for name in [
        "The start button starts the kettle",
        "The child lock holds the kettle",
    ] {
        assert_eq!(verdicts[named(&a, name).id.0 as usize], Verdict::Verified);
    }

    // Without the marker the same behaviour is an ordinary spec, and it fails.
    let unmarked = PENDING.replace("    Status: pending\n", "");
    let b = load(&[REQUIRED, &unmarked]).unwrap_or_else(|e| panic!("{e}"));
    let report = b.run(&mut world::driver(false), &RunOptions::default());
    assert!(!report.passed());
}

#[test]
fn a_failing_required_spec_still_fails_the_run() {
    let a = load(&[REQUIRED, PENDING]).unwrap_or_else(|e| panic!("{e}"));
    // The seeded defect: the child lock lets the dial through.
    let report = a.run(&mut world::driver(true), &RunOptions::default());
    assert!(!report.passed());
    assert!(report.count(Status::Pending) > 0);
    let verdicts = report.verdicts(&a.suite, &a.plans);
    assert_eq!(
        verdicts[named(&a, "The child lock holds the kettle").id.0 as usize],
        Verdict::Failed
    );
    assert_eq!(
        verdicts[named(&a, PENDING_NAME).id.0 as usize],
        Verdict::Pending
    );
}

#[test]
fn a_pending_spec_that_conflicts_with_a_required_one_fails_validation() {
    let conflicting = "Feature: Start
  Expectation: The start button does nothing
    Status: pending
    Given the kettle is off
    When the user presses start
    Then the kettle is off
";
    let Err(e) = load(&[REQUIRED, conflicting]) else {
        panic!("a conflict with a pending spec must still be an error");
    };
    assert!(e.contains("The start button does nothing"), "{e}");
    assert!(e.contains("The start button starts the kettle"), "{e}");
    // The same two, both required, conflict the same way.
    assert!(load(&[REQUIRED, &conflicting.replace("    Status: pending\n", "")]).is_err());
}

#[test]
fn a_suite_of_only_pending_specs_verifies_nothing() {
    let all_pending = REQUIRED.replace("    Given", "    Status: pending\n    Given");
    let a = load(&[&all_pending, PENDING]).unwrap_or_else(|e| panic!("{e}"));
    let report = a.run(&mut world::driver(false), &RunOptions::default());
    assert!(report.passed());
    assert!(!report.results.is_empty());
    assert!(report.results.iter().all(|r| r.status == Status::Pending));
    let verdicts = report.verdicts(&a.suite, &a.plans);
    assert!(
        verdicts.iter().all(|v| *v == Verdict::Pending),
        "{verdicts:?}"
    );
    let docs = a.docs(Some(&report));
    let pages = docs.pages();
    let tests = &pages.iter().find(|p| p.path == "tests.md").unwrap().content;
    assert!(tests.contains("pending"), "{tests}");
    assert!(!tests.contains("✅"), "{tests}");
}

#[test]
fn pending_is_not_a_missing_binding() {
    let a = load(&[REQUIRED, PENDING]).unwrap_or_else(|e| panic!("{e}"));
    // A driver that binds nothing: required tests can't run (a gap to fix), pending ones
    // are still just pending.
    let mut bare = Bindings::new(world::World::new(false));
    let report = a.run(&mut bare, &RunOptions::default());
    assert!(report.passed());
    for (plan, result) in a.plans.iter().zip(&report.results) {
        let want = if plan.pending {
            Status::Pending
        } else {
            Status::Unsupported
        };
        assert_eq!(result.status, want, "{}", plan.title);
    }
}

#[test]
fn pending_results_save_and_load_and_marking_a_spec_pending_drops_old_results() {
    let a = load(&[REQUIRED, PENDING]).unwrap_or_else(|e| panic!("{e}"));
    let report = a.run(&mut world::driver(false), &RunOptions::default());
    let loaded = Report::load(&report.save(&a.plans), &a.plans);
    for (x, y) in report.results.iter().zip(&loaded.results) {
        assert_eq!(x.status, y.status);
    }
    assert!(loaded.count(Status::Pending) > 0);

    // A result saved before the spec was marked pending is not the pending plan's result.
    let unmarked = PENDING.replace("    Status: pending\n", "");
    let before = load(&[REQUIRED, &unmarked]).unwrap_or_else(|e| panic!("{e}"));
    let old = before.run(&mut world::driver(false), &RunOptions::default());
    let now = Report::load(&old.save(&before.plans), &a.plans);
    for (plan, result) in a.plans.iter().zip(&now.results) {
        if plan.pending {
            assert_eq!(result.status, Status::NotRun, "{}", plan.title);
        }
    }
}
