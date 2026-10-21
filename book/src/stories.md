# From stories to acceptance tests

A story describes a journey. It doesn't have to say everything that happens along the way,
because the expectations already do. Planning walks each scenario through the expectations,
and turns it into a test **plan**: what to arrange, what to do at each step, and what to
check after it.

## What fires

An expectation **fires** at a step when the current state implies its Given, and the step's
action implies its trigger.

- In a compound action ("When the dial is 0 % and the user presses reset"), the other changes
  happen first, then the event.
- When an exception and the expectation it overrides both fire, only the exception's Then
  applies.
- After a step, the state is the old state with the Thens of what fired written over it.

## What's checked

Each step's checks are:

- the Thens of the expectations that fired,
- the Thens of invariants now in force,
- and the story's own Thens.

The story never has to restate what the expectations promise. The tests check it anyway, so
a story that only says "When the dial is 80 %" still checks that the kettle is heating.

## What's rejected

Planning is strict, because a story that relies on unspecified behaviour would test
something nobody agreed on.

| Code | | When |
| --- | --- | --- |
| `unclear-step` | error | The step *might* set off an expectation, but the story doesn't pin down whether. For example "the dial is at least 50 %" when an expectation needs "above 55 %". |
| `unspecified` | error | An event happens and no expectation says what it does. |
| `contradicts` | error | One of the story's Thens contradicts what the expectations say. |
| `clash` | error | Two expectations fire and disagree, and nothing overrides. |
| `unpromised` | warning | One of the story's Thens isn't promised by any expectation. It's checked anyway. |

## Where a test starts

A test starts from its section's setup (the Backgrounds, from the root down), with its
scenario's Givens written over it. The runner **arranges** that state before the first step,
choosing a value inside each constraint: the first allowed name, or the middle of a range.

## Coverage

An expectation that no story exercises gets an `untested` warning, with a route that would
reach it:

```text
warning[untested]: no story exercises «Running dry stops the kettle»
   = help: a story that would: When the dial is above 55 % / When the water runs out
```

`Acceptance::with_coverage()` (and `expectation plan --coverage`) adds a generated test for
each one, following that route from the expectation's own section setup. Generated tests are
named "Reaching «…»". They appear in the test report, but not as tutorials.

With coverage, every expectation gets a **verdict**:

| Verdict | Means |
| --- | --- |
| verified | Every check it promised passed in at least one test, and none failed. |
| failed | A check it promised failed. |
| not supported yet | The tests that reach it can't run yet: the driver can't do a step, or a service isn't available. |
| untested / not run | No test reaches it, or the tests haven't run. |
