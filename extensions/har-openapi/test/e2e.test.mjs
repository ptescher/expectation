// End to end, with the real Expectation: the quote example runs and keeps each step's HAR, this
// extension holds them against the contract, and `expectation docs` writes the contract page.
// It builds Rust, so it runs only with EXPECTATION_E2E=1 (as CI does).

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { main } from '../src/cli.js';

const ROOT = fileURLToPath(new URL('../../../', import.meta.url));
const FIXTURE = fileURLToPath(new URL('./fixtures/quote.yaml', import.meta.url));

function cargo(...args) {
  const r = spawnSync('cargo', ['run', '-q', ...args], { cwd: ROOT, encoding: 'utf8' });
  assert.equal(r.status, 0, `cargo ${args.join(' ')}\n${r.stdout}\n${r.stderr}`);
  return r.stdout;
}

function everything(dir) {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? everything(p) : [p];
  });
}

test('a run, its captures and the contract make a contract page', { skip: process.env.EXPECTATION_E2E !== '1' && 'set EXPECTATION_E2E=1' }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'har-openapi-e2e-'));
  const docs = join(dir, 'docs');
  const kept = join(dir, 'attachments');
  cargo('-p', 'expectation_harness', '--example', 'quote', '--', docs, kept);
  main(['--results', join(docs, 'results', 'app.tsv'), '--attachments', kept, '--openapi', FIXTURE, '--source', 'stand-in', '--complete'], () => {});
  cargo('-p', 'expectation_cli', '--', 'docs', 'crates/harness/examples/quote', docs);

  const page = readFileSync(join(docs, 'contracts', 'quote-api.md'), 'utf8');
  const row = (label) => page.split('\n').find((l) => l.startsWith(`| ${label}`)) ?? '';
  // What the tests saw, linked to the steps that saw it.
  assert.match(row('200 A quote'), /✅ observed/);
  assert.match(row('200 A quote'), /#step-1\) \(stand-in\)/);
  assert.match(row('200 kind = "firm"'), /✅ observed/);
  // What they never saw: candidates for a story.
  assert.match(row('429 Rate limited'), /🔎 not observed/);
  assert.match(row('200 kind = "indicative"'), /🔎 not observed/);
  // Fetched while the test was set up, linked to its start.
  assert.match(row('200 The latest quote'), /#step-0\)/);
  // The test page shows each step's evidence; the report links the contract.
  const test = readFileSync(join(docs, 'tests', 'get-a-quote--ask-dismiss-ask-again.md'), 'utf8');
  assert.match(test, /- POST \/quote → 200 \(stand-in, captured during this step\), in \[Quote API\]/);
  assert.match(readFileSync(join(docs, 'tests.md'), 'utf8'), /\[Quote API\]\(contracts\/quote-api\.md\)/);
  // The driver's captures held a token; nothing published does.
  for (const f of everything(docs)) {
    assert.ok(!readFileSync(f).includes('stand-in-token'), f);
  }
  assert.ok(everything(kept).some((f) => readFileSync(f, 'utf8').includes('stand-in-token')), 'the captures are kept, apart');

  // Regenerated from the saved files alone, the page is the same.
  cargo('-p', 'expectation_cli', '--', 'docs', 'crates/harness/examples/quote', docs);
  assert.equal(readFileSync(join(docs, 'contracts', 'quote-api.md'), 'utf8'), page);
});
