//! A start that several tests share is arranged once, by a driver that can save it: the others
//! have it put back, and what they are given is observed afterwards all the same.

use expectation_harness::{Acceptance, Began, Bindings, Checkpoint, Driver, RunOptions};
use expectation_model::{Event, Value, Variable};
use expectation_story::Report;
use std::cell::RefCell;
use std::rc::Rc;

/// «Dim», «Off» and «Dim and off» all start with the lamp on and the dimmer high, which no
/// test ends in; «Bright» and «Brighter» start from the background.
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
Story: Bright
  When the user flips the switch
  Then the lamp is on
Story: Dim
  Given the lamp is on
  And the dimmer is high
  When the user turns it back
  Then the dimmer is low
Story: Off
  Given the lamp is on
  And the dimmer is high
  When the user flips it back
  Then the lamp is off
Story: Brighter
  When the user turns the dimmer
  Then the dimmer is high
Story: Dim and off
  Given the lamp is on
  And the dimmer is high
  When the user turns it back
  Then the dimmer is low
  When the user flips it back
  Then the lamp is off
";

/// What was asked of the lamp, in order. It is not part of the lamp, so putting a saved lamp
/// back doesn't unsay it.
type Log = Rc<RefCell<Vec<&'static str>>>;

#[derive(Clone, Default)]
struct Lamp {
    on: bool,
    high: bool,
    /// How often the switch was flipped on since the reset: nothing in the suite says, and the
    /// lamp shows it only by wearing out.
    flips: u32,
    log: Log,
}

fn lamp() -> Acceptance {
    let loaded = expectation_language::load_sources("lamp", &[("lamp.expect", LAMP)]);
    Acceptance::from_loaded(loaded).unwrap_or_else(|e| panic!("{e}"))
}

/// A lamp that is reset before every test, so that a start is only ever shared by saving it.
fn driver() -> Bindings<Lamp> {
    let say = |w: &Lamp, what| w.log.borrow_mut().push(what);
    Bindings::new(Lamp::default())
        .observe("the lamp", |w: &Lamp| if w.on { "on" } else { "off" })
        .observe("the dimmer", |w: &Lamp| if w.high { "high" } else { "low" })
        .arrange("the lamp", move |w, v| {
            say(w, "arrange");
            w.on = v.to_string() == "on"
        })
        .arrange("the dimmer", |w, v| w.high = v.to_string() == "high")
        .on("the user flips the switch", move |w| {
            say(w, "flip");
            w.flips += 1;
            w.on = true
        })
        .on("the user flips it back", move |w| {
            say(w, "flip back");
            w.on = false
        })
        .on("the user turns the dimmer", move |w| {
            say(w, "turn");
            w.high = true
        })
        .on("the user turns it back", move |w| {
            say(w, "turn back");
            w.high = false
        })
        .on_reset(move |w| {
            say(w, "reset");
            w.on = false;
            w.high = false;
            w.flips = 0
        })
        .always_reset()
}

fn began(report: &Report) -> Vec<Option<Began>> {
    report.results.iter().map(|r| r.began).collect()
}

fn count(d: &Bindings<Lamp>, what: &str) -> usize {
    d.world.log.borrow().iter().filter(|l| **l == what).count()
}

const ARRANGED: Option<Began> = Some(Began::Arranged);
const RESTORED: Option<Began> = Some(Began::Restored);

#[test]
fn a_shared_start_is_arranged_once_and_put_back_for_the_others() {
    let a = lamp();
    let mut d = driver().checkpoints();
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    // «Brighter» is run second, from what was saved for «Bright»; the results stay in order.
    assert_eq!(
        began(&report),
        [ARRANGED, ARRANGED, RESTORED, RESTORED, RESTORED]
    );
    assert_eq!(
        *d.world.log.borrow(),
        [
            // Bright, and Brighter from its start.
            "reset",
            "arrange",
            "flip",
            "turn",
            // Dim, then Off and Dim and off from its start.
            "reset",
            "arrange",
            "turn back",
            "flip back",
            "turn back",
            "flip back",
        ]
    );

    // The same verdicts as resetting and arranging each one.
    let mut d = driver().checkpoints();
    let opts = RunOptions {
        share_starts: false,
        ..Default::default()
    };
    let each = a.run(&mut d, &opts);
    assert_eq!(began(&each), [ARRANGED; 5]);
    assert_eq!(count(&d, "reset"), 5);
    assert_eq!(
        report.verdicts(&a.suite, &a.plans),
        each.verdicts(&a.suite, &a.plans)
    );
    for (x, y) in report.results.iter().zip(&each.results) {
        let observed = |r: &expectation_story::PlanResult| -> Vec<String> {
            std::iter::once(&r.start)
                .chain(&r.steps)
                .flat_map(|s| s.checks.iter().map(|c| format!("{:?}", c.observed)))
                .collect()
        };
        assert_eq!(observed(x), observed(y));
    }
}

#[test]
fn a_driver_without_checkpoints_arranges_every_start() {
    let a = lamp();
    let mut d = driver();
    let report = a.run(&mut d, &RunOptions::default());
    assert!(report.passed());
    assert_eq!(began(&report), [ARRANGED; 5]);
    assert_eq!(count(&d, "reset"), 5);
}

