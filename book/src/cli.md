# The expectation command

`expectation` checks a suite, shows its graph and plans, and writes its docs, without running
anything. Running needs the system under test, so that happens through `expectation_harness`
in your own tests or program, which saves the results for the docs.

```bash
cargo install --git https://github.com/ptescher/expectation expectation_cli
```

Or, from a checkout: `cargo run -p expectation_cli -- <command>`.

## check

```bash
expectation check <dir>
```

Loads the suite, checks the vocabulary, conflicts, overrides and stories, and prints every
diagnostic with where it is and what to do. A suite with platforms is checked as each platform
reads it. Ends with a one-line summary:

```text
Kettle: 11 expectations in 3 features, 3 stories (3 tests); 2 exceptions; flow DAG 12 edges, 3 levels deep
```

Exits 1 if there are errors, so it can run in CI.

## graph

```bash
expectation graph <dir>          # Mermaid
expectation graph <dir> --dot    # Graphviz DOT
```

Prints the flow DAG: each feature as a group, each expectation as a node (invariants as
hexagons), arrows from the start and between expectations, dotted arrows for overrides, and
red ones for conflicts nobody has settled.

## states

```bash
expectation states <dir>                               # Mermaid state diagram
expectation states <dir> --dot                         # Graphviz DOT
expectation states <dir> --by "the screen,the dialog"  # tell states apart by these
```

Prints the state map: every state the app can get into from each section's setup, and every
action that moves it from one to another (`[*]` marks the starts). Where `graph` is about
expectations, this is about the app: its screens and modes and how to get around them. A section
whose setup another expectation `Hands over to:` is not a start: it is reached from there.

States are told apart by the vocabulary's enumerated states unless `--by` names others, so a
number like a dial's reading doesn't make a state per value. Name the state that is your app's
screen to get one node per screen; actions that change only what `--by` leaves out show as
staying put.

The docs have the same map as `site/states.html`: drag to pan, scroll to zoom, and pick a state
to see its screenshot (from a test step that was in it), what can be done there, the
expectations behind each move, and the ways in. It works without JavaScript too.

## plan

```bash
expectation plan <dir>
expectation plan <dir> --coverage
```

Prints the acceptance test each story compiles to: what's arranged, each step, which
expectations fire and what's checked. `--coverage` adds the generated tests for expectations
no story reaches.

## docs

```bash
expectation docs <dir> <out-dir>
```

Writes the manual (Markdown and the HTML site) to `<out-dir>`, using the results that runs
have saved in `<out-dir>/results/`. Without results, every test shows as "not run".

## In your own project

The CLI is also a library: `expectation_cli::main()` runs the command with the process's
arguments. A workspace can ship the command as its own package, so `cargo run -p <yours> --
check expectations` builds it at the version the workspace locks:

```rust
// tools/expectation/src/main.rs
fn main() {
    expectation_cli::main();
}
```

## Exit codes

| Code | Means |
| --- | --- |
| 0 | Done, with no errors (there may be warnings). |
| 1 | The suite has errors. |
| 2 | A usage error, or a directory that can't be read. |
