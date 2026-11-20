---
name: expectation
description: Work with Expectation, an acceptance-testing tool where .expect files are a checked model of a system. Use when a project has *.expect files or asks for acceptance tests, user stories, a generated user manual, or feature specs written as given/when/then expectations.
---

# Expectation

Expectation turns a suite of `*.expect` files into: conflict and reachability checks, acceptance
tests compiled from user stories, and a user manual (Markdown and HTML) with screenshots.
Every step is a declared **state** or **event**, so the suite is a model, not a script.
Docs: https://expectation.sim001.net/guide/ · all of it as one text file:
https://expectation.sim001.net/llms-full.txt · index: https://expectation.sim001.net/llms.txt

## Install

In an existing project, use its own setup first: a setup script, a Makefile target, or the
revision its README or CI pins. The suite, the runner and the in-app or adapter client were
written against that revision; a global install of the latest `main` may not match them.

For a new project, pin a revision yourself. There are no tagged releases yet, so take the
commit `main` is at, install it, and record it in the project's setup script, README or CI:

```bash
rev=$(git ls-remote https://github.com/ptescher/expectation HEAD | cut -f1)
cargo install --git https://github.com/ptescher/expectation --rev "$rev" expectation_cli     # `expectation`
cargo install --git https://github.com/ptescher/expectation --rev "$rev" expectation_remote  # `expectation-run`, for remote drivers
expectation --version
```

## The commands

```bash
expectation init [dir]                  # a starter suite to edit; never writes over one
expectation check <dir>                 # errors exit 1: vocabulary, conflicts, stories
expectation plan <dir> --coverage       # the tests the stories compile to, plus generated ones
expectation graph <dir> [--dot]         # the flow DAG
expectation states <dir>                # the map of the app's states
expectation docs <dir> <out>            # the manual
```

`docs --approved-only` writes only what is marked `Approval: approved`. **Unmarked counts as
not approved**, so on a suite that doesn't use `Approval:` it writes an empty manual. Check
that the suite marks approval before you reach for it.

## The language in one screen

```gherkin
Vocabulary:
  State: the kettle is off | heating | dry
  State: the dial is 0..100 %
  State: the preset is WARM | HOT
    # What the reader sees:
    Shows: HOT: the display reads HOT
  Event: the user presses lock
    # How the docs tell the reader to do it:
    Say: press LOCK

Background:
  Given the kettle is off

Feature: Heating
  # `approved` or `proposed`, right under the Feature line; see the guide.
  Approval: approved
  # Free text before the first step is the description:
  Turn the dial past 55 % and the kettle starts heating.

  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

  Expectation: Pressing lock stops the kettle
    Given the kettle is heating
    When the user presses lock
    Then the kettle is off

Story: Your first cup
  As a tea drinker
  I want to start the kettle
  So that I can make tea

  Scenario: Dial up
    When the preset is HOT
    When the dial is 80 %
    Then the kettle is heating
    When the user presses lock
    Then the kettle is off
```

- Comments are whole lines starting with `#`. There are no trailing comments: a `#` after a
  value is part of the value (`Approval: approved  # note` is a syntax error, and after `Say:`
  or a description it becomes text in the manual).
- A conflict is two expectations that both apply and promise different things. Settle it with
  `Overrides: <name>`, or narrow a `Given`.
- A story only describes the journey; the expectations it passes through supply the checks. A
  story that relies on unspecified behaviour is an error.
- `Status: pending` marks behaviour written before it is built. `Approval:` is separate: it
  says whether the *specification* is agreed.
- States are named values or numbers in a range. There are no strings, lists or records: a
  list is a count (`the cart has 0..20 items`, `Then the cart goes up by 1`), the record in
  view is a state (`the invoice is missing | a draft | sent`), and typed-in text is the case it
  falls in (`the email is empty | malformed | valid`).
  https://expectation.sim001.net/guide/modelling.html

## Which way round

- **The product exists, and you are specifying or documenting it.** Inspect it first (code, UI,
  docs). Don't invent behaviour.
- **The feature doesn't exist yet, and the suite is the specification.** Write what it will do,
  get it agreed, then build it. See "Building a feature from its specification" below.

Either way, where something is undecided, ask. To record the question, write it in the
description starting with `Unresolved:` and mark the feature or expectation
`Approval: proposed`. `Unresolved:` is ordinary description text, not a keyword; `Approval:` is
a keyword. Only a person who owns the decision makes something `approved`.

## Writing a suite

