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

The platforms share one docs directory, and so one `screenshots/`. The default platform's are in
`screenshots/<test>/`, and each other platform's in a folder of its own,
`screenshots/platforms/<name>/<test>/`, so the same test on two platforms never shows the other's
picture. `expectation-run --docs` does this. A driver in Rust gets it from
`acceptance.run_options("docs")`, or by setting `RunOptions::screenshots_under` to
`acceptance.screenshots_under()`:

```rust
let acceptance = Acceptance::load_for("expectations", "iOS")?;
let report = acceptance.run(&mut driver, &acceptance.run_options("docs"));
acceptance.publish("docs".as_ref(), "app", &report)?;
```

Results saved before this name a later platform's screenshots as `screenshots/<test>/…`. Its
manual shows a placeholder for those until the tests run again. Screenshots kept by content in
a store are the same on every platform.

## Saying what each platform is

The docs have a menu that picks the version of the manual. Say what each platform is with
`About:` lines under the `Platform:` line, and the menu is labelled "Version of this manual", each
choice gets a tooltip, and the first page lists the versions:

```gherkin
Vocabulary:
  Platform: desktop | iOS
    About: desktop: the app on a Mac, Linux or Windows computer
    About: iOS: the ground station on an iPad or iPhone
```

An `About:` for a platform the suite doesn't declare is an error.

## A section that runs on other platforms

A section can declare platforms of its own, in a `Vocabulary` block in its directory:

```gherkin
Section: Towers

Vocabulary:
  Platform: sim | field
```

Its tests don't run on the default platform, so the default manual would show them with no results.
Instead, its pages show the results of the section's first platform (`sim` here), say so ("Shown for
sim: …", with that platform's `About:` text), and the version menu has `sim` selected. The pages of
sections that run on the default platform are unchanged, and the other platforms' manuals show their
own results.
