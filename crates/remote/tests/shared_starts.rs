//! `expectation-run` starts a test where the last one ended when what it is given holds there,
//! says so, and resets before every test with `--no-shared-starts`.

use expectation_remote::client::{Client, Handlers};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// «Bright light» ends with the lamp on and the dimmer high, where «Dim again» starts.
const SUITE: &str = "
Vocabulary:
  State: the lamp is off | on
  State: the dimmer is low | high
  Event: the user flips the switch
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
  Expectation: Turning the dimmer brightens it
    Given the dimmer is low
    When the user turns the dimmer
    Then the dimmer is high
  Expectation: Turning it back dims it
    Given the dimmer is high
    When the user turns it back
    Then the dimmer is low
Story: Bright light
  When the user turns the dimmer
  Then the dimmer is high
  When the user flips the switch
  Then the lamp is on
Story: Dim again
  Given the lamp is on
  And the dimmer is high
  When the user turns it back
  Then the dimmer is low
";

/// The lines the runner prints, one per test.
fn run(suite: &Path, args: &[&str]) -> Vec<String> {
    let mut runner = Command::new(env!("CARGO_BIN_EXE_expectation-run"))
        .arg(suite)
        .args(["--addr", "127.0.0.1:0", "--settle", "0", "--wait", "20"])
        .args(args)
        .stdout(Stdio::piped())
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
    let h: Handlers<(bool, bool)> = Handlers::new()
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
        .on("the user turns the dimmer", |w| w.1 = true)
        .on("the user turns it back", |w| w.1 = false)
        .on_reset(|w| *w = (false, false));
    let client = Client::connect(&addr, h.hello("test", "Lamp"));
    std::thread::spawn(move || {
        let mut world = (false, false);
        while let Some(r) = client.recv_timeout(Duration::from_secs(20)) {
            h.handle(&mut world, r);
        }
    });
    let mut out = String::new();
    runner
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut out)
        .unwrap();
    assert!(runner.wait().unwrap().success(), "{line}");
    out.lines().map(String::from).collect()
}

#[test]
fn a_run_says_which_tests_continued_and_can_reset_before_each() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("shared-starts-run");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("lamp.expect"), SUITE).unwrap();

    let shared = run(&root, &["--no-state-pictures", "--strict"]);
    assert_eq!(shared, ["✓ Bright light", "✓ Dim again (continued)"]);
    let each = run(
        &root,
        &["--no-state-pictures", "--strict", "--no-shared-starts"],
    );
    assert_eq!(each, ["✓ Bright light", "✓ Dim again"]);
}
