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

```bash
cargo install --git https://github.com/ptescher/expectation expectation_cli     # `expectation`
cargo install --git https://github.com/ptescher/expectation expectation_remote  # `expectation-run`, for embedded drivers
```

## The commands

```bash
expectation check <dir>                 # errors exit 1: vocabulary, conflicts, stories
expectation plan <dir> --coverage       # the tests the stories compile to, plus generated ones
expectation graph <dir> [--dot]         # the flow DAG
expectation states <dir>                # the map of the app's states
expectation docs <dir> <out>            # the manual; add --approved-only for agreed behaviour
```

## The language in one screen

```gherkin
Vocabulary:
  State: the kettle is off | heating | dry
  State: the dial is 0..100 %
  Event: the user presses lock
    Say: press LOCK                      # how the docs tell the reader to do it
  State: the preset is WARM | HOT
    Shows: HOT: the display reads HOT    # what the reader sees

Background:
  Given the kettle is off

Feature: Heating
  What it does, for whom, and how to read the result (free text before the first step).
  Approval: approved                     # or `proposed`; see the guide

  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

Story: Your first cup
  As a tea drinker
  I want to start the kettle
  So that I can make tea

  Scenario: Dial up
    When the dial is 80 %
    Then the kettle is heating
```

- A conflict is two expectations that both apply and promise different things. Settle it with
  `Overrides: <name>`, or narrow a `Given`.
- A story only describes the journey; the expectations it passes through supply the checks. A
  story that relies on unspecified behaviour is an error.
- `Status: pending` marks behaviour written before it is built. `Approval:` is separate: it
  says whether the *specification* is agreed.

## Writing a suite

1. Inspect the product first. Don't invent behaviour: if something is undecided, ask, or write
   `Unresolved: <question>` in the description and mark it `Approval: proposed`.
2. Write the explanation and the expectations together, then `expectation check`, then
   `expectation plan --coverage`, then `expectation docs`, and read the generated pages as a
   newcomer would.
3. Report your assumptions, claims with no evidence, steps the driver can't do yet ("not
   supported yet" is not "passing"), and open decisions. Never present those as verified.

Depth and a worked example: https://expectation.sim001.net/guide/feature-docs.html.
Reference: https://expectation.sim001.net/guide/language.html.

## Running the tests

Tests run through a driver that binds the vocabulary to the system under test: Rust closures
(`Bindings`, https://expectation.sim001.net/guide/drivers.html) or an embedded client in a
Swift, JavaScript or Rust app driven by `expectation-run`
(https://expectation.sim001.net/guide/remote.html). Anything the driver does not bind is
reported as "not supported yet", not as a failure.
