//! A test starts where the last one ended, without a reset, when that test passed and what this
//! one is given is observed to hold there already.

use expectation_harness::{Acceptance, Began, Bindings, RunOptions};
use expectation_story::{Report, Status};

/// «Bright light» ends with the lamp on and the dimmer high, which is where «Dim again» and
/// «Off again» start. «Just the dimmer» starts from the background.
const LAMP: &str = "
Vocabulary:
  State: the lamp is off | on
  State: the dimmer is low | high
  Event: the user flips the switch
  Event: the user flips it back
  Event: the user turns the dimmer
  Event: the user turns it back
Background:
  Given the lamp is off
  And the dimmer is low
Feature: Lamp
  Expectation: Flipping turns it on
    Given the lamp is off
    When the user flips the switch
    Then the lamp is on
  Expectation: Flipping back turns it off
    Given the lamp is on
    When the user flips it back
    Then the lamp is off
  Expectation: Turning the dimmer brightens it
    Given the dimmer is low
    When the user turns the dimmer
    Then the dimmer is high
  Expectation: Turning it back dims it
    Given the dimmer is high
    When the user turns it back
    Then the dimmer is low
Story: Bright light
  When the user turns the dimmer
  Then the dimmer is high
  When the user flips the switch
  Then the lamp is on
Story: Just the dimmer
  When the user turns the dimmer
  Then the dimmer is high
Story: Dim again
  Given the lamp is on
  And the dimmer is high
  When the user turns it back
  Then the dimmer is low
Story: Off again
  Given the lamp is on
  And the dimmer is high
  When the user flips it back
  Then the lamp is off
";

#[derive(Default)]
struct Lamp {
    on: bool,
    high: bool,
    /// What was asked of it, in order: `reset`, and each event.
    log: Vec<&'static str>,
    /// Turning the dimmer back does nothing.
    stuck: bool,
    /// The bulb is about to go: once flipped on, it is seen on this many times and then off.
    fragile: Option<u32>,
    looks: std::cell::Cell<u32>,
    /// It was turned on at the switch, not arranged on (with a new bulb).
    flipped: bool,
}

fn lamp() -> Acceptance {
    let loaded = expectation_language::load_sources("lamp", &[("lamp.expect", LAMP)]);
    Acceptance::from_loaded(loaded).unwrap_or_else(|e| panic!("{e}"))
}

fn seen_on(w: &Lamp) -> bool {
    let looks = w.looks.get();
    w.looks.set(looks + 1);
    w.on && !(w.flipped && w.fragile.is_some_and(|n| looks >= n))
}

fn driver(stuck: bool) -> Bindings<Lamp> {
    let world = Lamp {
        stuck,
        ..Default::default()
    };
    Bindings::new(world)
        .observe("the lamp", |w: &Lamp| if seen_on(w) { "on" } else { "off" })
        .observe("the dimmer", |w: &Lamp| if w.high { "high" } else { "low" })
        .arrange("the lamp", |w, v| {
            w.flipped = false;
            w.on = v.to_string() == "on"
        })
        .arrange("the dimmer", |w, v| w.high = v.to_string() == "high")
        .on("the user flips the switch", |w| {
            w.log.push("flip");
            w.looks.set(0);
            w.flipped = true;
            w.on = true
        })
        .on("the user flips it back", |w| {
            w.log.push("flip back");
            w.on = false
        })
        .on("the user turns the dimmer", |w| {
            w.log.push("turn");
            w.high = true
        })
        .on("the user turns it back", |w| {
            w.log.push("turn back");
            w.high = w.stuck
        })
        .on_reset(|w| {
            w.log.push("reset");
            w.on = false;
            w.high = false
        })
}

fn began(report: &Report) -> Vec<Option<Began>> {
    report.results.iter().map(|r| r.began).collect()
}

fn titles(a: &Acceptance) -> Vec<&str> {
    a.plans.iter().map(|p| p.title.as_str()).collect()
}

const ARRANGED: Option<Began> = Some(Began::Arranged);
const CONTINUED: Option<Began> = Some(Began::Continued);

