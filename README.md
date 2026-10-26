# Expectation

Acceptance testing where the behaviour itself is the source of truth.

Features are written as **expectations**: given some state, when something happens, then
the system ends up in some state. Every step uses a shared **vocabulary** of states and
events, so expectations can be compared mechanically. Expectation uses that to:

1. find expectations that **conflict** (both apply, and they promise incompatible things),
   and let you mark one as an exception to the other with `Overrides:`. Overrides must form
   a DAG.
2. arrange everything into a **flow DAG** from the starting state. That shows what leads to
   what and gives each expectation a shortest route from the start.
3. compile **user stories** into **acceptance tests**. A story only describes the journey;
   at each step the expectations it passes through supply the checks. A story that relies on
   unspecified behaviour, or contradicts an expectation, is rejected before anything runs.
4. **run** those tests against the system under test, capturing a **screenshot** at every
   step.
5. write **documentation** from the stories, results and screenshots: a manual, where
   stories are step-by-step tutorials and features are topic pages, with the test output
   tucked away in "How this is tested" blocks and a test report.

```
.expect files ─language─▶ Suite ─graph─▶ conflicts · override DAG · flow DAG
                            │                         │
                            └────────story────────────┴─▶ plans (acceptance tests)
                                                             │
                                          harness + driver ──┴─▶ report + screenshots ─docs─▶ HTML + Markdown
```

There are no dependencies outside the standard library.

