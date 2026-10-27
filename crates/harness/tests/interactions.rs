//! Interaction coverage: the controls a driver lists, joined to the behaviour specified for the
//! event each is mapped to, the stories that exercise them, and the results.

#[path = "../examples/kettle/world.rs"]
mod world;

use expectation_harness::{Acceptance, Bindings, Control, RunOptions};
use expectation_story::{ControlRow, Gap, Status, control_coverage};

const SUITE: &str = "Suite: Shop
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
  Event: the user taps help
  Event: the user taps about

Background:
  Given the kettle is off
  And the dial is 0 %
  And the preset is WARM
  And the child lock is off

Feature: Heating
  Starting the kettle.

  Expectation: The start button starts the kettle
    Given the kettle is off
    When the user presses start
    Then the kettle is heating

  Expectation: Reset turns the kettle off
    Given the kettle is heating
    When the user presses reset
    Then the kettle is off

  Expectation: Lock turns the child lock on
    Status: pending
    Given the child lock is off
    When the user presses lock
    Then the child lock is on

Story: Making tea
  Scenario: Start it
    When the user presses start
    Then the kettle is heating
";

fn load() -> Acceptance {
    let loaded = expectation_language::load_sources("t", &[("t.expect", SUITE)]);
    Acceptance::from_loaded(loaded)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage()
}

/// The toy world's driver, with these controls on screen instead of the kettle's own.
fn driver(controls: Vec<Control>) -> Bindings<world::World> {
    world::driver(false)
        .on_controls(move |_| controls.clone())
        // The toy world says its controls are in the "Kettle" view; here that view is the Shop.
        .on_used(|w| {
            std::mem::take(&mut w.used)
                .into_iter()
                .map(|k| k.replace("Kettle\u{1f}", "Shop\u{1f}"))
                .collect()
        })
}

fn row<'a>(rows: &'a [ControlRow], view: &str, id: &str) -> &'a ControlRow {
    rows.iter()
        .find(|r| r.control.view == view && r.control.id == id)
        .unwrap_or_else(|| panic!("no control {view}/{id}"))
}

fn gaps(r: &ControlRow) -> Vec<Gap> {
    r.gaps.clone()
}

fn controls() -> Vec<Control> {
    vec![
        // Used by a story and by a generated test; verified.
        Control::new("Shop", "start", "button", "Start").mapped_to("the user presses start"),
        // Used by generated tests only: no authored story.
        Control::new("Shop", "reset", "button", "Reset").mapped_to("the user presses reset"),
        // Specified only by a pending expectation.
        Control::new("Shop", "lock", "button", "Lock").mapped_to("the user presses lock"),
        // Mapped to an event that no expectation is about.
        Control::new("Shop", "help", "button", "Help").mapped_to("the user taps help"),
        // Mapped to an event the vocabulary doesn't have.
        Control::new("Shop", "legal", "link", "Legal").mapped_to("the user taps legal"),
        // Not mapped at all.
        Control::new("Shop", "export", "button", "Export"),
        // The same event as the first, on another view no story is about.
        Control::new("Settings", "start", "button", "Start").mapped_to("the user presses start"),
    ]
}

#[test]
fn every_gap_has_its_reason() {
    let a = load();
    let mut d = driver(controls());
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    let rows = control_coverage(&a.suite, &a.plans, &report, &[]);
    assert_eq!(rows.len(), 7);

    // Mapped, specified, told as a story, and verified: no gap.
    let start = row(&rows, "Shop", "start");
    assert_eq!(start.specified.len(), 1);
    assert!(!start.stories.is_empty() && start.verified());
    assert!(gaps(start).is_empty(), "{:?}", start.gaps);

    // Only a generated test used it: verified, but still zero authored stories.
    let reset = row(&rows, "Shop", "reset");
    assert!(reset.stories.is_empty() && reset.documented.is_empty());
    assert!(!reset.generated.is_empty() && reset.verified());
    assert_eq!(gaps(reset), [Gap::NoAuthoredStory]);

    // Pending behaviour stays visible as work to do, and is not verification.
    let lock = row(&rows, "Shop", "lock");
    assert_eq!(lock.specified.len(), 1);
    assert_eq!(gaps(lock)[0], Gap::OnlyPending);
    assert!(gaps(lock).contains(&Gap::NeverUsed));

    assert_eq!(gaps(row(&rows, "Shop", "help"))[0], Gap::NoExpectation);
    assert_eq!(gaps(row(&rows, "Shop", "legal"))[0], Gap::UnknownEvent);
    assert_eq!(gaps(row(&rows, "Shop", "export"))[0], Gap::NoEventMapping);
    assert!(gaps(row(&rows, "Shop", "export")).contains(&Gap::NeverUsed));

    // The same event on another view is its own row: the story and the tests used the Shop's
    // control, so the Settings one is not covered by them.
    let other = row(&rows, "Settings", "start");
    assert_eq!(other.specified, start.specified);
    assert!(!other.used() && other.authored().is_empty());
    assert_eq!(gaps(other), [Gap::NoAuthoredStory, Gap::NeverUsed]);
}

