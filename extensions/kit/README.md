# expectation-extension-kit

What [Expectation](https://github.com/ptescher/expectation) extensions share, so a new one starts
with the parts every extension needs:

- `readResults(path)` / `parseResults(text, file)`: a results file (`results/<runner>[@platform].tsv`)
  as the run id, each test's slug, fingerprint, status, step count, step windows and attachments.
- `attachedFiles(results, dir, wanted)`: the files a driver attached to tests that ran, for the
  attachments `wanted` picks, with their full paths. It refuses a path that leaves `dir`, links
  included.
- `line(kind, ...fields)`: one record of an output file, with no tab, newline or other control
  character in a field and `-` for nothing.

An extension then does its own part: reads its evidence, works out what it means, and writes
`results/<name>[@platform].<kind>.tsv`. Expectation reads that file the same way whatever wrote
it (see `crates/story/src/sidecar.rs`): the file for another platform is left out, a big file isn't
read, fields are cut, and evidence counts only for its own run and test as it was.
