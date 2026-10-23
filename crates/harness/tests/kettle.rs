//! The kettle example as tests: the suite checks clean, every story and generated test
//! passes against the toy world and documents itself, and a seeded defect is caught by the
//! expectation it breaks.

#[path = "../examples/kettle/world.rs"]
mod world;

use expectation_harness::{Acceptance, Bindings, RunOptions};
use expectation_story::{Status, Verdict};
use std::path::Path;

fn suite() -> Acceptance {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/kettle");
    Acceptance::load(dir).unwrap_or_else(|e| panic!("{e}")).with_coverage()
}

#[test]
fn every_story_passes_and_documents_itself() {
    let a = suite();
    assert!(a.diagnostics.iter().all(|d| !d.is_error()));
    let docs = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kettle_docs");
    let opts = RunOptions { docs_dir: Some(docs.clone()), ..Default::default() };
    let report = a.assert_passes(&mut world::driver(false), &opts);

    // Generated tests reach what the stories don't, so everything gets verified.
    assert!(report.verdicts(&a.suite, &a.plans).iter().all(|v| *v == Verdict::Verified));
    let read = |page: &str| std::fs::read_to_string(docs.join(page)).unwrap();
    assert!(read("tests.md").contains("```mermaid"));
    assert!(read("features/heating.md").contains("**Except** when the child lock is on"));
    // The story is a tutorial in the reader's words, and a test report step by step.
    let tutorial = read("tutorials/first-cup--turn-it-up-then-turn-it-off.md");
    assert!(tutorial.contains("2. **Set the dial to 0 %. Then press reset.**"));
    let test = read("tests/first-cup--turn-it-up-then-turn-it-off.md");
    assert!(test.contains("## 2. When the dial is 0 % and the user presses reset"));
    assert!(docs.join("screenshots/first-cup--turn-it-up-then-turn-it-off/01.svg").exists());
}

#[test]
fn a_seeded_defect_fails_the_expectation_it_breaks() {
    let a = suite();
    let report = a.run(&mut world::driver(true), &RunOptions::default());
    assert!(!report.passed());
    let verdicts = report.verdicts(&a.suite, &a.plans);
    let broken = a.suite.expectations.iter().find(|e| e.name.starts_with("The child lock holds")).unwrap();
    assert_eq!(verdicts[broken.id.0 as usize], Verdict::Failed);
    // Steps after the failure are skipped, not failed.
    let failed = report.results.iter().find(|r| r.status == Status::Failed).unwrap();
    assert!(failed.steps.iter().any(|s| s.status == Status::Skipped));
    assert!(report.failures(&a.suite, &a.plans).contains("observed heating"));
}

#[test]
fn what_the_driver_cant_do_is_not_supported_yet() {
    let a = suite();
    let mut driver = Bindings::new(world::World::new(false)).observe("the kettle", |w| w.phase_name());
    let report = a.run(&mut driver, &RunOptions::default());
    let first = &report.results[0];
    // Not a failure: nothing ran.
    assert_eq!(first.status, Status::Unsupported);
    assert!(report.passed());
    let error = first.start.error.as_deref().unwrap();
    assert!(error.starts_with("The driver can't arrange the child lock"), "{error}");
    assert!(error.contains("more things"), "{error}");
    let verdicts = report.verdicts(&a.suite, &a.plans);
    assert!(verdicts.iter().all(|v| *v == Verdict::Unsupported), "{verdicts:?}");
}

#[test]
fn results_save_and_load() {
    let a = suite();
    let report = a.run(&mut world::driver(true), &RunOptions::default());
    let loaded = expectation_story::Report::load(&report.save(&a.plans), &a.plans);
    for (x, y) in report.results.iter().zip(&loaded.results) {
        assert_eq!(x.status, y.status);
        assert_eq!(x.steps.len(), y.steps.len());
        for (p, q) in x.steps.iter().zip(&y.steps) {
            assert_eq!((p.status, p.checks.len()), (q.status, q.checks.len()));
            for (c, d) in p.checks.iter().zip(&q.checks) {
                assert_eq!((c.passed, &c.observed), (d.passed, &d.observed));
            }
        }
    }
    // A plan the results don't mention hasn't run; merging prefers what ran.
    let none = expectation_story::Report::not_run(&a.plans);
    assert!(none.results.iter().all(|r| r.status == Status::NotRun));
    let merged = none.merge(loaded);
    assert_eq!(merged.count(Status::Failed), 1);
    // Results saved for an older version of a plan aren't its results.
    let mut changed = a.plans.clone();
    changed[0].steps[0].checks.clear();
    let stale = expectation_story::Report::load(&report.save(&a.plans), &changed);
    assert_eq!(stale.results[0].status, Status::NotRun);
    assert_eq!(stale.results[1].status, report.results[1].status);
}

/// A driver that does everything the toy world does, except start with the preset WARM.
struct NoWarmStart(Bindings<world::World>);

