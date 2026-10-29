# Feature docs that explain

A suite can pass every check and still make a thin manual. The usual cause is a description
that lists test conditions and caveats and never says what the feature is for. This chapter
is the depth to aim for, a workflow to get there, and a worked example you can run:

```bash
expectation check examples/rate-watch
expectation docs examples/rate-watch docs
```

The example is [`examples/rate-watch`](https://github.com/ptescher/expectation/tree/main/examples/rate-watch):
a small app that shows an exchange rate from an outside provider and alerts you when it reaches
a target. It has a feature that depends on an outside service, a state for when that service
is unavailable, and a decision nobody has made yet. The manuals it generates are published: [with its drafts](../rate-watch/site/index.html) and
[approved only](../rate-watch-approved/site/index.html). If you are an AI agent, there is a
[skill](https://github.com/ptescher/expectation/tree/main/.claude/skills/write-feature-specs/SKILL.md)
with this chapter's checklist.

## What a feature page needs

The text under `Feature:` is the first thing a reader sees. Write it for someone who has never
talked to you. For each user-facing feature, cover:

1. **What it does, who uses it, and what it helps them decide or do.** One or two sentences,
   in the user's words ("a traveller who wants to know whether today is a good day").
2. **The normal workflow.** What the user does, what goes in, what comes out.
3. **How to read the result.** What a dot, a number, a banner or an empty list means, and
   what to do about it.
4. **The states that are not the happy path.** Empty, unavailable, invalid, partial, late;
   and any limit that matters, like "one target per pair".
5. **What is settled and what is not.** Say what is agreed. If something is undecided, write
   `Unresolved:` and the question, and mark the expectation `Approval: proposed`. Do not
   invent an answer to make the page read smoothly; ask for the decision.
6. **The promises the tests check.** These are the `Expectation:`s under the description.

Leave filler out, not context. A paragraph per point is normal. A description that a reader
could not follow without having sat in the planning meeting is too thin, however short.

Each expectation can have a description too. Use it for the *why* of one rule ("A number that
might be hours old is worse than none"). Stories get an introduction that says the goal and
the situation; the narrative (`As a … I want … So that …`) is the reader's reason.

## An end-to-end example

Start with what the user can see about things you do not control. The rate comes from an
outside provider, so its health is a state of the vocabulary, with a `Shows:` for how the app
tells the user:

```gherkin
Vocabulary:
  State: the rate feed is live | delayed | unavailable
    Set: switch the test provider to {value}
    Shows: live: the rate has a green dot
    Shows: delayed: the rate has an amber dot and says when it was last updated
    Shows: unavailable: the rate is replaced by the message "Rates unavailable" and a Retry button
  State: the alert is off | waiting | triggered
  Event: the feed stops answering
    Say: switch the test provider off
  Term: Rate provider
    The outside service that sends the exchange rates. Rate watch only shows what the provider sends.
    Reference: Provider API | https://example.com/docs/rates
```

The feature description explains the feature, links the term, and covers the unavailable state
in plain words:

```gherkin
Feature: Seeing the current rate
  Approval: approved

  Rate watch is for people who exchange money now and then, such as a traveller who wants to
  know whether today is a good day. They open the app, pick a pair of currencies and read the
  rate. They use it to decide whether to exchange now or wait.

  The rate comes from an outside provider ([[Rate provider]]). Rate watch shows what the
  provider sends and does not work out rates by itself. Next to the rate, a dot says how much
  to trust it. Green means the rate is current. Amber means the provider is late, so the rate
  is old: the app says when it was last updated, and you should treat the rate as a guide only.

  If the provider does not answer, the app has no current rate to show. It says so and does not
  show an old rate as if it were new. When the provider is back, tap Retry to get the rate again.

  Expectation: A silent provider means no rate
    The app says "Rates unavailable" and shows no number. A number that might be hours old is worse than none.
    Given the rate feed is live
    When the feed stops answering
    Then the rate feed is unavailable
```

The undecided part is written down, not guessed. This expectation is `proposed`, and its
description says what the question is:

```gherkin
  Expectation: A delayed rate can fire the alert
    Approval: proposed
    Unresolved: should an old rate fire an alert? Firing tells you sooner, but the real
    rate may already have moved back. Product has not decided. Until it does, this is
    proposed and an approved manual leaves it out.
    Given the alert is waiting
    And the rate feed is delayed
    When the rate reaches the target
    Then the alert is triggered
```

And a story walks the journey, including the unavailable state, because that is when a reader
most needs to know what the app does:

```gherkin
Story: Get told when the rate is right
  Approval: approved
  As a traveller
  I want to be told when the euro gets cheap enough
  So that I can exchange my money without watching the rate all day

  You are going abroad next month and want to exchange money at a good rate. Instead of
  checking every day, you set an alert. This tutorial also shows what happens when the rate
  provider goes quiet, because that is when you most need to know what the app is doing.

  Scenario: Set an alert, lose the provider, get the alert
    When the user sets an alert
    Then the alert is waiting
    When the feed stops answering
    Then the rate feed is unavailable
    And the alert is waiting
    When the feed answers again
    Then the rate feed is live
    When the rate reaches the target
    Then the alert is triggered
```

### What it generates

The feature page (`features/price-alerts.md`, the HTML site's page is the same) opens with the
description, then one section per expectation, with the *why*, the instruction, and the test
folded away:

> # Price alerts
>
> An alert tells you when the rate reaches a target you chose, so you do not have to keep
> looking. […]
>
> ## Reaching the target fires the alert
>
> The banner appears the moment a live rate reaches your target.
>
> If the alert is waiting and the rate feed is live, have the test provider send a rate at or
> past your target. A banner says the rate reached your target.

The tutorial turns the story into steps, with the setup first:

> # Get told when the rate is right
>
> You are going abroad next month […]
>
> ## Before you begin
>
> - Switch the test provider to live.
> - The alert is off.
>
> ## Steps
>
> 1. **Type a target rate and tap Set alert.** The Alert button reads "Alert set".
> 2. **Switch the test provider off.** The rate is replaced by the message "Rates unavailable"
>    and a Retry button. The Alert button reads "Alert set".
> 3. **Switch the test provider back on and tap Retry.** The rate has a green dot.
> 4. **Have the test provider send a rate at or past your target.** A banner says the rate
>    reached your target.

Where the model has no `Say:` or `Shows:` for something, the page falls back to a plain
sentence made from the step. That is the cue to add one.

## The workflow

1. **Look at the product first.** Read the code, the UI, existing docs, tickets and the
   people who know it. Note what you can see and what you are assuming.
2. **Write the explanation and the expectations together.** The description, the vocabulary's
   `Say:`/`Set:`/`Shows:`, the expectations and one story per user goal. A description
   written afterwards drifts from what is tested.
3. **Check.** `expectation check <dir>` must pass. Fix every error; read every warning.
4. **Plan with coverage.** `expectation plan <dir> --coverage` shows the tests each story
   becomes and the generated ones for expectations no story reaches. Add a story, or accept a
   generated test, for each `untested` warning.
5. **Generate the manual.** `expectation docs <dir> <out>`, with results from a run if you
   have them.
6. **Read the pages as the reader.** Open the feature pages and tutorials. Ask what a newcomer
   would not understand, and what you would have to be told in person.
7. **Report what you found.** List the assumptions you made, the claims with no evidence
   (nothing in the code or product backs them), the steps the driver cannot do yet (they show
   as "not supported yet", not as passing), and the open decisions. Do not present any of
   those as verified behaviour. Mark undecided behaviour `Approval: proposed` and ask.

## Settled and proposed

`Approval: approved` and `Approval: proposed` mark what is agreed and what is not; see
[Approved-only manuals](manual.md#approved-only-manuals). A suite that does not use them is
unchanged. Approval is about the *specification*; it is separate from `Status: pending` (the
behaviour is not built yet) and from the test results.
