//! A gate that wants the tests to have run: what isn't supported yet, or wasn't run, stops it,
//! where an ordinary run lets it by.

use expectation_harness::{Acceptance, Bindings, Policy, RunOptions};
use expectation_story::{Report, Status};

const LAMP: &str = "
Vocabulary:
  State: the lamp is off | on
  State: the dimmer is low | high
  Event: the user flips the switch
  Event: the user turns the dimmer
Background:
  Given the lamp is off
  And the dimmer is low
Feature: Lamp
  Expectation: Flipping turns it on
    Given the lamp is off
    When the user flips the switch
    Then the lamp is on
  Expectation: Turning the dimmer brightens it
    Given the dimmer is low
    When the user turns the dimmer
    Then the dimmer is high
Story: Light
  When the user flips the switch
  Then the lamp is on
Story: Bright
  When the user turns the dimmer
  Then the dimmer is high
";

const PENDING: &str = "
Feature: Dimming
  Expectation: Turning the dimmer again dims it
    Status: pending
    Given the dimmer is high
    When the user turns the dimmer
    Then the dimmer is low
Story: Dim again
  Given the dimmer is high
  When the user turns the dimmer
  Then the dimmer is low
";

fn lamp(src: &str) -> Acceptance {
    let loaded = expectation_language::load_sources("lamp", &[("lamp.expect", src)]);
    Acceptance::from_loaded(loaded).unwrap_or_else(|e| panic!("{e}"))
}

/// A lamp whose dimmer the driver can turn, or not yet.
fn driver(dimmer: bool, broken: bool) -> Bindings<(bool, bool)> {
    let b = Bindings::new((false, false))
        .observe(
            "the lamp",
            |w: &(bool, bool)| if w.0 { "on" } else { "off" },
        )
        .observe(
            "the dimmer",
            |w: &(bool, bool)| if w.1 { "high" } else { "low" },
        )
        .arrange("the lamp", |w, v| w.0 = v.to_string() == "on")
        .arrange("the dimmer", |w, v| w.1 = v.to_string() == "high")
        .on("the user flips the switch", move |w| w.0 = !broken)
        .on_reset(|w| *w = (false, false));
    if dimmer {
        b.on("the user turns the dimmer", |w| w.1 = true)
    } else {
        b
    }
}

fn status(a: &Acceptance, report: &Report, title: &str) -> Status {
    let i = a.plans.iter().position(|p| p.title == title).unwrap();
    report.results[i].status
}

#[test]
fn a_test_the_driver_cant_run_passes_an_ordinary_run_and_stops_a_strict_one() {
    let a = lamp(LAMP);
    let report = a.run(&mut driver(false, false), &RunOptions::default());
    assert_eq!(status(&a, &report, "Light"), Status::Passed);
    assert_eq!(status(&a, &report, "Bright"), Status::Unsupported);

    // As it always was.
    assert!(report.passed());
    assert!(report.meets(Policy::Lenient));
    assert_eq!(report.incomplete(&a.plans, Policy::Lenient), "");
    // Required execution is incomplete, and it says which test and why.
    assert!(!report.meets(Policy::Required));
    assert!(!report.meets(Policy::All));
    let why = report.incomplete(&a.plans, Policy::Required);
    assert!(
        why.contains("– unsupported: Bright (The driver can't"),
        "{why}"
    );
    assert!(!why.contains("Light"), "{why}");

    // With the dimmer bound, everything runs and every policy is met.
    let report = a.run(&mut driver(true, false), &RunOptions::default());
    for policy in [Policy::Lenient, Policy::Required, Policy::All] {
        assert!(report.meets(policy));
        assert_eq!(report.incomplete(&a.plans, policy), "");
    }
}

#[test]
fn pending_is_let_by_unless_everything_is_asked_for_and_is_listed_apart() {
    let a = lamp(&format!("{LAMP}{PENDING}"));
    let report = a.run(&mut driver(true, false), &RunOptions::default());
    assert_eq!(status(&a, &report, "Dim again"), Status::Pending);
    assert!(report.meets(Policy::Lenient));
    assert!(report.meets(Policy::Required));
    assert_eq!(report.incomplete(&a.plans, Policy::Required), "");
    assert!(!report.meets(Policy::All));
    let why = report.incomplete(&a.plans, Policy::All);
    assert!(
        why.contains("– pending: Dim again (Pending: it depends on"),
        "{why}"
    );

    // With something else missing, the pending test is named, but not as what is missing.
    let report = a.run(&mut driver(false, false), &RunOptions::default());
    let why = report.incomplete(&a.plans, Policy::Required);
    let (missing, pending) = why.split_once("Pending, and not required:\n").expect(&why);
    assert!(missing.contains("– unsupported: Bright"), "{why}");
    assert!(!missing.contains("Dim again"), "{why}");
    assert!(pending.contains("– pending: Dim again"), "{why}");
}

#[test]
fn a_failure_fails_every_policy_and_is_told_as_a_failure() {
    let a = lamp(LAMP);
    let report = a.run(&mut driver(true, true), &RunOptions::default());
    assert_eq!(status(&a, &report, "Light"), Status::Failed);
    for policy in [Policy::Lenient, Policy::Required, Policy::All] {
        assert!(!report.meets(policy));
        // What failed is in `failures`, once.
        assert_eq!(report.incomplete(&a.plans, policy), "");
    }
    assert!(report.failures(&a.suite, &a.plans).contains("✗ Light"));
}

#[test]
fn what_other_runs_saved_doesnt_stand_in_for_a_test_that_didnt_run() {
    let a = lamp(LAMP);
    let before = a.run(&mut driver(true, false), &RunOptions::default());
    assert!(before.meets(Policy::Required));
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("strict-merged");
    let _ = std::fs::remove_dir_all(&dir);

    // An earlier run saved a pass for every test; this one can't run the dimmer's.
    a.publish(&dir, "earlier", &before).unwrap();
    let now = a.run(&mut driver(false, false), &RunOptions::default());
    let merged = a.publish(&dir, "now", &now).unwrap();
    // The manual has the earlier pass, and the gate still sees that this run didn't make it.
    assert_eq!(status(&a, &merged, "Bright"), Status::Passed);
    assert!(!now.meets(Policy::Required));
}

#[test]
fn a_run_of_no_tests_verified_nothing() {
    let none = Report::default();
    assert!(none.passed());
    assert!(none.meets(Policy::Lenient));
    for policy in [Policy::Required, Policy::All] {
        assert!(!none.meets(policy));
        assert!(
            none.incomplete(&[], policy)
                .contains("No tests were selected")
        );
    }
    // And tests that were never run aren't complete either.
    let a = lamp(LAMP);
    let not_run = Report::not_run(&a.plans);
    assert!(not_run.passed());
    assert!(!not_run.meets(Policy::Required));
    assert!(
        not_run
            .incomplete(&a.plans, Policy::Required)
            .contains("– not-run: Light")
    );
}

#[test]
#[should_panic(expected = "acceptance tests didn't all run")]
fn a_strict_assertion_panics_when_a_test_didnt_run() {
    lamp(LAMP).assert_passes_with(
        &mut driver(false, false),
        &RunOptions::default(),
        Policy::Required,
    );
}

#[test]
fn an_ordinary_assertion_still_lets_it_by() {
    let a = lamp(LAMP);
    let report = a.assert_passes(&mut driver(false, false), &RunOptions::default());
    assert_eq!(report.count(Status::Unsupported), 1);
}
