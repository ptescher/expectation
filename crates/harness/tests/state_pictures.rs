//! The tests that only visit a state, for the state map's picture of it, are asked for apart
//! from the ones that give every expectation a verdict.

use expectation_harness::{Acceptance, Bindings, RunOptions};

/// The story reaches both expectations, but flips the switch with the dimmer high: the lamp on
/// with the dimmer low is a state one move from the start that no test passes through.
const LAMP: &str = "
Vocabulary:
  State: the lamp is off | on
  State: the dimmer is low | high
  Event: the user flips the switch
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
";

fn lamp() -> Acceptance {
    let loaded = expectation_language::load_sources("lamp", &[("lamp.expect", LAMP)]);
    Acceptance::from_loaded(loaded).unwrap_or_else(|e| panic!("{e}"))
}

fn driver() -> Bindings<(bool, bool)> {
    Bindings::new((false, false))
        .observe(
            "the lamp",
            |w: &(bool, bool)| if w.0 { "on" } else { "off" },
        )
        .observe(
            "the dimmer",
            |w: &(bool, bool)| {
                if w.1 { "high" } else { "low" }
            },
        )
        .arrange("the lamp", |w, v| w.0 = v.to_string() == "on")
        .arrange("the dimmer", |w, v| w.1 = v.to_string() == "high")
        .on("the user flips the switch", |w| w.0 = true)
        .on("the user turns the dimmer", |w| w.1 = true)
        .on("the user turns it back", |w| w.1 = false)
        .on_reset(|w| *w = (false, false))
}

fn titles(a: &Acceptance) -> Vec<&str> {
    a.plans.iter().map(|p| p.title.as_str()).collect()
}

#[test]
fn coverage_gives_the_verdicts_and_state_pictures_are_asked_for_apart() {
    let stories = lamp();
    assert_eq!(titles(&stories), ["Bright light"]);

    // Coverage: a test for the expectation no story reaches, and no more.
    let covered = lamp().with_coverage();
    assert_eq!(
        titles(&covered),
        ["Bright light", "Reaching «Turning it back dims it»"]
    );

    // The pictures: the states a move from the start that those tests don't pass through.
    let pictured = lamp().with_coverage().with_state_pictures();
    let visits: Vec<&str> = titles(&pictured)[covered.plans.len()..].to_vec();
    assert!(!visits.is_empty());
    assert!(
        visits
            .iter()
            .all(|t| t.starts_with("Visiting the state after «")),
        "{visits:?}"
    );
    // The tests they are added to are the same ones, in the same order.
    assert_eq!(
        titles(&pictured)[..covered.plans.len()],
        titles(&covered)[..]
    );
    // Asked for twice, there is nothing more to visit.
    let again = lamp()
        .with_coverage()
        .with_state_pictures()
        .with_state_pictures();
    assert_eq!(titles(&again), titles(&pictured));

    // They add pictures, not verdicts: every expectation's is the same without them.
    let quick = covered.run(&mut driver(), &RunOptions::default());
    let full = pictured.run(&mut driver(), &RunOptions::default());
    assert!(quick.passed() && full.passed());
    assert_eq!(
        quick.verdicts(&covered.suite, &covered.plans),
        full.verdicts(&pictured.suite, &pictured.plans)
    );
}

#[test]
fn state_pictures_alone_visit_what_the_stories_dont() {
    let a = lamp().with_state_pictures();
    assert_eq!(a.plans[0].title, "Bright light");
    assert!(a.plans.len() > 1);
    assert!(a.plans[1..].iter().all(|p| p.title.starts_with("Visiting")));
}
