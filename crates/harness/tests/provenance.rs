//! What a run was against (the program's revision, its data) is kept with its results, so a
//! manual made of several runs says which build each result is about.

use expectation_harness::{Acceptance, Bindings, Provenance, RunOptions};
use expectation_story::{Agreement, Report, Status};
use std::path::{Path, PathBuf};

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

fn lamp() -> Acceptance {
    let loaded = expectation_language::load_sources("lamp", &[("lamp.expect", LAMP)]);
    Acceptance::from_loaded(loaded).unwrap_or_else(|e| panic!("{e}"))
}

/// A lamp, with a dimmer the driver can turn or not.
fn driver(dimmer: bool) -> Bindings<(bool, bool)> {
    let b = Bindings::new((false, false))
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
        .on_reset(|w| *w = (false, false));
    if dimmer {
        b.on("the user turns the dimmer", |w| w.1 = true)
    } else {
        b
    }
}

fn build(revision: &str) -> Provenance {
    Provenance {
        app_revision: Some(revision.into()),
        data_mode: Some("fixtures".into()),
        fixture: Some("seed-7".into()),
    }
}

fn run(a: &Acceptance, dimmer: bool, provenance: Provenance) -> Report {
    a.run(
        &mut driver(dimmer),
        &RunOptions {
            provenance,
            ..Default::default()
        },
    )
}

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn of<'a>(a: &Acceptance, report: &'a Report, title: &str) -> &'a expectation_story::PlanResult {
    &report.results[a.plans.iter().position(|p| p.title == title).unwrap()]
}

