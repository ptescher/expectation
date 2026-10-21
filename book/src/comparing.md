# How expectations are compared

A condition (a Given, a Then, a trigger) stands for a set of states: every combination of
values that satisfies it. Everything below comes down to three questions about those sets:

- do two overlap?
- does one imply the other?
- what's left after an action overwrites some states?

Named values are bit sets and numbers are intervals, so the answers are exact. They are
never guesses from sampling.

## Outcome conflicts

Two expectations **conflict** when the same action can set off both, their Givens overlap,
and their Thens contradict. In the overlap, the system can't do both.

The error names the overlap ("both apply when the kettle is off and the child lock is on,
and the dial is above 55 %") and suggests which is the exception: the more specific one.

## Invariant conflicts

An expectation with no `When` is an invariant: it holds whenever its Given does. It
conflicts with an expectation that leaves the system where the invariant's Given holds but
its Then can't, and with a Background that does the same.

## Exceptions

An `Overrides:` between the two, directly or through a chain, settles a conflict. Where both
apply, the overriding one's Then wins.

- Overrides must form a DAG. A cycle is an error (`override-cycle`).
- An override between two expectations that never disagree gets a warning
  (`unneeded-override`).
- Settled conflicts are listed in the docs as the suite's exceptions, and the overridden
  expectation's page links to each one: "**Except** when the child lock is on: see …".

## Relative Thens

Two different relative Thens ("goes up by 1" and "goes up by 2") always disagree. A relative
one and an absolute one disagree unless the shared situation pins down the one value where
they meet.

## The flow DAG

Starting from each section's setup, and from each scenario's arranged start, a breadth-first
search applies every transition that can fire, and sees what state it leaves.

- It only uses expectations from the same top-level section.
- An expectation's **level** is the fewest steps before it applies. Its parents are the
  expectations that take the last step on those shortest routes.
- Levels only grow along edges, so the result is a DAG even though the behaviour loops
  (start, run dry, reset, start again).
- Each expectation gets a shortest **route** from the start, which is a ready-made test for
  it. That's what generated coverage tests follow.
- An expectation the search never reaches gets an `unreachable` warning: nothing can test it.

The search has limits on depth and the number of states it visits. A counter that can keep
going up ("the coin box goes up by 1") makes new states forever, so the search stops at its
limit and says "search truncated". By then it has usually found every expectation.

`expectation graph` prints the DAG as Mermaid (or DOT with `--dot`), and the manual's map
page draws it, coloured by verdict.

## Ways into a section

For each section, a goal-directed search finds the chain of expectations that gets from its
parent's setup to its own: "press PEN", or "sign in, open a project, press PEN" for something
deeper. That chain, without any step it works without, is the "Before you begin" of the
section's page.

If there is no such chain, it's a `no-way-in` warning, because then nothing explains how to
get there.
