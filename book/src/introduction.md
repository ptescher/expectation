# Introduction

Expectation is acceptance testing where the behaviour itself is the source of truth.

You describe a system as **expectations**: given some state, when something happens, then the
system ends up in some state. Every step is written in a shared **vocabulary** of states and
events, so expectations can be compared with each other mechanically, not just run.

```gherkin
Expectation: Turning the dial up starts the kettle
  Given the kettle is off
  When the dial is above 55 %
  Then the kettle is heating
```

From a suite of expectations, Expectation:

1. **finds conflicts.** Two expectations that both apply, and promise incompatible things, are
   an error. It names the situation where they clash and suggests which one is the exception.
   You settle it with `Overrides:`, and overrides must form a DAG.
2. **builds a flow DAG** from the starting state. It shows what leads to what, warns about
   behaviour nothing can reach, and gives every expectation a shortest route from the start.
3. **compiles user stories into acceptance tests.** A story only describes the journey. At each
   step, the expectations it passes through supply the checks. A story that relies on
   unspecified behaviour, or contradicts an expectation, is rejected before anything runs.
   Generated tests reach whatever no story does.
4. **runs those tests** against your system through a small driver, and takes a screenshot at
   every step.
5. **writes the documentation**: a manual where stories are step-by-step tutorials and features
   are topic pages, with the test evidence folded away under "How this is tested", plus a full
   test report.

```text
.expect files ─language─▶ Suite ─graph─▶ conflicts · override DAG · flow DAG
                            │                         │
                            └────────story────────────┴─▶ plans (acceptance tests)
                                                             │
                                          harness + driver ──┴─▶ report + screenshots ─docs─▶ HTML + Markdown
```

Writing a suite that also reads as good documentation takes some care: see
[Feature docs that explain](feature-docs.md), and its [generated feature page](../smart-kettle/site/feature-boiling-when-it-is-cheap.html).

## Why

Most acceptance tests are scripts: do this, check that. Nothing connects one script to the
next, so two tests can quietly assume opposite things, and nobody notices which behaviour is
never tested at all. Gherkin made scripts readable, but its steps are free text bound to
code with regular expressions, so a tool can't tell what they mean.

Expectation makes the steps mean something. Because every step is a fact about a declared
state, or a declared event, the suite is a model of the system. That model can be checked
before any test runs. A story doesn't have to restate every outcome, because the expectations
already promise them. And the same model, written in the user's words, is good enough to
publish as the user manual.

## What it is not

- It doesn't drive your UI for you. You write a **driver** (usually a dozen closures) that
  sets states, makes events happen and reads states back. Anything you can control from Rust
  can be tested: a library, a game loop, a web app through its API or a browser, a device
  over a serial line.
- It isn't a property-based tester. Generated tests follow the shortest routes through the
  flow graph to cover every expectation; they don't search for counterexamples.

## Where to go next

- [Getting started](getting-started.md) runs a whole suite in five minutes.
- [A worked example](kettle.md) walks through conflicts, exceptions, stories, coverage and a
  failing run.
- [The language](language.md) is the reference for `.expect` files.
- The [example manual](../example/site/index.html) is what the generated docs look like.
