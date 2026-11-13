# APIs: requests as evidence

A suite can describe a system that has no screen: an HTTP API. The vocabulary, the
expectations, the stories and the checks work as they do for an app. Only the evidence is
different. Where an app's manual shows a screenshot of each step, an API's manual shows the
request the step sent, as a `curl` command, and the response it got.

```sh
curl -X POST "$BASE_URL/v1/invoices/in_0002/void" \
  -H "Authorization: Bearer $MEMBER_API_KEY"
```

```http
HTTP/1.1 403 Forbidden
Content-Type: application/json

{
  "error": {
    "type": "forbidden",
    "message": "Only an admin of the account can void an invoice."
  }
}
```

Every example in the manual is a request a test made against the API, so it can't drift away
from what the API really does. [`crates/harness/examples/invoices`](https://github.com/ptescher/expectation/tree/main/crates/harness/examples/invoices)
runs the whole thing against a stand-in invoices API:

```bash
cargo run -p expectation_harness --example invoices                  # docs in target/expectation-docs/invoices
cargo run -p expectation_harness --example invoices -- out --bug     # members can void: a failure
```

## Writing the suite

Write it about what callers do and what the API promises, as you would for an app. The events
are what a caller asks for ("the caller voids the invoice"), and the states are what a caller
can read back ("the invoice is open"). Who is calling is a state too, so the rules that depend
on it can be written as exceptions:

```gherkin
Vocabulary:
  State: the invoice is none | draft | open | void
    Shows: void: the invoice's `status` is `void`
  State: the caller is admin | member
    Set: use the {value}'s API key
  Event: the caller voids the invoice
    Say: void the invoice

Feature: Voiding
  Expectation: Voiding an open invoice voids it
    Given the invoice is open
    When the caller voids the invoice
    Then the invoice is void

  Expectation: Members can't void
    Overrides: Voiding an open invoice voids it
    Given the invoice is open
    And the caller is member
    When the caller voids the invoice
    Then the invoice is open
```

`expectation check` finds the permission rule that conflicts with the rule it is an exception
to, as it would for any two expectations that promise different things in the same situation.

## The driver

The driver calls the API with whatever HTTP client you use, and keeps each request it sends
with the response it got as an `Exchange`. The runner takes them with `on_exchanges` (or
`Driver::take_exchanges`):

```rust,ignore
use expectation_harness::{Bindings, Exchange};

let x = Exchange::new("POST", format!("{base}/v1/invoices/{id}/void"))
    .header("Authorization", format!("Bearer {key}"))
    .response(403)
    .response_header("Content-Type", "application/json")
    .response_body(body);

Bindings::new(World::new())
    .observe("the invoice", World::status)        // GET /v1/invoices/{id}
    .on("the caller voids the invoice", World::void)
    .on_exchanges(|w| std::mem::take(&mut w.log))
```

Hand them over as they were sent, credentials and all: the runner redacts them (below). It asks
after arranging a test's start, after each action and after each step's checks, so it knows
what each request was for:

| Role | When | Where the manual shows it |
| --- | --- | --- |
| `setup` | while the test's start was arranged | on the test's page, folded away |
| `action` | while the step's action ran | on the tutorial, the feature page and the test's page |
| `check` | while the step's Thens were read back | on the test's page, folded away |

The same request sent again in a row, such as a check that polled until it held, is shown
once, with how many times it was sent.

## What is kept

Exchanges are kept in the results file (`results/api.tsv`), which is published with the
manual. So the runner keeps only what `RunOptions::redaction` lets through:

- Only a few headers: `Accept`, `Content-Type`, `Idempotency-Key`, `If-Match`, `If-None-Match`
  and `Prefer` on requests, and `Content-Type`, `ETag`, `Location` and `Retry-After` on
  responses. Cookies and anything a gateway added are dropped.
- `Authorization` keeps its scheme, and the key becomes `$API_KEY`, or the name you give it in
  `keys`. So does the value of an `X-Api-Key` or `Api-Key` header.
- Query parameters, form fields and JSON fields named like a secret (`password`, `token`,
  `client_secret`, `api_key`, `cvc` and the like), and those you list in `fields`, read
  `REDACTED`.
- A loopback address (`http://127.0.0.1:53211`) becomes `$BASE_URL`. Set `base_url` to show
  another origin instead, such as `https://api.example.com`.
- JSON bodies are pretty-printed, with numbers kept as they were written. A JSON body that
  doesn't parse can't be checked for secrets, so it is replaced with a note. A body over
  `max_body` (16 KiB) is cut short.

```rust,ignore
RunOptions {
    redaction: Redaction {
        keys: vec![
            (ADMIN_KEY.into(), "$ADMIN_API_KEY".into()),
            (MEMBER_KEY.into(), "$MEMBER_API_KEY".into()),
        ],
        fields: vec!["email".into()],
        ..Default::default()
    },
    ..Default::default()
}
```

Name the keys when who is calling matters, so the reader can tell an admin's request from a
member's. A name is a `$` followed by capitals, digits and `_`. The `curl` commands leave those
for the shell to fill in, and escape any other `$`.

Redaction is by name and by header, so it can't know that a field called `note` holds a
customer's address. Read what the manual shows before you publish it, and add such fields to
`fields`.

## Limits

- Bodies are text. Hand over a binary body as a note of what it was.
- The [remote drivers](remote.md) (Swift, JavaScript, the embedded Rust client) don't send
  exchanges yet.
- A step with both a screenshot and requests shows both: an app that calls an API can show its
  screen and its requests.
- Exchanges are for the manual. To hold the traffic against an OpenAPI document, capture it as
  HAR too and use [the HAR and OpenAPI extension](evidence.md#the-har-and-openapi-extension).
