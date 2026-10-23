# Diagnostics

Every diagnostic has a code, a place and usually a hint. Errors stop the suite loading.
Warnings don't, but most of them mean something won't be tested or documented.

```text
error[bad-value]: `boiling` isn't a value of the kettle
  --> heating.expect:9
   |     Then the kettle is boiling
   = help: the kettle is one of: off, starting, heating, dry
```

## The language

| Code | | Means |
| --- | --- | --- |
| `syntax` | error | A line that isn't part of the language here: a keyword in the wrong place, or a missing name. |
| `empty-suite` | error | The directory has no `.expect` files. |
| `unknown-step` | error | A step that's neither a declared state nor a declared event. The hint suggests the closest. |
| `bad-value` | error | A value the state doesn't have, a number outside its range, or a comparison that doesn't fit. |
| `bad-state` | error | A state declaration that can't be read, or one declared twice differently. |
| `bad-event` | error | An event with no name. |
| `bad-phrase` | error | A phrase that mixes Givens and Whens, or has no steps. |
| `bad-override` | error | `Overrides:` names an expectation that doesn't exist. |
| `duplicate` | error | Two expectations, terms or other named things with the same name. |
| `two-names` | error / warning | A section named twice (error), or the suite named twice (warning). |
| `no-then` | error | An expectation with no Then. |
| `contradiction` | error | Steps in one condition that can't hold together ("off" and "heating"). |
| `misplaced-event` | error | An event in a Given or a Then. Events only happen in a When. |
| `misplaced-shift` | error | A relative step ("goes up by 1") anywhere but an expectation's Then. |
| `two-events` | error | One action with two events. Put a Then between them, or split the step. |
| `empty-story` | error | A story or scenario with no steps. |
| `maybe-step` | warning | Description text that looks like a misspelled step ("Gvien the kettle is off"). |
| `says-nothing` | warning | A step that's true of every value ("off or heating" of a state with only those). |
| `shows-what` | warning | A `Shows:` line that doesn't say which value it's for. |

## Platforms

| Code | | Means |
| --- | --- | --- |
| `unknown-platform` | error | A tag or `On:` naming a platform that isn't declared, or any platform in a suite that declares none. |
| `default-platform` | error | A tag for the default platform. Write its lines plainly. |

## Conflicts and the graph

| Code | | Means |
| --- | --- | --- |
| `conflict` | error | Two expectations can both apply and promise contradicting things. The note names where they overlap; the hint suggests which is the exception. |
| `override-cycle` | error | Overrides that go round in a circle. |
| `unneeded-override` | warning | An override between two expectations that never disagree. |
| `unreachable` | warning | No chain of expectations gets the system into this expectation's Given, so nothing can test it. |
| `no-way-in` | warning | Nothing leads from a section's parent into the section, so the docs can't say how to get there. |
| `follows-no-way-in` | error | A section `Follows:` a setup, the expectations change every state its own setup needs, and still no chain of them gets there. The route it claims is broken. |
| `follows-unreached` | warning | Nothing gets to the setup a section `Follows:`: no section's setup has it, and no chain of expectations leads there. |

## Stories

| Code | | Means |
| --- | --- | --- |
| `unclear-step` | error | The step might set off an expectation, and the story doesn't pin down whether. |
| `unspecified` | error | An event happens and no expectation says what it does. |
| `contradicts` | error | A Then in the story contradicts what the expectations say. |
| `clash` | error | Two expectations fire at this step and disagree. Settle the conflict first. |
| `unpromised` | warning | A Then in the story that no expectation promises. It's checked anyway. |
| `untested` | warning | No story exercises this expectation. The hint is a story that would. |
