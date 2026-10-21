# Platforms

One suite can describe a system that exists on several platforms: a desktop app and a phone
app that do the same job, say. Everything written plainly is true on every platform. Where
one differs, the suite says so, and each platform reads the suite as its own.

```gherkin
Vocabulary:
  # The first is the default: plain lines are about it.
  Platform: desktop | iOS

  Event: the user unlocks the door
    Say: press UNLOCK
    Say[iOS]: tap UNLOCK          # how the docs tell an iOS reader to do it
  State: the door is locked | unlocked
    Shows[iOS]: unlocked: the UNLOCKED badge turns green   # replaces the plain Shows lines

  Phrase: the app is open on a new account
    Given the tool is pan
  Phrase[iOS]: the app is open on a new account
    Given the user is signed in   # a start that iOS can reach

Section: Drawing
  On: desktop                     # also under a Feature, Expectation or Story
```

## The rules

- `Platform: <a> | <b> | …` declares the platforms. The first is the default.
- `Say[P]:`, `Set[P]:` and `Shows[P]:` replace the plain line for platform `P`.
- `Phrase[P]:` replaces the plain phrase of the same name for `P`.
- `On: desktop, iOS` says that the section (the whole directory), feature, expectation or
  story is only about those platforms. Use it for what a platform doesn't have. An exception
  to an expectation that isn't on a platform is dropped there.
- Tag only the platforms that differ from the default. Tagging the default is an error
  (`default-platform`), and so is a name that isn't declared (`unknown-platform`).

## Checking, running and docs

`expectation check` checks the suite as each platform reads it, and every one must check
clean.

A driver reads the suite for its platform and runs its tests:

```rust
let acceptance = Acceptance::load_for("expectations", "iOS")?;
```

Its results are its own (`results/app@iOS.tsv`), apart from those of runners that are the
same everywhere (`api.tsv`). `Acceptance::publish(dir, runner, &report)` saves a run's results
and merges them with what other runners saved.

`expectation docs` writes one manual per platform: the default at the root and the others
under `platforms/<name>/`. Every HTML page has a Platform menu that opens the same page in the
other manual.