#[test]
fn a_test_starts_where_the_last_one_ended_when_its_givens_hold_there() {
    let a = lamp();
    assert_eq!(
        titles(&a),
        ["Bright light", "Just the dimmer", "Dim again", "Off again"]
    );
    let mut d = driver(false);
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed());
    // «Dim again» is run next, where «Bright light» ended, and the results are still in the
    // plans' order. It leaves the dimmer low, so «Off again» has to be arranged.
    assert_eq!(began(&report), [ARRANGED, ARRANGED, CONTINUED, ARRANGED]);
    assert_eq!(
        d.world.log,
        [
            "reset",
            "turn",
            "flip",
            "turn back",
            "reset",
            "turn",
            "reset",
            "flip back"
        ]
    );

    // The same verdicts as resetting before each one.
    let mut d = driver(false);
    let opts = RunOptions {
        share_starts: false,
        ..Default::default()
    };
    let each = a.run(&mut d, &opts);
    assert_eq!(began(&each), [ARRANGED; 4]);
    assert_eq!(d.world.log.iter().filter(|l| **l == "reset").count(), 4);
    assert_eq!(
        report.verdicts(&a.suite, &a.plans),
        each.verdicts(&a.suite, &a.plans)
    );
}

#[test]
fn a_driver_can_refuse_to_continue() {
    let a = lamp();
    let mut d = driver(false).always_reset();
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed());
    assert_eq!(began(&report), [ARRANGED; 4]);
    // And then the tests are run in the order they are in.
    assert_eq!(
        d.world.log,
        [
            "reset",
            "turn",
            "flip",
            "reset",
            "turn",
            "reset",
            "turn back",
            "reset",
            "flip back"
        ]
    );
}

#[test]
fn nothing_continues_from_a_test_that_failed() {
    // Only the two that start bright: with the dimmer stuck, «Dim again» fails and leaves the
    // system where «Off again» starts, which is reset all the same.
    let mut a = lamp();
    a.plans.retain(|p| p.title.ends_with("again"));
    let mut d = driver(true);
    let report = a.run(&mut d, &RunOptions::default());
    let status: Vec<Status> = report.results.iter().map(|r| r.status).collect();
    assert_eq!(status, [Status::Failed, Status::Passed]);
    assert_eq!(began(&report), [ARRANGED, ARRANGED]);
}

#[test]
fn what_a_test_is_given_is_observed_not_assumed() {
    // «Bright light» passes, but the lamp goes out after it was seen on, before the next
    // test: «Dim again» should start there by the model, and is arranged because it doesn't.
    let mut a = lamp();
    a.plans
        .retain(|p| ["Bright light", "Dim again"].contains(&p.title.as_str()));
    let first = a.run(&mut driver(false), &RunOptions::default());
    assert_eq!(began(&first), [ARRANGED, CONTINUED]);

    let mut d = driver(false);
    d.world.fragile = Some(1);
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    assert_eq!(began(&report), [ARRANGED, ARRANGED]);
}

#[test]
fn a_test_given_nothing_is_always_reset() {
    // Nothing to observe says nothing about where the system is.
    let mut a = lamp();
    a.plans.retain(|p| p.title == "Just the dimmer");
    a.plans.push(a.plans[0].clone());
    for p in &mut a.plans {
        p.start_checks.clear();
    }
    let report = a.run(&mut driver(false), &RunOptions::default());
    assert_eq!(began(&report), [ARRANGED, ARRANGED]);
}

#[test]
fn saved_results_and_the_manual_say_how_a_test_began() {
    let a = lamp();
    let report = a.run(&mut driver(false), &RunOptions::default());
    let saved = report.save(&a.plans);
    assert_eq!(saved.matches("\nbegan\tcontinued\n").count(), 1);
    assert_eq!(saved.matches("\nbegan\tarranged\n").count(), 3);
    let loaded = Report::load(&saved, &a.plans);
    assert_eq!(began(&loaded), began(&report));

    let dir = std::env::temp_dir().join(format!("expectation-shared-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    a.publish(&dir, "test", &report).unwrap();
    let page = |name: &str| std::fs::read_to_string(a.tree(&dir).join(name)).unwrap();
    let said = "started where the test that ran before it had ended";
    assert!(page("tests/dim-again.md").contains(said));
    assert!(!page("tests/off-again.md").contains(said));
    assert!(page("site/test-dim-again.html").contains(said));
    let _ = std::fs::remove_dir_all(&dir);
}
