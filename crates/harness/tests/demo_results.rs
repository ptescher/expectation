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
