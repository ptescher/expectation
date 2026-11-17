//! `status_json`: where each feature and story stands, with approval, being built and the tests'
//! verdict kept apart, for a board or a CI gate.

use expectation_harness::{Acceptance, Bindings, RunOptions};
use expectation_story::status_json;

const SUITE: &str = "Suite: Lamp
Vocabulary:
  State: the lamp is off | on
  State: the fan is off | on
  Event: the user flips the switch
  Event: the user spins the fan
Background:
  Given the lamp is off
  And the fan is off

Feature: Lamp
  Approval: approved
  Expectation: Flipping turns it on
    Given the lamp is off
    When the user flips the switch
    Then the lamp is on

Feature: Fan
  Expectation: Spinning turns it on
    Approval: approved
    Given the fan is off
    When the user spins the fan
    Then the fan is on
  Expectation: Spinning again turns it off
    Status: pending
    Given the fan is on
    When the user spins the fan
    Then the fan is off

Story: Light
  Approval: approved
  When the user flips the switch
  Then the lamp is on
";

fn run(broken: bool) -> String {
    let loaded = expectation_language::load_sources("lamp", &[("lamp.expect", SUITE)]);
    let a = Acceptance::from_loaded(loaded)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage();
    let mut d = Bindings::new((false, false))
        .observe(
            "the lamp",
            |w: &(bool, bool)| if w.0 { "on" } else { "off" },
        )
        .observe("the fan", |w: &(bool, bool)| if w.1 { "on" } else { "off" })
        .arrange("the lamp", |w, v| w.0 = v.to_string() == "on")
        .arrange("the fan", |w, v| w.1 = v.to_string() == "on")
        .on("the user flips the switch", move |w| w.0 = !broken)
        .on("the user spins the fan", |w| w.1 = !w.1)
        .on_reset(|w| *w = (false, false));
    let report = a.run(&mut d, &RunOptions::default());
    status_json(&a.suite, &a.plans, &report)
}

fn feature<'a>(json: &'a str, name: &str) -> &'a str {
    let at = json
        .find(&format!("{{\"name\":\"{name}\",\"slug\""))
        .unwrap();
    let end = json[at..].find("]}").unwrap();
    &json[at..at + end]
}

#[test]
fn approval_being_built_and_the_verdict_are_separate_facts() {
    let json = run(false);
    let lamp = feature(&json, "Lamp");
    assert!(lamp.contains("\"approval\":\"approved\",\"verdict\":\"verified\",\"pending\":0"));
    // One expectation approved on its own under a feature that says nothing, one pending: the
    // feature is partly approved, and verified apart from what isn't built yet.
    let fan = feature(&json, "Fan");
    assert!(fan.contains("\"approval\":\"partial\",\"verdict\":\"verified\",\"pending\":1"));
    assert!(fan.contains("\"slug\":\"spinning-again-turns-it-off\",\"approval\":\"unmarked\",\"pending\":true,\"verdict\":\"pending\""));
    assert!(json.contains("\"name\":\"Light\""));
    assert!(json.contains("\"approval\":\"approved\",\"status\":\"passed\""));
}

#[test]
fn a_failure_is_the_features_verdict_and_the_storys_status() {
    let json = run(true);
    assert!(feature(&json, "Lamp").contains("\"verdict\":\"failed\""));
    assert!(feature(&json, "Fan").contains("\"verdict\":\"verified\""));
    assert!(json.contains("\"approval\":\"approved\",\"status\":\"failed\""));
}