/// A driver whose checkpoints are no good: what it puts back is not what it saved.
struct Forgetful {
    inner: Bindings<Lamp>,
    /// Putting back fails outright, instead of leaving the lamp as it is.
    refuses: bool,
    restores: u32,
}

impl Driver for Forgetful {
    fn may_continue(&self) -> bool {
        false
    }
    fn reset(&mut self) -> Result<(), String> {
        Driver::reset(&mut self.inner)
    }
    fn arrange(&mut self, var: &Variable, value: &Value) -> Result<(), String> {
        Driver::arrange(&mut self.inner, var, value)
    }
    fn set(&mut self, var: &Variable, value: &Value) -> Result<(), String> {
        Driver::set(&mut self.inner, var, value)
    }
    fn perform(&mut self, event: &Event) -> Result<(), String> {
        Driver::perform(&mut self.inner, event)
    }
    fn observe(&mut self, var: &Variable) -> Result<Value, String> {
        Driver::observe(&mut self.inner, var)
    }
    fn checkpoint(&mut self) -> Result<Option<Checkpoint>, String> {
        Ok(Some(Checkpoint::new(())))
    }
    fn restore(&mut self, _: &Checkpoint) -> Result<(), String> {
        self.restores += 1;
        if self.refuses {
            return Err("the snapshot is gone".into());
        }
        Ok(())
    }
}

#[test]
fn a_start_that_does_not_hold_once_put_back_is_arranged_after_all() {
    let mut a = lamp();
    a.plans
        .retain(|p| p.title.starts_with("Dim") || p.title == "Off");
    for refuses in [false, true] {
        let mut d = Forgetful {
            inner: driver(),
            refuses,
            restores: 0,
        };
        let report = a.run(&mut d, &RunOptions::default());
        assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
        // The lamp is left dim after «Dim», which is not where «Off» starts. The checkpoint is
        // dropped, and not taken again: «Dim and off» is arranged without asking.
        assert_eq!(began(&report), [ARRANGED; 3]);
        assert_eq!(d.restores, 1);
        assert_eq!(count(&d.inner, "reset"), 3);
    }
}

#[test]
fn what_no_state_describes_is_put_back_too() {
    // «Bright» flips the switch; «Brighter», put back to the start they share, starts with a
    // switch that was never flipped, as it would after a reset.
    let mut a = lamp();
    a.plans.retain(|p| p.title.starts_with("Bright"));
    let mut d = driver().checkpoints();
    let report = a.run(&mut d, &RunOptions::default());
    assert_eq!(began(&report), [ARRANGED, RESTORED]);
    assert_eq!(count(&d, "flip"), 1);
    assert_eq!(d.world.flips, 0);
}

#[test]
fn a_start_is_only_saved_when_another_test_is_waiting_for_it() {
    struct Counting {
        inner: Bindings<Lamp>,
        saved: u32,
    }
    impl Driver for Counting {
        fn may_continue(&self) -> bool {
            false
        }
        fn reset(&mut self) -> Result<(), String> {
            Driver::reset(&mut self.inner)
        }
        fn arrange(&mut self, var: &Variable, value: &Value) -> Result<(), String> {
            Driver::arrange(&mut self.inner, var, value)
        }
        fn set(&mut self, var: &Variable, value: &Value) -> Result<(), String> {
            Driver::set(&mut self.inner, var, value)
        }
        fn perform(&mut self, event: &Event) -> Result<(), String> {
            Driver::perform(&mut self.inner, event)
        }
        fn observe(&mut self, var: &Variable) -> Result<Value, String> {
            Driver::observe(&mut self.inner, var)
        }
        fn checkpoint(&mut self) -> Result<Option<Checkpoint>, String> {
            self.saved += 1;
            Driver::checkpoint(&mut self.inner)
        }
        fn restore(&mut self, saved: &Checkpoint) -> Result<(), String> {
            Driver::restore(&mut self.inner, saved)
        }
    }
    let a = lamp();
    let mut d = Counting {
        inner: driver().checkpoints(),
        saved: 0,
    };
    let report = a.run(&mut d, &RunOptions::default());
    assert_eq!(
        began(&report),
        [ARRANGED, ARRANGED, RESTORED, RESTORED, RESTORED]
    );
    // Once for each of the two starts, and not again after the last test of each.
    assert_eq!(d.saved, 2);
}

#[test]
fn saved_results_and_the_manual_say_that_a_start_was_put_back() {
    let a = lamp();
    let report = a.run(&mut driver().checkpoints(), &RunOptions::default());
    let saved = report.save(&a.plans);
    assert_eq!(saved.matches("\nbegan\trestored\n").count(), 3);
    assert_eq!(saved.matches("\nbegan\tarranged\n").count(), 2);
    let loaded = Report::load(&saved, &a.plans);
    assert_eq!(began(&loaded), began(&report));

    let dir = std::env::temp_dir().join(format!("expectation-restored-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    a.publish(&dir, "test", &report).unwrap();
    let page = |name: &str| std::fs::read_to_string(a.tree(&dir).join(name)).unwrap();
    let said = "started from a saved copy of its start";
    assert!(page("tests/off.md").contains(said));
    assert!(!page("tests/dim.md").contains(said));
    assert!(page("site/test-off.html").contains(said));
    let _ = std::fs::remove_dir_all(&dir);
}