#[test]
fn a_story_that_could_not_run_documents_a_control_without_verifying_it() {
    let a = load();
    // A driver that can't do anything: every test is unsupported, nothing is used.
    let mut bare = Bindings::new(world::World::new(false));
    let report = a.run(&mut bare, &RunOptions::default());
    let declared = vec![
        Control::new("Shop", "start", "button", "Start").mapped_to("the user presses start"),
        Control::new("Settings", "start", "button", "Start").mapped_to("the user presses start"),
    ];
    let rows = control_coverage(&a.suite, &a.plans, &report, &declared);
    assert_eq!(rows.len(), 2);
    let shop = row(&rows, "Shop", "start");
    // Documented by the story in this view, not used, not verified.
    assert!(!shop.documented.is_empty() && !shop.used() && !shop.verified());
    assert!(shop.waiting.iter().any(|(s, _)| *s == Status::Unsupported));
    assert_eq!(gaps(shop), [Gap::NeverUsed]);
    // The story is about the Shop; the same event elsewhere is not documented by it.
    let settings = row(&rows, "Settings", "start");
    assert!(settings.documented.is_empty());
    assert_eq!(gaps(settings), [Gap::NoAuthoredStory, Gap::NeverUsed]);
}

#[test]
fn a_declared_inventory_shows_controls_no_run_reached() {
    let a = load().with_inventory([
        Control::new("Shop", "secret", "button", "Secret"),
        Control::new("Shop", "start", "button", "Start").mapped_to("the user presses start"),
    ]);
    let mut d = driver(vec![
        Control::new("Shop", "start", "button", "Start").mapped_to("the user presses start"),
    ]);
    let report = a.run(&mut d, &RunOptions::default());
    let rows = control_coverage(&a.suite, &a.plans, &report, &a.inventory);
    // The one both lists is a single row.
    assert_eq!(rows.len(), 2);
    assert!(row(&rows, "Shop", "start").used());
    let secret = row(&rows, "Shop", "secret");
    assert!(!secret.used());
    assert_eq!(gaps(secret)[0], Gap::NoEventMapping);

    let docs = a.docs(Some(&report)).pages();
    let page = |p: &str| docs.iter().find(|x| x.path == p).unwrap().content.clone();
    let md = page("interactions.md");
    assert!(md.contains("`Secret` (button)"), "{md}");
    assert!(md.contains("no event mapping"), "{md}");
    assert!(
        md.contains("Yes: [The start button starts the kettle]"),
        "{md}"
    );
    let json = page("interactions.json");
    assert!(json.contains("\"gaps\": [\"no-event-mapping\""), "{json}");
    assert!(
        json.contains("\"event\": \"the user presses start\""),
        "{json}"
    );
    assert!(page("site/interactions.html").contains("no event mapping"));
}

#[test]
fn a_controls_event_survives_saving_results() {
    let a = load();
    let mut d = driver(controls());
    let report = a.run(&mut d, &RunOptions::default());
    let loaded = expectation_story::Report::load(&report.save(&a.plans), &a.plans);
    let rows = control_coverage(&a.suite, &a.plans, &loaded, &[]);
    assert_eq!(rows.len(), 7);
    assert_eq!(
        row(&rows, "Shop", "start").control.event.as_deref(),
        Some("the user presses start")
    );
    assert!(row(&rows, "Shop", "export").control.event.is_none());
}
