//! A test whose start can't be set up fails, and still shows what the system looked like.

use expectation_harness::{Acceptance, Capture, Driver, RunOptions, Screenshot};
use expectation_model::{Event, Value, Variable};
use expectation_story::Status;
use std::path::{Path, PathBuf};

const LAMP: &str = "
Vocabulary:
  State: the lamp is off | on
  Event: the user flips the switch
Background:
  Given the lamp is off
Feature: Lamp
  Expectation: Flipping turns it on
    Given the lamp is off
    When the user flips the switch
    Then the lamp is on
  Expectation: Flipping turns it off
    Given the lamp is on
    When the user flips the switch
    Then the lamp is off
Story: Off again
  Given the lamp is on
  When the user flips the switch
  Then the lamp is off
";

/// A lamp whose reset or arrangement can break, and whose camera can too.
#[derive(Default)]
struct Lamp {
    on: bool,
    reset_fails: bool,
    arrange_fails: bool,
    camera_fails: bool,
    shots: usize,
    performed: usize,
    /// One capture waits for whoever asks next.
    captured: bool,
}

impl Driver for Lamp {
    fn reset(&mut self) -> Result<(), String> {
        self.on = false;
        self.captured = true;
        if self.reset_fails {
            return Err("the lamp is unplugged".into());
        }
        Ok(())
    }
    fn arrange(&mut self, _: &Variable, _: &Value) -> Result<(), String> {
        if self.arrange_fails {
            return Err("the switch is stuck".into());
        }
        self.on = true;
        Ok(())
    }
    fn set(&mut self, _: &Variable, _: &Value) -> Result<(), String> {
        Ok(())
    }
    fn perform(&mut self, _: &Event) -> Result<(), String> {
        self.performed += 1;
        self.on = !self.on;
        Ok(())
    }
    fn observe(&mut self, _: &Variable) -> Result<Value, String> {
        Ok(Value::Enum(if self.on { "on" } else { "off" }.into()))
    }
    fn take_attachments(&mut self) -> Vec<Capture> {
        if !std::mem::take(&mut self.captured) {
            return vec![];
        }
        vec![Capture {
            name: "lamp.log".into(),
            media_type: "text/plain".into(),
            bytes: b"click".to_vec(),
        }]
    }
    fn screenshot(&mut self) -> Result<Option<Screenshot>, String> {
        self.shots += 1;
        if self.camera_fails {
            return Err("no camera".into());
        }
        Ok(Some(Screenshot::svg(format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><text>broken {}</text></svg>",
            self.shots
        ))))
    }
}

fn lamp() -> Acceptance {
    let loaded = expectation_language::load_sources("lamp", &[("lamp.expect", LAMP)]);
    Acceptance::from_loaded(loaded).unwrap_or_else(|e| panic!("{e}"))
}

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Both ways a start can fail, each with both places a screenshot can go.
#[test]
fn a_start_that_cant_be_set_up_is_pictured_as_it_failed() {
    let a = lamp();
    for (reset_fails, error) in [
        (true, "couldn't reset: the lamp is unplugged"),
        (
            false,
            "couldn't arrange the lamp is on: the switch is stuck",
        ),
    ] {
        for stored in [false, true] {
            let root = fresh(&format!("setup-failure-{reset_fails}-{stored}"));
            let (docs, store, kept) = (root.join("docs"), root.join("store"), root.join("kept"));
            let mut driver = Lamp {
                reset_fails,
                arrange_fails: !reset_fails,
                ..Default::default()
            };
            let report = a.run(
                &mut driver,
                &RunOptions {
                    docs_dir: (!stored).then(|| docs.clone()),
                    screenshot_store: stored.then(|| store.clone()),
                    attachments_dir: Some(kept.clone()),
                    ..Default::default()
                },
            );
            let r = &report.results[0];
            // The failure is the setup's, as it was.
            assert_eq!(r.status, Status::Failed);
            assert_eq!(r.start.status, Status::Failed);
            assert_eq!(r.start.error.as_deref(), Some(error));
            // Nothing after it ran.
            assert!(r.steps.iter().all(|s| s.status == Status::Skipped));
            assert!(r.steps.iter().all(|s| s.screenshot.is_none()));
            assert_eq!(driver.performed, 0);
            // What it looked like is the start's screenshot, where screenshots go.
            let shot = r.start.screenshot.as_deref().expect("a screenshot");
            let file = if stored {
                store.join(shot.strip_prefix("screenshots/").unwrap())
            } else {
                assert_eq!(shot, format!("screenshots/{}/00.svg", a.plans[0].slug));
                docs.join(shot)
            };
            assert!(std::fs::read_to_string(file).unwrap().contains("broken"));
            assert!(r.start.window.is_some());
            // And what the driver captured is still kept, for the start.
            assert_eq!(r.attachments.len(), 1);
            assert_eq!(r.attachments[0].step, 0);
            assert!(kept.join(&r.attachments[0].path).is_file());

            // The manual says the picture is of the failure.
            if !stored {
                a.docs(Some(&report)).write(&docs).unwrap();
                let page = std::fs::read_to_string(
                    docs.join("tests").join(format!("{}.md", a.plans[0].slug)),
                )
                .unwrap();
                assert!(page.contains(shot), "{page}");
                assert!(page.contains("What it looked like when this failed."));
            }
        }
    }
}

#[test]
fn a_camera_that_fails_too_leaves_the_setup_failure_as_it_is() {
    let a = lamp();
    let docs = fresh("setup-failure-no-camera");
    let mut driver = Lamp {
        reset_fails: true,
        camera_fails: true,
        ..Default::default()
    };
    let report = a.run(
        &mut driver,
        &RunOptions {
            docs_dir: Some(docs),
            ..Default::default()
        },
    );
    let r = &report.results[0];
    assert_eq!(r.status, Status::Failed);
    assert_eq!(
        r.start.error.as_deref(),
        Some("couldn't reset: the lamp is unplugged")
    );
    assert_eq!(r.start.screenshot, None);
    assert!(driver.shots > 0, "it tried");
}

#[test]
fn without_screenshots_a_failed_setup_asks_for_none() {
    let a = lamp();
    let mut driver = Lamp {
        reset_fails: true,
        ..Default::default()
    };
    let report = a.run(&mut driver, &RunOptions::default());
    assert_eq!(report.results[0].status, Status::Failed);
    assert_eq!(report.results[0].start.screenshot, None);
    assert_eq!(driver.shots, 0);
}
