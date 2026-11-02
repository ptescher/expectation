# Feature docs that explain

A suite can pass every check and still make a thin manual. The usual cause is a description
that lists test conditions and caveats and never says what the feature is for. This chapter
is the depth to aim for, a workflow to get there, how to audit the result, and a worked
example you can run:

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
might be hours old is worse than none").

## What a story needs

A story becomes a tutorial: the page a reader follows to get something done. It has to teach
the feature, not replay a test. A test needs steps that reach each state; a reader needs to
know why they are there, what they are looking at and what it means. For each story, cover:

1. **A real goal, and where the reader starts.** The narrative (`As a … I want … So that …`)
   is the reader's reason. The introduction says the situation, what they want to do or find
   out, and where in the product they begin: "You hold USDC and want to know who else holds
   it", not "Toggle the currency".
2. **The path someone with that goal would take.** Steps in the order a user would do them,
   ending when the goal is met. Reaching every state and transition is the expectations' job,
   and `plan --coverage` generates tests for the ones no story reaches. A story that flips a
   toggle back and forth to exercise it is a test sequence, not a tutorial.
3. **How it works.** The mechanism the reader relies on: where the data comes from, what the
   product does for them and what it does not, why a result can be late or missing. Say it in
   the introduction and link the feature and terms (`[[Price provider]]`) that say more.
4. **How to read what they see.** Units (SOL or dollars, per token or total), what a row,
   number, dot or badge means, and what to do next. The `Shows:` of each state is what a step
   says the reader sees; make it something they can check, and explain in the introduction or
   the linked feature what it tells them.
5. **What goes wrong along the way.** The empty, late or failed case a user is likely to meet on
   this journey, and what to do about it, as the kettle's main story does when the provider
   goes quiet.
6. **Where they end up.** The last step reaches the goal, worded as the reader sees it. A
   story's Thens are what the reader should notice, not everything that is true; the
   expectations check the rest.

Together, a feature's page and its stories explain the whole feature: each main goal it serves
has a story. The test for a story: could someone who has never seen the product follow the
tutorial on its own, and afterwards explain the feature to someone else? If they could only
repeat the taps, it is too thin.

## The product story, not the test rig

The main prose is the product as a user meets it: they sign in with Google, the provider goes
quiet, the price reaches their rate. It is not the mock OIDC server, the fixture account or the
switch a test flips. Put the machinery where contributors look for it: the driver, a comment in
the vocabulary, the test report or contributor docs. In the vocabulary, a `Say:` for something
the user does not cause describes what happens ("the price provider stops answering"), not how
a test makes it happen ("switch the test provider off").

Moving the machinery out of the story does not hide what it means. If a caveat changes what a
reader can conclude, keep it on the page in plain words: "the sign-in tests use a stand-in
provider, so they do not show that Google or Apple sign-in works". A caveat about how a test is
built belongs with the test; a caveat about what the product has been shown to do belongs with
the claim.

## An end-to-end example

