# A Linear board

`expectation status` says where each feature stands; `integrations/linear` keeps a
[Linear](https://linear.app) team's issues in step with it, one issue per feature, so a board
shows which specs are approved, built and passing.

```bash
expectation status expectations docs > status.json
LINEAR_API_KEY=lin_api_… node integrations/linear/bin/expectation-linear.js \
  --status status.json --team ENG --dry-run
```

It needs Node 18 or later and has no dependencies. Without `--dry-run` it writes.

## What it puts on the board

- **One issue per feature**, titled with the feature's name. The description lists its
  expectations with their approval and verdict, links to the source line (with `sourceUrl` in the
  config), and ends with two lines it finds the issue by: `expectation-key:` (`section/slug`) and
  `expectation-hash:` (a digest of what it wrote). Leave those two lines in.
- **Labels**: `expectation` on every issue it manages, `spec:approved | partial | proposed |
  unmarked`, and `tests:verified | failed | unsupported | untested | not_run | pending`. Labels,
  not workflow states, because a workflow is one axis and a feature has two: whether the
  specification is agreed, and what the tests said. Labels you add yourself are kept.
- **A comment** when a feature's approval or verdict changes (`tests: verified → failed`), and
  only then.
- To also move issues through your workflow, say which state each verdict goes to:

```json
{
  "states": { "verified": "Done", "failed": "Blocked" },
  "sourceUrl": "https://github.com/you/repo/blob/main"
}
```

A verdict not listed leaves the state alone. Pass the file with `--config`.

## What it doesn't do

- **It never changes the spec.** Approval is in the `.expect` file, reviewed in git. Moving a
  card to «Approved» changes nothing there; to approve a feature, edit its `Approval:` line.
- **A renamed feature is a new issue**, since the key is its slug. The old one is reported as an
  orphan and left alone for you to close.
- Stories aren't on the board; `expectation status` lists them for other uses.
- It writes to the one team you name, and creates the labels it needs there.

## In CI

Run it after the tests have saved their results, on the main branch:

```yaml
- run: expectation status expectations docs > status.json
- run: node integrations/linear/bin/expectation-linear.js --status status.json --team ENG
  env:
    LINEAR_API_KEY: ${{ secrets.LINEAR_API_KEY }}
```

A second run with nothing changed writes nothing. An API key is an account's: use one for a
bot user, and let the board be the only thing it can touch.
