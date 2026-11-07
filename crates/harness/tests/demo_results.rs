//! The three-platform demo's manual is built from results its runs saved, since CI can't run
//! the apps. Those results only count while each test's fingerprint matches, so a change to
//! how fingerprints are made, or a reworded demo story, would quietly turn the published
//! manual into "not run". This keeps that from happening unnoticed.
use expectation_harness::Acceptance;
use expectation_story::Status;
use std::path::Path;

#[test]
fn the_demos_saved_results_still_match_its_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demos/kettle");
    for platform in ["GPUI", "SwiftUI", "React Native"] {
        let a = Acceptance::load_for(root.join("expectations"), platform)
            .unwrap_or_else(|e| panic!("{e}"))
            .with_coverage();
        let file = root.join(format!("docs/results/app@{platform}.tsv"));
        let saved = std::fs::read_to_string(&file).unwrap();
        let report = expectation_story::load_results_for(
            &root.join("docs/results"),
            &a.plans,
            Some(platform),
        );
        // Plans the runs saved results for; a test added since then has none yet, and that's fine.
        let ran: Vec<&str> = saved
            .lines()
            .filter_map(|l| l.strip_prefix("plan\t")?.split('\t').next())
            .collect();
        assert!(!ran.is_empty(), "{}", file.display());
        for slug in ran {
            let Some(i) = a.plans.iter().position(|p| p.slug == slug) else {
                panic!("{platform}: results for «{slug}», which is no longer a test");
            };
            assert_ne!(
                report.results[i].status,
                Status::NotRun,
                "{platform}: the saved result for «{slug}» no longer matches the test. If the \
                 story changed, re-run the demo (demos/kettle/run.sh); if fingerprints changed, \
                 keep them stable for unchanged tests"
            );
        }
    }
}

/// A platform other than the first is written under `platforms/<name>/`, but its screenshots
/// are the run's, in the docs' own `screenshots/`: its pages show them from there.
#[test]
fn a_later_platforms_manual_shows_the_shared_screenshots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demos/kettle");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("shared-shots");
    let _ = std::fs::remove_dir_all(&out);
    let a = Acceptance::load_for(root.join("expectations"), "SwiftUI")
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage();
    let report =
        expectation_story::load_results_for(&root.join("docs/results"), &a.plans, Some("SwiftUI"));
    let named: Vec<String> = report
        .results
        .iter()
        .flat_map(|r| std::iter::once(&r.start).chain(&r.steps))
        .filter_map(|s| s.screenshot.clone())
        .collect();
    assert!(
        !named.is_empty(),
        "the demo's SwiftUI results name no screenshots"
    );
    // Only the shared folder has them, as a run writes them.
    for p in &named {
        let path = out.join(p);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
    }
    a.publish(&out, "app", &report).unwrap();

    let tree = out.join("platforms/swiftui");
    let mut shown = 0;
    for entry in std::fs::read_dir(tree.join("site")).unwrap().flatten() {
        let page = std::fs::read_to_string(entry.path()).unwrap_or_default();
        shown += page.matches("../../../screenshots/").count();
        assert!(
            !page.contains(r#"src="../screenshots/"#),
            "{}",
            entry.path().display()
        );
    }
    assert!(shown > 0, "no page links the shared screenshots");
    assert!(expectation_docs::broken_links(&tree).is_empty());
}
