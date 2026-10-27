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

The sections themselves stay where they are, because their setups inherit from their parents.
A top-level section that no part names goes into a last part, "More", with a warning.

## The look

The site is plain HTML and CSS, with no scripts. It follows the reader's light or dark
preference. The colours, type and corners come from the `expectation_theme` crate, and the
theme page in each manual shows them.
