---
name: write-feature-specs
description: Write or improve an Expectation (.expect) suite so it is both an acceptance model and useful feature documentation. Use when asked to spec a feature, write or review feature descriptions, stories or a generated manual, or to mark specs proposed or approved.
---

# Writing feature specs that make useful docs

Follow `book/src/feature-docs.md`; the worked example is `examples/smart-kettle/`. Language
reference: `book/src/language.md`.

## Before writing

Inspect the product: code, UI, existing docs, tickets. Note what you can see and what you are
assuming. Do not invent behaviour. If something is undecided, ask the user, or record it.

Inventory the product's main user goals *from the product*, not from the suite, and keep a
coverage map: goal → feature page → tutorial → expectations → evidence → gap. List goals with
no page or only partial coverage.

## For each user-facing feature, the description says

1. What it does, who uses it, and the task or decision it supports.
2. The normal workflow, the inputs and outputs.
3. How to read the result (what a dot, number or banner means, and what to do).
4. Empty, unavailable, invalid or partial states, and limits that matter.
5. What is settled and what is not. Write `Unresolved: <question>`, mark the expectation
   `Approval: proposed`, and ask for the decision. Never fill the gap yourself.
6. The promises that the `Expectation:`s verify.

Enough detail that a reader needs no prior conversation; cut filler, not context. Use `Say:`,
`Set:`, `Shows:` on the vocabulary, and `Term:` for jargon. Model an outside service's health
as a state so the unavailable case can be specified and tested.

## For each story (it becomes a tutorial)

A story teaches the feature; it is not a replay of test steps. Its introduction and steps say:

1. The real goal and the situation, and where in the product the reader starts.
2. The path a user with that goal would take, ending when the goal is met. Not a sequence that
   exercises states (toggling a control back and forth); expectations and `plan --coverage`
   do that.
3. How the feature works: where the data comes from, what the product does and doesn't do.
   Link the feature and `[[terms]]`.
4. How to read what they see: units, what a row, number, dot or badge means, what to do next.
5. The empty, late or failed case they're likely to meet on the way.
6. Where they end up: the goal reached, as the reader sees it.

A feature's page and stories together cover the whole feature, one story per main goal. Test:
could a newcomer follow the tutorial alone and then explain the feature? If they could only
repeat the taps, it's too thin.

## The product, not the test rig

Write the product workflow, not the test rig: social sign-in, not the mock OIDC server. Mock
providers, fixtures, test accounts and driver limits go in the driver, vocabulary comments,
test evidence or contributor docs. A `Say:` for something the user doesn't cause describes what
happens ("the provider stops answering"), not how the test fakes it. Keep material caveats on
the page: "the sign-in tests use a stand-in provider; they don't show Google sign-in works".

## Workflow

1. Write the explanation and the expectations and stories together.
2. `expectation check <dir>` (fix errors, read warnings).
3. `expectation plan <dir> --coverage` (a story or generated test for each `untested`). This is
   coverage of the model, not of the product.
4. Safety preflight before running or demoing anything that trades, pays, posts, follows,
   notifies or creates an account: what can change, which environment/account/network receives
   it, and what actually isolates it (check the code path). Not provably isolated → don't run
   it; use a safe fixture or leave the step unbound ("not supported yet"). Then evidence
   privacy: what each screenshot, accessibility tree, log or HAR captures (recovery phrases,
   keys, credentials, account data), what redacts it at the moment of capture, and where the
   results and manual are shared. Can't keep a secret out → don't capture it, and say so.
5. `expectation docs <dir> <out>`.
6. Zero-context read: a fresh reviewer without your context (e.g. a subagent given only the
   rendered page and its screenshots) reads one tutorial on its own and answers: where do I
   start; what do the numbers, units and symbols mean; what changed after each step; what can I
   safely conclude, and what not; what do I do if it's empty or fails. Does it teach the goal,
   or just walk a test? Fix the specs, not a note.
7. Evidence pass: for each claim, is it observed (real run), simulated (fixture, mock, test
   account, fake data: say which), source-inspected, unsupported, or none? Is saved evidence
   stale (spec or UI changed since), or from a failed step? A screenshot proves what's in it,
   not the whole page.
8. Report three verdicts separately:
   - specification: complete and readable? (coverage gaps, zero-context findings; `check` and
     `plan --coverage` support this but don't decide it);
   - tests: what ran, passed, failed, is unsupported, observed vs simulated, embedded vs
     external driver. Read the per-plan results, not the exit code: a run exits 0 when nothing
     failed, even with nothing bound, and a failed screenshot is a warning;
   - approval/product status: approved, proposed, `Status: pending`, open decisions.
   Also list assumptions and claims with no evidence. Don't present any as verified. A passing
   test doesn't approve the prose; an approved spec doesn't prove the product works.

## Approval

`Approval: approved | proposed` under a Feature, Expectation or Story. Only a person who owns
the decision approves; leave things unmarked or `proposed` otherwise.
`expectation docs <dir> <out> --approved-only` writes the agreed part. Unmarked counts as not
approved, so a suite that doesn't mark approval gives an empty manual. It is separate from
`Status: pending` and from test results.