1. Write the explanation and the expectations together: the vocabulary with `Say:`, `Set:` and
   `Shows:`, a description under each `Feature:`, the expectations, and one story per user goal.
2. `expectation check`, then `expectation plan --coverage`, then `expectation docs`, and read
   the pages it wrote.
3. A story becomes a tutorial: a real goal, the path a user would take, 3 to 8 steps. It is not
   a sequence of test steps; generated tests reach what no story does.
4. Before running anything that trades, pays, posts or creates accounts, know what it changes,
   where it goes and what isolates it. Know what screenshots, logs and HARs capture before they
   are saved or published.
5. Report three verdicts separately: the specification (complete, readable), the tests (what
   ran, observed or simulated; "not supported yet" is not "passing"), and approval/status.

The `write-feature-specs` skill has the full checklist for descriptions, stories, auditing and
reporting: https://expectation.sim001.net/skills/write-feature-specs/SKILL.md. Depth and a
worked example: https://expectation.sim001.net/guide/feature-docs.html. Reference:
https://expectation.sim001.net/guide/language.html.

## Building a feature from its specification

1. Add the feature to the suite: its words, a description, expectations and a story. Mark each
   expectation that isn't built `Status: pending`, and the feature and story
   `Approval: proposed` until a person agrees.
2. `expectation check`: a conflict with an existing expectation is a product question. Raise
   it; don't pick a winner yourself.
3. Build the behaviour, and bind its new words in the driver.
4. Remove `Status: pending` from what you built, and run. A test that depends on a pending
   expectation is not run and is printed with `–`, like "not supported yet".
5. Gate: `expectation-run --strict` (or `Policy::Required`) passes only when every test that
   isn't pending ran and passed; `--strict=all` also fails while anything is pending.

Don't edit an approved expectation to make a test pass. If the specification is wrong, say so.
https://expectation.sim001.net/guide/spec-first.html

## Writing a driver

A driver has one handler per word of the vocabulary. A state is named by its subject
(`State: the cart has 0..20 items` is `the cart`); an event by its whole phrase.

| Handler | Does | Needed for |
| --- | --- | --- |
| `observe(subject)` | reads a state: a string for a named value, a number for a number | every check |
| `on(event)` | makes an event happen | every `When <event>` |
| `set(subject)` | changes a state the outside world changes | every `When <state> is …` |
| `arrange(subject)` | puts a state in place before a test, however is quickest | a Background, a scenario's Givens, generated tests |
| reset | returns the system to a clean start | every test |

```js
new ExpectationDriver({ platform: 'node', app: 'Cart' })
  .observe('the cart', () => cart.items.length)
  .observe('the checkout', () => cart.checkout)
  .arrange('the cart', n => cart.fill(n))
  .on('the shopper adds an item', () => cart.add('tea'))
  .onReset(() => { cart = new Cart(); })
  .start('http://127.0.0.1:4477');   // where expectation-run listens
```

- `observe` must return exactly a declared value. Anything else fails the check.
- A word with no handler is not an error: the tests that need it are "not supported yet". Bind
  what the system really does; don't fake a handler to turn a `–` into a `✓`.
- Rust is the same with `Bindings::new(world).observe(…).on(…)`, in-process, from `cargo test`.
- Runnable examples: https://github.com/ptescher/expectation/tree/main/examples/cart-node
  (JavaScript) and the guide's getting-started chapters.
  https://expectation.sim001.net/guide/getting-started-node.html ·
  https://expectation.sim001.net/guide/drivers.html

## Running the tests

Tests run through a driver that binds the vocabulary to the system under test:

- Rust closures (`Bindings`, https://expectation.sim001.net/guide/drivers.html), in-process.
- A remote driver that `expectation-run` (or a `RemoteDriver`) talks to over HTTP
  (https://expectation.sim001.net/guide/remote.html). It is either **embedded** in the app
  (Swift, JavaScript or Rust client calling the app's own actions and state) or an **external**
  adapter in the test process (a client inside a WebdriverIO, Appium, XCUITest or Playwright
  run, driving the app through its UI). Embedded shows the behaviour but
  not that the buttons are wired up; external shows what a user can reach but only observes
  what the UI exposes. Say which one produced the evidence.

**A successful command is not coverage.** Anything the driver does not bind is "not supported
yet", not a failure, so a run where nothing was bound exits 0; a failed screenshot is a
warning. Read the per-plan results (`✓`, `✗`, `–` lines, or the `plan` rows in
`docs/results/*.tsv`) and check that the stories and screenshots you expect actually passed
and were taken. Never infer "verified" from the exit code alone.