impl expectation_harness::Driver for NoWarmStart {
    fn reset(&mut self) -> Result<(), String> {
        expectation_harness::Driver::reset(&mut self.0)
    }
    fn arrange(&mut self, var: &expectation_model::Variable, value: &expectation_model::Value) -> Result<(), String> {
        expectation_harness::Driver::arrange(&mut self.0, var, value)
    }
    fn set(&mut self, var: &expectation_model::Variable, value: &expectation_model::Value) -> Result<(), String> {
        expectation_harness::Driver::set(&mut self.0, var, value)
    }
    fn perform(&mut self, event: &expectation_model::Event) -> Result<(), String> {
        expectation_harness::Driver::perform(&mut self.0, event)
    }
    fn observe(&mut self, var: &expectation_model::Variable) -> Result<expectation_model::Value, String> {
        expectation_harness::Driver::observe(&mut self.0, var)
    }
    fn settle(&mut self) {
        expectation_harness::Driver::settle(&mut self.0)
    }
    fn advance(&mut self) -> expectation_harness::Progress {
        expectation_harness::Driver::advance(&mut self.0)
    }
    fn supports(&self, need: expectation_harness::Need<'_>) -> bool {
        expectation_harness::Driver::supports(&self.0, need)
    }
    fn can_arrange(&self, var: &expectation_model::Variable, value: &expectation_model::Value) -> bool {
        !(var.key == "preset" && value.as_str() == Some("WARM"))
    }
}

#[test]
fn a_value_the_driver_cant_arrange_is_not_supported_yet() {
    let a = suite();
    let mut driver = NoWarmStart(world::driver(false));
    let report = a.run(&mut driver, &RunOptions::default());
    // Not a failure: every test starts on WARM, and this system can't be put there.
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    assert!(report.results.iter().all(|r| r.status == Status::Unsupported));
    assert!(report.results.iter().all(|r| r.start.error.as_deref().unwrap().contains("arrange the preset as WARM")));
}

#[test]
fn workers_in_parallel_each_get_their_own_driver_and_report_in_plan_order() {
    use std::sync::Mutex;
    let a = suite();
    let serial = a.run(&mut world::driver(false), &RunOptions::default());
    let built = Mutex::new(vec![]);
    let report = a.run_parallel(3, &RunOptions::default(), |n| {
        built.lock().unwrap().push(n);
        expectation_harness::Worker::new(world::driver(false))
    });
    let statuses = |r: &expectation_story::Report| r.results.iter().map(|r| r.status).collect::<Vec<_>>();
    assert_eq!(statuses(&report), statuses(&serial), "the same results, in the same order");
    let mut built = built.into_inner().unwrap();
    built.sort();
    // A worker builds its driver when it gets a plan, so a fast one can leave another nothing to do.
    assert!(!built.is_empty() && built.iter().all(|n| *n < 3), "{built:?}");
    assert_eq!(built.len(), built.iter().collect::<std::collections::BTreeSet<_>>().len(), "at most one driver for each worker");

    // More workers than tests is fine, and a defect is still caught.
    let leaky = a.run_parallel(1000, &RunOptions::default(), |_| expectation_harness::Worker::new(world::driver(true)));
    assert!(!leaky.passed());
}

/// Results stay valid while the story is the same: a word added to the vocabulary renumbers the
/// states and events, and changes nothing a reader sees. A story reworded is another test.
#[test]
fn saved_results_survive_an_unrelated_change_to_the_vocabulary_but_not_a_reworded_story() {
    let copy = |name: &str, edit: &dyn Fn(&str, &str) -> String| {
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/kettle");
        let to = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _ = std::fs::remove_dir_all(&to);
        std::fs::create_dir_all(&to).unwrap();
        for f in std::fs::read_dir(&from).unwrap().flatten() {
            if f.path().extension().is_some_and(|e| e == "expect") {
                let file = f.file_name().to_string_lossy().into_owned();
                let text = std::fs::read_to_string(f.path()).unwrap();
                std::fs::write(to.join(&file), edit(&file, &text)).unwrap();
            }
        }
        Acceptance::load(&to).unwrap_or_else(|e| panic!("{e}")).with_coverage()
    };
    let prints = |a: &Acceptance| -> Vec<(String, String)> {
        a.plans.iter().map(|p| (p.slug.clone(), expectation_story::fingerprint(p))).collect()
    };
    let before = prints(&copy("print_before", &|_, t| t.to_string()));
    // A new state first in the vocabulary: every id after it moves.
    let moved = copy("print_moved", &|f, t| {
        if f == "vocabulary.expect" {
            t.replace("Vocabulary:\n", "Vocabulary:\n  State: the lamp is on | off\n")
        } else {
            t.to_string()
        }
    });
    let after = prints(&moved);
    for (slug, print) in &before {
        assert_eq!(after.iter().find(|(s, _)| s == slug).map(|(_, p)| p), Some(print), "{slug}");
    }
    let reworded = prints(&copy("print_reworded", &|f, t| {
        if f == "stories.expect" { t.replacen("Then the kettle is heating\n", "Then the kettle is heating\n    And the child lock is off\n", 1) } else { t.to_string() }
    }));
    assert!(before.iter().any(|b| !reworded.contains(b)), "a story that is reworded keeps its print");
}
