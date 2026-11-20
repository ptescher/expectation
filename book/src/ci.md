# In CI

A suite gives CI three things to do, from cheapest to most thorough:

1. **Check the suite.** No product needed, a second or two. Run it on every pull request.
2. **Run the tests.** Needs the product and its driver.
3. **Keep the manual.** The run writes it; CI stores or publishes it.

## Check the suite

```bash
expectation check expectations
```

It exits 1 when the suite has errors (a conflict, a story that relies on unspecified
behaviour, a word outside the vocabulary) and 0 otherwise. Warnings don't fail it.

## Run the tests

How the run is started depends on the driver:

- **A Rust test** (`Bindings`): `cargo test` runs it, and `assert_passes` fails the test when a
  check fails. For a gate, use `assert_passes_with(&mut driver, &opts, Policy::Required)`.
- **A remote driver** (JavaScript, Swift, anything else): start `expectation-run` and the
  driver, as in [Getting started without Rust](getting-started-node.md#run-it). For a gate,
  pass `--strict`.

A run without a policy passes when nothing failed, even if nothing ran. That is the right
default while a suite is being bound a word at a time, and the wrong one for a branch you
release from. [A run as a gate](drivers.md#a-run-as-a-gate) has the three policies, and
[Spec first](spec-first.md#6-gate-on-it) says which fits when.

## A GitHub Actions workflow

For a JavaScript project whose `run.mjs` starts the runner and the driver:

```yaml
name: Acceptance
on:
  pull_request:
  push:
    branches: [main]

env:
  # The commit of Expectation the suite was written against.
  EXPECTATION_REV: <commit>

jobs:
  acceptance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-node@v4
        with:
          node-version: 22

      # Built from source, so cache it: the key changes only when the pinned commit does.
      - uses: actions/cache@v4
        id: expectation
        with:
          path: ~/.cargo/bin/expectation*
          key: expectation-${{ runner.os }}-${{ env.EXPECTATION_REV }}
      - if: steps.expectation.outputs.cache-hit != 'true'
        run: |
          cargo install --git https://github.com/ptescher/expectation --rev "$EXPECTATION_REV" expectation_cli
          cargo install --git https://github.com/ptescher/expectation --rev "$EXPECTATION_REV" expectation_remote

      - run: expectation check expectations
      - run: npm ci
      - run: node run.mjs          # passes --strict to expectation-run

      # The manual and the saved results, also when the run failed.
      - if: always()
        uses: actions/upload-artifact@v4
        with:
          name: manual
          path: docs
```

A Rust project needs no install step: `cargo test` builds the harness from the project's own
`Cargo.lock`, and `cargo run -p <your tools package> -- check expectations` runs the command
at the same version (see [In your own project](cli.md#in-your-own-project)).

## Keep the manual

`docs/` after a run holds the manual and `docs/results/*.tsv`, the results it was written
from. Two ways to keep them:

- **Commit them.** The Markdown manual renders on GitHub, and a pull request's diff shows how
  the documentation changed. `expectation docs expectations docs` rebuilds the manual from the
  saved results without running anything.
- **Publish the site.** Upload `docs/site/` to any static host. For readers, not authors,
  build [the reader edition](manual.md#the-reader-edition).

Screenshots can make a repository large; [Screenshots in large
projects](manual.md#screenshots-in-large-projects) keeps them in a store or a bucket.

Before either: a manual holds screenshots and, if the driver hands them over, requests and
logs. Anything committed or published is visible to everyone who can see it. Read [Evidence
privacy](feature-docs.md#evidence-privacy) first, and run CI against test accounts and
fixtures.

## Several runs, one manual

A unit-level run and a browser run can each cover part of one suite. Give each a name
(`--runner api`, `--runner web`) and the same `--docs` directory, and the manual shows both.
A policy judges only the run that was just made: a pass saved by an earlier run doesn't stand
in for a test that didn't run now. See [Several runners](drivers.md#several-runners).
