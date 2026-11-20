//! A start that is slow to arrange is arranged once, saved by the driver, and put back for the
//! other tests that begin there.

#[path = "../examples/kettle/world.rs"]
mod world;

use expectation_harness::{Acceptance, Began, Bindings, Checkpoint, RunOptions};
use expectation_story::{Report, Status};
use std::cell::Cell;
use std::rc::Rc;

/// Three tests begin in flight with the gear up, and each ends somewhere else, so none can
/// start where another ended. Getting a plane into the air is the slow part.
const PLANE: &str = "
Vocabulary:
  State: the plane is parked | flying | gliding
  State: the gear is up | down
  Event: the pilot lowers the gear
  Event: the pilot lands
  Event: the engine cuts out
Background:
  Given the plane is parked
  And the gear is up
Feature: Flight
  Expectation: The gear comes down
    Given the plane is flying
    And the gear is up
    When the pilot lowers the gear
    Then the gear is down
  Expectation: It lands
    Given the plane is flying
    And the gear is up
    When the pilot lands
    Then the plane is parked
  Expectation: It glides when the engine cuts out
    Given the plane is flying
    And the gear is up
    When the engine cuts out
    Then the plane is gliding
  Expectation: Lowering the gear on the ground
    Given the plane is parked
    And the gear is up
    When the pilot lowers the gear
    Then the gear is down
";

#[derive(Clone, Default)]
struct Plane {
    phase: &'static str,
    gear_down: bool,
    /// What it cost to get here: shared with the copies, so a restore doesn't hide the work.
    flights: Rc<Cell<u32>>,
    saved: Rc<Cell<u32>>,
}

fn plane() -> Acceptance {
    let loaded = expectation_language::load_sources("plane", &[("plane.expect", PLANE)]);
    Acceptance::from_loaded(loaded)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage()
}

fn driver(world: Plane) -> Bindings<Plane> {
    Bindings::new(world)
        .observe("the plane", |w: &Plane| w.phase)
        .observe(
            "the gear",
            |w: &Plane| if w.gear_down { "down" } else { "up" },
        )
        .arrange("the plane", |w, v| {
            if v.to_string() == "flying" {
                // The slow part.
                w.flights.set(w.flights.get() + 1);
            }
            w.phase = match v.to_string().as_str() {
                "flying" => "flying",
                "gliding" => "gliding",
                _ => "parked",
            }
        })
        .arrange("the gear", |w, v| w.gear_down = v.to_string() == "down")
        .on("the pilot lowers the gear", |w| w.gear_down = true)
        .on("the pilot lands", |w| w.phase = "parked")
        .on("the engine cuts out", |w| w.phase = "gliding")
        .on_reset(|w| {
            w.phase = "parked";
            w.gear_down = false
        })
}

fn began(report: &Report) -> Vec<Option<Began>> {
    report.results.iter().map(|r| r.began).collect()
}

fn statuses(report: &Report) -> Vec<Status> {
    report.results.iter().map(|r| r.status).collect()
}

fn count(report: &Report, how: Began) -> usize {
    began(report).iter().filter(|b| **b == Some(how)).count()
}

#[test]
fn a_shared_start_is_arranged_once_and_put_back_for_the_rest() {
    let a = plane();
    let world = Plane::default();
    let flights = world.flights.clone();
    let mut plain = driver(world);
    let report = a.run(&mut plain, &RunOptions::default());
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    // Without checkpoints every one of the three arranges the flight.
    assert_eq!(flights.get(), 3);
    assert_eq!(count(&report, Began::Restored), 0);

    let world = Plane::default();
    let flights = world.flights.clone();
    let mut shared = driver(world).checkpointed();
    let with = a.run(&mut shared, &RunOptions::default());
    assert!(with.passed(), "{}", with.failures(&a.suite, &a.plans));
    assert_eq!(flights.get(), 1, "the flight is arranged once");
    assert_eq!(count(&with, Began::Restored), 2);
    // The same tests reach the same verdicts, step for step, as when each arranged its own.
    assert_eq!(statuses(&with), statuses(&report));
    for (x, y) in with.results.iter().zip(&report.results) {
        assert_eq!(
            x.steps.iter().map(|s| s.status).collect::<Vec<_>>(),
            y.steps.iter().map(|s| s.status).collect::<Vec<_>>()
        );
    }
    // And the results say which began how, and keep saying it.
    let saved = Report::load(&with.save(&a.plans), &a.plans);
    assert_eq!(began(&saved), began(&with));
    let docs = a.docs(Some(&with)).pages();
    assert!(
        docs.iter()
            .any(|p| p.content.contains("started from a saved copy of its start"))
    );
}

