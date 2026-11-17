# Expectation

A spec for how a system behaves, written so a tool can check it, run it and publish it.

You write **expectations** (given / when / then) in a **vocabulary** of declared states and events. Because every step is a fact about a declared state or a declared event, the suite is a model rather than a script, and the tool can work with it:

- `expectation check` finds expectations that conflict, behaviour nothing can reach, and stories that rely on behaviour nobody specified.
- `expectation plan` compiles user stories into acceptance tests. The expectations a story passes through supply its checks, and generated tests cover whatever no story reaches.
- A **driver** binds the vocabulary to the real system (a library, an API, a browser, a device, an app on three platforms) and the runner executes the plans against it.
- `expectation docs` writes the manual from the same suite and the run's evidence.

## What it is for

**Spec-driven development.** The spec is a checked artifact, not a document that drifts. Behaviour can be written before it is built (`Status: pending`), and what is agreed is marked (`Approval:`), so a suite can say what is decided, what is built and what is proven. Contradictions surface while you write the spec, not while you debug the code.

**Keeping agents on task.** An agent working from prose can invent behaviour and declare itself done. Here, a step that names no declared state or event is an error, a story can't rely on behaviour the spec doesn't give, and `check`, `plan --coverage` and the run results are commands with exit codes and per-test verdicts. "Not supported yet" is reported apart from "failed", so a run that bound nothing doesn't read as a pass. The guide and skills are plain text for agents ([llms.txt](https://expectation.sim001.net/llms.txt)).

**Deterministic results.** The tests are derived from the model by an algorithm, not written or sampled at run time: the same suite gives the same plans, in the same order, with the same checks. A test that starts from where the last one ended does so only after observing that the system really is in the state it needs; otherwise it resets. Each run is saved with an id, per-step timing, the evidence it saw (screenshots, HTTP requests with secrets redacted) and a verdict, so the manual can be rebuilt from saved results.

