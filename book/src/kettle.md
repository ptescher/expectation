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

`vocabulary.expect` declares every word the suite may use, how the manual words it, and where
every test starts (shortened here):

```gherkin
Vocabulary:
  State: the kettle is off | starting | heating | dry
    Shows: off: the display reads OFF and the light is grey
    Shows: heating: the display reads HEATING, the light is red and steam rises
    Shows: dry: the display reads DRY in red and says "Refill, then press reset"
  State: the dial is 0..100 %
    Set: turn the dial to {value}
  State: the preset is WARM | HOT | BOIL
    Shows: the PRESET line reads {value}
  State: the child lock is on | off
  Event: the user presses start
    Say: press START
  Event: the user presses reset
  Event: the user presses preset
  Event: the user presses lock
  # A sensor, not something the user does: the tests make it happen.
  Event: the water runs out
    Say: the kettle boils dry

  Phrase: the kettle is plugged in and full
    Say: fill the kettle and plug it in
    Given the kettle is off
    And the dial is 0 %
    And the preset is WARM
    And the child lock is off

Background:
  Given the kettle is plugged in and full
```

The dial is a number, so steps can compare it: `above 55 %`, `between 20 and 40 %`. The
others have named values. `Say:`, `Set:` and `Shows:` are what the manual tells the reader to
do and what they will see. "The water runs out" is an event without a person, so its `Say:`
describes what happens rather than telling the reader to do it. The phrase makes "Before you
begin" one line.

## Features and an exception

`heating.expect` says how the kettle starts and stops:

```gherkin
Feature: Heating
  The kettle starts off. Turn the [[dial]] past 55 % to start it, the way you would an old
  stovetop knob, or press START. It heats the water to the preset's temperature and holds it
  there; the [[display]] shows the water's temperature as it goes. RESET turns it off, but only
  with the dial down: a dial left up starts it again.

  If the water runs out while it is heating, the kettle stops and reads DRY until you refill it
  and reset it.

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
  --> child_lock.expect:16
   |   Expectation: The child lock holds the kettle while the dial turns
   = «The child lock holds the kettle while the dial turns» says the kettle is off
  --> heating.expect:10
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

`stories.expect` has four stories, one for each thing a person uses the kettle for. Each
becomes a tutorial, so its introduction explains what the reader needs to follow it: here, why
the dial goes back to 0 before unlocking.

```gherkin
Story: Keeping little hands off
  As a parent
  I want the child lock to keep the kettle off
  So that a curious toddler turning the dial doesn't start it

  LOCK turns the child lock on and off, and a CHILD LOCK sign shows while it is on. While it is
  on, the dial, START and PRESET do nothing, so a turned dial cannot start the kettle.

  The lock stops the dial from reaching the kettle; it does not move the dial. If the dial is
  still above the 55 % mark when you unlock, the kettle starts as soon as the lock comes off.
  So turn the dial down before you press LOCK again, as this tutorial does.

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

Three expectations have no story. `check` warns about each one, with a story that would reach it:

```text
warning[untested]: no story exercises «Unlocking with the dial up starts the kettle»
  --> child_lock.expect:30
   |   Expectation: Unlocking with the dial up starts the kettle
   = help: a story that would: When the user presses lock / When the dial is above 55 % / When the user presses lock
```

`Acceptance::with_coverage()` turns those routes into generated tests, so every expectation
is verified even if no story mentions it:

```text
✓ First cup › Set it to boil, turn it up, then turn it off
✓ Choosing a preset › Step to HOT, start, and turn it off
✓ Keeping little hands off › Turn the dial with the lock on, then unlock and boil
✓ When the kettle boils dry › Boil dry, then refill and reset
✓ Reaching «The child lock ignores the start button»
✓ Reaching «Unlocking with the dial up starts the kettle»
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

The generated test for «Unlocking with the dial up starts the kettle» fails the same way,
since it turns the dial up with the lock on before it unlocks. The failure names the step, the check, the expectation that promised it, and what the kettle
actually did. The steps after it are skipped, not failed. In the docs, that expectation is
marked failed, and so is the tutorial.

## The manual

The run writes a manual. The story became this tutorial, in the reader's words:

```markdown
# First cup

You want boiling water for a cup of black tea. The kettle starts the way an old stovetop
knob does: turn the dial past the dashed 55 % mark and hold it there for a moment. […]

## Before you begin

- Fill the kettle and plug it in.

## Steps

1. **Press PRESET.**

   The PRESET line reads HOT.

   <screenshot>

2. **Press PRESET.**

   The PRESET line reads BOIL.

3. **Turn the dial to 80 %.**

   The display reads HEATING, the light is red and steam rises.

4. **Turn the dial to 0 %. Then press RESET.**

   The display reads OFF and the light is grey.
```

Each feature is a topic page, and the test evidence is collapsed under "How this is tested".
The wording comes from the vocabulary's `Say:`, `Set:` and `Shows:`; without them, it is
worked out from the steps ("Set the dial to 80 %. The kettle is heating."). See
[Writing specs that read well](writing-specs.md) and [What a story needs](feature-docs.md#what-a-story-needs).

Browse the result: [the kettle manual](../example/site/index.html).
