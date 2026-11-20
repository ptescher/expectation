# Spec first: a feature before it is built

Most of this guide describes a product that exists. This chapter is the other direction: the
suite says what a feature will do, someone agrees to it, and then it is built until the tests
pass. It suits a team where a person decides the behaviour and an AI agent, or another
developer, writes the code.

Three markers keep the three questions apart, and none of them stands in for another:

| Question | Marker | Who changes it |
| --- | --- | --- |
| Is the behaviour agreed? | `Approval: proposed`, then `approved` | The person who owns the decision |
| Is it built? | `Status: pending`, then removed | Whoever builds it |
| Does it work? | The run's result | The run |

## 1. Write what it will do

Add the feature to the suite as if it existed: its words in the vocabulary, a description for
the reader, its expectations and a story. Mark the feature and its story `Approval: proposed`,
and each expectation that isn't built `Status: pending`.

This adds a way to take an item out of the [cart example](https://github.com/ptescher/expectation/tree/main/examples/cart-node):

```gherkin
Vocabulary:
  Event: the shopper removes an item
    Say: press Remove next to the item

Feature: Changing your mind
  Approval: proposed

  You can take an item out of the cart again before you pay.

  Expectation: Removing an item takes it out of the cart
    Status: pending
    Given the checkout is closed
    And the cart has at least 1 item
    When the shopper removes an item
    Then the cart goes down by 1

Story: Take one back out
  Approval: proposed
  As a shopper
  I want to remove something I added by mistake
  So that I only pay for what I want

  When the shopper adds an item
  Then the cart has 1 item
  When the shopper adds an item
  Then the cart has 2 items
  When the shopper removes an item
  Then the cart has 1 item
```

Where the behaviour is not decided, don't pick an answer. Write the question in the
description, starting with the word `Unresolved:` so it is easy to find, and ask. `Unresolved:`
is not a keyword: it is ordinary description text, and the manual prints it as written.

## 2. Check it against what is already there

```bash
expectation check expectations
expectation plan expectations --coverage
```

A pending expectation is part of the model, so this is where the new feature meets the old
ones, before any code exists:

- A **conflict** means the new behaviour and an existing one both apply somewhere and promise
  different things. That is a product question: one is an exception (`Overrides:`), or a Given
  is too wide.
- `unspecified` or `unclear-step` on the story means it leans on behaviour nobody wrote down.
- `unreachable` means nothing gets the product into the state the feature starts from.

`plan` prints the test each story will become. Read it: it is what "done" will mean.

## 3. Get it agreed

Generate the manual and give the feature's page and its tutorial to whoever decides:

```bash
expectation docs expectations docs
```

They read prose and numbered steps, not Gherkin. When they agree, they (or you, on their word)
change `Approval: proposed` to `Approval: approved`. An agent does not approve its own
specification. A suite can keep drafts and agreed behaviour side by side;
`expectation docs expectations out --approved-only` writes the agreed part alone.

## 4. Build it

Now write the code, and bind the new words in the driver (`.on("the shopper removes an
item", …)`). While the expectation is pending, a test that depends on it is not run, and a run
reports it apart from the rest:

```text
✓ Buy two things
– Take one back out
✓ Reaching «An empty cart has nothing to check out»
```

The run exits 0: `–` is "pending", not "passed". Tests of everything else still run, so the
suite keeps catching what the new code breaks.

## 5. Take the pending mark off

When the behaviour is built, delete its `Status: pending` line and run again. The story is now
a real test. It passes, or it fails with the check that didn't hold, or it is "not supported
yet" because the driver doesn't bind a word it needs.

## 6. Gate on it

A plain run passes when nothing failed, which is right while a feature is under way and wrong
for a release. Ask for more with a policy (see [A run as a gate](drivers.md#a-run-as-a-gate)):

| When | Run with | Passes when |
| --- | --- | --- |
| Every pull request | the default | nothing failed |
| Before merging a finished feature | `--strict` (`Policy::Required`) | every test that isn't pending ran and passed |
| Before a release | `--strict=all` (`Policy::All`) | every test ran and passed, and nothing is pending |

With the pending story above, `--strict` exits 0 and `--strict=all` exits 1:

```text
Not every test ran and passed (--strict):
– pending: Take one back out (Pending: it depends on «Removing an item takes it out of the cart», which isn't built yet.)
```

[In CI](ci.md) has a workflow that does this.

## For an agent building the feature

- The specification is the task. Don't change an approved expectation to make a test pass; if
  the specification is wrong, say so and ask.
- Remove `Status: pending` only for behaviour you built in this change, and run the suite
  after you do.
- A word the driver doesn't bind makes a test "not supported yet", which a default run lets
  by. Bind it, or say plainly that it isn't bound.
- Report what the run showed, test by test: passed, failed, pending, not supported yet. An
  exit code of 0 is not that report.
