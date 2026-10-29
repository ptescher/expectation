//! `Approval:` only decides what an approved-only manual shows: the full suite is checked,
//! planned and run as before, and an approved story can't lean on behaviour nobody approved.

use expectation_harness::Acceptance;
use expectation_language::load_sources_with;

const LAMP: &str = "
Vocabulary:
  State: the lamp is off | on
  Event: the user flips the switch
Background:
  Given the lamp is off
Feature: Switching
  Approval: approved
  Expectation: Flip on
    Given the lamp is off
    When the user flips the switch
    Then the lamp is on
  Expectation: Flip off
    Approval: proposed
    Given the lamp is on
    When the user flips the switch
    Then the lamp is off
Story: Light it
  Approval: approved
  Scenario: On
    When the user flips the switch
    Then the lamp is on
Story: Round trip
  Scenario: On and off
    When the user flips the switch
    Then the lamp is on
    When the user flips the switch
    Then the lamp is off
";

fn acceptance(src: &str, approved: bool) -> Result<Acceptance, String> {
    Acceptance::from_loaded(load_sources_with("S", &[("s.expect", src)], None, approved))
}

#[test]
fn approval_does_not_change_the_full_suite() {
    let marked = acceptance(LAMP, false).unwrap().with_coverage();
    let plain = LAMP
        .replace("  Approval: approved\n", "")
        .replace("  Approval: proposed\n", "")
        .replace("    Approval: proposed\n", "");
    let unmarked = acceptance(&plain, false).unwrap().with_coverage();
    let titles = |a: &Acceptance| a.plans.iter().map(|p| p.title.clone()).collect::<Vec<_>>();
    assert_eq!(titles(&marked), titles(&unmarked));
    assert_eq!(marked.plans.len(), unmarked.plans.len());
}

#[test]
fn the_approved_part_has_only_approved_tests() {
    let a = acceptance(LAMP, true).unwrap().with_coverage();
    let titles: Vec<_> = a.plans.iter().map(|p| p.title.as_str()).collect();
    assert!(titles.iter().any(|t| t.contains("Light it")), "{titles:?}");
    assert!(
        !titles.iter().any(|t| t.contains("Round trip")),
        "{titles:?}"
    );
    assert!(!titles.iter().any(|t| t.contains("Flip off")), "{titles:?}");
}

#[test]
fn an_approved_story_cannot_lean_on_a_proposed_expectation() {
    let src = LAMP.replace(
        "Story: Round trip\n",
        "Story: Round trip\n  Approval: approved\n",
    );
    assert!(acceptance(&src, false).is_ok());
    let err = acceptance(&src, true).err().expect("should be refused");
    assert!(err.contains("unspecified"), "{err}");
}
