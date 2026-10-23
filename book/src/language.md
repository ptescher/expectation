# The language

A suite is every `*.expect` file under a directory. The format is line oriented, like
Gherkin. A line's first word decides what it is, a line starting with `#` is a comment, and
indentation is only for people.

```gherkin
Vocabulary:
  State: the kettle is off | starting | heating | dry
  State: the dial is 0..100 %
  State: the preset is WARM | HOT | BOIL
    # How the docs say to change it:
    Set: press PRESET until the display reads {value}
    # What the reader sees:
    Shows: the display reads {value}
  Event: the user starts the kettle
    # How the docs say to do it:
    Say: press START

  # A setup, in one line:
  Phrase: the kettle is plugged in and full
    Say: fill the kettle and plug it in
    Given the kettle is off
    And the dial is 0 %
    And the preset is WARM

  Phrase: the user starts the kettle with the dial
    Say: turn the dial past 55 % and hold it there
    When the dial is 80 %

Background:
  Given the kettle is plugged in and full

Feature: Heating
  # Free text before the first step is a description:
  The kettle starts off.

  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

  Expectation: START starts the kettle
    Given the kettle is off
    When the user starts the kettle
    Then the kettle is heating

Story: Your first cup
  As a tea drinker
  I want to start the kettle by turning the dial
  So that it works like the one I already have

  # The tutorial's introduction:
  Every kettle in the range has the same dial.

  Scenario: Dial up
    When the user starts the kettle with the dial
    Then the kettle is heating
```

## Suites and sections

An optional `Suite: <name>` line names the suite. Otherwise it's named after its directory.

Each directory under the suite is a **section**: a nested set of features that share a setup.
Its `Section: <name>` line (in any of its files) and the text under it name and describe it.
A leading `01-` in a directory name only sets the order.

A section's `Background:` is its setup, written over its parent's. So everything in
`app/drawing/` starts where `app/` does, with the pen picked on top. Pages about a section
start from that setup and don't re-explain how to get there, the way a manual assumes you've
read the chapter before.

A section's `Launch:` line, right under its `Section:` line, says how to start the program
under test: `Launch: run cargo run -p kettle`. It is always shown first under "Before you begin"
and never as a numbered step, every sub-section inherits it, and a sub-section's own `Launch:`
replaces it (`Launch[iOS]:` for another platform). Drivers get it as `Driver::launch`, called
once for the tests that share it, so a runner doesn't start its own server. A `Say:` that holds
a shell command in backticks (one that starts with a program name or a path, like `` `cargo run` `` or `` `./app` ``) gets a `launch-in-step` warning.

Each top-level section is a system of its own: stories and the flow graph only use the
expectations in their own top-level section.

## Vocabulary

| Line | Declares |
| --- | --- |
| `State: <subject> is <a> \| <b> \| …` | A state with named values. |
| `State: <subject> is <min>..<max> [unit]` | A number, optionally with a unit. |
| `Event: <phrase>` | Something that happens. |
| `Phrase: <name>` | A step of your own, made of other steps. |
| `Term: <name>` | A word for the glossary, defined by the text under it. |

A state can use `are`, `has` or `have` instead of `is` ("the drawing has 0..650 shapes"),
and steps about it read the same way. Vocabularies from all files merge into one, so a
section can declare the words only it uses.

### How the docs say things

Name events for what the user means, not the button they press: "the user starts the
kettle", not "the user presses START". The button goes in lines under the declaration:

| Line | Under | Says |
| --- | --- | --- |
| `Say: <instruction>` | an event or phrase | What the docs tell the reader to do: "Press START." |
| `Set: <instruction with {value}>` | a state | How to change it: "press PRESET until the display reads {value}". |
| `Shows: <value>: <what you see>` | a state | What the reader sees when it has that value: `Shows: dry: the display says DRY`. |
| `Shows: <text with {value}>` | a state | The same for any exact value: `Shows: the counter reads {value} SHAPES`. |

For a number, a `Shows:` can start with a comparison as steps write it: `at least 99 %:`.

Without these, the docs make do. "The user clicks the canvas" becomes "Click the canvas",
"the water runs out" becomes "Wait until the water runs out", and a state becomes "Set the
dial to 80 %".

### Phrases

A phrase is a step of your own, for saying more with less.

- A phrase of `Given`s is a **setup** ("the kettle is plugged in and full"). Use it in a
  Background, a Given, or at the start of a scenario.
- A phrase of `When`s is a **sequence of actions** ("the user draws two shapes", one `When`
  per action). An expectation can use one that's a single action; a story step can use any.
  It becomes several test steps, but stays one numbered step in the docs.

A phrase's `Say:` is how the docs put it. Phrases can use the phrases declared before them.

## Steps

A step is either an event or a fact `<subject> is [not] <predicate>`. The predicate is:

- one or more values: `off`, `off or heating`, `not dry`;
- for numbers, a comparison: `80 %`, `above 55 %`, `below 10`, `at least 20`, `at most 40`,
  `between 20 and 40 %`.

Matching ignores case, extra spaces and a leading "the", and "1 shape" matches a unit of
"shapes". A step outside the vocabulary is an error, with a "did you mean" hint.

An expectation's Then can also be **relative**: `the drawing goes up by 1`, `goes down by
10 m`, `stays the same`.

## Expectations

```gherkin
Expectation: <name>
  <description>
  Overrides: <another expectation>
  Hands over to: <a setup>
  Given …
  When …
  Then …
```

- `Given` is the state before. Several are joined with `And`.
- `When` is at most one event, plus any states the outside world changes (such as the dial).
- `Then` is the state after. States the Then doesn't mention keep their values.
- An expectation with no `When` is an **invariant**: it holds whenever its Given does.
- `Overrides: <name>` makes it an exception to another expectation: where both apply, this
  one wins. Overrides must form a DAG.
- `Hands over to: <setup>` says that when this fires, a different program (or part of one)
  takes over, and it starts in that setup: the launcher starts the sim, so afterwards the sim
  is "just started" and nothing of the launcher's buttons carries over. A section whose
  Background is that setup is then reached from here instead of being a start of the state
  map. Tests don't check it; they check the expectation's Then.
- `Docs: hidden` keeps it off its topic page. It's still tested, and it's in the test report.

Text before the first step is the expectation's description.

## Stories

```gherkin
Story: <name>
  As a <someone>
  I want <something>
  So that <a reason>

  <introduction>

  Scenario: <name>
    Given …
    When …
    Then …
```

- The narrative is the "As a …", "I want …", "So that …" lines.
- Any other text is the introduction, which opens the tutorial in the docs.
- A story has one or more `Scenario:`s. Steps written straight under the story form a single
  scenario.
- A scenario may start with `Given`s, which are arranged on top of the Background.
- After that, each `When` starts a step, `And`/`But` add to it, and its `Then`s are what the
  story checks afterwards.

## Links and the glossary

`Term: <name>` and the text under it define a word for the glossary. Any text can link to a
term, a section, a feature, an expectation or a story with `[[its name]]`, and mark code with
backticks.

## Platforms

`Platform:`, `On:` and the `[platform]` tags are covered in [Platforms](platforms.md).
