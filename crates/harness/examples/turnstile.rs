//! The smallest whole run: a coin-operated turnstile, its suite inline, bound with closures.
//!
//! ```text
//! cargo run -p expectation_harness --example turnstile
//! ```

use expectation_harness::{Acceptance, Bindings, RunOptions};
use expectation_story::Status;
use std::process::exit;

const SUITE: &str = "
Suite: Turnstile

Vocabulary:
  State: the turnstile is locked | unlocked
  State: the coin box has 0..1000 coins
  Event: a visitor inserts a coin
  Event: a visitor pushes the arm

Background:
  Given the turnstile is locked
  And the coin box has 0 coins

Feature: Paying to go through
  Expectation: A coin unlocks it
    Given the turnstile is locked
    When a visitor inserts a coin
    Then the turnstile is unlocked
    And the coin box goes up by 1

  Expectation: Pushing through locks it again
    Given the turnstile is unlocked
    When a visitor pushes the arm
    Then the turnstile is locked

  Expectation: Pushing a locked turnstile does nothing
    Given the turnstile is locked
    When a visitor pushes the arm
    Then the turnstile is locked

Story: A day at the zoo
  As a visitor
  I want to pay once and go through
  So that I can see the animals

  When a visitor inserts a coin
  Then the turnstile is unlocked
  When a visitor pushes the arm
  Then the turnstile is locked
";

#[derive(Default)]
struct Turnstile {
    locked: bool,
    coins: u32,
}

fn main() {
    let loaded = expectation_language::load_sources("turnstile", &[("turnstile.expect", SUITE)]);
    let acceptance = match Acceptance::from_loaded(loaded) {
        Ok(a) => a.with_coverage(),
        Err(e) => {
            eprint!("{e}");
            exit(1);
        }
    };

    let mut driver = Bindings::new(Turnstile { locked: true, coins: 0 })
        .observe("the turnstile", |t| if t.locked { "locked" } else { "unlocked" })
        .observe("the coin box", |t| t.coins as f64)
        .arrange("the turnstile", |t, v| t.locked = v.as_str() == Some("locked"))
        .arrange("the coin box", |t, v| t.coins = v.as_f64().unwrap() as u32)
        .on("a visitor inserts a coin", |t| {
            t.coins += 1;
            t.locked = false;
        })
        .on("a visitor pushes the arm", |t| t.locked = true)
        .on_reset(|t| *t = Turnstile { locked: true, coins: 0 });

    let report = acceptance.run(&mut driver, &RunOptions::default());
    for (plan, r) in acceptance.plans.iter().zip(&report.results) {
        let mark = if r.status == Status::Passed { "✓" } else { "✗" };
        println!("{mark} {}", plan.title);
    }
    if !report.passed() {
        eprint!("\n{}", report.failures(&acceptance.suite, &acceptance.plans));
        exit(1);
    }
}
