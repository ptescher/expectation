//! A whole suite through the wire: the runner listens, an app embeds the Rust client on its own
//! thread, and every command and answer crosses HTTP.

use expectation_harness::{Acceptance, Control, RunOptions};
use expectation_remote::RemoteDriver;
use expectation_remote::client::{Client, Handlers, Reply};
use expectation_story::Status;
use std::time::Duration;

const SUITE: &str = "
Vocabulary:
  State: the turnstile is locked | unlocked
  State: the coin box has 0..1000 coins
  Event: a visitor inserts a coin
  Event: a visitor pushes the arm
Background:
  Given the turnstile is locked
  And the coin box has 0 coins
Feature: Paying
  Expectation: A coin unlocks it
    Given the turnstile is locked
    When a visitor inserts a coin
    Then the turnstile is unlocked
    And the coin box goes up by 1
  Expectation: Pushing through locks it again
    Given the turnstile is unlocked
    When a visitor pushes the arm
    Then the turnstile is locked
Story: A day at the zoo
  When a visitor inserts a coin
  Then the turnstile is unlocked
  When a visitor pushes the arm
  Then the turnstile is locked
";

#[derive(Default)]
struct Turnstile {
    unlocked: bool,
    coins: u32,
    used: Vec<(String, String)>,
}

fn handlers(broken: bool) -> Handlers<Turnstile> {
    Handlers::new()
        .observe("The turnstile", |t: &Turnstile| {
            if t.unlocked { "unlocked" } else { "locked" }
        })
        .observe("the coin box", |t: &Turnstile| t.coins as f64)
        .arrange("the turnstile", |t, v| {
            t.unlocked = v.as_str() == Some("unlocked")
        })
        .arrange("the coin box", |t, v| t.coins = v.as_f64().unwrap() as u32)
        .on("a visitor inserts a coin", move |t| {
            t.used.push(("Gate".into(), "coin-slot".into()));
            t.coins += 1;
            t.unlocked = !broken;
        })
        .on("a visitor pushes the arm", |t| {
            t.used.push(("Gate".into(), "arm".into()));
            t.unlocked = false
        })
        .on_controls(|_| {
            vec![
                Control::new("Gate", "coin-slot", "slot", "Coins")
                    .mapped_to("a visitor inserts a coin"),
                Control::new("Gate", "arm", "lever", "").mapped_to("a visitor pushes the arm"),
                Control::new("Gate", "donate", "button", "Donate"),
            ]
        })
        .on_used(|t| std::mem::take(&mut t.used))
        .on_reset(|t| *t = Turnstile::default())
        .on_screenshot(|t| Ok(Reply::Svg(format!("<svg><text>{}</text></svg>", t.coins))))
}

fn run(broken: bool) -> (expectation_story::Report, Acceptance) {
    let loaded = expectation_language::load_sources("t", &[("t.expect", SUITE)]);
    let acceptance = Acceptance::from_loaded(loaded).unwrap().with_coverage();
    let mut driver = RemoteDriver::listen("127.0.0.1:0")
        .unwrap()
        .settle_for(Duration::ZERO)
        .poll_every(Duration::from_millis(5));

    let h = handlers(broken);
    let client = Client::connect(
        driver.url().trim_start_matches("http://"),
        h.hello("test", "Turnstile"),
    );
    std::thread::spawn(move || {
        let mut world = Turnstile::default();
        while let Some(r) = client.recv_timeout(Duration::from_secs(30)) {
            h.handle(&mut world, r);
        }
    });

    let app = driver.wait_for_app(Duration::from_secs(5)).unwrap();
    assert_eq!((app.platform.as_str(), app.screenshot), ("test", true));
    let docs = std::env::temp_dir().join(format!(
        "expectation-remote-{}-{broken}",
        std::process::id()
    ));
    let report = acceptance.run(
        &mut driver,
        &RunOptions {
            docs_dir: Some(docs),
            patience: 10,
            ..Default::default()
        },
    );
    (report, acceptance)
}

#[test]
fn a_suite_runs_over_the_wire() {
    let (report, a) = run(false);
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    assert!(report.results.iter().all(|r| r.status == Status::Passed));
    assert!(
        report.results[0].steps[0]
            .screenshot
            .as_deref()
            .is_some_and(|s| s.ends_with(".svg"))
    );
}

#[test]
fn a_broken_app_fails_over_the_wire() {
    let (report, a) = run(true);
    assert!(!report.passed());
    assert!(
        report
            .failures(&a.suite, &a.plans)
            .contains("observed locked"),
        "{}",
        report.failures(&a.suite, &a.plans)
    );
}

#[test]
fn without_an_app_nothing_is_supported() {
    let loaded = expectation_language::load_sources("t", &[("t.expect", SUITE)]);
    let acceptance = Acceptance::from_loaded(loaded).unwrap();
    let mut driver = RemoteDriver::listen("127.0.0.1:0").unwrap();
    let report = acceptance.run(&mut driver, &RunOptions::default());
    assert!(
        report.passed()
            && report
                .results
                .iter()
                .all(|r| r.status == Status::Unsupported)
    );
}

