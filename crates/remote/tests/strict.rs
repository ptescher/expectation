//! `expectation-run --strict`: a test the app can't run yet exits 0 as it always did, and 1
//! when the run is a gate.

use expectation_remote::client::{Client, Handlers};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const SUITE: &str = "
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

/// Run the suite with the real runner and `args`, against a lamp that has a dimmer or not.
/// Says how it exited and what it wrote to stderr.
fn run(suite: &Path, dimmer: bool, args: &[&str]) -> (Option<i32>, String) {
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
    let mut h: Handlers<(bool, bool)> = Handlers::new()
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
        h = h.on("the user turns the dimmer", |w| w.1 = true);
    }
    let client = Client::connect(&addr, h.hello("test", "Lamp"));
    std::thread::spawn(move || {
        let mut world = (false, false);
        while let Some(r) = client.recv_timeout(Duration::from_secs(20)) {
            h.handle(&mut world, r);
        }
    });
    let mut rest = String::new();
    said.read_to_string(&mut rest).unwrap();
    (runner.wait().unwrap().code(), rest)
}

#[test]
fn a_strict_run_exits_1_when_a_test_didnt_run() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("strict-run");
    let _ = std::fs::remove_dir_all(&root);
    let (suite, docs) = (root.join("suite"), root.join("docs"));
    std::fs::create_dir_all(&suite).unwrap();
    std::fs::write(suite.join("lamp.expect"), SUITE).unwrap();
    let docs = docs.to_str().unwrap();

    // Everything runs: an earlier run saves a pass for every test.
    for strict in [&[][..], &["--strict"], &["--strict=all"]] {
        let (code, said) = run(&suite, true, &[strict, &["--docs", docs]].concat());
        assert_eq!(code, Some(0), "{said}");
    }

    // The app can't turn the dimmer. As it always was, that is not a failure.
    let (code, said) = run(&suite, false, &["--docs", docs]);
    assert_eq!(code, Some(0), "{said}");
    assert!(!said.contains("--strict"), "{said}");
    // A gate says which test didn't run, and the passes saved before don't stand in for it.
    for strict in ["--strict", "--strict=required", "--strict=all"] {
        let (code, said) = run(&suite, false, &[strict, "--docs", docs]);
        assert_eq!(code, Some(1), "{said}");
        assert!(
            said.contains("– unsupported: Bright (The driver can't"),
            "{said}"
        );
        assert!(!said.contains("Light"), "{said}");
    }
    // A policy it doesn't know is a mistake in the command, before anything runs.
    let unknown = Command::new(env!("CARGO_BIN_EXE_expectation-run"))
        .arg(&suite)
        .arg("--strict=sometimes")
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
}
