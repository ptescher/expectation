# Remote drivers

Some systems can't be driven from inside a Rust test: a phone app in a simulator, a desktop
app in its own process, a React Native bundle. For those, the runner talks over a small
protocol to a client that binds the vocabulary. That client lives in one of two places:

- **Embedded in the app.** It calls the app's own actions and reads its state directly.
- **In a test process outside the app.** An adapter in a WebdriverIO, Appium, XCUITest,
  Espresso or Playwright run drives the app through its UI and reads what the UI exposes.

```text
 runner (Rust)                                    client: in the app, or an adapter driving it
┌──────────────────────────┐   POST /hello          ┌───────────────────────────┐
│ Acceptance + RemoteDriver│ ◀───────────────────── │ Expectation client        │
│ listens on :4477         │   GET /next (long poll)│  observe / set / arrange /│
│                          │ ─────────────────────▶ │  on(event) / reset /      │
│                          │   POST /reply          │  screenshot               │
└──────────────────────────┘ ◀───────────────────── └───────────────────────────┘
```

The client connects out to the runner, so it never hosts a server: that suits phones,
sandboxes and simulators. It says hello, long-polls for commands, runs each one (on the app's
own thread when embedded), and posts the answer. When no runner is listening it waits quietly
and retries, so an embedded client can stay in development builds.

The two placements show different things, so a report should say which produced the evidence:

| | Embedded | External adapter |
| --- | --- | --- |
| An event is | the action the button calls | a real tap, click or typed text |
| An observation is | the app's own state | what the screen or accessibility tree shows |
| `arrange` and `reset` | set state directly | need a UI path, a launch argument or a test backdoor |
| It shows | the behaviour | that a user can reach and see it |
| It doesn't show | that the control is wired up or visible | state the UI doesn't expose |
| It needs | a client in the app's build | only a test build the automation can drive |

