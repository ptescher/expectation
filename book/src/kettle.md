# A worked example: the kettle

The [`kettle` example](https://github.com/ptescher/expectation/tree/main/crates/harness/examples/kettle)
is a suite of five files about an electric kettle, run against a toy kettle with time in it.
This chapter walks through it: the vocabulary, a conflict and its exception, stories,
generated coverage, a failing run, and the manual it writes.

```bash
cargo run -p expectation_harness --example kettle
```

The manual it writes is published here: [the kettle manual](../example/site/index.html).

## The vocabulary

`vocabulary.expect` declares every word the suite may use, and where every test starts:

```gherkin
Vocabulary:
  State: the kettle is off | starting | heating | dry
  State: the dial is 0..100 %
  State: the preset is WARM | HOT | BOIL
  State: the child lock is on | off
  Event: the user presses start
  Event: the user presses reset
  Event: the user presses preset
  Event: the user presses lock
  Event: the water runs out

Background:
  Given the kettle is off
  And the dial is 0 %
  And the preset is WARM
  And the child lock is off
```

The dial is a number, so steps can compare it: `above 55 %`, `between 20 and 40 %`. The
others have named values. "The water runs out" is an event without a person: the docs will
say "wait until the water runs out" instead of telling the reader to do it.

## Features and an exception

`heating.expect` says how the kettle starts and stops:

```gherkin
Feature: Heating
  The kettle starts off. Turn the dial past 55 % to start it, the way you would an old
  stovetop knob, or press start.

  Expectation: Turning the dial up starts the kettle
    Holding the dial above 55 % for a moment starts it.
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating
  ...
```

`child_lock.expect` adds a child lock, which keeps the dial from doing anything:

```gherkin
  Expectation: The child lock holds the kettle while the dial turns
    Given the kettle is off
    And the child lock is on
    When the dial is above 55 %
    Then the kettle is off
```

Both expectations apply when the kettle is off, the lock is on and the dial goes above 55 %,
and they say opposite things. `expectation check` refuses the suite:

```text
error[conflict]: «The child lock holds the kettle while the dial turns» and «Turning the dial up starts the kettle» disagree about the kettle
  --> child_lock.expect:15
   |   Expectation: The child lock holds the kettle while the dial turns
   = «The child lock holds the kettle while the dial turns» says the kettle is off
  --> heating.expect:5
   |   Expectation: Turning the dial up starts the kettle
   = «Turning the dial up starts the kettle» says the kettle is heating
   = note: both apply when the kettle is off and the child lock is on, and the dial is above 55 %
   = help: «The child lock holds the kettle while the dial turns» is the more specific; if it's an exception, add `Overrides: Turning the dial up starts the kettle` to it. Otherwise narrow a Given so they can't both apply
```

It is an exception, so the example says so:

```gherkin
  Expectation: The child lock holds the kettle while the dial turns
    Overrides: Turning the dial up starts the kettle
    Given the kettle is off
    And the child lock is on
    When the dial is above 55 %
    Then the kettle is off
```

Now the suite checks, and the docs for «Turning the dial up starts the kettle» carry a note:
"**Except** when the child lock is on: see The child lock holds the kettle while the dial
turns."

## Stories

`stories.expect` has three stories. Here's one:

```gherkin
Story: Keeping little hands off
  As a parent
  I want the child lock to keep the kettle off
  So that a curious toddler turning the dial doesn't start it

  Scenario: Turn the dial with the lock on, then unlock and boil
    When the user presses lock
    Then the child lock is on
    When the dial is 100 %
    Then the kettle is off
    When the dial is 0 %
    And the user presses lock
    Then the child lock is off
    When the dial is 80 %
    Then the kettle is heating
```

`expectation plan` shows the test it compiles to. Every check comes from an expectation that
fires at that step, so the story's own Thens are only a statement of what the reader should
see:

```text
Keeping little hands off › Turn the dial with the lock on, then unlock and boil
  arrange  the kettle is off, the dial is 0 %, the preset is WARM and the child lock is off
  1. when   the user presses lock
       fires  «Lock turns the child lock on»
       check  the child lock is on  («Lock turns the child lock on»)
  2. when   the dial is 100 %
       fires  «The child lock holds the kettle while the dial turns»
       check  the kettle is off  («The child lock holds the kettle while the dial turns»)
  3. when   the dial is 0 % and the user presses lock
       fires  «Lock again turns it off»
       check  the child lock is off  («Lock again turns it off»)
  4. when   the dial is 80 %
       fires  «Turning the dial up starts the kettle»
       check  the kettle is heating  («Turning the dial up starts the kettle»)
```

A story can't wander off the specification. Without the `Overrides:` line above, step 2 is
an error too, because two expectations fire and disagree. A step where nothing is expected
to happen is `unspecified`, and a Then that contradicts the expectations is `contradicts`.

## Coverage

Four expectations have no story. `check` warns about each one, with a story that would reach it:

```text
warning[untested]: no story exercises «Running dry stops the kettle»
  --> heating.expect:16
   |   Expectation: Running dry stops the kettle
   = help: a story that would: When the dial is above 55 % / When the water runs out
```

`Acceptance::with_coverage()` turns those routes into generated tests, so every expectation
is verified even if no story mentions it:

```text
✓ First cup › Turn it up, then turn it off
✓ Keeping little hands off › Turn the dial with the lock on, then unlock and boil
✓ Choosing a preset › Step to HOT and start
✓ Reaching «The child lock ignores the start button»
✓ Reaching «Running dry stops the kettle»
✓ Reaching «HOT to BOIL»
✓ Reaching «BOIL back to WARM»
```

## The driver

[`world.rs`](https://github.com/ptescher/expectation/blob/main/crates/harness/examples/kettle/world.rs)
is the toy kettle. Like a real one, it takes time: the dial has to stay above 55 % for 0.6 s
before the kettle starts, and starting takes another 0.22 s. The driver binds each term and
says how time passes:

```rust
Bindings::new(World::new(lock_leaks))
    .observe("the kettle", |w| w.phase_name())
    .observe("the dial", |w| w.dial)
    .set("the dial", |w, v| w.dial = v.as_f64().unwrap())
    .arrange("the kettle", |w, v| /* put it in that phase */)
    .on("the user presses lock", World::press_lock)
    // ...
    .on_settle(|w| (0..20).for_each(|_| w.tick(0.05)))   // a second after each action
    .on_advance(|w| { w.tick(0.1); Progress::Advanced }) // then 0.1 s per extra look
    .on_screenshot(|w| Screenshot::svg(w.svg()))
```

Settling a full second after each action is what makes "Then the kettle is off" mean it
*stays* off, not that it was off for the instant before it switched on. See
[Drivers](drivers.md).

## A failing run

`--bug` seeds a defect: the child lock lets the dial through.

```bash
cargo run -p expectation_harness --example kettle -- out --bug
```

```text
✗ Keeping little hands off › Turn the dial with the lock on, then unlock and boil
    at: When the dial is 100 % Then the kettle is off
    expected the kettle is off («The child lock holds the kettle while the dial turns»); observed heating
```

The failure names the step, the check, the expectation that promised it, and what the kettle
actually did. The steps after it are skipped, not failed. In the docs, that expectation is
marked failed, and so is the tutorial.

## The manual

The run writes a manual. The story became this tutorial, in the reader's words:

```markdown
# First cup

## Before you begin

- The kettle is off.
- The dial is 0 %.
- The preset is WARM.
- The child lock is off.

## Steps

1. **Set the dial to 80 %.**

   The kettle is heating.

   <screenshot>

2. **Set the dial to 0 %. Then press reset.**

   The kettle is off.
```

Each feature is a topic page, and the test evidence is collapsed under "How this is tested".
The vocabulary here has no `Say:`, `Set:` or `Shows:` lines, so the wording is worked out
from the steps. With them, step 1 could read "Turn the dial past halfway. The red light
comes on." See [Writing specs that read well](writing-specs.md).

Browse the result: [the kettle manual](../example/site/index.html).
