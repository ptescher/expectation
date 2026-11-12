//! `expectation-run --app-revision` saves what the run was against, and `expectation docs
//! --expect-app-revision` says whether a saved result is about the build the manual is for.

use expectation_remote::client::{Client, Handlers};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const SUITE: &str = "
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
Story: Light
  When the user flips the switch
  Then the lamp is on
";

fn run(suite: &Path, args: &[&str]) {
    let mut runner = Command::new(env!("CARGO_BIN_EXE_expectation-run"))
        .arg(suite)
        .args(["--addr", "127.0.0.1:0", "--settle", "0", "--wait", "20"])
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut said = BufReader::new(runner.stderr.take().unwrap());
    let mut line = String::new();
    said.read_line(&mut line).unwrap();
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or_else(|| panic!("no address in {line:?}"))
        .to_string();
    let h: Handlers<bool> = Handlers::new()
        .observe("the lamp", |on: &bool| if *on { "on" } else { "off" })
        .arrange("the lamp", |on, v| *on = v.to_string() == "on")
        .on("the user flips the switch", |on| *on = true)
        .on_reset(|on| *on = false);
    let client = Client::connect(&addr, h.hello("test", "Lamp"));
    std::thread::spawn(move || {
        let mut on = false;
        while let Some(r) = client.recv_timeout(Duration::from_secs(20)) {
            h.handle(&mut on, r);
        }
    });
    let mut rest = String::new();
    said.read_to_string(&mut rest).unwrap();
    assert!(runner.wait().unwrap().success(), "{line}{rest}");
}

fn docs(suite: &Path, out: &Path, args: &[&str]) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cli/Cargo.toml");
    let written = Command::new(env!("CARGO"))
        .args(["run", "-q", "--manifest-path"])
        .arg(manifest)
        .args(["--bin", "expectation", "--", "docs"])
        .arg(suite)
        .arg(out)
        .args(args)
        .output()
        .unwrap();
    assert!(
        written.status.success(),
        "{}",
        String::from_utf8_lossy(&written.stderr)
    );
}

#[test]
fn a_run_says_what_it_was_against_and_the_docs_say_whether_that_is_this_build() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("provenance-run");
    let _ = std::fs::remove_dir_all(&root);
    let (suite, out) = (root.join("suite"), root.join("docs"));
    std::fs::create_dir_all(&suite).unwrap();
    std::fs::write(suite.join("lamp.expect"), SUITE).unwrap();
    let page = || std::fs::read_to_string(out.join("tests/light.md")).unwrap();

    run(
        &suite,
        &["--runner", "app", "--docs", out.to_str().unwrap()],
    );
    // Nothing was said, so nothing is saved or shown.
    let saved = std::fs::read_to_string(out.join("results/app.tsv")).unwrap();
    assert!(!saved.contains("provenance"), "{saved}");
    assert!(!page().contains("Run against"));

    let args = ["--runner", "app", "--docs", out.to_str().unwrap()];
    let build = ["--app-revision", "aaa111", "--data-mode", "fixtures"];
    run(
        &suite,
        &[&args[..], &build[..], &["--fixture", "seed-7"]].concat(),
    );
    let saved = std::fs::read_to_string(out.join("results/app.tsv")).unwrap();
    assert!(saved.contains("provenance\tapp-revision\taaa111\n"));
    assert!(saved.contains("provenance\tdata-mode\tfixtures\n"));
    assert!(saved.contains("provenance\tfixture\tseed-7\n"));
    // The run's own manual is for what it ran against.
    assert!(page().contains("application revision `aaa111`"));
    assert!(page().contains("This is what this manual is for."));

    // The manual for the next build, written without running again: the pass is build A's.
    docs(&suite, &out, &["--expect-app-revision", "bbb222"]);
    assert!(page().contains("application revision `aaa111`"));
    assert!(page().contains("This manual is for application revision bbb222"));
    assert!(page().contains("✅"), "the result is still the run's");
    // For the same build, and for a part the run did say.
    docs(
        &suite,
        &out,
        &[
            "--expect-app-revision",
            "aaa111",
            "--expect-fixture",
            "seed-7",
        ],
    );
    assert!(page().contains("This is what this manual is for."));
    // With nothing expected, it only says what the run said.
    docs(&suite, &out, &[]);
    assert!(page().contains("application revision `aaa111`"));
    assert!(!page().contains("This manual is for") && !page().contains("what this manual is for"));
}