See [Events: real taps or the app's actions?](#events-real-taps-or-the-apps-actions) for
mixing them.

## In the app

Each client binds the vocabulary the same way `Bindings` does in Rust: one closure
per term.

### Swift (iOS, macOS, SwiftUI, UIKit)

```swift
// Package: https://github.com/ptescher/expectation, path clients/swift
import ExpectationDriver

let driver = ExpectationDriver(platform: "SwiftUI", app: "Kettle")
    .observe("the kettle") { .name(kettle.phase.name) }
    .observe("the dial") { .number(kettle.dial) }
    .set("the dial") { kettle.dial = $0.number ?? 0 }
    .arrange("the kettle") { kettle.force($0.string ?? "off") }
    .on("the user presses lock") { kettle.pressLock() }
    .onReset { kettle.restore() }
driver.start()   // connects only when EXPECTATION_URL is set
```

Handlers run on the main actor. On UIKit platforms, screenshots of the key window are taken
for you. The app needs `NSAllowsLocalNetworking` under `NSAppTransportSecurity` in its
Info.plist to reach the runner over plain HTTP. In the simulator, pass the runner's address
with `SIMCTL_CHILD_EXPECTATION_URL=http://127.0.0.1:4477 xcrun simctl launch …`.

### JavaScript (React Native, web, Node)

```js
// npm: clients/js in the repository (expectation-driver)
import { ExpectationDriver } from 'expectation-driver';

const driver = new ExpectationDriver({ platform: 'React Native', app: 'Kettle' })
  .observe('the kettle', () => kettle.phase)
  .set('the dial', v => kettle.setDial(v))
  .on('the user presses lock', () => kettle.pressLock())
  .onReset(() => kettle.restore());
if (__DEV__) driver.start('http://127.0.0.1:4477');
```

Handlers may be async. There's no built-in screenshot in JavaScript: pass `onScreenshot`
(returning a base64 PNG), or let the runner take them from outside.

### Rust (GPUI, egui, Bevy, anything with a main loop)

```rust
use expectation_remote::client::{Client, Handlers};

let handlers = Handlers::new()
    .observe("the kettle", |v: &KettleView| v.kettle.phase_name())
    .set("the dial", |v, x| v.kettle.dial = x.as_f64().unwrap_or(0.0))
    .on("the user presses lock", |v| v.kettle.press_lock());
let client = Client::connect_env(handlers.hello("GPUI", "Kettle"));

// On the UI thread, every frame or tick:
while let Some(request) = client.try_recv() {
    handlers.handle(&mut view, request);
}
```

The client runs on its own thread; `try_recv` hands each command to the thread that owns the
app.

## Outside the app

The JavaScript and Rust clients don't have to run in the app. In a UI-automation run they run
in the test process, and each handler drives the app through the automation API. (Any test
framework that can make HTTP requests can do the same; see [the protocol](#the-protocol).) With
WebdriverIO, for example:

```js
import { ExpectationDriver } from 'expectation-driver';

const driver = new ExpectationDriver({ platform: 'iOS', app: 'Kettle' })
  .observe('the kettle', async () => (await $('~kettle-status')).getText())
  .on('the user presses lock', async () => (await $('~lock')).click())
  .onScreenshot(() => browser.takeScreenshot());
driver.start(process.env.EXPECTATION_URL);
```

Bind only what the UI really shows and lets a person do. Anything else stays unbound and is
reported as "not supported yet", which is honest: a binding that reaches round the UI into the
app's internals is an embedded driver in disguise, and should be reported as one.


```rust
use expectation_remote::RemoteDriver;

let mut driver = RemoteDriver::listen("127.0.0.1:4477")?
    .settle_for(Duration::from_millis(1000))
    .screenshot_with(|| simulator_screenshot());   // optional: from outside the app
let app = driver.wait_for_app(Duration::from_secs(60))?;
let acceptance = Acceptance::load_for("expectations", &app.platform)?.with_coverage();
let report = acceptance.run(&mut driver, &RunOptions::default());
```

`RemoteDriver` knows what the client can do from its hello, so anything it didn't bind is
"not supported yet", not a failure. Time is real: it sleeps to settle after each action and
between looks. If no app connects, every test is "not supported yet".

So `report.passed()` means only that nothing failed. A run where nothing was bound passes.
Check what you expect: that the stories meant to pass did pass (`report.count(Status::Passed)`
and each result's status), and that the screenshots you need were taken
(`driver.screenshot_error()`).

Each command the runner sends gets 10 seconds to be answered. For apps whose actions take
longer, `.command_timeout(Duration::from_secs(240))?` raises that deadline (it must be positive;
there is no unlimited mode). It is separate from `wait_for_app`, which only covers the first
hello, and from the settle and poll intervals. A timeout fails the command, naming it and the
deadline, but it doesn't cancel the work in the app: let it finish, or stop it safely, before
the next action or before ending the session.

Or skip writing a runner and use `expectation-run`:

```bash
cargo install --git https://github.com/ptescher/expectation expectation_remote
expectation-run expectations --platform SwiftUI --docs docs --runner app
```

`--stylesheet CSS` and `--state-shot-ratio W:H` style the generated docs as they do for
`expectation docs` (see [the manual](manual.md#your-own-theme)); bad values fail before the run.
`--command-timeout SECONDS` sets that per-command deadline (`--wait` only covers connecting).
It listens, waits for the app, runs the suite for that platform, prints the results, and
with `--docs` saves them and writes the manual. `--screenshot-cmd 'xcrun simctl io booted
screenshot "$1"'` takes screenshots from outside, for apps that don't take their own.
`--screenshot-store DIR` keeps screenshots by content in `DIR` and builds the docs from there
(see [Screenshots in large projects](manual.md#screenshots-in-large-projects)).

It exits 1 if a test failed and 0 otherwise, **including when every test was "not supported
yet"**. A failed screenshot is only a warning on stderr. Read the per-plan lines it prints
(`✓` passed, `✗` failed, `–` anything else) or the `plan` rows in
`docs/results/<runner>@<platform>.tsv`, and check the ones you expect to pass, before calling
anything verified.

## The protocol

JSON over HTTP/1.1. Anything that can make an HTTP request can be a client.

| Request | Body | Answer |
| --- | --- | --- |
| `POST /hello` | `{"platform", "app", "observe": [..], "set": [..], "arrange": [..], "events": [..], "screenshot": bool, "reset": bool, "controls": bool, "attachments": bool}` | `{"session": n}` |
| `GET /next?session=n` | | a command (200), nothing yet (204: ask again), or 410: say hello again |
| `POST /reply?session=n` | `{"id", "value"}`, `{"id", "png"}`, `{"id", "svg"}`, `{"id"}` or `{"id", "error"}` | 204 |

A command is one of:

```json
{"id": 1, "op": "reset"}
{"id": 2, "op": "arrange", "subject": "the kettle", "value": "off"}
{"id": 3, "op": "set", "subject": "the dial", "value": 80}
{"id": 4, "op": "perform", "event": "the user presses lock"}
{"id": 5, "op": "observe", "subject": "the kettle"}
{"id": 6, "op": "screenshot"}
{"id": 7, "op": "controls"}
{"id": 8, "op": "used"}
{"id": 9, "op": "attachments"}
```

Terms are the app's own spelling from its hello. A named value is a JSON string and a number
is a JSON number. `png` is base64.

An app that says `"controls": true` can list what a person can act on, and the report then
shows which controls no test used (see [Controls](drivers.md#controls-what-exists-and-what-was-used)).
`controls` answers with `controls`, a list of `{"view", "id", "kind", "label", "event"}` (the last
three optional; `event` is the vocabulary event using the control makes happen). `used` answers
with `used`, a list of `{"view", "id"}` for the controls `set` and `perform` acted on since the
last `used`, and empties it. In Rust that is `Handlers::on_controls` and `on_used`, in JavaScript
`onControls` and `onUsed`, in Swift `controls` and `used`.

An app that says `"attachments": true` can hand over files it captured, such as a network log.
After a test's start and after each step, the runner sends `attachments`. The app answers with
`attachments`, a list of `{"name", "type", "text"}` or `{"name", "type", "base64"}` for what it
captured since it was last asked, and empties it. `expectation-run --attachments DIR` keeps
them. In Rust that is `Handlers::on_attachments`. The JavaScript and Swift clients don't offer
it yet, but any client can answer the command. See
[Evidence and contract coverage](evidence.md).

## Events: real taps or the app's actions?

An embedded client makes an event happen by calling the same action the button calls, not by
synthesising a tap. That's fast, works the same on every stack, and tests the behaviour. It
doesn't test that the button is wired to the action, or that it's visible. If that matters,
pair the suite with a UI-automation smoke test (XCUITest, Espresso, Detox), bind the event
to a handler that drives the UI, or run the suite through an [external adapter](#outside-the-app).