#[test]
fn only_a_start_another_test_needs_is_saved() {
    let a = plane();
    let world = Plane::default();
    let saved = world.saved.clone();
    let mut d = driver(world).on_checkpoint(
        {
            let saved = saved.clone();
            move |w| {
                saved.set(saved.get() + 1);
                Checkpoint::new((w.phase, w.gear_down))
            }
        },
        |w, cp| {
            let &(phase, gear_down) = cp.get::<(&'static str, bool)>().ok_or("not mine")?;
            (w.phase, w.gear_down) = (phase, gear_down);
            Ok(())
        },
    );
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed());
    // The parked start has one test; only the flying one is shared.
    assert_eq!(saved.get(), 1);
}

#[test]
fn a_restore_that_does_not_hold_is_not_trusted() {
    let a = plane();
    let world = Plane::default();
    let flights = world.flights.clone();
    // A driver that says it restored and put nothing back.
    let mut d = driver(world).on_checkpoint(|_| Checkpoint::new(()), |_, _| Ok(()));
    let report = a.run(&mut d, &RunOptions::default());
    // Every test still gets its start, by arranging, and passes; none is called restored.
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    assert_eq!(count(&report, Began::Restored), 0);
    assert_eq!(flights.get(), 3);
}

#[test]
fn a_failed_restore_falls_back_to_arranging() {
    let a = plane();
    let world = Plane::default();
    let flights = world.flights.clone();
    let mut d = driver(world).on_checkpoint(|_| Checkpoint::new(()), |_, _| Err("gone".into()));
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    assert_eq!(count(&report, Began::Restored), 0);
    assert_eq!(flights.get(), 3);
}

#[test]
fn checkpoints_are_off_when_starts_are_not_shared() {
    let a = plane();
    let world = Plane::default();
    let flights = world.flights.clone();
    let mut d = driver(world).checkpointed();
    let off = RunOptions {
        share_starts: false,
        ..Default::default()
    };
    let report = a.run(&mut d, &off);
    assert!(report.passed());
    assert_eq!((count(&report, Began::Restored), flights.get()), (0, 3));

    // A driver can refuse for itself.
    let world = Plane::default();
    let flights = world.flights.clone();
    let mut d = driver(world).checkpointed().always_reset();
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed());
    assert_eq!((count(&report, Began::Restored), flights.get()), (0, 3));
}

#[test]
fn a_failing_test_still_fails_after_a_restore() {
    let a = plane();
    // The engine does nothing: its test must fail, on a restored start as on an arranged one.
    let mut d = driver(Plane::default())
        .on("the engine cuts out", |_| {})
        .checkpointed();
    let report = a.run(&mut d, &RunOptions::default());
    let failed: Vec<_> = a
        .plans
        .iter()
        .zip(&report.results)
        .filter(|(_, r)| r.status == Status::Failed)
        .map(|(p, _)| p.title.as_str())
        .collect();
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert!(
        failed[0].contains("glides") || failed[0].contains("engine"),
        "{failed:?}"
    );
}

#[test]
fn the_kettle_gives_the_same_verdicts_restored_as_arranged() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/kettle");
    let a = Acceptance::load(dir)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage()
        .with_state_pictures();
    let plain = a.run(&mut world::driver(false), &RunOptions::default());
    let shared = a.run(
        &mut world::driver(false).checkpointed(),
        &RunOptions::default(),
    );
    assert!(plain.passed() && shared.passed());
    assert_eq!(statuses(&plain), statuses(&shared));
    assert!(
        count(&shared, Began::Restored) > 0,
        "the kettle shares starts"
    );
    assert_eq!(
        plain.verdicts(&a.suite, &a.plans),
        shared.verdicts(&a.suite, &a.plans)
    );
}