#[test]
fn a_runner_for_one_platform_turns_others_away() {
    let driver = RemoteDriver::listen("127.0.0.1:0")
        .unwrap()
        .for_platform("iOS");
    let addr = driver.url().trim_start_matches("http://").to_string();
    let _other = Client::connect(&addr, handlers(false).hello("Android", "Turnstile"));
    assert!(driver.wait_for_app(Duration::from_millis(500)).is_err());
    let _ios = Client::connect(&addr, handlers(false).hello("ios", "Turnstile"));
    assert_eq!(
        driver
            .wait_for_app(Duration::from_secs(5))
            .unwrap()
            .platform,
        "ios"
    );
}

const SLOW: &str = "
Vocabulary:
  State: the turnstile is locked | unlocked
  Event: a visitor inserts a coin
Background:
  Given the turnstile is locked
Feature: Paying
  Expectation: A coin unlocks it
    Given the turnstile is locked
    When a visitor inserts a coin
    Then the turnstile is unlocked
Story: One coin
  Given the turnstile is locked
  When a visitor inserts a coin
  Then the turnstile is unlocked
";

fn slow_run(timeout: Duration, fail: bool) -> (expectation_story::Report, Acceptance) {
    let loaded = expectation_language::load_sources("t", &[("t.expect", SLOW)]);
    let acceptance = Acceptance::from_loaded(loaded).unwrap();
    let mut driver = RemoteDriver::listen("127.0.0.1:0")
        .unwrap()
        .settle_for(Duration::ZERO)
        .poll_every(Duration::from_millis(5))
        .command_timeout(timeout)
        .unwrap();
    let h = Handlers::new()
        .observe("the turnstile", |t: &Turnstile| {
            if t.unlocked { "unlocked" } else { "locked" }
        })
        .arrange("the turnstile", |t, v| {
            t.unlocked = v.as_str() == Some("unlocked")
        })
        .on("a visitor inserts a coin", move |t| {
            std::thread::sleep(Duration::from_millis(400));
            t.unlocked = !fail;
        })
        .on_reset(|t| *t = Turnstile::default());
    let client = Client::connect(
        driver.url().trim_start_matches("http://"),
        h.hello("test", "Turnstile"),
    );
    std::thread::spawn(move || {
        let mut world = Turnstile::default();
        while let Some(r) = client.recv_timeout(Duration::from_secs(30)) {
            h.handle(&mut world, r);
        }
    });
    driver.wait_for_app(Duration::from_secs(5)).unwrap();
    let report = acceptance.run(&mut driver, &RunOptions::default());
    (report, acceptance)
}

#[test]
fn a_slow_command_times_out_with_diagnostics() {
    let (report, a) = slow_run(Duration::from_millis(100), false);
    assert!(!report.passed());
    let failures = report.failures(&a.suite, &a.plans);
    assert!(
        failures.contains("perform command within the 100ms command timeout"),
        "{failures}"
    );
    assert!(!failures.contains("inserts a coin\""), "{failures}");
}

#[test]
fn a_longer_command_timeout_lets_it_finish_and_real_errors_through() {
    let (report, a) = slow_run(Duration::from_secs(5), false);
    assert!(report.passed(), "{}", report.failures(&a.suite, &a.plans));
    let (report, a) = slow_run(Duration::from_secs(5), true);
    let failures = report.failures(&a.suite, &a.plans);
    assert!(!report.passed());
    assert!(failures.contains("observed locked"), "{failures}");
}

#[test]
fn a_zero_command_timeout_is_refused() {
    let driver = RemoteDriver::listen("127.0.0.1:0").unwrap();
    assert!(driver.command_timeout(Duration::ZERO).is_err());
}

#[test]
fn controls_and_which_were_used_cross_the_wire() {
    let (report, acceptance) = run(false);
    assert!(report.passed());
    let rows =
        expectation_story::control_coverage(&acceptance.suite, &acceptance.plans, &report, &[]);
    let by = |id: &str| rows.iter().find(|r| r.control.id == id).unwrap();
    assert_eq!(rows.len(), 3);
    // Kind, label and the event mapping all came over the wire.
    assert_eq!(by("coin-slot").control.kind, "slot");
    assert_eq!(by("coin-slot").control.label, "Coins");
    assert_eq!(
        by("coin-slot").control.event.as_deref(),
        Some("a visitor inserts a coin")
    );
    assert!(by("coin-slot").used() && by("coin-slot").verified());
    assert!(by("arm").used());
    // Nothing used the donate button, and nothing says what it does.
    let donate = by("donate");
    assert!(!donate.used());
    assert_eq!(donate.gaps[0], expectation_story::Gap::NoEventMapping);
    assert!(!by("coin-slot").stories.is_empty());
}
