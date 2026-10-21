# Embedded drivers

Some systems can't be driven from inside a Rust test: a phone app in a simulator, a desktop
app in its own process, a React Native bundle. For those, the driver goes **inside the app**,
and the runner talks to it over a small protocol.

```text
 runner (Rust)                                   app (any stack)
┌────────────────────────┐   POST /hello        ┌─────────────────────────────┐
│ Acceptance + RemoteDriver│ ◀──────────────────── │ embedded Expectation driver │
│ listens on :4477       │   GET /next (long poll) │  observe / set / arrange /  │
│                        │ ────────────────────▶ │  on(event) / reset /        │
│                        │   POST /reply         │  screenshot                 │
└────────────────────────┘ ◀──────────────────── └─────────────────────────────┘
```

The app connects out to the runner, so it never hosts a server: that suits phones,
sandboxes and simulators. The embedded driver says hello, long-polls for commands, runs each
one on the app's own thread, and posts the answer. When no runner is listening it waits
quietly and retries, so it can stay in development builds.

## In the app

Each client binds the vocabulary to the app the same way `Bindings` does in Rust: one closure
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

## In the runner

```rust
use expectation_remote::RemoteDriver;

let mut driver = RemoteDriver::listen("127.0.0.1:4477")?
    .settle_for(Duration::from_millis(1000))
    .screenshot_with(|| simulator_screenshot());   // optional: from outside the app
let app = driver.wait_for_app(Duration::from_secs(60))?;
let acceptance = Acceptance::load_for("expectations", &app.platform)?.with_coverage();
let report = acceptance.run(&mut driver, &RunOptions::default());
```

`RemoteDriver` knows what the app can do from its hello, so anything the app didn't bind is
"not supported yet", not a failure. Time is real: it sleeps to settle after each action and
between looks. If no app connects, every test is "not supported yet".

Or skip writing a runner and use `expectation-run`:

```bash
cargo install --git https://github.com/ptescher/expectation expectation_remote
expectation-run expectations --platform SwiftUI --docs docs --runner app
```

It listens, waits for the app, runs the suite for that platform, prints the results, and
with `--docs` saves them and writes the manual. `--screenshot-cmd 'xcrun simctl io booted
screenshot "$1"'` takes screenshots from outside, for apps that don't take their own.

## The protocol

JSON over HTTP/1.1. Anything that can make an HTTP request can be a client.

| Request | Body | Answer |
| --- | --- | --- |
| `POST /hello` | `{"platform", "app", "observe": [..], "set": [..], "arrange": [..], "events": [..], "screenshot": bool, "reset": bool}` | `{"session": n}` |
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
```

Terms are the app's own spelling from its hello. A named value is a JSON string and a number
is a JSON number. `png` is base64.

## Events: real taps or the app's actions?

An embedded driver makes an event happen by calling the same action the button calls, not by
synthesising a tap. That's fast, works the same on every stack, and tests the behaviour. It
doesn't test that the button is wired to the action, or that it's visible. If that matters,
pair the suite with a UI-automation smoke test (XCUITest, Espresso, Detox), or bind the event
to a handler that drives the UI.
