# Getting started

This chapter runs a complete suite against a tiny system: a coin-operated turnstile. It's the
[`turnstile` example](https://github.com/ptescher/expectation/blob/main/crates/harness/examples/turnstile.rs)
in the repository.

## Add the crates

Expectation isn't on crates.io yet. Depend on the git repository:

```toml
[dev-dependencies]
expectation_harness = { git = "https://github.com/ptescher/expectation" }
expectation_story = { git = "https://github.com/ptescher/expectation" }
```

For the command-line tool:

```bash
cargo install --git https://github.com/ptescher/expectation expectation_cli
```

That installs a binary called `expectation`.

## Write the suite

A suite is every `*.expect` file under a directory. Make `expectations/turnstile.expect`:

```gherkin
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
```

- The **vocabulary** declares the states the system can be in, and the events that can
  happen. Every other line must use these words.
- The **Background** is the state every test starts from.
- **Expectations** are the rules. `goes up by 1` is a relative Then: it checks the change,
  whatever the count was.
- The **story** is a journey through them.

## Check it

```bash
expectation check expectations
```

```text
warning[untested]: no story exercises «Pushing a locked turnstile does nothing»
  --> turnstile.expect:25
   |   Expectation: Pushing a locked turnstile does nothing
   = help: a story that would: When a visitor pushes the arm

0 errors, 1 warning
Turnstile: 3 expectations in 1 feature, 1 story (1 test); 0 exceptions; flow DAG 3 edges, 2 levels deep (search truncated)
```

Nothing ran, but the suite was checked: every step uses the vocabulary, no two expectations
disagree, and the story only relies on behaviour the expectations specify. The warning says
one expectation has no story, and how a story could reach it.

"Search truncated" is normal here: every coin makes a new state, so the search for what
leads to what stops at its depth limit instead of counting to 1000. It has already found
everything by then.

Try breaking it. Add a fourth expectation:

```gherkin
  Expectation: A hard push forces it
    Given the turnstile is locked
    When a visitor pushes the arm
    Then the turnstile is unlocked
```

`check` now fails. Both this and «Pushing a locked turnstile does nothing» apply when a
visitor pushes a locked turnstile, and they disagree about the turnstile. The error names
that situation and suggests settling it with `Overrides:` if one is an exception to the
other. Here neither is, so take the new one out again.

## Bind it to the system

The runner talks to the system through a **driver**. `Bindings` builds one from closures,
one per vocabulary term:

```rust
use expectation_harness::{Acceptance, Bindings, RunOptions};

#[derive(Default)]
struct Turnstile {
    locked: bool,
    coins: u32,
}

#[test]
fn turnstile_behaves() {
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

    Acceptance::load("expectations")
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage()
        .assert_passes(&mut driver, &RunOptions::default());
}
```

- `observe` reads a state back. Named values are strings; numbers are `f64`.
- `arrange` puts a state in place before a test starts (the Background, a scenario's Givens).
- `on` makes an event happen.
- `on_reset` runs before every test.

`with_coverage()` adds a generated test for each expectation no story reaches, so all three
are verified. `assert_passes` panics with every failing check if anything fails.

```bash
cargo test
```

## Write the docs

Pass a `docs_dir` to take screenshots (if the driver has `on_screenshot`), then write the manual:

```rust
let acceptance = Acceptance::load("expectations")?.with_coverage();
let opts = RunOptions { docs_dir: Some("docs".into()), ..Default::default() };
let report = acceptance.run(&mut driver, &opts);
acceptance.docs(Some(&report)).write("docs")?;
```

`docs/README.md` is the manual in Markdown, and `docs/site/index.html` is the same as a
website. The story is now a tutorial called "A day at the zoo".

## Next

The [worked example](kettle.md) is a richer system, with time, exceptions, several stories
and screenshots.
