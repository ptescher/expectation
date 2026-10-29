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

## Workflow

1. Write the explanation and the expectations and stories together.
2. `expectation check <dir>` (fix errors, read warnings).
3. `expectation plan <dir> --coverage` (a story or generated test for each `untested`).
4. `expectation docs <dir> <out>`; read the feature pages and tutorials as a newcomer.
5. Report: your assumptions, claims with no evidence, steps the driver can't do yet ("not
   supported yet" is not "passing"), and open decisions. Don't present any as verified.

## Approval

`Approval: approved | proposed` under a Feature, Expectation or Story. Only a person who owns
the decision approves; leave things unmarked or `proposed` otherwise.
`expectation docs <dir> <out> --approved-only` writes the agreed part. It is separate from
`Status: pending` and from test results.
