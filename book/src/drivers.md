# Drivers

The runner talks to the system under test through a **driver**, in the vocabulary's terms: set
this state, make this event happen, what is that state now. You can implement the `Driver`
trait, or build one from closures with `Bindings`.

## Bindings

`Bindings` wraps your system (any value, called the world) and one closure per vocabulary term:

```rust
use expectation_harness::{Bindings, Progress, Screenshot};

let mut driver = Bindings::new(Kettle::default())
    .observe("the kettle", |k| k.phase_name())                  // read a state
    .observe("the dial", |k| k.dial)
    .set("the dial", |k, v| k.dial = v.as_f64().unwrap())       // change a state, as an action
    .arrange("the kettle", |k, v| k.force(v.as_str().unwrap()))  // put a state in place before a test
    .on("the user presses reset", Kettle::reset)                 // make an event happen
    .on("the water runs out", Kettle::run_out)
    .on_reset(|k| *k = Kettle::default())                        // before every test
    .on_settle(|k| k.run_for(1.0))                               // after each action
    .on_advance(|k| { k.run_for(0.1); Progress::Advanced })      // between looks
    .on_screenshot(|k| Screenshot::svg(k.svg()));                // after each step
```

Terms are matched the way steps are: ignoring case, extra spaces and a leading "the".

| Method | Used for |
| --- | --- |
| `observe(subject, f)` | Reading a state. Return a `&str` for a named value, or a number. Named values match the vocabulary ignoring case. |
| `set(subject, f)` | A `When` that changes a state ("When the dial is 80 %"). Also used to arrange, unless there's an `arrange`. |
| `arrange(subject, f)` | Putting a state in place before the first step (the Background, a scenario's Givens). |
| `on(event, f)` | Making an event happen. |
| `on_reset(f)` | Putting the system back to the start before each test. |
| `on_settle(f)` | Letting the system settle after each action, before anything is observed. |
| `on_advance(f)` | Letting a little time pass before the runner looks again. |
| `on_screenshot(f)` | A picture of the system after each step: `Screenshot::png`, `jpeg` or `svg`. |
| `needs(&[…])`, `fresh(&[…])` | The [external services](services.md) the tests need. |

The values passed to `set` and `arrange` are `Value`s: `v.as_str()` for a named value,
`v.as_f64()` for a number.

## Time: settling and polling

Real systems take time to react. For each step the runner:

1. acts: sets the states and makes the event happen;
2. lets the system **settle**;
3. observes every check. If they don't all hold, it **advances** and observes again, up to
   `RunOptions::patience` times (100 by default). The step passes as soon as every check holds
   at once.

Polling is what lets "Then the kettle is heating" wait for the kettle to warm up. Settling is
what makes "Then the kettle is off" mean that it *stays* off. Without it, the check could
pass in the instant before the kettle switches on. Make the settle long enough for whatever
should happen to happen, and for whatever shouldn't to show.

A system with no time in it needs neither: the default `advance` returns `Progress::Static`,
and the runner stops looking after the first observation.

## Not supported yet

Before a test starts, the runner checks that the driver can do everything the plan needs:
arrange each starting state, set each changed state, perform each event, observe each check.
If it can't, the test is **not supported yet**:

```text
The driver can't arrange the child lock, arrange the dial, arrange the kettle and 6 more things yet.
```

That isn't a failure. The system, or its driver, just doesn't do it yet. The test's steps get a
placeholder instead of a screenshot, and the expectations only it would check are "not
supported yet" in the docs. This lets a suite describe a whole product while its drivers
catch up, and keeps the gap visible.

A driver that can arrange some values of a state and not others overrides
`Driver::can_arrange`.

## The Driver trait

For full control, implement `Driver` yourself. Only `set`, `perform` and `observe` are
required:

```rust
pub trait Driver {
    fn needs(&self) -> Vec<String> { vec![] }
    fn fresh(&self) -> Vec<String> { vec![] }
    fn reset(&mut self) -> Result<(), String> { Ok(()) }
    fn arrange(&mut self, var: &Variable, value: &Value) -> Result<(), String> { self.set(var, value) }
    fn set(&mut self, var: &Variable, value: &Value) -> Result<(), String>;
    fn perform(&mut self, event: &Event) -> Result<(), String>;
    fn observe(&mut self, var: &Variable) -> Result<Value, String>;
    fn can_arrange(&self, var: &Variable, value: &Value) -> bool { true }
    fn settle(&mut self) {}
    fn advance(&mut self) -> Progress { Progress::Static }
    fn screenshot(&mut self) -> Result<Option<Screenshot>, String> { Ok(None) }
    fn supports(&self, need: Need<'_>) -> bool { true }
}
```

`Variable::key` and `Event`'s `key` are the normalised names, for matching. Calls may block,
so a driver can talk to a browser, a device or another thread. Any `Err` fails the step with
that message.

## Running

```rust
let acceptance = Acceptance::load("expectations")?.with_coverage();

// Run, and panic with every failure if anything failed:
acceptance.assert_passes(&mut driver, &RunOptions::default());

// Or keep the report:
let opts = RunOptions { docs_dir: Some("docs".into()), ..Default::default() };
let report = acceptance.run(&mut driver, &opts);
println!("{}", report.failures(&acceptance.suite, &acceptance.plans));
acceptance.docs(Some(&report)).write("docs")?;
```

With a `docs_dir`, screenshots go to `<docs_dir>/screenshots/<test>/<step>.<ext>`.

### Several runners

`Acceptance::publish(dir, runner, &report)` saves a run's results as
`dir/results/<runner>.tsv`, merges them with what other runners saved there, and writes the
docs from the merged report. An API runner and a UI runner can each run their own part of one
suite, and the manual shows both. `expectation docs` rebuilds the docs from saved results
without running anything.

Results are keyed by each test's name and a fingerprint of its plan, so results saved for an
older version of a test are ignored, not misapplied.