Start with what the user can see about things you do not control. The price comes from an
outside provider, so its health is a state of the vocabulary, with a `Shows:` for how the
kettle tells the user. The tests make the provider fail through a test provider, but the
`Say:` words the step as what happens to the user; how a test fakes it is the driver's
business (see [The product story, not the test rig](#the-product-story-not-the-test-rig)):

```gherkin
Vocabulary:
  State: the price feed is live | delayed | unavailable
    Shows: live: the price has a green dot
    Shows: delayed: the price has an amber dot and says when it was last updated
    Shows: unavailable: the price is replaced by the message "Prices unavailable" and a Retry button
  State: the cheap boil is off | waiting | boiling
  Event: the feed stops answering
    Say: the price provider stops answering
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
> If the cheap boil is waiting and the price feed is live, wait for the price to reach your
> cheap rate. The display reads HEATING and a banner says the price is cheap.

The tutorial turns the story into steps, with the setup first:

> # Boil when the power is cheap
>
> You make tea every morning […]
>
> ## Before you begin
>
> - The price feed is live.
> - The cheap boil is off.
>
> ## Steps
>
> 1. **Fill the kettle, set the price you call cheap and tap Boil when cheap.** The Cheap boil
>    button reads "Waiting for a cheap price".
> 2. **The price provider stops answering.** The price is replaced by the message "Prices
>    unavailable" and a Retry button. The Cheap boil button reads "Waiting for a cheap price".
> 3. **When the price provider is back, tap Retry.** The price has a green dot.
> 4. **Wait for the price to reach your cheap rate.** The display reads HEATING and a banner
>    says the price is cheap.

Where the model has no `Say:` or `Shows:` for something, the page falls back to a plain
sentence made from the step. That is the cue to add one.

## The workflow

1. **Look at the product first.** Read the code, the UI, existing docs, tickets and the
   people who know it. Note what you can see and what you are assuming.
2. **Inventory the user goals.** List what people use the product to do (buy a token, find a
   holder, follow a wallet, sign in), from the product, not from the suite you are about to
   write. This is the [coverage map](#the-coverage-map); keep it up to date as you go.
3. **Write the explanation and the expectations together.** The description, the vocabulary's
   `Say:`/`Set:`/`Shows:`, the expectations and one story per user goal. A description
   written afterwards drifts from what is tested.
4. **Check.** `expectation check <dir>` must pass. Fix every error; read every warning.
5. **Plan with coverage.** `expectation plan <dir> --coverage` shows the tests each story
   becomes and the generated ones for expectations no story reaches. Add a story, or accept a
   generated test, for each `untested` warning. This is coverage of the *model*: it says
   nothing about goals the suite never mentions.
6. **Preflight anything that changes state, then run.** Before you run a journey that trades,
   pays, posts, follows, notifies or creates an account, do the
   [safety preflight](#safety-preflight). Then run what is safe to run.
7. **Generate the manual.** `expectation docs <dir> <out>`, with results from a run if you
   have them.
8. **Audit it.** Read the rendered pages [without the author's context](#the-zero-context-read),
   do the [evidence pass](#the-evidence-pass), and update the coverage map.
9. **Report three verdicts, separately.** See [Reporting](#reporting).

## Auditing a suite

Writing the pages and auditing them are separate jobs, and an audit has three parts that
answer different questions: is the product covered, can a reader follow the pages, and does the
evidence back the claims. Record their findings separately: missing features, unclear steps,
unsupported checks, simulated data, stale evidence and unsafe-to-run actions are different
problems with different owners. A passing test does not approve the prose, and an approved
specification does not prove the product works.

### The coverage map

Map each main user goal to where it is explained, shown, promised and evidenced:

| User goal | Feature page | Tutorial | Expectations | Evidence | Gap |
| --- | --- | --- | --- | --- | --- |
| Boil when power is cheap | Boiling when it is cheap | Boil when the power is cheap | 4 (1 proposed) | none: no driver | not run |
| Judge how current the price is | Seeing the electricity price | Knowing how far to trust the price | 4 | none: no driver | not run |
| Set a boil temperature | — | — | — | — | no page |

(The last row is invented, to show a gap.) Build the left column from the product (its screens, menus, API, release notes, support
tickets), not from the suite, or the map only confirms what you already wrote. List each goal
with no page, or with only part of it covered ("can see a quote, cannot submit the trade"), so
the gap is visible instead of implied by silence.

### The zero-context read

"Read it as a newcomer" fails when the author does it, because the author fills the gaps without
noticing. Give a fresh reader, ideally another agent or person without your context, one
rendered tutorial *on its own*, with its screenshots, and no other page. They answer:

1. **Where do I start?** Which screen, account and state; is anything assumed that the page
   does not set up?
2. **What do the numbers, units and symbols mean?** SOL or dollars, per unit or total, a dot,
   a badge, a percentage of what.
3. **What changed?** After each step, what is different on screen, and why it matters.
4. **What can I safely conclude?** And what can I *not* conclude from this page.
5. **What do I do if it is empty, slow or fails?** Is the non-happy path explained, or only
   tested?

Then they say whether the page teaches the user's goal or just walks a test: a tutorial that
toggles a currency back and forth five times passes and teaches nothing. Do the same for each
feature page, with its linked terms. Fix what they could not answer in the descriptions, the
`Say:`/`Shows:` and the stories, not in a note to the reviewer.

### The evidence pass

A screenshot or a "passed" proves less than it seems to. It can come from a step that failed,
show synthetic data, or mix fixtures with a live stream. For each claim on a page, say what
supports it:

- **Observed**: a run against the real system saw it, with results and screenshots saved.
- **Simulated**: a run saw it, but against a fixture, mock provider, test account or fake data.
  Say which part was simulated.
- **Source-inspected**: read in the code or docs, not run. Useful, not verified.
- **Unsupported**: the driver cannot do the step yet. It shows as "not supported yet", which is
  not "passing".
- **None**: an assumption. Say so, or ask.

Then check the evidence still matches the spec: results saved before the expectation, the
`Shows:` or the UI changed are stale, and a screenshot from a failed or skipped step does not
show the promised state. A screenshot proves what is in it, at that step, with that data; it
does not prove every claim on the page around it.

### Safety preflight

Some journeys change the world: a trade, a payment, a follow, a notification, a new account.
Before you run one, or offer someone a demo of one, answer:

- **What can it change?** Every side effect of every step, including the ones the story does not
  mention (a confirmation email, a webhook, an on-chain transaction).
- **Where does it go?** Which environment, account, network and provider receives it. Staging
  sign-in does not make the trade path go to staging.
- **What actually isolates it?** A fixture, a sandbox, a testnet, a binding that refuses, or
  only a convention. Check the code path; do not assume from a variable name.

If you cannot show it is isolated, do not run it. Use a safe fixture, or leave the step unbound
so it reports "not supported yet". A gap that says so is better than a "pass" from a run that
might have spent real money, and a demo must say plainly where its boundary is.

### Reporting

Report three verdicts, because each can pass while another fails:

1. **The specification**: is it complete and readable? The coverage map's gaps and the
   zero-context read's findings. `expectation check` and `plan --coverage` support this verdict
   but do not decide it: they check the model, not whether the manual teaches the product.
2. **The tests and observed behaviour**: what ran, what passed, what failed, what is not
   supported yet, and how much of it was observed and how much simulated. An unsupported step
   did not run successfully, whatever the rest of the run did.
3. **Approval and product status**: what is approved, what is proposed, what is
   `Status: pending`, and the open decisions with the question for each.

A tutorial can pass as a test and fail the documentation audit; it can read well and have no
evidence. Say which. Also list the assumptions you made and the claims with no evidence, and do
not present any of those as verified behaviour. Mark undecided behaviour `Approval: proposed`
and ask.

## Settled and proposed

`Approval: approved` and `Approval: proposed` mark what is agreed and what is not; see
[Approved-only manuals](manual.md#approved-only-manuals). A suite that does not use them is
unchanged. Approval is about the *specification*; it is separate from `Status: pending` (the
behaviour is not built yet) and from the test results.
