# ExpectationDriver (Swift)

The [Expectation](https://github.com/ptescher/expectation) driver for Swift apps: iOS, macOS,
tvOS and visionOS, in SwiftUI or UIKit. You bind the suite's vocabulary to your app, and the
runner (`expectation-run`) drives it.

## Install

The package is in a subdirectory of the repository, which Swift Package Manager can't use by
URL. Add the repository as a git submodule (or copy `clients/swift`) and add it as a local
package:

```swift
// Package.swift
.package(path: "vendor/expectation/clients/swift")
```

In Xcode: File, Add Package Dependencies, Add Local, and pick `clients/swift`. Use the same
commit as the `expectation-run` you installed.

## Use

```swift
import ExpectationDriver

let driver = ExpectationDriver(platform: "iOS", app: "Kettle")
    .observe("the kettle") { .name(kettle.phase.name) }      // read a state
    .observe("the dial") { .number(kettle.dial) }
    .set("the dial") { kettle.dial = $0.number ?? 0 }         // a state the outside world changes
    .arrange("the kettle") { kettle.force($0.string ?? "off") } // put a state in place before a test
    .on("the user presses lock") { kettle.pressLock() }       // make an event happen
    .onReset { kettle.restore() }                             // before each test
driver.start()   // connects only when EXPECTATION_URL is set
```

- Handlers run on the main actor.
- On UIKit platforms, screenshots of the key window are taken for you.
- The app needs `NSAllowsLocalNetworking` under `NSAppTransportSecurity` in its Info.plist to
  reach the runner over plain HTTP.
- In the simulator, pass the runner's address at launch:
  `SIMCTL_CHILD_EXPECTATION_URL=http://127.0.0.1:4477 xcrun simctl launch booted <bundle id>`.
- A word with no handler is not an error. Tests that need it are reported as "not supported
  yet", and `expectation-run --strict` fails the run for them.

## Running a suite

```bash
expectation-run expectations --platform iOS --docs docs --runner app \
  --screenshot-cmd 'xcrun simctl io booted screenshot "$1"'
```

The [kettle demo](../../demos/kettle) runs one suite against a SwiftUI app, a React Native app
and a GPUI app. The guide's [Remote drivers](https://expectation.sim001.net/guide/remote.html)
chapter has the rest.
