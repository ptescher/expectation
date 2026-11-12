//! `expectation-run --no-state-pictures`: the same verdicts, without the tests that only visit
//! a state for the state map's picture of it.

use expectation_remote::client::{Client, Handlers};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// The story flips the switch with the dimmer high, so the lamp on with the dimmer low is a
/// state one move from the start that no story passes through.
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
Story: Bright light
  When the user turns the dimmer
  Then the dimmer is high
  When the user flips the switch
  Then the lamp is on
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
fn a_run_can_leave_the_state_pictures_out() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("state-pictures-run");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("lamp.expect"), SUITE).unwrap();

    let full = run(&root, &[]);
    let quick = run(&root, &["--no-state-pictures", "--strict"]);
    assert_eq!(quick, ["✓ Bright light"]);
    assert_eq!(full[..quick.len()], quick[..]);
    assert!(full.len() > quick.len());
    assert!(
        full[quick.len()..]
            .iter()
            .all(|l| l.starts_with("✓ Visiting the state after «")),
        "{full:?}"
    );
}
