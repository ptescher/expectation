# Feature docs that explain

A suite can pass every check and still make a thin manual. The usual cause is a description
that lists test conditions and caveats and never says what the feature is for. This chapter
is the depth to aim for, a workflow to get there, and a worked example you can run:

```bash
expectation check examples/smart-kettle
expectation docs examples/smart-kettle docs
```

The example is [`examples/smart-kettle`](https://github.com/ptescher/expectation/tree/main/examples/smart-kettle):
a smart kettle that shows the electricity price from an outside provider and boils when the
price is cheap. It has a feature that depends on an outside service, a state for when that service
is unavailable, and a decision nobody has made yet. The manuals it generates are published: [with its drafts](../smart-kettle/site/index.html) and
[approved only](../smart-kettle-approved/site/index.html). If you are an AI agent, there is a
[skill](https://github.com/ptescher/expectation/tree/main/.claude/skills/write-feature-specs/SKILL.md)
with this chapter's checklist.

## What a feature page needs

The text under `Feature:` is the first thing a reader sees. Write it for someone who has never
talked to you. For each user-facing feature, cover:

1. **What it does, who uses it, and what it helps them decide or do.** One or two sentences,
   in the user's words ("someone who makes tea every morning").
2. **The normal workflow.** What the user does, what goes in, what comes out.
3. **How to read the result.** What a dot, a number, a banner or an empty list means, and
   what to do about it.
4. **The states that are not the happy path.** Empty, unavailable, invalid, partial, late;
   and any limit that matters, like "one cheap rate per boil".
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

Start with what the user can see about things you do not control. The price comes from an
outside provider, so its health is a state of the vocabulary, with a `Shows:` for how the
kettle tells the user:

```gherkin
Vocabulary:
  State: the price feed is live | delayed | unavailable
    Set: switch the test provider to {value}
    Shows: live: the price has a green dot
    Shows: delayed: the price has an amber dot and says when it was last updated
    Shows: unavailable: the price is replaced by the message "Prices unavailable" and a Retry button
  State: the cheap boil is off | waiting | boiling
  Event: the feed stops answering
    Say: switch the test provider off
  Term: Price provider
    The outside service that sends the electricity price. The kettle only uses the price the provider sends.
    Reference: Provider API | https://example.com/docs/prices
```

The feature description explains the feature, links the term, and covers the unavailable state
in plain words:

```gherkin
Feature: Seeing the electricity price
  Approval: approved

  The smart kettle is for people who want to spend less on hot drinks, such as someone who
  makes tea every morning and would rather boil when power is cheap. They open the app and
  read the price. They use it to decide whether to boil now or wait.

  The price comes from an outside provider ([[Price provider]]). The kettle shows what the
  provider sends and does not work out prices by itself. Next to the price, a dot says how much
  to trust it. Green means the price is current. Amber means the provider is late, so the price
  is old: the app says when it was last updated, and you should treat the price as a guide only.

  If the provider does not answer, the app has no current price to show. It says so and does not
  show an old price as if it were new. When the provider is back, tap Retry to get the price again.

  Expectation: A silent provider means no price
    The app says "Prices unavailable" and shows no number. A price that might be hours old is worse than none.
    Given the price feed is live
    When the feed stops answering
    Then the price feed is unavailable
```

The undecided part is written down, not guessed. This expectation is `proposed`, and its
description says what the question is:

```gherkin
  Expectation: A delayed price can start the boil
    Approval: proposed
    Unresolved: should an old price start a boil? Boiling sooner may save money, but the real
    price may already have gone back up, and nobody has decided whether the kettle may start
    while the price is in doubt. Until it does, this is proposed and an approved manual leaves
    it out.
    Given the cheap boil is waiting
    And the price feed is delayed
    When the price drops to the cheap rate
    Then the cheap boil is boiling
```

And a story walks the journey, including the unavailable state, because that is when a reader
most needs to know what the kettle does:

```gherkin
Story: Boil when the power is cheap
  Approval: approved
  As a tea drinker
  I want the kettle to boil when electricity is cheap
  So that my morning cup costs less without me watching the price

  You make tea every morning and your power is cheaper at some hours. Instead of checking
  the price, you ask for a cheap boil. This tutorial also shows what happens when the price
  provider goes quiet, because that is when you most need to know what the kettle is doing.

  Scenario: Ask for a cheap boil, lose the provider, get the boil
    When the user asks for a cheap boil
    Then the cheap boil is waiting
    When the feed stops answering
    Then the price feed is unavailable
    And the cheap boil is waiting
    When the feed answers again
    Then the price feed is live
    When the price drops to the cheap rate
    Then the cheap boil is boiling
```

### What it generates

The feature page (`features/boiling-when-it-is-cheap.md`; the HTML site's page is the same)
opens with the description, then one section per expectation, with the *why*, the instruction,
and the test folded away:

> # Boiling when it is cheap
>
> A cheap boil tells the kettle to wait until the price is low, so you do not have to watch
> it. […]
>
> ## A cheap price starts the boil
>
> The kettle starts the moment a live price reaches your cheap rate.
>
> If the cheap boil is waiting and the price feed is live, have the test provider send a price
> at or below your cheap rate. The display reads HEATING and a banner says the price is cheap.

The tutorial turns the story into steps, with the setup first:

> # Boil when the power is cheap
>
> You make tea every morning […]
>
> ## Before you begin
>
> - Switch the test provider to live.
> - The cheap boil is off.
>
> ## Steps
>
> 1. **Fill the kettle, set the price you call cheap and tap Boil when cheap.** The Cheap boil
>    button reads "Waiting for a cheap price".
> 2. **Switch the test provider off.** The price is replaced by the message "Prices
>    unavailable" and a Retry button. The Cheap boil button reads "Waiting for a cheap price".
> 3. **Switch the test provider back on and tap Retry.** The price has a green dot.
> 4. **Have the test provider send a price at or below your cheap rate.** The display reads
>    HEATING and a banner says the price is cheap.

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
