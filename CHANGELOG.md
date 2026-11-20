# Changelog

What changed for people who use Expectation. There is no tagged release yet: until there is,
pin a commit (see [Pinning a version](https://expectation.sim001.net/guide/cli.html#pinning-a-version)).
Pushing a tag `v<version>` builds the `expectation` and `expectation-run` binaries for Linux
and macOS and attaches them to a GitHub release
([`release.yml`](.github/workflows/release.yml)).

## Unreleased

### Added

- Checkpoints: a driver that can save the system (`Driver::checkpoint` and `restore`, or
  `Bindings::checkpoints()` for a world that is `Clone`) has a start that several tests share
  arranged once and put back for the others. Such a test's result says `began restored`. See
  [Drivers](https://expectation.sim001.net/guide/drivers.html#saving-a-start-that-several-tests-share).
- `expectation init [dir]` writes a starter suite that checks clean.
- `expectation --help` and `--version`; `expectation-run --version`. Help that was asked for
  goes to stdout and exits 0.
- A `#` after `Approval:`, `Status:` or `Docs:` is named as the cause of the error: the
  language has no trailing comments. A `#` after a declaration, a step or an instruction gets a
  `trailing-comment` warning.
- A state's value can start with `not` ("the job is not started | running"), and
  `the job is not started` names it. Declaring both `started` and `not started` is an error.
- A state's subject can hold a verb of its own: `State: the page that has focus is home |
  settings`.
- [`examples/cart-node`](examples/cart-node): a whole run without Rust, with a JavaScript
  driver and `expectation-run`.
- READMEs for the [JavaScript](clients/js) and [Swift](clients/swift) clients.
- [`editors/vscode`](editors/vscode): syntax highlighting for `.expect` files.
- Guide chapters: Getting started without Rust, Spec first, Modelling an app with data, In CI.
- The agent skills cover building a feature from its specification and writing a driver.

### Changed

- A one-letter unit has no plural: with a unit of `ms`, `5 m` is an error instead of 5 ms.
- In a story with `Scenario:`s, everything before the first is its introduction, even a
  sentence that starts with "When".
- A number's `Shows:` reads a comparison in any form a step accepts (`over 50 %:`), and finds
  it whichever form the docs use.
- The guide's language chapter lists every keyword, the comparison words and the rules for
  values, subjects, units and descriptions.
- The guide's diagnostics chapter lists every code the tools emit.
