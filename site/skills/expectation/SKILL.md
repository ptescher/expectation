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

For a new project, pin a revision yourself:

```bash
cargo install --git https://github.com/ptescher/expectation --rev <commit> expectation_cli     # `expectation`
cargo install --git https://github.com/ptescher/expectation --rev <commit> expectation_remote  # `expectation-run`, for remote drivers
```

## The commands

```bash
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

## Writing a suite

1. Inspect the product first. Don't invent behaviour: if something is undecided, ask, or write
   `Unresolved: <question>` in the description and mark it `Approval: proposed`.
2. Map the product's user goals (from the product, not the suite) to feature pages,
   tutorials, expectations and evidence; list the gaps.
3. Write the explanation and the expectations together, then `expectation check`, then
   `expectation plan --coverage`, then `expectation docs`. Write the product workflow; keep mock
   providers and fixtures out of the main prose, but keep caveats that change what a reader can
   conclude. Each story becomes a tutorial: it follows a real user goal from where the reader
   starts, explains how the feature works and how to read the result (units, what a number or
   badge means), and covers what goes wrong on the way. It is not a sequence of test steps.
4. Before running anything that trades, pays, posts or creates accounts, check what it changes,
   where it goes, and what isolates it. If you can't show it's isolated, don't run it.
   Also check what the evidence captures: screenshots, accessibility trees, logs and HARs can
   hold recovery phrases, keys, passwords, tokens and account data. Know what is captured, what
   redacts it (or keeps the screen off it), and where the saved results and manual will be
   shared, before you run.
5. Audit: have a reviewer without your context read a rendered tutorial on its own; mark each
   claim's evidence as observed, simulated, source-inspected, unsupported or none; check for
   stale evidence.
6. Report three verdicts separately: the specification (complete, readable), the tests (what
   ran, observed vs simulated; "not supported yet" is not "passing"), and approval/status. List
   assumptions, claims with no evidence and open decisions. Never present those as verified.

Depth and a worked example: https://expectation.sim001.net/guide/feature-docs.html.
Reference: https://expectation.sim001.net/guide/language.html.

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
