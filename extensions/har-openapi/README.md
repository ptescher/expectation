# expectation-har-openapi

An [Expectation](https://github.com/ptescher/expectation) extension. It holds the HTTP traffic a
test run captured (HAR) against an OpenAPI document, and writes the contract coverage that
`expectation docs` turns into a page of the manual. The page shows which documented responses
and body variants the tests' requests got, which they didn't, and which can't be told. Each
observation is linked to the test step that made the request.

It finds candidates for review. A rate-limit response no test ever got may deserve a story;
whether it does, and what the user should see, is for a person to decide. Nothing here passes or
fails a test.

## Use

```bash
# 1. Run the tests, keeping what the driver captured (or capture with any other tool).
expectation-run expectations --docs docs --attachments captures
# 2. Hold the captures against the contract.
npx expectation-har-openapi --results docs/results/app.tsv --attachments captures \
  --openapi api.yaml --source live --complete
# 3. Write the manual: it has a page for the contract.
expectation docs expectations docs
```

The coverage file goes next to the results, as `docs/results/api[@platform].coverage.tsv`.
The format is in [Evidence and contract coverage](https://expectation.sim001.net/guide/evidence.html).

| Option | |
| --- | --- |
| `--results <file>` | The run's results, as Expectation saved them. |
| `--openapi <file>` | The contract: OpenAPI 3.0 or 3.1, JSON or YAML. |
| `--attachments <dir>` | Where the run kept what the driver captured. Its HAR files are already placed in their steps. |
| `--har <file>` | A capture made by any tool (a browser, Playwright, a proxy). Its requests are placed in steps by time. Repeatable. |
| `--source <s>` | Where the traffic came from: `live`, `replay`, `stand-in`, `fault` or `unknown` (the default). An entry's `_expectation.source` overrides it. |
| `--complete` | The capture holds all of each run's traffic, with every body. Without it, Expectation reports nothing as not observed. |
| `--server <url>` | A base URL to match requests against as well as the document's servers, such as staging's. Repeatable. |
| `--host <host>` | Only hold requests to this host to the contract. By default, the hosts the servers name (every host, if a server URL is relative). Repeatable. |
| `--operations "POST /quote, GET /prices"` | Also report these operations, even if no test called them. |
| `--all` | Report every operation in the document. |
| `--clock-offset <ms>` | Add this to the capture's times, if its clock was off from the runner's. |
| `--id <name>` | The coverage file's name (default `api`). |
| `--name <text>` | The contract's name in the manual (default the document's title). |
| `--out <file>` | Write the coverage file here instead. |

## What it does

**Placing requests in steps.**

- A HAR the driver attached to a step is *declared* for that step.
- A HAR entry can declare its own run, test and step in `_expectation: {"run", "test", "step"}`.
- Otherwise an entry is placed by time, against the step windows in the results:
  - *timed*: it began and ended inside a step's window;
  - *spans*: it began in a step and ended in a later one;
  - *setup*: it happened while the test was being set up;
  - *unassigned*: it fell outside every window, or inside two tests' windows at once (tests run in parallel).

**Matching operations.**

- A request matches by server (host, scheme and base path; server variables take their defaults
  and enum values), then path template, then method.
- Concrete paths win over templated ones.
- Two equally specific matches are reported as ambiguous, not guessed.
- Requests to the contract's hosts that match nothing are listed as unmatched. Requests to other
  hosts are counted and left out.

**Responses.**

- A status matches the exact code, then a range (`4XX`), then `default`. The item says which
  one it is.
- A media type matches exactly (ignoring parameters), then `type/*`, then `*/*`.
- A status or media type the contract doesn't document is listed as undocumented.

**Body variants.** For a JSON body, every `enum` value and every `oneOf`/`anyOf` branch of the
response schema is an outcome.

- Local `$ref`s are followed, and 3.1 `$ref`s with siblings are treated as `allOf`.
- The rules for `properties`, `additionalProperties`, `items`, `allOf`, `type` (with `nullable`
  or `null`), `enum`, `const`, `required`, lengths, ranges and `pattern` are applied.
- A discriminator picks its branch.
- A value at an enum that isn't one of its values is undocumented. It is described by type and
  length, never shown.

**Unknown, never guessed.** These make an outcome unknown:

- a body that is missing, truncated (shorter than its `size`), redacted (`_expectation.redacted`,
  `content._redacted`, or a comment that says so), not JSON, invalid, or over 5 MiB;
- a body that can't be placed (two `oneOf` branches fit);
- a schema that uses something this extension doesn't assess: `not`, `if`/`then`/`else`,
  `patternProperties`, `prefixItems`, `unevaluated*`, `dependent*`, `contains`,
  `propertyNames`, dynamic references, external `$ref`s, or references that loop or lead nowhere;
- SSE and WebSocket messages. The connection's status is assessed, its messages aren't.

**Completeness.** A run's capture is complete only with `--complete`, and only if:

- every HAR entry could be read;
- with `--attachments`, every test that ran has a capture.

Otherwise it's partial, and Expectation reports what wasn't seen as unknown rather than not
observed.

## Safety

HARs hold authorization headers, cookies, query strings, request bodies and personal data. This
extension reads them and writes none of it:

- Labels are a method, a path template and a status. A path that matches no operation keeps only
  segments the contract uses, versions and short lowercase words; numbers become `{n}`, UUIDs
  `{uuid}`, anything else `{…}`.
- Response bodies are parsed to be analysed and then dropped. No header, query value or request
  body is read at all.
- Fields are stripped of control characters and cut to length.
- HAR files over 256 MiB and contracts over 16 MiB aren't read. YAML aliases are capped.
- References can't reach outside the document, or into `__proto__`.
- An attachment path in a results file can't point outside `--attachments`.

Keep the captures themselves out of anything published. Expectation keeps attachments outside the
docs directory for that reason.

## Develop

```bash
npm test                         # the unit tests
EXPECTATION_E2E=1 npm test       # also end to end, with cargo: the quote example, this, `expectation docs`
```

The end-to-end test uses `crates/harness/examples/quote` with the contract in
`test/fixtures/quote.yaml`. Its quote service is a stand-in that always answers with a firm price,
so the contract page lists the rate limit and the indicative quote as never observed.
