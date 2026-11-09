# expectation-code-coverage

An [Expectation](https://github.com/ptescher/expectation) extension. It associates the code
coverage an instrumented app handed over (LCOV, or Istanbul's `coverage-final.json`, as step
attachments) with the run, test and step that produced it, and writes the file `expectation docs`
turns into a "Code coverage" section on each test's page.

It reports observed execution. It doesn't pass or fail a test, and a covered line is not an
assertion.

## Use

```bash
expectation-run expectations --docs docs --attachments captures
npx expectation-code-coverage --results docs/results/app.tsv --attachments captures \
  --revision "$(git rev-parse HEAD)" --scope "src/**, without tests" \
  --report-url "https://coverage.example.com/{run}/{slug}/"
expectation docs expectations docs
```

The app hands over a snapshot after the test starts and after each step (`onAttachments` in
the JavaScript client). Every attachment that looks like coverage (`*.lcov`, `*.info`,
`coverage-final.json`, or an LCOV or Istanbul media type) counts; several at one step are added up.
The file is `docs/results/<runner>[@platform].code.tsv` (`--id` changes its name).

## What it will and won't say

- Counters are taken to be cumulative, from when the app started. A step's part is what ran in
  its snapshot and not the one before. Use `--counters per-step` if the app empties its counters
  when it hands them over.
- A counter that goes down means a restart: that step gets no attribution, and later steps are
  compared with the restarted program.
- A step with no capture is `missing`, and an unreadable one is `incomplete`. Neither is
  reported as zero coverage. The next difference after a gap is labelled with the step it is since.
- Background activity can't be told from the step's own, so numbers are "new since", not "caused
  by".
- Raw files stay in the attachments directory. Nothing in the coverage file holds file paths or
  source.

`npm test` runs the unit tests and an end-to-end test (build `expectation` and
`expectation-run` first).
