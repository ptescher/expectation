# One suite, three apps

The [kettle demo](https://github.com/ptescher/expectation/tree/main/demos/kettle) is the same
app built three times, in **GPUI**, **SwiftUI** and **React Native**, and held to one suite.
Each app embeds the Expectation driver for its stack (see [Embedded drivers](remote.md)), and
the same runner drives all three.

Browse the result: [the demo manual](../demo/site/index.html). Its Platform menu switches
between the three apps' manuals: the same tutorials, with each app's own screenshots and the
instructions worded for it.

## One suite

The behaviour is written once. The suite declares the platforms, and says where the wording
differs:

```gherkin
Vocabulary:
  Platform: GPUI | SwiftUI | React Native

  State: the kettle is off | starting | heating | dry
    Shows: heating: the display reads HEATING and the light turns red
  State: the dial is 0..100 %
    Set: drag the DIAL slider to {value}

  Event: the user presses start
    Say: click START
    Say[SwiftUI]: tap START
    Say[React Native]: tap START
```

Everything else (the features, the exception for the child lock, the three stories) is
plain, so it's true on every platform. `expectation check` checks the suite as each
platform reads it.

## Three apps

Each app implements the kettle in its own language and binds the vocabulary to it:

| | GPUI | SwiftUI | React Native |
| --- | --- | --- | --- |
| Kettle | `kettle.rs` | `Kettle.swift` | `kettle.ts` |
| Driver | `expectation_remote::client` | `ExpectationDriver` (Swift package) | `expectation-driver` (npm) |
| Commands run on | the GPUI foreground, every tick | the main actor | the JS thread |
| Screenshots | the app, with `screencapture` | the app, from its key window | the runner, from the simulator |

The SwiftUI binding, for example:

```swift
let driver = ExpectationDriver(platform: "SwiftUI", app: "Kettle")
    .observe("the kettle") { .name(k.phase.name) }
    .observe("the dial") { .number(k.dial) }
    .set("the dial") { k.dial = $0.number ?? 0 }
    .on("the user presses start") { k.start() }
    .on("the water runs out") { k.runOut() }
    .onReset { k.restore() }
driver.start()
```

## Running it

```bash
cd demos/kettle
./run.sh gpui
./run.sh swiftui
./run.sh react-native
```

Each run builds the app, starts `expectation-run --platform <name>`, launches the app, which
connects, and saves the results and screenshots. A runner for one platform turns away apps
for the others, so leftover apps from an earlier run can't interfere. The site build then
writes the manual for all three from what the runs saved:

```bash
expectation docs demos/kettle/expectations demos/kettle/docs
```

## What it shows

The three apps are independent implementations, written in three languages. Nothing but the
suite keeps them the same. The test reports show the same tests passing on each, and a
behaviour change starts in the suite, where all three apps will be held to it.
