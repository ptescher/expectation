# Evidence and contract coverage

A story can pass while only ever exercising an API's happy path. Contract coverage shows what
the tests' runs actually did against a contract, such as an API's documented responses, an
event catalogue or a list of error codes. It shows what was seen, what was not, and what nobody
can tell, with each observation linked to the test step that made it.

Expectation doesn't read network captures or API specifications. It records what an extension
needs to line evidence up with the tests: the run's id, when each step ran, and any files the
driver captured. It also reads back what the extension made of that evidence. The extension
does the rest: it parses a HAR, matches requests to an OpenAPI document, and decides which
responses and values were seen. Any language will do.

Contract coverage finds candidates for a person to review. It doesn't pass or fail anything,
and it changes no test's result or expectation's verdict. If a documented outcome was never
observed, it may deserve a story, but whether it does is a product decision.

## What a run records

Every run has an id, and every step that runs has a window: wall-clock milliseconds since the
Unix epoch, from the start of its action (or, for the test's start, from resetting it) to after
its screenshot. Both are saved in the results file, as lines that older readers skip:

```text
expectation-results 1
run         <run id>  <started ms>
plan        <slug>  <status>  <fingerprint>
step        <n>  <status>  <polls>  <screenshot>  <error>
window      <n>  <start ms>  <end ms>
attachment  <n>  <media type>  <path>
check       ...
```

A `run` line applies to the plans after it. Step `0` is the test's start; `1` is its first
step. Results saved before runs had ids still load. Their tests have no run, so no evidence can
count for them.

## Attachments: what the driver captured

A driver can hand over files after the test's start and after every step, such as the network
requests that step made or a log. The runner saves each one as it is, under the step it was
taken after, and records where in the results:

```rust,ignore
Bindings::new(World::default())
    .on_attachments(|w| vec![Capture {
        name: "network.har".into(),
        media_type: "application/har+json".into(),
        bytes: w.take_har(),
    }])
```

```rust,ignore
RunOptions { attachments_dir: Some("captures".into()), ..Default::default() }
```

Files go to `<attachments_dir>/<run>/<test>/<step>/<name>`. Names are reduced to letters,
digits, `.`, `_` and `-`, and can't leave the directory. Without `attachments_dir` the runner
still asks for them, so the driver's buffer is emptied every step, but keeps nothing.

An app with an embedded driver says `"attachments": true` in its hello and answers the
`attachments` command (see [Embedded drivers](remote.md#the-protocol)). In Rust that is
`Handlers::on_attachments`. With `expectation-run`, pass `--attachments DIR`.

**Keep attachments out of the docs directory.** A capture can hold authorization headers,
cookies, request bodies and personal data, and a docs directory is made to be published. The
runner never copies attachments into the manual, and nothing in a coverage file has room for
them. Keeping secrets out of a capture in the first place is the driver's job.

Attachments are optional. An extension can also read a capture made by any other tool (a
browser's HAR export, Playwright, a proxy) and match its entries to steps by time.

## The HAR and OpenAPI extension

[`extensions/har-openapi`](https://github.com/ptescher/expectation/tree/main/extensions/har-openapi)
holds captured HTTP traffic against an OpenAPI 3.0 or 3.1 document:

```bash
expectation-run expectations --docs docs --attachments captures
npx expectation-har-openapi --results docs/results/app.tsv --attachments captures \
  --openapi api.yaml --source live --complete
expectation docs expectations docs
```

Each documented status (exact, range or `default`), each `enum` value and each `oneOf`/`anyOf`
branch of a JSON response becomes an outcome. It also reads a HAR made by any other tool
(`--har`), and places its requests in steps by time. Its README lists what it assesses and what
it leaves unknown. `crates/harness/examples/quote` runs the whole pipeline, with a stand-in
quote service.

## Code coverage

Contract coverage asks which documented outcomes were seen. Code coverage asks which of the
program's own source ran, and links that to the test that ran it. It is a separate thing with a
separate section on the test's page, because it means something different.

The application collects it (Istanbul, or anything that writes LCOV) and hands it over as step
attachments, in JavaScript with `onAttachments`. Then
[`extensions/code-coverage`](https://github.com/ptescher/expectation/tree/main/extensions/code-coverage)
reads LCOV or Istanbul's `coverage-final.json` and writes `results/code[@platform].code.tsv`:

```bash
expectation-run expectations --docs docs --attachments captures
npx expectation-code-coverage --results docs/results/app.tsv --attachments captures \
  --revision "$(git rev-parse HEAD)" --scope "src/**, without tests" \
  --report-url "https://coverage.example.com/{run}/{slug}/"
expectation docs expectations docs
```

Each test's page then has a "Code coverage" section: the source revision and scope that were
instrumented, the lines, branches and functions hit at the start and after each step, what each
step added, and a link to a full HTML report that is hosted somewhere else (`{run}` and `{slug}`
are filled in for each test). Raw coverage files stay in the attachments directory, outside the
manual.

What the section claims is limited on purpose:

- **It is observed execution.** It never changes a test's result or an expectation's verdict,
  and it doesn't say anything was asserted. A line that ran in a failed test ran.
- **Missing is not uncovered.** A step with no capture, or with one that can't be read, is shown
  as that. Zero lines hit and "no data" are different.
- **A step's part is only claimed when it can be.** Counters count from when the app started, so
  a step's part is the difference from the capture before. If a counter goes down, the app
  restarted, and nothing is attributed to that step. If a step's capture is missing, the next
  difference is labelled as spanning the steps between. Anything else that ran in the same
  window, such as background work, is in the numbers, and the page says so. With
  `--counters per-step` the app empties its counters each time it hands them over.
- **It counts for its own run and test.** Like contract evidence, a capture is shown only with
  the run it came from and a test that hasn't changed since.

Code coverage files are read the same way as the others: tab-separated, a few hundred characters
a field, 16 MiB at most, and the report link must be an `http` or `https` address. The format is
described at the top of
[`crates/story/src/codecov.rs`](https://github.com/ptescher/expectation/blob/main/crates/story/src/codecov.rs).
Expectation doesn't instrument the app: collecting coverage is the app's and the driver's job.

## Writing an extension

An extension reads:

- the results file (`results/<runner>[@platform].tsv`), for the run id, each test's slug and
  fingerprint, and each step's window;
- the evidence: the attachments, or another tool's capture;
- the contract (an OpenAPI document, say).

It writes `results/<name>[@platform].coverage.tsv`, which `expectation docs` and
`Acceptance::publish` read next to the results. Coverage files are filtered by platform the
same way results are. The format is tab-separated, with `-` for nothing:

```text
expectation-coverage 1
contract      <name>  <kind>  <revision>
evidence      <id>  <run>  <test slug>  <fingerprint>  <step>  <attribution>  <source>  <label>
item          <id>  <group>  <label>  <location>  <observed|not-observed|unknown>  <reason>
seen          <item id>  <evidence id>
complete      <run>  <yes|partial|no>  <note>
undocumented  <group>  <label>  <evidence id>
unmatched     <label>  <evidence id>  <reason>
excluded      <group>  <reason>
```

- **contract**: what it is and which version: `Quote API`, `API contract`, `sha256:…`.
- **evidence**: one observation, such as a request and its response. `<step>` is the step it
  belongs to (0 for the test's start). `<attribution>` says how it was tied to that step:
  `declared` (the driver captured it for that step), `timed` (it began and ended inside the
  step's window), `spans` (it began in that step and ended in a later one), `setup` (while the
  test was being set up) or `unassigned`. `<source>` is `live`, `replay`, `stand-in`, `fault`
  (an injected failure) or `unknown`. `<label>` is what a reader sees, such as
  `POST /quote → 429`. Keep it free of secrets: a path template, not a path with ids in it.
- **item**: one outcome the contract describes: a status code, an enum value, a `oneOf`
  branch. `<group>` gathers them (`POST /quote`), and `<location>` says where the contract
  defines it (`#/paths/~1quote/post/responses/429`).
- **seen**: which evidence saw which item. Items and evidence come before the `seen` lines that
  name them.
- **complete**: whether the extension had all of a run's traffic, with every body it needed.
- **undocumented**: something seen that the contract doesn't describe, such as a status it
  doesn't list.
- **unmatched**: evidence the extension couldn't place in the contract, such as a request that
  matches no operation or matches two.
- **excluded**: a group left out of the counts on purpose, with why.

Lines of a kind Expectation doesn't know are skipped, so later versions can add kinds. Lines it
can't read are left out and listed on the page. Every field is cut to a few hundred characters
with control characters removed, ids to 128. A file over 16 MiB isn't read.

## The rules

Expectation holds every coverage file to the same rules, whatever the extension claims:

- **Evidence counts only for its own run.** Its run id must be the run of the result shown for
  that test, and its fingerprint must be the test's. Evidence from another run, from a test that
  has changed since, from a test that didn't run, or from a step the test doesn't have covers
  nothing. The page counts it under "Evidence that doesn't count".
- **Only evidence tied to a step counts.** Evidence that names no test or step is unassigned,
  whatever its attribution says.
- **"Not observed" needs a complete capture of every run.** It stands only if every run that ran
  tests is marked `complete yes`. Otherwise, or if some results have no run id, the item is
  unknown, with the reason.
- **Observed is observed, even in a failed test.** The response happened. The test's result is
  shown as it is, and evidence never makes a failed or unsupported test pass.
- **Counts are of what's listed.** Totals cover the items in the file, minus excluded groups.
  There is no percentage of a whole API.

An extension should mark an item `unknown`, with the reason, whenever it can't tell:

- a body that is missing, redacted, truncated or not JSON;
- a schema construct it doesn't support;
- a stream (SSE, WebSocket) when it only reads HTTP.

A recorded response is not proof that the app showed the right thing, and evidence from an
injected fault shows how the app handles an outcome, not that the real service produces it.
The page marks fault evidence as injected.

## In the manual

Each contract gets a page, `contracts/<name>.md` and `site/contract-<name>.html`, linked from
the test report. It shows:

- the counts, and how complete each run's capture was;
- one table per group: each outcome, its state, the test steps that saw it (with source and
  attribution), and why it is unknown;
- what was seen but isn't in the contract, and what matched nothing;
- the evidence that doesn't count and why, the excluded groups, and any lines of the file that
  were left out.

`contracts/<name>.json` has the same facts for machines. Each test page lists, under the step,
the evidence that counts for it. Steps have anchors (`#step-3`), so links go straight there.

Everything comes from the saved files, so `expectation docs` rebuilds the same pages without
running the app.
