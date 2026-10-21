# One suite, three apps

The same kettle built three times (in GPUI, SwiftUI and React Native) and held to one
Expectation suite. Each app embeds the Expectation driver for its stack; the runner is the same
for all three.

| Platform | App | Embedded driver | Screenshots |
| --- | --- | --- | --- |
| GPUI | [gpui/](gpui) (Rust, macOS) | `expectation_remote::client` | the app, through `screencapture` |
| SwiftUI | [swiftui/](swiftui) (iOS) | [clients/swift](../../clients/swift) | the app, from its key window |
| React Native | [react-native/](react-native) (bare, iOS) | [clients/js](../../clients/js) | the runner, from the simulator |

The suite is [expectations/](expectations). It says what every kettle does, once:

```gherkin
Vocabulary:
  Platform: GPUI | SwiftUI | React Native

  Event: the user presses start
    Say: click START
    Say[SwiftUI]: tap START
    Say[React Native]: tap START
```

Plain lines are about every platform. A `[tag]` says how one differs: here, only how the
docs tell the reader to press a button. The behaviour is identical, so one set of
expectations and stories tests all three apps, and each gets its own manual.

## Running it

```bash
./run.sh gpui            # macOS
./run.sh swiftui         # an iOS simulator: xcodegen and Xcode
./run.sh react-native    # an iOS simulator: Node, CocoaPods and Xcode
```

Each run builds the app, starts `expectation-run` for that platform, launches the app (which
connects to the runner), runs every story and the generated coverage tests, and saves the
results and screenshots in [docs/](docs). The manual for all three is generated from those
results:

```bash
cargo run -p expectation_cli -- docs demos/kettle/expectations demos/kettle/docs
```

It's published at [expectation.sim001.net/demo](https://expectation.sim001.net/demo/site/index.html),
with a Platform menu that switches between the three manuals.

## The kettle

Every app implements the same rules, in its own language (`kettle.rs`, `Kettle.swift`,
`kettle.ts`):

- The dial has to stay above 55 % for 0.6 s before the kettle starts, and starting takes 0.22 s.
- START starts it straight away. RESET turns it off.
- PRESET steps through WARM, HOT and BOIL (70, 85 and 100 °C).
- LOCK turns the child lock on and off. While it's on, the dial, START and PRESET do nothing.
- "The water runs out" is a sensor: only the tests can make it happen.

Because the apps are independent implementations, the suite is what keeps them the same.
