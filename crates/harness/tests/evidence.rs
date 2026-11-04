//! What a run records for evidence gathered outside it: the run's id, when each step ran, and
//! the files the driver captured, kept where the docs can't publish them.
#[path = "../examples/kettle/world.rs"]
mod world;

use expectation_harness::{Acceptance, Capture, RunOptions};
use expectation_story::{Report, Status};
use std::path::Path;

fn suite() -> Acceptance {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/kettle");
    Acceptance::load(dir)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage()
}

#[test]
fn a_run_records_its_id_its_step_windows_and_what_the_driver_captured() {
    let a = suite();
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("evidence");
    let _ = std::fs::remove_dir_all(&root);
    let (docs, kept) = (root.join("docs"), root.join("attachments"));
    let mut n = 0;
    let mut driver = world::driver(false).on_attachments(move |_| {
        n += 1;
        // A name that tries to leave the directory, and two captures with the same name.
        vec![
            Capture {
                name: "../../escape me.har".into(),
                media_type: "application/har+json".into(),
                bytes: format!("{{\"n\": {n}}}").into_bytes(),
            },
            Capture {
                name: "../../escape me.har".into(),
                media_type: "application/har+json\tsneaky".into(),
                bytes: b"{}".to_vec(),
            },
        ]
    });
    let report = a.run(
        &mut driver,
        &RunOptions {
            docs_dir: Some(docs.clone()),
            attachments_dir: Some(kept.clone()),
            ..Default::default()
        },
    );
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));

    // One run for every test.
    let run = report.results[0].run.clone().expect("a run id");
    assert!(report.results.iter().all(|r| r.run.as_ref() == Some(&run)));

    for (plan, r) in a.plans.iter().zip(&report.results) {
        // Every step that ran has a window, and they follow one another.
        let windows: Vec<_> = std::iter::once(&r.start)
            .chain(&r.steps)
            .map(|s| s.window.expect("a window"))
            .collect();
        for w in &windows {
            assert!(w.start_ms <= w.end_ms);
        }
        for pair in windows.windows(2) {
            assert!(pair[0].end_ms <= pair[1].start_ms);
        }
        assert!(windows[0].start_ms >= run.started_ms);
        // Two captures after the start and after each step, kept inside the directory.
        assert_eq!(
            r.attachments.len(),
            2 * (plan.steps.len() + 1),
            "{}",
            plan.slug
        );
        for att in &r.attachments {
            assert!(!att.path.contains(".."), "{}", att.path);
            assert!(att.path.starts_with(&format!("{}/{}/", run.id, plan.slug)));
            assert!(kept.join(&att.path).is_file(), "{}", att.path);
            assert!(!att.media_type.contains('\t'));
        }
        assert_eq!(
            r.attachments[0].path,
            format!("{}/{}/00/escape_me.har", run.id, plan.slug)
        );
        assert_eq!(
            r.attachments[1].path,
            format!("{}/{}/00/2-escape_me.har", run.id, plan.slug)
        );
    }
    // None of it is in the docs.
    let in_docs = std::fs::read_dir(&docs)
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().contains("attachments"));
    assert!(!in_docs);

    // Saved and loaded, it is all still there.
    let loaded = Report::load(&report.save(&a.plans), &a.plans);
    for (x, y) in report.results.iter().zip(&loaded.results) {
        assert_eq!(x.run, y.run);
        assert_eq!(x.attachments, y.attachments);
        assert_eq!(x.start.window, y.start.window);
        let (wx, wy): (Vec<_>, Vec<_>) = (
            x.steps.iter().map(|s| s.window).collect(),
            y.steps.iter().map(|s| s.window).collect(),
        );
        assert_eq!(wx, wy);
    }
}

#[test]
fn without_somewhere_to_keep_them_captures_are_taken_and_dropped() {
    let a = suite();
    let asked = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = asked.clone();
    let mut driver = world::driver(false).on_attachments(move |_| {
        count.set(count.get() + 1);
        vec![Capture {
            name: "log.txt".into(),
            media_type: "text/plain".into(),
            bytes: b"x".to_vec(),
        }]
    });
    let report = a.run(&mut driver, &RunOptions::default());
    assert!(report.results.iter().all(|r| r.attachments.is_empty()));
    // The driver's buffer is still emptied after every step.
    let steps: usize = a.plans.iter().map(|p| p.steps.len() + 1).sum();
    assert_eq!(asked.get(), steps);
}

