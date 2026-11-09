// The whole path: a JS app hands over coverage as step attachments, the real runner keeps them,
// the extension associates them with the steps, and the real `expectation docs` shows them.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import fs from 'node:fs';
import os from 'node:os';
import { main } from '../src/cli.js';

const require = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));
const { ExpectationDriver } = require('../../../clients/js/index.js');
const runner = path.join(here, '../../../target/debug/expectation-run');
const cli = path.join(here, '../../../target/debug/expectation');
const fixture = path.join(here, '../../../clients/js/fixture');

// It needs the Rust binaries built, so it runs only with EXPECTATION_E2E=1 (as CI does).
test('coverage handed over by an app shows on the test page, apart from its result', { skip: process.env.EXPECTATION_E2E !== '1' && 'set EXPECTATION_E2E=1' }, async () => {
  const port = 6000 + Math.floor(Math.random() * 1000);
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-cov-'));
  const docs = path.join(work, 'docs');
  const keep = path.join(work, 'captures');
  const run = spawn(runner, [fixture, '--addr', `127.0.0.1:${port}`, '--settle', '0', '--wait', '20', '--docs', docs, '--runner', 'app', '--attachments', keep]);
  let out = '';
  run.stdout.on('data', (d) => (out += d));
  run.stderr.on('data', (d) => (out += d));

  // A cumulative counter: each step runs a line more of the app's source.
  const t = { unlocked: false, coins: 0, ran: 1 };
  const lcov = () => `SF:src/gate.js\n${[1, 2, 3, 4, 5].map((l) => `DA:${l},${l <= t.ran ? 1 : 0}`).join('\n')}\nend_of_record\n`;
  const driver = new ExpectationDriver({ platform: 'node', app: 'Turnstile' })
    .observe('the turnstile', () => (t.unlocked ? 'unlocked' : 'locked'))
    .observe('the coin box', () => t.coins)
    .arrange('the turnstile', (v) => { t.unlocked = v === 'unlocked'; })
    .arrange('the coin box', (v) => { t.coins = v; })
    .on('a visitor inserts a coin', () => { t.coins += 1; t.unlocked = true; t.ran += 1; })
    .on('a visitor pushes the arm', () => { t.unlocked = false; t.ran += 2; })
    .onReset(() => { t.unlocked = false; t.coins = 0; })
    .onAttachments(() => [{ name: 'lcov.info', type: 'text/x-lcov', text: lcov() }]);
  driver.start(`http://127.0.0.1:${port}`);
  const code = await new Promise((r) => run.on('exit', r));
  driver.stop();
  assert.equal(code, 0, out);

  const results = path.join(docs, 'results/app.tsv');
  const written = main(['--results', results, '--attachments', keep, '--revision', 'abc1234', '--scope', 'src/**', '--report-url', 'https://cov.example/{run}/{slug}/'], () => {});
  assert.equal(written, path.join(docs, 'results/code.code.tsv'));

  const made = spawnSync(cli, ['docs', fixture, docs], { encoding: 'utf8' });
  assert.equal(made.status, 0, made.stderr);
  const slug = 'a-day-at-the-zoo';
  const page = fs.readFileSync(path.join(docs, `site/test-${slug}.html`), 'utf8');
  assert.match(page, /Code coverage/);
  assert.match(page, /abc1234/);
  assert.match(page, /observed execution/);
  assert.match(page, /1 of 5/);
  assert.match(page, /<a href="https:\/\/cov\.example\/[^"/]+\/a-day-at-the-zoo\/"/);
  // The step that ran a line more is credited with it; the run's result is unchanged.
  assert.match(page, /1 lines, 0 branches, 0 functions/);
  assert.match(page, /passed/);
  // The attachments themselves are not in the manual.
  const all = fs.readdirSync(docs, { recursive: true }).map(String);
  assert.deepEqual(all.filter((p) => /lcov\.info$/.test(p)), []);
});
