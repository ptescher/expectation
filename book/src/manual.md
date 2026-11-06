# The generated manual

`Docs::write` produces a manual, both as an HTML site in `site/` and as Markdown that GitHub
renders. It reads like a project's user documentation, not a test report. The test evidence
is all there, folded away.

```rust
acceptance.docs(Some(&report)).write("docs")?;
```

or, from results that runs have saved in `docs/results/`:

```bash
expectation docs expectations docs
```

`expectation docs` checks the links in the HTML it wrote and warns about any that go to a file
or an anchor that isn't there.

A test whose wording has changed since its results were saved is shown as not run, not as
unsupported (another runner that can't do it says nothing about whether this one did). If its
steps are still the same, each keeps its last screenshot, captioned "From before this page
changed", so a published manual keeps its pictures between full runs.

## Navigation and size

Each page lists only the sections on the way to it, with the others as links. Opening one fills
in what is under it from `site/assets/nav.js`, which is written once for the whole site, so a
page's size does not grow with the manual. A section with more than 30 subsections shows them
under one "N more sections" entry. It works from disk and from any static host; without scripts
the sidebar shows the current section and links to the rest.

With more than 300 tests, the test report (`site/tests.html`) is an index: one row per section,
with its results, linking to that section's own report (`site/tests-<section>.html`), which lists
its tests and what isn't verified yet. With more than 300 expectations, the map
(`site/map.html`) is likewise a list of sections, each drawn on its own page
(`site/map-<section>.html`). Smaller manuals keep one page each. The Markdown report is always one
file.

## Approved-only manuals

Mark what is agreed with `Approval: approved` and what is not with `Approval: proposed` (see
[the language](language.md#approval)), then write a manual of just the agreed part:

```bash
expectation docs expectations docs-approved --approved-only --results docs/results
```

The full suite is still checked first and must be clean. Then only what is approved is kept,
before the suite is read, so the manual is one where the rest was never written:

- A feature stays if any of its expectations is approved (its own, or its feature's). Its
  description stays only if the feature itself is approved. A feature with nothing approved is
  gone, with its page, navigation entry and every link to it; a `[[link]]` to something left
  out reads as plain text.
- A story is a tutorial only if it is approved itself. Its test page goes with it.
- The vocabulary, terms and backgrounds stay. They aren't behaviour.
- An approved story or expectation that depends on one that isn't (a story steps through a
  proposed expectation, or an approved expectation overrides one) is an error that names the
  problem, so the approved manual can't present proposed behaviour as agreed. Approve the other
  one, or leave this out.
- Nothing marked approved means an empty manual, with a warning.

`--results` says where the saved results are, since the output directory is usually not the
one the full manual and its `results/` live in. Test results are what runs saw and are shown as
they are: an approved specification may still fail, or not be testable yet. Without the flag,
nothing changes, and neither do checking, planning or running.

## Your own theme

The site loads a CSS file of yours after its built-in styles, so a team can give the reports
its documentation's look without forking or editing generated files:

```bash
expectation docs expectations docs --stylesheet brand.css
```

```rust
let acceptance = Acceptance::load("expectations")?.with_stylesheet("brand.css")?;
```

`expectation-run` takes `--stylesheet` too. The file is copied to `site/assets/custom.css` and
linked from every page of every platform's manual: tutorials, test results, interaction
coverage, both maps and the theme page. Put a file called `theme.css` in the suite's
directory and it is used without the flag, so regenerating, or restyling from saved results
without running a test, keeps the theme. Results, screenshots, links and pass/fail never depend
on it. A stylesheet that is missing, empty or has unbalanced braces is an error, not a
fallback. Without one, the site is exactly the built-in Day and Night.

Override the colour, type and shape tokens (`--canvas`, `--ok`, `--font-sans`,
`--radius-panel`, …; the generated theme page lists them all) or any component's CSS by
selector. The built-in dark rule is more specific than `:root`, so set a token for Day
(`:root`), for dark readers (`@media (prefers-color-scheme: dark) { :root:not([data-theme="day"]) { … } }`)
and for `:root[data-theme="night"]`. The file is copied as written: a relative `url()` in it
resolves beside `site/assets/custom.css`, so use `data:` URLs to stay self-contained and
usable offline. For accessibility keep text at 4.5:1 against its surface in each mode, and keep
result states distinguishable without colour: the words *verified*, *failed* and *not
supported yet* stay on the page.

The state map's cards draw each screenshot at the shape you give with `--state-shot-ratio`
(`with_shot_ratio` in Rust), for example `9:19.5` for phone screens.

See [the kettle manual](../example/site/index.html) for a whole one.

## Pages

- **Tutorials**, one per story scenario: the story's introduction, **Before you begin** (the
  setup, in the specs' own words), then numbered steps. Each step says what to do (from the
  `Say:`s and `Set:`s), what you'll see (the story's Thens, or what the expectations promise)
  and shows the screenshot the test took there.
- **Topic pages**, one per feature: its description, then each expectation as a sentence ("If
  the kettle is off, press START. The light comes on and the water starts to heat."), its
  exceptions as notes, a screenshot of it in action, and the tutorials that show it.
- **Section pages**: the description, Before you begin (the setup, or the steps that get
  there from the parent section), the topics in it, and its tutorials as cards.
- **Reference**: the glossary of `Term:`s, the map of the flow DAG, the theme, and the
  **test report**: every test and its result, and a page per test with each step, check and
  screenshot.
- **Test vocabulary**: each state, event and phrase, and how the docs word it.

On every page, the test details of each expectation (its Gherkin, what it follows and leads
to, its verdict and the tests that cover it) are collapsed under "How this is tested".
Generated coverage tests appear only in the test report. A step without a screenshot shows a
"Not supported yet" or "No screenshot" placeholder.

## Previews of some sections

A pull request that touches one area needs a preview of that area, not the whole manual:

```bash
expectation docs expectations preview --section Billing --section 04-orders/returns \
  --base-url https://docs.example.com/manual/
```

`--section` takes a section's name (ignoring case), its slug, or its directory. Give it more than
once for several sections. A name that two sections share is an error that lists their
directories.

**What the preview has:**
- the pages of those sections and everything under them: their features, tests and tutorials,
  and their own test report and map pages when the manual is split by section;
- the Markdown overview (`README.md`) and the site's assets.

**What it leaves out:** the pages about the whole suite (the test report, the maps, the
glossary, the vocabulary, the interactions and contracts).

**Links.** The navigation still shows the whole tree. A link to a page the preview doesn't have
goes to the same page under `--base-url`: the address of the full manual, the directory
`expectation docs` wrote it to. Without `--base-url`, such a link reads as plain text. Either way
no link is broken.

**What doesn't change:**
- The whole suite is still loaded, checked and planned, and the results are the whole suite's.
  A preview of one section is not a way past an error in another.
- Writing a preview into a directory that holds a full manual removes nothing from it.

## Screenshots in large projects

By default a run writes each step's screenshot to `screenshots/<test>/<step>.png` in the docs
directory, replacing the last run's. That is simple, but every run rewrites every image. At
2,000 tests of five steps that is around a gigabyte per platform, and none of it diffs.

**Keep them by content instead.** Give the run a store:

- `RunOptions { screenshot_store: Some(dir), .. }`;
- `expectation-run --screenshot-store DIR`.

Each screenshot is then named by its SHA-256 (`DIR/<hash>.png`) and written only if no
screenshot with the same content is there yet. Identical frames across steps and runs are
stored once, and an unchanged test writes no new file. The results name it
`screenshots/<hash>.png`, the same kind of path as before, so older readers still work. Re-run
unchanged tests and the results file is the same, apart from its `run` and `window` lines, which
say which run it was and when each step ran (see [Evidence and contract coverage](evidence.md)).

**Tell the docs where the store is** with `expectation docs --screenshots <dir|url>` (or
`Acceptance::with_screenshots`; `expectation-run --screenshot-store` does both):

- **A directory.** The screenshots the pages show are copied into the docs' `screenshots/`.
- **A URL prefix**, for a store you sync to a bucket or keep as a CI artifact. The pages link to
  `<prefix>/<hash>.png` and nothing is copied: `https://cdn.example.com/shots` makes
  `screenshots/abc….png` into `https://cdn.example.com/shots/abc….png`. Images load lazily.

A store can also be the docs' own `screenshots/` directory; then there is nothing to copy.

**Thumbnails.** The state map's cards and the story cards show thumbnails when a tool can make
them: ImageMagick (`magick`, or `mogrify` from version 6), or `sips` on macOS. Each is made once,
as `screenshots/thumbs/<name>`, and again only when its screenshot changes. Without a tool, or
with `EXPECTATION_THUMBNAILS=off`, the cards show the screenshots themselves. SVG screenshots
need none, since the browser draws them at any size. With a URL prefix there are no thumbnails.

**Prune** what no run names any more:

```bash
expectation screenshots prune DIR docs/results [more/results ...] [--dry-run]
```

It reads every results file in the directories given, every platform's, and removes the files
in the store (and its `thumbs/`) that none of them names. It only touches files named by a hash,
and with no results files at all it removes nothing.

**Where the store lives** is your choice. Commit the specs and `results/*.tsv` in any case, and
leave out the generated pages and thumbnails (add `screenshots/thumbs/` to your `.gitignore`, as
this repository does).

- **In Git.** Make the store a directory in the repository, the docs' own `screenshots/` say, and
  commit it. Because names are content, an unchanged screenshot is never a new commit, and only
  changed frames add to the history. Run `screenshots prune` before committing to drop the ones
  no test takes any more. This is the simplest, and enough for most projects.
- **In Git LFS**, when the images are large or many. Run `expectation screenshots lfs <store>`
  once. It writes `<store>/.gitattributes` so that PNG, JPEG and WebP files go to LFS; SVGs stay
  in Git, as text. Then commit as above. Git LFS must be installed (`git lfs install`). A clone
  fetches only the versions it checks out.
- **In a bucket**, when it shouldn't be in the repository at all:

  ```bash
  expectation screenshots push DIR s3://bucket/shots   # after a run
  expectation screenshots pull s3://bucket/shots DIR   # before building docs elsewhere
  ```

  These run the provider's own tool, with its own credentials and configuration:
  - `s3://…` uses the AWS CLI. S3-compatible stores such as R2 or MinIO work with
    `AWS_ENDPOINT_URL` set.
  - `gs://…` uses `gcloud storage`, or `gsutil` without it.
  - `name:path` is an [rclone](https://rclone.org) remote, which covers most others.

  Both directions add what the other side lacks and delete nothing; `--dry-run` shows what they
  would copy. To build the docs, either pull the store first and pass `--screenshots DIR`, with
  thumbnails, or pass the bucket's public or CDN address, `--screenshots https://…`, so the pages
  link there.

None of these is required. Without a store, screenshots stay where they always were.

**Keep hashes stable.** A screenshot only matches the last run's if every pixel does:

- hide or freeze clocks and anything else that shows the time;
- turn off animations and the blinking caret;
- use the same fonts, data, device and scale each run.

A screenshot that changes on every run is stored again on every run.

## Markdown

The Markdown has the same pages:

```text
README.md        the overview and contents
tutorials/       one per scenario
features/        one per feature
glossary.md
tests.md         the test report, with a Mermaid map of the flow DAG
tests/           one per test
vocabulary.md
screenshots/
site/            the HTML version
```

Pages read in order: each section, then its tutorials, its topics and its subsections. Each
page's footer names the spec file to edit to change it.

## The reader edition

The manual is read by people who want to learn the product, not test it. The default edition
keeps every gap in view for the authors; for readers, build the reader edition:

```bash
expectation docs expectations docs --reader
```

(`Edition: reader` in `outline.txt`, `--reader` on `expectation-run`, or `Acceptance::with_reader`
do the same.) Tutorials and topic pages then have no "How this is tested" block, no "Tested by"
list and no "Its report" link, and a step with no screenshot shows just the step and what you
see, with no placeholder or caption. The test report, the map, the state map and the vocabulary
are still written, linked from one "For developers" entry in the sidebar. Nothing about the
tests or their results changes.

## The outline

By default the manual follows the sections. A suite can have an `outline.txt` beside its
sections to shape it like a book:

```text
Part: Start here
  Page: installing          manual/installing.md
  Page: the-installer [desktop]
Part: Brewing
  Section: The kettle
```

- **Parts** group the top-level sections. They're the navigation's labels and the README's
  contents.
- The order is the order the reader meets them in, and so the previous and next links.
- **Pages** are hand-written Markdown files in `manual/` (install, reference,
  troubleshooting), placed among the generated ones. A page's title is its first `# ` line.
  They can use `[[links]]` into the generated pages. `[desktop]` keeps a page out of another
  platform's manual.

- `Next: tutorials` (a line of its own) makes Next and Previous follow the pages, the section
  pages and the tutorials only. The topic pages stay in the sidebar and in each tutorial's
  "Learn more", so a beginner pressing Next goes from tutorial to tutorial. With an outline,
  the glossary is the last page of the chain.
- A section page lists its own tutorials first, then those of its subsections, even where the
  outline reads a subsection earlier than the section that holds it.

The sections themselves stay where they are, because their setups inherit from their parents.
A top-level section that no part names goes into a last part, "More", with a warning.

## The look

The site is static HTML and CSS with a little JavaScript: `assets/nav.js` fills in the sidebar
(see [Navigation and size](#navigation-and-size)), and the states map (`states.html`) draws
itself from data in the page. Neither is needed to read it: without scripts every page reads and
links, the sidebar shows the current section, and the states map falls back to a list of the
start states and where they lead. It follows the reader's light or dark preference. The
colours, type and corners come from the `expectation_theme` crate, and the
theme page in each manual shows them.