#[test]
fn results_saved_before_runs_had_ids_still_load() {
    let a = suite();
    let report = a.run(&mut world::driver(false), &RunOptions::default());
    // An old file: no run, window or attachment lines.
    let old: String = report
        .save(&a.plans)
        .lines()
        .filter(|l| !l.starts_with("run\t") && !l.starts_with("window\t"))
        .map(|l| format!("{l}\n"))
        .collect();
    let loaded = Report::load(&old, &a.plans);
    assert!(loaded.results.iter().all(|r| r.status == Status::Passed));
    assert!(loaded.results.iter().all(|r| r.run.is_none()));
    assert!(loaded.results[0].start.window.is_none());
}

#[test]
fn the_manual_shows_contract_coverage_from_saved_files_alone() {
    let a = suite();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("evidence-manual");
    let _ = std::fs::remove_dir_all(&dir);
    let report = a.run(&mut world::driver(false), &RunOptions::default());
    let run = report.results[0].run.clone().unwrap();
    let (i, plan) = a
        .plans
        .iter()
        .enumerate()
        .find(|(_, p)| p.slug.starts_with("first-cup"))
        .unwrap();
    let print = expectation_story::fingerprint(plan);
    let results = dir.join("results");
    std::fs::create_dir_all(&results).unwrap();
    // What an extension would write: one call the dial made, one from a run long gone, and a
    // label that tries to be markup.
    std::fs::write(
        results.join("kettle-api.coverage.tsv"),
        format!(
            "expectation-coverage 1
contract\tKettle API\tAPI contract\tsha256:feed
evidence\te1\t{id}\t{slug}\t{print}\t3\ttimed\tlive\tPUT /dial → 200
evidence\te2\told-run\t{slug}\t{print}\t3\ttimed\tlive\tPUT /dial → 503
evidence\te3\t{id}\t{slug}\t{print}\t3\tspans\tfault\t<script>alert(1)</script>
item\tok\tPUT /dial\t200 dial set\t#/paths/~1dial/put/responses/200\tobserved\t-
item\tbusy\tPUT /dial\t503 busy\t#/paths/~1dial/put/responses/503\tobserved\t-
item\tbad\tPUT /dial\t400 bad dial\t#/paths/~1dial/put/responses/400\tnot-observed\t-
seen\tok\te1
seen\tbusy\te2
complete\t{id}\tyes\t-
undocumented\tPUT /dial\t418 teapot\te3
",
            id = run.id,
            slug = plan.slug
        ),
    )
    .unwrap();
    a.publish(&dir, "app", &report).unwrap();
    let read = |page: &str| std::fs::read_to_string(dir.join(page)).unwrap();

    let md = read("contracts/kettle-api.md");
    assert!(
        md.contains("**3 outcomes:** 1 observed, 1 not observed, 1 unknown."),
        "{md}"
    );
    // The observation links to the step of the test that made it.
    assert!(
        md.contains(&format!("(../tests/{}.md#step-3)", plan.slug)),
        "{md}"
    );
    // Evidence from another run doesn't count.
    assert!(
        md.contains("seen only in evidence from another run"),
        "{md}"
    );
    assert!(md.contains("- 1 from another run"), "{md}");
    // Labels can't become markup.
    assert!(!md.contains("<script>"));
    let html = read("site/contract-kettle-api.html");
    assert!(!html.contains("<script>alert"), "labels are escaped");
    assert!(html.contains(&format!("test-{}.html#step-3", plan.slug)));
    assert!(html.contains("injected"), "a fault is marked as one");
    // The test page shows the step's evidence where it happened.
    let test = read(&format!("site/test-{}.html", plan.slug));
    assert!(test.contains(r#"id="step-3""#));
    assert!(test.contains("PUT /dial → 200"));
    assert!(!test.contains("PUT /dial → 503"), "not from another run");
    let report_page = read("tests.md");
    assert!(report_page.contains("[Kettle API](contracts/kettle-api.md)"));
    let json = read("contracts/kettle-api.json");
    assert!(
        json.contains("\"observed\": 1, \"not_observed\": 1, \"unknown\": 1"),
        "{json}"
    );
    assert_eq!(
        report.results[i].status,
        Status::Passed,
        "results unchanged"
    );

    // From the saved files alone, without running anything, the same pages.
    let saved = expectation_story::load_results_for(&results, &a.plans, None);
    let (found, errors) = expectation_story::load_contracts_for(&results, None);
    assert!(errors.is_empty());
    let judged: Vec<_> = found
        .into_iter()
        .map(|c| expectation_story::Coverage::judge(c, &a.plans, &saved))
        .collect();
    let mut docs = a.docs(Some(&saved));
    docs.contracts = &judged;
    let again = dir.join("again");
    docs.write(&again).unwrap();
    assert_eq!(
        std::fs::read_to_string(again.join("contracts/kettle-api.md")).unwrap(),
        md
    );
}
