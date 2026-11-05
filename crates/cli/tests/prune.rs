//! `expectation screenshots prune`: what no results file names goes, and only that.

use std::path::Path;
use std::process::Command;

fn hash(c: char) -> String {
    std::iter::repeat_n(c, 64).collect()
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_expectation"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn pruning_removes_what_no_results_name_and_nothing_else() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("prune");
    let _ = std::fs::remove_dir_all(&root);
    let (store, results, empty) = (root.join("store"), root.join("results"), root.join("none"));
    for d in [&store, &store.join("thumbs"), &results, &empty] {
        std::fs::create_dir_all(d).unwrap();
    }
    let (kept, gone) = (format!("{}.png", hash('a')), format!("{}.png", hash('b')));
    for name in [&kept, &gone] {
        std::fs::write(store.join(name), b"png").unwrap();
        std::fs::write(store.join("thumbs").join(name), b"small").unwrap();
    }
    // Not named by a hash: never the store's to remove.
    std::fs::write(store.join("README.md"), b"mine").unwrap();
    std::fs::write(
        results.join("app@iOS.tsv"),
        format!(
            "expectation-results 1\nrun\tr1\t1\nplan\tt\tpassed\tfp\nstep\t0\tpassed\t1\tscreenshots/{kept}\t-\n"
        ),
    )
    .unwrap();
    // A coverage file and other files there aren't results.
    std::fs::write(
        results.join("api.coverage.tsv"),
        format!("expectation-coverage 1\nstep\t0\tx\t1\tscreenshots/{gone}\t-\n"),
    )
    .unwrap();

    let s = |p: &Path| p.to_str().unwrap().to_string();
    // With no results at all, nothing goes.
    let out = run(&["screenshots", "prune", &s(&store), &s(&empty)]);
    assert_eq!(out.status.code(), Some(2));
    assert!(store.join(&gone).is_file());

    // A dry run says what would go, and leaves it.
    let out = run(&[
        "screenshots",
        "prune",
        &s(&store),
        &s(&results),
        "--dry-run",
    ]);
    assert!(out.status.success());
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        said.contains(&gone) && said.contains("would remove 2 screenshots"),
        "{said}"
    );
    assert!(store.join(&gone).is_file());

    let out = run(&["screenshots", "prune", &s(&store), &s(&results)]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!store.join(&gone).exists() && !store.join("thumbs").join(&gone).exists());
    assert!(store.join(&kept).is_file() && store.join("thumbs").join(&kept).is_file());
    assert!(store.join("README.md").is_file());
}