fn page(dir: &Path, a: &Acceptance, title: &str, html: bool) -> String {
    let slug = &a.plans.iter().find(|p| p.title == title).unwrap().slug;
    let path = if html {
        dir.join("site").join(format!("test-{slug}.html"))
    } else {
        dir.join("tests").join(format!("{slug}.md"))
    };
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn what_a_run_was_against_is_saved_with_its_results_and_loads_back() {
    let a = lamp();
    let report = run(&a, true, build("abc123"));
    let saved = report.save(&a.plans);
    assert!(saved.contains("provenance\tapp-revision\tabc123\n"));
    assert!(saved.contains("provenance\tdata-mode\tfixtures\n"));
    assert!(saved.contains("provenance\tfixture\tseed-7\n"));
    let loaded = Report::load(&saved, &a.plans);
    for (was, is) in report.results.iter().zip(&loaded.results) {
        assert_eq!(was.run, is.run);
        assert_eq!(is.run.as_ref().unwrap().provenance, build("abc123"));
    }
    // The rest of the run is what it was.
    assert_eq!(loaded.save(&a.plans), saved);

    // Only what was said is written, and nothing is filled in.
    let partial = Provenance {
        app_revision: Some("abc123".into()),
        ..Default::default()
    };
    let saved = run(&a, true, partial.clone()).save(&a.plans);
    assert!(!saved.contains("data-mode") && !saved.contains("fixture"));
    let loaded = Report::load(&saved, &a.plans);
    assert_eq!(loaded.results[0].run.as_ref().unwrap().provenance, partial);
    let saved = run(&a, true, Provenance::default()).save(&a.plans);
    assert!(!saved.contains("provenance"), "{saved}");
}

#[test]
fn results_from_before_runs_said_are_unknown_not_guessed() {
    let a = lamp();
    let saved = run(&a, true, build("abc123")).save(&a.plans);
    let legacy: String = saved
        .lines()
        .filter(|l| !l.starts_with("provenance\t"))
        .map(|l| format!("{l}\n"))
        .collect();
    let loaded = Report::load(&legacy, &a.plans);
    assert!(loaded.results.iter().all(|r| r.status == Status::Passed));
    let run = loaded.results[0].run.as_ref().unwrap();
    assert!(run.provenance.is_empty());
    assert_eq!(
        run.provenance.agreement(&build("abc123")),
        Some(Agreement::Unknown)
    );
    // A part a later version adds is skipped, and the parts it knows still load.
    let newer = saved.replace(
        "provenance\tfixture\tseed-7\n",
        "provenance\tfixture\tseed-7\nprovenance\tregion\teu\n",
    );
    let loaded = Report::load(&newer, &a.plans);
    assert_eq!(
        loaded.results[0].run.as_ref().unwrap().provenance,
        build("abc123")
    );
}

#[test]
fn a_manual_of_several_runs_says_which_build_each_result_is_about() {
    let a = lamp().with_expected(build("bbb222"));
    let docs = fresh("provenance-mixed");
    // Build A ran everything. Build B's run can't turn the dimmer, so its test keeps A's result.
    let first = run(&a, true, build("aaa111"));
    a.publish(&docs, "nightly", &first).unwrap();
    let second = run(&a, false, build("bbb222"));
    let merged = a.publish(&docs, "ci", &second).unwrap();

    // Each result keeps where it came from; the older pass is still a pass.
    let (light, bright) = (of(&a, &merged, "Light"), of(&a, &merged, "Bright"));
    assert_eq!(bright.status, Status::Passed);
    let against = |r: &expectation_story::PlanResult| r.run.as_ref().unwrap().provenance.clone();
    assert_eq!(against(bright), build("aaa111"));
    assert!([build("aaa111"), build("bbb222")].contains(&against(light)));
    assert_eq!(
        against(bright).agreement(&build("bbb222")),
        Some(Agreement::Mismatched)
    );

    for html in [false, true] {
        let page = page(&docs, &a, "Bright", html);
        assert!(page.contains("aaa111"), "{page}");
        assert!(page.contains("fixtures") && page.contains("seed-7"));
        assert!(page.contains("This manual is for application revision bbb222"));
        assert!(page.contains("not a failure of it"));
    }
    let index = std::fs::read_to_string(docs.join("tests.md")).unwrap();
    assert!(
        index.contains("This manual is for application revision bbb222"),
        "{index}"
    );
    assert!(index.contains("against something else"));
    // The verdicts are the runs': nothing failed because a build differs.
    assert!(merged.passed());
    assert!(index.contains("2 passed, 0 failed"), "{index}");
}

#[test]
fn a_manual_says_matching_unknown_and_nothing_when_it_isnt_for_anything() {
    // The same build: it says so.
    let a = lamp().with_expected(build("abc123"));
    let docs = fresh("provenance-matching");
    a.publish(&docs, "ci", &run(&a, true, build("abc123")))
        .unwrap();
    for html in [false, true] {
        let page = page(&docs, &a, "Light", html);
        assert!(page.contains("abc123") && page.contains("This is what this manual is for."));
    }

    // A run that didn't say: unknown, by name.
    let docs = fresh("provenance-unknown");
    a.publish(&docs, "ci", &run(&a, true, Provenance::default()))
        .unwrap();
    for html in [false, true] {
        let page = page(&docs, &a, "Light", html);
        assert!(
            page.contains("The run didn't say its application revision, data mode and fixture"),
            "{page}"
        );
    }

    // A manual for nothing in particular shows what was said, and compares nothing.
    let a = lamp();
    let docs = fresh("provenance-shown");
    a.publish(&docs, "ci", &run(&a, true, build("abc123")))
        .unwrap();
    let page = page(&docs, &a, "Light", false);
    assert!(page.contains("**Run against:** application revision `abc123`"));
    assert!(!page.contains("This manual is for") && !page.contains("didn't say"));
    assert!(
        !std::fs::read_to_string(docs.join("tests.md"))
            .unwrap()
            .contains("This manual is for")
    );

    // And with nothing said and nothing expected, the page is as it always was.
    let docs = fresh("provenance-none");
    a.publish(&docs, "ci", &run(&a, true, Provenance::default()))
        .unwrap();
    assert!(!page_has_against(&docs, &a));
}

fn page_has_against(docs: &Path, a: &Acceptance) -> bool {
    page(docs, a, "Light", false).contains("Run against")
        || page(docs, a, "Light", true).contains("against")
}
