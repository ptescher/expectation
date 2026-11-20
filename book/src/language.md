# The language

A suite is every `*.expect` file under a directory. The format is line oriented, like
Gherkin. A line's first word decides what it is, and indentation is only for people.

A line starting with `#` is a comment. There are no comments at the end of a line: a `#` after
the text is part of it, so `State: the door is open | shut   # the two` gives the door a value
called `shut   # the two`. `expectation check` warns about it (`trailing-comment`).

[Every keyword](#every-keyword) at the end lists each line, where it goes and what it can say.

```gherkin
Section: Kettle
  Launch: run cargo run -p kettle

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
  # Free text before the first expectation is a description:
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

A **setup** is a state the tests can arrange the system in, written as `Given`s: a section's
Background, or a phrase made only of `Given`s. `Follows:` and `Hands over to:` name one.

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
| `Reference: <label> \| <url>` | In a Term: a link to outside documentation. |

A state can use `are`, `has` or `have` instead of `is` ("the drawing has 0..650 shapes"),
and steps about it read the same way. The subject can hold one of those words itself: in
`State: the page that has focus is home | settings` the subject is "the page that has focus",
because the verb that counts is the last one before the first value.

A state with named values has 2 to 64 of them. A value can't contain ` or `, which steps use to
list values. It can start with `not` ("the job is not started | running | done"), and
`the job is not started` then names it, unless `started` is a value too, which is an error
because the step could mean either.

A number's range can use decimals and negative numbers (`-20..40 °C`), and its minimum must be
below its maximum.

Vocabularies from all files merge into one, by name, so a section can declare the words only
it uses.

### Scoping the words of a large suite

In a big suite with several teams, merging by name means two sections that each declare "the
user is signed in" quietly get one state, and `expectation check` warns about it
(`vocabulary-collision`). To give each top-level section its own words, say so once in a
Vocabulary block in the suite's top directory:

```gherkin
Vocabulary:
  Scopes: sections
  # Shared, because it is declared in the top directory:
  State: the user is signed in | out
```

Then:

- Words declared in a file under a top-level section (`billing/`, `search/`, …) belong to that
  section and the sections inside it. Two sections can declare "the invoice is open | paid" and
  "the invoice is 0..5" without meeting.
- Words declared in the top directory are shared, and every section can use them.
- A section can't declare a word that is also shared (`vocabulary-shadow`), and a step can't use
  another section's word (it is an unknown step).
- Two sections that declare the same state the same way get a `duplicate-word` warning: if it is
  one word, move it to the top directory. A section's own word counts as used only by its own steps.
- Stories and the flow graph already stay within a top-level section, so nothing else changes.

A suite without `Scopes:` behaves as before.

### How the docs say things

Name events for what the user means, not the button they press: "the user starts the
kettle", not "the user presses START". The button goes in lines under the declaration:

| Line | Under | Says |
| --- | --- | --- |
| `Say: <instruction>` | an event or phrase | What the docs tell the reader to do: "Press START." |
| `Set: <instruction with {value}>` | a state | How to change it: "press PRESET until the display reads {value}". |
| `Set: <value>: <instruction>` | a state with named values | How to change it to that value: `Set: lost: switch off the radio`. |
| `Shows: <value>: <what you see>` | a state | What the reader sees when it has that value: `Shows: dry: the display says DRY`. |
| `Shows: <text with {value}>` | a state | The same for any exact value: `Shows: the counter reads {value} SHAPES`. |

For a number, a `Shows:` can start with a comparison in any form a step accepts (see
[Steps](#steps)): `at least 99 %:`, `over 50 %:`. A `Shows:` can also start with a change, as a
relative Then writes it: `goes up by 1:`.

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

A phrase can't mix `Given`s and `When`s (`bad-phrase`). Its `Say:` is how the docs put it. A
phrase can use any other phrase, wherever in the suite that one is written; two that use each
other are an error.

## Steps

A step is an event, a phrase, or a fact `<subject> is [not] <predicate>`.

The negative forms are `is not`, `are not`, `isn't`, `aren't`, `doesn't have` and `don't have`.

For a state with named values, the predicate is one or more values: `off`, `off or heating`,
`not dry`.

For a number, the predicate is the number itself or a comparison:

| Write | Means |
| --- | --- |
| `80 %` | exactly 80 |
| `above 55 %`, `over`, `more than`, `greater than`, `higher than` | more than 55, not 55 itself |
| `below 10`, `under`, `less than`, `fewer than`, `lower than` | less than 10, not 10 itself |
| `at least 20` | 20 or more |
| `at most 40` | 40 or less |
| `between 20 and 40 %` | 20 to 40, both included |

The unit is optional. A number can't be negated: write `below 5` or `above 5`, not `not 5`.

Matching ignores case, extra spaces, a full stop at the end and a leading "the", "a" or "an".
A unit matches its plural, so "1 shape" matches a unit of "shapes". A unit of one letter has no
plural: with a unit of `ms`, `5 m` is an error. A step outside the vocabulary is an error, with a
"did you mean" hint.

An expectation's Then can also be **relative**: `the drawing goes up by 1`, `goes down by
10 m` (for numbers), or `stays the same` (for any state). Relative steps only go in an
expectation's Then (`misplaced-shift`).

## Expectations

```gherkin
Expectation: <name>
  Approval: approved | proposed
  On: <platform>, …
  Status: pending
  Docs: hidden
  Overrides: <another expectation>
  Hands over to: <a setup>
  <description>
  Given …
  When …
  Then …
```

Every line but the name and a Then is optional.

- `Given` is the state before. Several are joined with `And` (or `But`).
- `When` is at most one event, plus any states the outside world changes (such as the dial).
  Several `When` lines make one action, so two events need two expectations (`two-events`).
- `Then` is the state after. States the Then doesn't mention keep their values.
- The steps go Given, When, Then, in that order, and can't start with `And` or `But`.
- An expectation with no `When` is an **invariant**: it holds whenever its Given does.
- `Docs: hidden` keeps it off its topic page. It's still tested, and it's in the test report;
  links to it from the maps and test pages go to a test that covers it.
- `Approval:` says whether the specification itself is agreed. See [Approval](#approval).
- `On:` limits it to some platforms. See [Platforms](platforms.md).

Text before the first step is the expectation's description. A line there that starts with
`Given`, `When`, `Then`, `And` or `But` is a step, so start description sentences with another
word.

### Exceptions

`Overrides: <name>` makes an expectation an exception to another: where both apply, this one
wins. An expectation can override several, one line each, and overrides must form a DAG
(`override-cycle`).

### Handing over to another program

`Hands over to: <setup>` says that when the expectation fires, a different program (or part of
one) takes over, and it starts in that setup: the launcher starts the sim, so afterwards the sim
is "just started".

A section whose Background is that setup is then reached from here instead of being a start of
the state map. The state map goes on from the setup alone (the launcher's buttons say nothing
about the sim). Stories and tests don't see the hand-over: they stay in the launcher, and check
the expectation's Then.

### Written before it is built

`Status: pending` says the behaviour is written ahead of its implementation. A pending
expectation stays in the model: it appears in the map and the state map, it can be reached, and
conflicts with it (pending or required) are still errors.

What changes is testing. A test that depends on it, because it fires it or because it is an
invariant in force there, is not run. The report shows that test as **pending**, which is
neither passed, failed nor "not supported yet" (that one means the driver lacks a binding, and
is still reported as such for everything else). Pending tests don't fail the run and never count
as verified, and the expectation's own verdict is `pending`. Failures in the other tests still
fail the run.

Remove the line when the behaviour is built. Because an invariant applies wherever its Given
holds, a pending invariant makes the tests in its section pending, so prefer pending transitions.

## Approval

`Approval: approved` or `Approval: proposed` goes right under a `Feature:`, `Expectation:` or
`Story:` line (before its description or steps), once, and says whether the specification
itself has been agreed. It is for teams that keep drafts and agreed behaviour in one suite, and
it changes nothing about checking, planning or running.

- An expectation without its own `Approval:` has its feature's. Nothing says it, and
  unmarked counts as not approved when an approved-only manual is asked for (see
  [the manual](manual.md#approved-only-manuals)).
- An approved expectation inside a proposed feature is an error (`approval-conflict`).
- It is separate from `Status: pending`. A pending expectation is agreed but not built; a
  proposed one may be built and passing but is not agreed.

## Backgrounds that follow a setup

A section's Background can say which setup it is reached from:

```gherkin
Background:
  Follows: the launcher has just opened
  Given the station is connected to the TX15
  And the other plane is silent
```

The section is reached from that setup, not a start of the state map: only the setup that
nothing leads to is. Its "Before you begin" starts from the section whose setup it follows.

How it's reached depends on whether the expectations can say:

- **They can.** Where a chain of expectations gets from the followed setup to this one (a
  website's home page has a link to billing), that chain is the way in. The state map shows
  its real moves, and the docs give its steps. With coverage on, a generated test
  "Reaching «Billing»" arranges the followed setup, takes the route and checks everything the
  section's setup says. The section's own tests arrange its setup directly (a deep link), so
  this is the test that shows the way in gets to the same place.
- **They change every state the setup needs, but nothing gets there.** That's an error
  (`follows-no-way-in`): the route it claims is broken. Changing the link so it goes
  somewhere else gives you this error.
- **Some state the setup needs, nothing changes** (the station is connected to the TX15).
  The Background's steps (and their `Say:`) are what you do, and the state map has one move
  from the followed setup to the section, named by the section. That move is a claim about
  how to set up, which nothing tests.

If nothing gets to the setup a section follows, that's a `follows-unreached` warning.
`Hands over to:` is the same idea for what a tested expectation does.

## Stories

```gherkin
Story: <name>
  Approval: approved | proposed
  On: <platform>, …
  As a <someone>
  I want <something>
  So that <a reason>

  <introduction>

  Scenario: <name>
    <description>
    Given …
    When …
    Then …
```

- The narrative is the lines that start with "As a", "As an", "I want", "So that" or "In order
  to", and a line that starts with "As" when the next one starts with "I want" ("As someone who
  drinks green tea").
- Any other text is the introduction, which opens the tutorial in the docs. Before the first
  `Scenario:` it is all introduction, even a sentence that starts with "When".
- A story has one or more `Scenario:`s. A story with none can have its steps straight under
  it, and they form a single scenario; in such a story the first line that starts with a step
  keyword ends the introduction.
- A scenario may start with `Given`s, which are arranged on top of the Background.
- After that, each `When` starts a step, `And`/`But` add to it, and its `Then`s are what the
  story checks afterwards.

## Links and the glossary

`Term: <name>` and the text under it define a word for the glossary. Any text can link to a
term, a section, a feature, an expectation or a story with `[[its name]]`, and mark code with
backticks.

A term can point to documentation outside the suite with a `Reference:` line, either
`Reference: <label> | <url>` or just `Reference: <url>`. The glossary shows it as a link under
the definition, using the label when there is one. The term keeps its own name and anchor, so
the label can be a different name for the same thing, such as an API schema:

```
Term: Account
  The customer's account within this domain.
  Reference: AccountResponse | https://api.example.com/docs/models/AccountResponse
```

Only `http` and `https` URLs are accepted, without spaces, quotes or angle brackets. A term
has at most one reference, and it is documentation only: it changes nothing in the vocabulary
or the tests.

## Platforms

`Platform:`, `About:`, `On:` and the `[platform]` tags are covered in [Platforms](platforms.md).

## Every keyword

| Line | Goes | Says |
| --- | --- | --- |
| `Suite: <name>` | once | The suite's name, and the top section's. |
| `Section: <name>` | once per directory, in any of its files | The section's name; the text under it describes it. |
| `Launch: <instruction>` | right under `Section:` | How to start the program under test. `Launch[P]:` for platform P. |
| `Vocabulary:` | top level | Starts a block of declarations. |
| `Scopes: sections` | in a Vocabulary block in the top directory | Words declared in a top-level section belong to it. |
| `Platform: <a> \| <b> \| …` | in a Vocabulary block | The platforms; the first is the default. |
| `About: <platform>: <text>` | in a Vocabulary block | What a platform is, for the manual's version menu. |
| `State: <subject> is <values>` | in a Vocabulary block | A state. Can't be tagged. |
| `Set: …`, `Shows: …` | under a `State:` | How to change it, and what the reader sees. `Set[P]:`, `Shows[P]:` for platform P. |
| `Event: <text>` | in a Vocabulary block | An event. Can't be tagged. |
| `Say: <instruction>` | under an `Event:` or `Phrase:` | How the docs tell the reader to do it. `Say[P]:` for platform P. |
| `Phrase: <name>` | in a Vocabulary block | A step of your own; its steps follow. `Phrase[P]:` replaces it on platform P. |
| `Background:` | top level | The section's setup, as `Given`s. |
| `Follows: <setup>` | in a Background | The setup the section is reached from. |
| `Feature: <name>` | top level | A topic; the text under it describes it. |
| `Expectation: <name>` | in a Feature | One rule: Given, When, Then. |
| `Overrides: <name>` | in an Expectation | It is an exception to that one. One line each. |
| `Hands over to: <setup>` | in an Expectation | Another program takes over, in that setup. |
| `Docs: hidden` | in an Expectation | Keep it off its topic page. The only value. |
| `Status: pending` | in an Expectation | Written before it is built. The only value. |
| `Approval: approved` or `proposed` | right under a Feature, Expectation or Story, once | Whether the specification is agreed. |
| `On: <a>, <b>, …` | right under a Section, Feature, Expectation or Story | Only about these platforms. |
| `Story: <name>` | top level | A tutorial: narrative, introduction and scenarios. |
| `Scenario: <name>` | in a Story | One path through it. |
| `Term: <name>` | top level | A glossary entry; the text under it defines it. |
| `Reference: [<label> \|] <url>` | in a Term, once | A link to outside documentation. |
| `Given`, `When`, `Then`, `And`, `But` | in a Background, Phrase, Expectation, Story or Scenario | A step. |
| `# …` | anywhere, as the whole line | A comment. |