There are no dependencies outside the standard library. Docs: [the guide](https://expectation.sim001.net/guide/), [an example manual](https://expectation.sim001.net/example/site/index.html), [the API](https://expectation.sim001.net/api/expectation_harness/index.html), at [expectation.sim001.net](https://expectation.sim001.net). For agents: [llms.txt](https://expectation.sim001.net/llms.txt), [llms-full.txt](https://expectation.sim001.net/llms-full.txt) and [skill.md](https://expectation.sim001.net/skill.md).

```
.expect files ─language─▶ Suite ─graph─▶ conflicts · override DAG · flow DAG
                            │                         │
                            └────────story────────────┴─▶ plans (acceptance tests)
                                                             │
                                          harness + driver ──┴─▶ report + evidence ─docs─▶ HTML + Markdown
```

## A taste

```gherkin
Vocabulary:
  State: the kettle is off | heating | dry
  State: the dial is 0..100 %
  State: the child lock is on | off
  Event: the user presses lock
  Event: the water runs out

Background:
  Given the kettle is off
  And the dial is 0 %
  And the child lock is off

Feature: Heating
  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

  Expectation: Lock turns the child lock on
    Given the child lock is off
    When the user presses lock
    Then the child lock is on

  Expectation: The child lock holds the kettle
    Overrides: Turning the dial up starts the kettle
    Given the kettle is off
    And the child lock is on
    When the dial is above 55 %
    Then the kettle is off

  Expectation: Running dry stops the kettle
    Given the kettle is heating
    When the water runs out
    Then the kettle is dry

Story: First cup
  As a tea drinker
  I want to start the kettle by turning the dial
  So that it works like the one I already have

  When the dial is 80 %
  Then the kettle is heating
```

Without the `Overrides:` line, `expectation check` reports that both expectations apply
"when the kettle is off and the child lock is on, and the dial is above 55 %", and suggests
the more specific one as the exception. The story never says what happens to the child lock
or the dial; the expectations do, so the generated test checks them anyway.

## Getting started

Add the harness to your project's dev-dependencies:

```toml
[dev-dependencies]
expectation_harness = { git = "https://github.com/ptescher/expectation" }
expectation_story = { git = "https://github.com/ptescher/expectation" }
```

Write a suite as `*.expect` files in a directory (say `expectations/`), bind its vocabulary to
your system, and run it from a test:

```rust
use expectation_harness::{Acceptance, Bindings, Progress, RunOptions, Screenshot};

#[test]
fn behaves_as_expected() {
    let mut driver = Bindings::new(Kettle::default())
        .observe("the kettle", |k| k.phase_name())
        .observe("the dial", |k| k.dial)
        .set("the dial", |k, v| k.dial = v.as_f64().unwrap())
        .arrange("the kettle", |k, v| k.force(v.as_str().unwrap()))
        .on("the user presses reset", Kettle::reset)
        .on("the water runs out", Kettle::run_out)
        .on_settle(|k| k.run_for(1.0))                          // after each action
        .on_advance(|k| { k.run_for(0.1); Progress::Advanced }) // between looks
        .on_screenshot(|k| Screenshot::svg(k.svg()));

    Acceptance::load("expectations")
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage()
        .assert_passes(&mut driver, &RunOptions { docs_dir: Some("docs/behaviour".into()), ..Default::default() });
}
```

`assert_passes` fails the test with every failing check, and writes screenshots under
`docs_dir`. `acceptance.docs(Some(&report)).write(dir)` writes the manual.

## Examples

| Example | What it shows |
| --- | --- |
| [turnstile](crates/harness/examples/turnstile.rs) | The smallest whole run: an inline suite, closures for the driver, one story and a generated test. |
| [kettle](crates/harness/examples/kettle) | A suite across several files: two exceptions, four stories, three generated tests, a toy world with time in it, SVG screenshots and the generated manual. |
| [invoices](crates/harness/examples/invoices) | An API's manual: the driver calls a stand-in invoices API over HTTP and hands over each request, and the manual shows them as `curl` commands with their responses, where a screenshot would be. Admin and member API keys, and an exception only a member's request meets. |
| [quote](crates/harness/examples/quote) | Contract coverage end to end: the driver hands over each step's HTTP requests as HAR, [the HAR and OpenAPI extension](extensions/har-openapi) holds them against the contract, and the manual shows what the tests saw. |

```bash
cargo run -p expectation_harness --example turnstile
cargo run -p expectation_harness --example kettle                      # docs in target/expectation-docs/kettle
cargo run -p expectation_harness --example kettle -- out --bug         # a seeded defect, to see a failure
cargo run -p expectation_harness --example invoices                    # an API's manual, in target/expectation-docs/invoices
cargo run -p expectation_cli -- check crates/harness/examples/kettle   # check it without running anything
cargo test --workspace                                                 # the examples, as tests too
git config core.hooksPath .githooks                                    # check fmt and clippy before each commit
```

In the kettle, the dial has to be held above 55 % for 0.6 s before the kettle starts, and the
child lock keeps the dial and buttons from reaching it. `--bug` lets the dial through the
lock, and the run fails on «The child lock holds the kettle while the dial turns».

## Any front end

For apps that can't be driven from a Rust test, the runner talks over a small protocol to a
client that is either embedded in the app or runs in a UI-automation test process (WebdriverIO,
Appium, XCUITest) and drives the app from outside: [Swift](clients/swift) (iOS, SwiftUI, UIKit),
[JavaScript](clients/js) (React Native, web) and [Rust](crates/remote) (GPUI and other Rust
UIs). The [kettle demo](demos/kettle) is one suite run against the same app built in GPUI,
SwiftUI and React Native.

## Crates

| Crate | Path | Role |
| --- | --- | --- |
| `expectation_model` | [crates/model](crates/model) | Vocabulary, the condition algebra, expectations and stories as plain data |
| `expectation_language` | [crates/language](crates/language) | The `.expect` format: parsing, and resolving steps against the vocabulary |
| `expectation_graph` | [crates/graph](crates/graph) | Conflicts, override precedence, the flow DAG, Mermaid and DOT export |
| `expectation_story` | [crates/story](crates/story) | Stories → acceptance-test plans, coverage, results and verdicts |
| `expectation_docs` | [crates/docs](crates/docs) | The manual, as an HTML site and Markdown, from all of the above |
| `expectation_theme` | [crates/theme](crates/theme) | The colours, type and shapes the docs and graphs are drawn in |
| `expectation_service` | [crates/service](crates/service) | The external things a run needs (a server, a database, a container): started once, stopped after, and "not supported yet" with how to get them when they can't be had |
| `expectation_harness` | [crates/harness](crates/harness) | The `Driver` trait, closure `Bindings`, the runner, and the `Acceptance` pipeline |
| `expectation_remote` | [crates/remote](crates/remote) | Driving an app through a driver embedded in it (Swift, JavaScript and Rust clients), and `expectation-run` |
| `expectation_cli` | [crates/cli](crates/cli) | The `expectation` command: `check`, `graph`, `states`, `plan`, `docs`, `screenshots` |


Everything else (the language, platforms, conflict rules, drivers, services, the manual, the command's options) is in [the guide](https://expectation.sim001.net/guide/).

## The `expectation` command

```bash
cargo run -p expectation_cli -- check <dir>                # diagnostics; exits 1 on errors
cargo run -p expectation_cli -- graph <dir> [--dot]        # the flow DAG as Mermaid or DOT
cargo run -p expectation_cli -- states <dir> [--by <states>] [--dot]  # the app's states and the moves between them
cargo run -p expectation_cli -- plan  <dir> [--coverage]   # the acceptance tests, step by step
cargo run -p expectation_cli -- docs  <dir> <out-dir>      # docs from the results saved in <out-dir>/results
```

[The command's chapter](https://expectation.sim001.net/guide/cli.html) has every option, and
the `screenshots` commands.

Or install it: `cargo install --git https://github.com/ptescher/expectation expectation_cli`.

## License

MIT. See [LICENSE](LICENSE).
