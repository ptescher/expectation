# For AI agents

Everything an agent needs is available as plain text, so it does not have to scrape HTML.

| What | Where |
| --- | --- |
| Index of the docs, for LLMs | [`/llms.txt`](../llms.txt) |
| The whole guide as one file | [`/llms-full.txt`](../llms-full.txt) |
| The overview skill | [`/skills/expectation/SKILL.md`](../skills/expectation/SKILL.md), also [`/skill.md`](../skill.md) |
| The feature-specs skill | [`/skills/write-feature-specs/SKILL.md`](../skills/write-feature-specs/SKILL.md) |
| Guide source (Markdown) | [`book/src`](https://github.com/ptescher/expectation/tree/main/book/src) in the repository |

To use a skill, save the `SKILL.md` to `.claude/skills/<name>/SKILL.md` in your project (or
your agent's equivalent).

## The short version

In a project that already uses Expectation, install it the way the project does (its setup
script, or the revision its README or CI pins), so the command, the runner and the client match
the suite. Otherwise pin one:

```bash
cargo install --git https://github.com/ptescher/expectation --rev <commit> expectation_cli
expectation check expectations              # errors exit 1
expectation plan expectations --coverage    # the tests the stories compile to
expectation docs expectations docs          # the manual
```

A suite is every `*.expect` file under a directory: a vocabulary of states and events,
expectations (given, when, then), and stories that become tutorials and tests. [The
language](language.md) is the reference.

## Rules that matter

- Don't invent behaviour. Inspect the product; if something is undecided, write
  `Unresolved: <question>` in the description, mark it `Approval: proposed`, and ask.
- Write the explanation and the expectations together, then check, plan, generate and read
  the pages. See [Feature docs that explain](feature-docs.md).
- Stories become tutorials, so they teach the feature: a real goal, the path a user would
  take, how it works and how to read the result. Not a sequence of test steps. See
  [What a story needs](feature-docs.md#what-a-story-needs).
- Map the product's user goals to pages, tutorials, expectations and evidence, and list the
  gaps. `plan --coverage` covers the model, not the product.
- Don't run a journey that trades, pays, posts or creates accounts until you know what it
  changes, where it goes and what isolates it. See [Safety preflight](feature-docs.md#safety-preflight).
- Don't capture what you can't share. Screenshots, accessibility trees, logs and HARs can hold
  recovery phrases, keys, credentials and account data. Check what is captured, what redacts
  it, and where the results and manual end up. See [Evidence privacy](feature-docs.md#evidence-privacy).
- A command that exits 0 hasn't verified anything by itself. Unbound steps are "not supported
  yet", not failures, so a run with nothing bound passes, and a failed screenshot is a
  warning. Read the per-plan results and check that the stories and screenshots you expect
  are there. See [Remote drivers](remote.md#in-the-runner).
- Audit with a reader who lacks your context, and say for each claim whether it was observed,
  simulated, only read in the source, or not shown at all.
- Report three verdicts separately: the specification, the tests, and approval. "Not supported
  yet" is not "passing", a passing test does not approve the prose, and `Approval:` is separate
  from both `Status: pending` and the test results. See [Reporting](feature-docs.md#reporting).
- A manual of just the agreed behaviour: `expectation docs <dir> <out> --approved-only`. It
  keeps only what is marked `Approval: approved`; unmarked counts as not approved, so on a
  suite that doesn't mark approval it writes an empty manual. Check the suite first.