**Docs:** [the guide](https://expectation.sim001.net/guide/), [an example manual](https://expectation.sim001.net/example/site/index.html) and [the API](https://expectation.sim001.net/api/expectation_harness/index.html), at [expectation.sim001.net](https://expectation.sim001.net).

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
| [kettle](crates/harness/examples/kettle) | A suite across several files: two exceptions, three stories, four generated tests, a toy world with time in it, SVG screenshots and the generated manual. |

```bash
cargo run -p expectation_harness --example turnstile
cargo run -p expectation_harness --example kettle                      # docs in target/expectation-docs/kettle
cargo run -p expectation_harness --example kettle -- out --bug         # a seeded defect, to see a failure
cargo run -p expectation_cli -- check crates/harness/examples/kettle   # check it without running anything
cargo test --workspace                                                 # the examples, as tests too
git config core.hooksPath .githooks                                    # check fmt and clippy before each commit
```

In the kettle, the dial has to be held above 55 % for 0.6 s before the kettle starts, and the
child lock keeps the dial and buttons from reaching it. `--bug` lets the dial through the
lock, and the run fails on «The child lock holds the kettle while the dial turns».

## Any front end

For apps that can't be driven from a Rust test, a driver goes inside the app and the runner
talks to it over a small protocol: [Swift](clients/swift) (iOS, SwiftUI, UIKit),
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
| `expectation_cli` | [crates/cli](crates/cli) | The `expectation` command: `check`, `graph`, `plan`, `docs` |

## The language

A suite is every `*.expect` file under a directory. The format is line oriented like
Gherkin. A line's first word decides what it is, a line starting with `#` is a comment, and
indentation is only for people.

```gherkin
Vocabulary:
  State: the kettle is off | starting | heating | dry
  State: the dial is 0..100 %
  State: the preset is WARM | HOT | BOIL
    # How the docs say to change it:
    Set: press PRESET until the display reads {value}
    # What the reader sees:
    Shows: the display reads {value}
  Event: the user starts the kettle
    # How the docs say to do it:
    Say: press START

  # A setup, in one line:
  Phrase: the kettle is plugged in and full
    Say: fill the kettle and plug it in
    Given the kettle is off
    And the dial is 0 %
    And the preset is WARM

  Phrase: the user starts the kettle with the dial
    Say: turn the dial past 55 % and hold it there
    When the dial is 80 %

Background:
  Given the kettle is plugged in and full

Feature: Heating
  # Free text before the first step is a description:
  The kettle starts off.

  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

  Expectation: START starts the kettle
    Given the kettle is off
    When the user starts the kettle
    Then the kettle is heating

Story: Your first cup
  As a tea drinker
  I want to start the kettle by turning the dial
  So that it works like the one I already have

  # The tutorial's introduction:
  Every kettle in the range has the same dial.

  Scenario: Dial up
    When the user starts the kettle with the dial
    Then the kettle is heating
```

**Suite and sections.** An optional `Suite: <name>` line names the suite; otherwise it's
named after its directory. Each directory under the suite is a **section**, a nested set of
features that share a setup. Its `Section: <name>` line (in any of its files) and the text
under it name and describe it. A leading `01-` in a directory name only orders it. A
section's `Background:` is its setup, written over its parent's, so everything in
`app/drawing/` starts where `app/` does with the pen picked on top. Pages about a section
start from that setup and don't re-explain how to get there, the way a manual assumes you've
read the chapter before.

**Vocabulary.** `State: <subject> is <a> | <b> | …` declares a state with named values.
`State: <subject> is <min>..<max> [unit]` declares a number. Either can use `are`, `has` or
`have` instead of `is` ("the drawing has 0..650 shapes"), and steps about it read the same
way. `Event: <phrase>` declares something that happens. Vocabularies from all files merge
into one.

Name events for what the user means, not the button they press: "the user starts the
kettle", not "the user presses START". The button goes in a `Say:` line under the event,
which is what the docs tell the reader to do ("Press START."). A `Set:` line under a state
says how to change it, with `{value}` for the value. A state can also have a `Set:` for one
value, as `Shows:` lines do: `Set: lost: switch off the radio`. Without them, the docs make do: "the
user clicks the canvas" becomes "Click the canvas", and a state becomes "Set the dial to
80 %".

A `Shows:` line under a state says what the reader sees when it has a value, in the screen's
own words: `Shows: dry: the display says DRY`. Start the line with the value and a colon (for
a number, a value or comparison as steps write it: `at least 99 %:`), or put `{value}` in it
for any exact value (`Shows: the counter reads {value} SHAPES`). The docs use it for every
outcome, so "Then the kettle is dry" reads "The display says DRY".

`Phrase: <name>` is a step of your own, made of other steps, for saying more with less. A
phrase of `Given`s is a setup ("the kettle is plugged in and full"), usable in a Background,
a Given or a scenario's start. A phrase of `When`s is a sequence of actions ("the user draws
two shapes", one `When` per action); an expectation can use one that's a single action, and
a story step can use any. A phrase's `Say:` is how the docs put it. Phrases can use the
phrases declared before them.

**Steps.** A step is either an event or a fact `<subject> is [not] <predicate>`. The
predicate is one or more values (`off or heating`) or, for numbers, a comparison: `80 %`,
`above 55 %`, `below 10`, `at least 20`, `at most 40`, `between 20 and 40 %`. Matching
ignores case, extra spaces and a leading "the", and "1 shape" matches a unit of shapes.
Anything outside the vocabulary is an error, with a "did you mean" hint. An expectation's
Then can also be relative: `the drawing goes up by 1`, `goes down by 10 m`, `stays the same`.

**Glossary and links.** `Term: <name>` and the text under it define a word for the
glossary. Any text can link to a term, a section, a feature, an expectation or a story with
`[[its name]]`, and mark code with backticks. A `Docs: hidden` line in an expectation keeps
it off the topic page; it's still tested, and it's in the test report. A `Reference: <label> |
<url>` line in a term links it to outside documentation (an `http` or `https` URL), which the
glossary shows under the definition.

**Expectations** read `Given …` (the state before), `When …` (at most one event, plus any
states the outside world changes, such as the dial) and `Then …` (the state after). States
the Then doesn't mention keep their values. An expectation without a `When` is an
**invariant**: it holds whenever its Given does. `Overrides: <name>` makes it an exception
to another expectation: where both apply, this one wins.

**Stories** have a narrative (the "As a …", "I want …", "So that …" lines), an introduction
(any other text, which opens the tutorial in the docs) and one or more `Scenario:`s. Steps
written straight under the story form a single scenario. A scenario may start with `Given`s,
which are arranged on top of the Background. After that, each `When` starts a step,
`And`/`But` add to it, and its `Then`s are what the story checks afterwards. A step that's a
phrase of several actions becomes several test steps but stays one numbered step in the docs.
Each top-level section is a system of its own: a story only goes through its own section's
expectations.

## Platforms

One suite can describe a system that exists on several platforms (a desktop app and a phone
app that do the same job). Everything written plainly is true on every platform. Where one
differs, the suite says so, and each platform reads the suite as its own:

```gherkin
Vocabulary:
  # The first is the default: plain lines are about it.
  Platform: desktop | iOS

  Event: the user unlocks the door
    Say: press UNLOCK
    Say[iOS]: tap UNLOCK          # how the docs tell an iOS reader to do it
  State: the door is locked | unlocked
    Shows[iOS]: unlocked: the UNLOCKED badge turns green   # replaces the plain Shows lines

  Phrase: the app is open on a new account
    Given the tool is pan
  Phrase[iOS]: the app is open on a new account
    Given the user is signed in   # a start that iOS can reach

Section: Drawing
  On: desktop                     # also under a Feature, Expectation or Story
```

- `Say[P]:`, `Set[P]:` and `Shows[P]:` replace the plain line for platform `P`.
- `Phrase[P]:` replaces the plain phrase of the same name for `P`.
- `On: desktop, iOS` says the section (the whole directory), feature, expectation or story
  is only about those platforms. Use it for what a platform does not have. An exception to
  an expectation that is not on a platform is dropped there.
- Tag only the platforms that differ from the default. A name that is not declared is an error.

`expectation check` checks every platform. A driver reads the suite for its platform
(`Acceptance::load_for(dir, "iOS")`) and runs its tests; its results are its own
(`results/app@iOS.tsv`), apart from those of runners that are the same everywhere
(`api.tsv`). `expectation docs` writes one manual per platform, the default at the root and
the others under `platforms/<name>/`, and every HTML page has a Platform dropdown that opens
the same page in the other manual.

## How expectations are compared

A condition (a Given, a Then, a trigger) stands for a set of states. Everything below reduces
to three questions about those sets: do two overlap, does one imply the other, and what is
left after an action overwrites some states.

- **Outcome conflict.** Two expectations can be set off by the same action, their Givens
  overlap, and their Thens contradict. In the overlap the system can't do both. The error
  names the overlap ("both apply when the kettle is off and the child lock is on, and the
  dial is above 55 %") and suggests which is the exception: the more specific one.
- **Invariant conflict.** An expectation leaves the system in a state where an invariant's
  Given holds but its Then can't, or the Background does.
- **Exceptions.** An `Overrides:` between the two, directly or through a chain, settles a
  conflict. Overrides must form a DAG, so a cycle is an error. An override that settles
  nothing gets a warning. Settled conflicts are listed in the docs as the suite's exceptions.
- **Relative Thens.** Two different relative Thens ("goes up by 1" and "goes up by 2") always
  disagree. A relative one and an absolute one disagree unless the shared situation pins down
  the one value where they meet.
- **Flow DAG.** Starting from each section's setup, and from each scenario's arranged start, a
  breadth-first search applies every transition that can fire. It only uses expectations from
  the same top-level section. An expectation's *level* is the fewest steps before it applies.
  Its parents are the expectations that take the last step on those shortest routes. Levels
  only increase along edges, so the result is a DAG even though the behaviour loops (start,
  run dry, reset, start again). An expectation the search never reaches gets a warning,
  because no story can get there; only a generated test that arranges its Given can test it.
- **Ways into a section.** For each section, a goal-directed search finds the chain of
  expectations that gets from its parent's setup to its own: "press PEN", or "sign in, open
  a project, press PEN" for something deeper. That chain, with any step it works without
  dropped, is the "Before you begin" of the section's page. If there's none, it's a warning,
  because then nothing explains how to get there.

## From stories to acceptance tests

Planning walks each scenario through the expectations:

- An expectation **fires** when the current state implies its Given and the action implies
  its trigger. In a compound action ("When the dial is 0 % and the user presses reset") the
  other changes happen first. When both an exception and the expectation it overrides fire,
  only the exception's Then applies.
- Each step's checks are the Thens of what fired (and of invariants now in force), plus the
  story's own Thens. The story never has to restate what the expectations already promise.
- It's an **error** if a step *might* set off an expectation but the story doesn't pin it
  down (`unclear-step`), if an event happens and nothing is expected (`unspecified`), or if
  a story's Then contradicts the expectations (`contradicts`). A Then that no expectation
  promises is a warning (`unpromised`), and it's checked anyway.
- Expectations no story exercises get a warning with a route that would reach them.
  `Acceptance::with_coverage()` and `expectation plan --coverage` add generated tests that
  follow those routes, starting from the expectation's own section setup. An expectation with
  no route is tested on its own: its Given arranged on the section setup, then its When.
  They also add a test for each state of the state map a move from a start that no other
  test passes through ("Visiting the state after «…»"), so the map's first view has a
  screenshot of every state.
- A test starts from its section's setup, with its scenario's Givens written over it.

## Running against the system under test

The system plugs in through the `Driver` trait, or through `Bindings` for one closure per
vocabulary term (see [Getting started](#getting-started)).

For each test, the runner resets the driver and arranges the start (a value inside each
constraint: the first allowed name, or the middle of a range). Then for each step it acts,
lets the system **settle**, and observes until every check holds at once or its patience
runs out. Settling is what makes "Then the kettle is off" mean that it stays off, not just
that it was off an instant before it switched on. Polling is what lets "Then the kettle is
heating" wait for it to warm up. Steps after a failure are skipped. Before a test starts,
the runner checks that the driver can do everything the plan needs. If it can't, the test is
**not supported yet**. That isn't a failure: the system, or its driver, just doesn't do it
yet, and its steps get a placeholder instead of a screenshot.

`Acceptance::publish(dir, runner, &report)` saves a run's results as
`dir/results/<runner>.tsv`, merges them with the results other runners saved there (say, an
API runner and a UI runner that each run their own part), and writes the docs from the merged
report. `expectation docs` rebuilds the docs from saved results without running anything.

## External services

Tests sometimes need something that is not part of the system under test: a database, a
server, a container. The harness treats that as a **service**, so no test, driver or script
has to start it:

```rust
let mut services = Services::new()
    .with(Command::new("server", "./target/debug/server").ready(Ready::Tcp(8080)))
    .with(Docker::new("database", "postgres:16").port(5432, 55432)   // a database is a container
        .env("POSTGRES_PASSWORD", "test").ready(Ready::Tcp(55432)));
let mut driver = Bindings::new(world).needs(&["server", "database"]).fresh(&["database"]);
let report = acceptance.run_with(&mut driver, &RunOptions::default(), &mut services);
```

- **Needs.** A driver says what it needs (`Driver::needs`). Before a plan the runner makes sure
  each is up: one that is up stays, one that died is started again. All stop when `services`
  is dropped, the last started first.
- **Fresh.** What a test leaves behind (rows the last one wrote) is cleared by naming the
  service in `Driver::fresh`: it is started afresh before each test that runs.
- **Not supported yet, not failed.** A service that can't run here (no Docker, no build) makes
  its plans "not supported yet", and the reason says how to get it. Plans the driver can't run
  anyway start nothing.
- **Providers.** `Command` (a program, ready when a port answers), `Docker` (a container, its
  image built from a directory when it isn't there, Docker Desktop woken on a Mac), `Present`
  (a build or a file that must be there), and `Fallback` (the first of several ways that is
  available: a native build, else a container). A port that something else already holds is an
  error, not a ready service: what answers there is a leftover of an earlier run.

## Docs

`Docs::write` produces a manual, as an HTML site in `site/` and as Markdown that GitHub
renders. Its navigation follows the sections. It reads like a project's user docs, not a
test report:

- **Tutorials**, one per story scenario: the story's introduction, **Before you begin** (the
  setup, in the specs' own words), then numbered steps. Each says what to do (from the
  `Say:`s and `Set:`s), what you'll see (the story's Thens, or what the expectations promise)
  and the screenshot the test took there.
- **Topic pages**, one per feature: its description, then each expectation as a sentence
  ("If the kettle is off, press START. The light comes on and the water starts to heat."),
  its exceptions as notes, a screenshot and the tutorials that show it.
- **Section pages**: the description, Before you begin (the setup, or the steps that get
  there from the parent section), the topics in it and its tutorials as cards.
- **Reference**: the glossary of `Term:`s, the DAG map, the theme, and the **test report**:
  every test and its result, a page per test with each step, check and screenshot, and the
  test vocabulary (each state, event and phrase, and how the docs word it).

The test details (each expectation's Gherkin, what it follows and leads to, its verdict and
the tests that cover it) are on every page too, collapsed under "How this is tested".
Generated coverage tests only appear in the test report. A step without a screenshot shows a
"Not supported yet" or "No screenshot" placeholder.

The Markdown has the same pages: `README.md` (the overview), `tutorials/`, `features/`,
`glossary.md`, `tests.md` (the report, with a Mermaid map of the flow DAG), `tests/` and
`vocabulary.md`. Pages read in order: each section, then its tutorials, its topics and its
subsections. Each page's footer names the spec file to edit to change it.

**The outline.** A suite can have an `outline.txt` beside its sections. It groups the
top-level sections into **parts** (the navigation's labels and the README's contents), sets
the order the reader meets them in (and so the previous and next links), and places
**hand-written pages** among them: Markdown files in `manual/` (install, reference,
troubleshooting) with `[[links]]` into the generated pages and `[desktop]` to keep a page out
of another platform's manual. The sections themselves stay where they are, because their
setups inherit from their parents. A top-level section no part names goes into a last part,
"More", with a warning. Without an `outline.txt` the manual follows the sections.

## Writing specs that read well

The docs are only as good as the words in the specs. Some habits that help:

- Start with high-level expectations in the user's words, and let `expectation check` and
  `expectation graph` tell you what conflicts and what can't be reached.
- Write stories as tutorials: one goal each, in the order a new user would meet things.
- Give events and states `Say:`, `Set:` and `Shows:` lines, so the docs say what to press and
  what appears instead of restating the spec.
- Keep reader-facing text short and plain. Simplified Technical English (ASD-STE100) is a
  good target: one instruction per sentence, active voice, common words.
- Generate the docs and read them as a newcomer would, then fix the specs, not the output.

`expectation check` warns when the pages would be badly sized (the limits are constants in
[`sizing.rs`](crates/language/src/sizing.rs)):

| Warning | When |
| --- | --- |
| `many-scenarios` | A story has more than one scenario. Each scenario is a tutorial page of its own. |
| `short-story` | A scenario has fewer than 3 numbered steps (each `When` starts one). |
| `long-story` | A scenario has more than 8 numbered steps. |
| `big-feature` | A feature has more than 8 expectations on its topic page (`Docs: hidden` ones don't count). |
| `small-feature` | A feature has 1 expectation on its page, and its section has other features. |
| `crowded-section` | A section (not counting its subsections) has more than 8 tutorials or more than 8 topics. |
| `no-tutorial` | A section has 2 or more features and no story. |

## The `expectation` command

```bash
cargo run -p expectation_cli -- check <dir>                # diagnostics; exits 1 on errors
cargo run -p expectation_cli -- graph <dir> [--dot]        # the flow DAG as Mermaid or DOT
cargo run -p expectation_cli -- states <dir> [--by <states>] [--dot]  # the app's states and the moves between them
cargo run -p expectation_cli -- plan  <dir> [--coverage]   # the acceptance tests, step by step
cargo run -p expectation_cli -- docs  <dir> <out-dir>      # docs from the results saved in <out-dir>/results
```

Or install it: `cargo install --git https://github.com/ptescher/expectation expectation_cli`.

## License

MIT. See [LICENSE](LICENSE).
