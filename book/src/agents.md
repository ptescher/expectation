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

```bash
cargo install --git https://github.com/ptescher/expectation expectation_cli
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
- Report assumptions, claims with no evidence, steps the driver can't do yet and open
  decisions. "Not supported yet" is not "passing", and `Approval:` is separate from both
  `Status: pending` and the test results.
- A manual of just the agreed behaviour: `expectation docs <dir> <out> --approved-only`.
