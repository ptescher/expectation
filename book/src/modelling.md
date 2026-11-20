# Modelling an app with data

The vocabulary has two kinds of state: one of a few named values (`locked | unlocked`), and a
number in a range (`0..20 items`). There are no strings, lists or records. That is what lets
`expectation check` compare every expectation with every other one, and it is enough for
products that are mostly data (invoices, tasks, users), once you know how to say them.

The rule: **a state is something the user would notice changing, sorted into the cases the
product treats differently.** The exact data stays in the driver.

## A list is a count

Don't model each record. Model how many there are, and say changes relatively:

```gherkin
Vocabulary:
  State: the account has 0..50 invoices

Feature: Invoices
  Expectation: Creating an invoice adds a draft
    Given the viewer is not signed out
    And the account has below 50 invoices
    When the viewer creates an invoice
    Then the account goes up by 1
    And the invoice is a draft
```

- Keep the range small. It only has to hold the cases that differ (none, one, some, full), not
  what the database allows. A wide range makes `check` print "search truncated", which is
  harmless: the search stops at its depth limit.
- **A relative Then needs room.** "Goes down by 1" from 0 is impossible, so say `And the
  account has at least 1 invoice` in the Given of anything that removes one, and `below 50`
  in anything that adds one. A generated test keeps the Background's value wherever the Given
  allows it, so a Given that allows the Background's 0 gives a test that can't pass.

## The record in front of the user is a state

"The invoice" is the one the reader is looking at. Its life is a named state, and `missing`
(or `none`) is a value like any other:

```gherkin
  State: the invoice is missing | a draft | sent | paid
```

Values can be more than one word. The driver decides which real record "the invoice" is:
usually the one it created in `arrange`, or the newest.

When a journey really involves two records at once, name each by its role, not by a number:
"the sender's balance" and "the recipient's balance", not "account 1" and "account 2".

## Typed-in text is the case it falls in

A text field can hold anything, but the product only does a few different things with it.
Model those:

```gherkin
  # What was typed, sorted by what the product does with it. The driver picks a real value
  # for each: "" for empty, "not-an-email" for malformed, "pat@example.com" for valid.
  State: the recipient's email is empty | malformed | valid

  Expectation: A draft with a valid recipient can be sent
    Given the invoice is a draft
    And the recipient's email is valid
    When the viewer sends the invoice
    Then the invoice is sent

  Expectation: A draft without a valid recipient stays a draft
    Given the invoice is a draft
    And the recipient's email is empty or malformed
    When the viewer sends the invoice
    Then the invoice is a draft
```

A story sets it like any state the outside world changes: `When the recipient's email is
valid`. Say how the reader does that with a `Set:` line (`Set: valid: type the customer's
email address`). The same works for a search with `no | some | too many` results, a password
that is `too short | accepted`, or an upload that is `too large`.

## Who is looking is a state

Roles and permissions are a state, and a rule for one role is an ordinary expectation whose
Given names it:

```gherkin
  State: the viewer is signed out | a member | an admin

  Expectation: Only an admin deletes a sent invoice
    Given the viewer is an admin
    And the invoice is sent
    And the account has at least 1 invoice
    When the viewer deletes the invoice
    Then the invoice is missing
    And the account goes down by 1

  Expectation: A member cannot delete a sent invoice
    Given the viewer is a member
    And the invoice is sent
    When the viewer deletes the invoice
    Then the invoice is sent
```

The two Givens don't overlap, so there is no conflict. If a rule is "anyone can, except…",
write the general expectation and make the exception `Overrides:` it.

## Things you don't control

A payment provider, a mail service or a price feed is a state of its health (`live | delayed
| unavailable`) with an event for it failing. [Feature docs that
explain](feature-docs.md#an-end-to-end-example) works one through.

## What to leave out

- **Field-by-field content.** That the invoice shows the right total is a unit test's job.
  The suite says a total is shown, or that it is `correct | stale` if the product can show a
  stale one.
- **Every validation rule.** One state with the cases that behave differently. Ten ways to be
  malformed are still `malformed`.
- **Internals.** No "the cache is warm", "the job is queued", unless the user sees it.

If a part of the product needs none of this, such as a pure calculation, it doesn't need to be
in the suite.

## What the driver does with it

The driver turns each case into real data, and that is the only place the data lives:

```js
.arrange('the account', async n => { await db.reset(); await db.seedInvoices(n); })
.arrange('the invoice', async v => { if (v !== 'missing') current = await db.createInvoice({ status: statusOf(v) }); })
.arrange('the recipient\'s email', v => { email = { empty: '', malformed: 'not-an-email', valid: 'pat@example.com' }[v]; })
.observe('the account', () => db.countInvoices())
.observe('the invoice', async () => nameOf(await db.find(current?.id)))
```

`arrange` gets a value inside what the test's start asks for: the first name allowed, or a
number in the range. `observe` must give back exactly a declared value: a name as a string, a
count as a number.

## Several areas of one product

Billing, search and settings rarely share states. Give each its own directory under the suite:
each top-level section is a system of its own, with its own Background, stories and flow
graph. When two teams would otherwise pick the same words, `Scopes: sections` gives each
section its own vocabulary (see [The language](language.md#scoping-the-words-of-a-large-suite)).
