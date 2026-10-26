//! A whole suite through the wire: the runner listens, an app embeds the Rust client on its own
//! thread, and every command and answer crosses HTTP.

use expectation_harness::{Acceptance, RunOptions};
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
            t.coins += 1;
            t.unlocked = !broken;
        })
        .on("a visitor pushes the arm", |t| t.unlocked = false)
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
