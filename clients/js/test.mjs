// The JS driver against the real runner: `npm test` after `cargo build -p expectation_remote`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import fs from 'node:fs';
import os from 'node:os';

const require = createRequire(import.meta.url);
const { ExpectationDriver } = require('./index.js');
const here = path.dirname(fileURLToPath(import.meta.url));
const runner = path.join(here, '../../target/debug/expectation-run');

test('a suite runs through the JS driver', async () => {
  const port = 4000 + Math.floor(Math.random() * 1000);
  const docs = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-js-'));
  const run = spawn(runner, [path.join(here, 'fixture'), '--addr', `127.0.0.1:${port}`, '--settle', '0', '--wait', '20', '--docs', docs]);
  let out = '';
  run.stdout.on('data', d => (out += d));
  run.stderr.on('data', d => (out += d));

  const t = { unlocked: false, coins: 0, used: [] };
  const driver = new ExpectationDriver({ platform: 'node', app: 'Turnstile' })
    .observe('the turnstile', () => (t.unlocked ? 'unlocked' : 'locked'))
    .observe('the coin box', () => t.coins)
    .arrange('the turnstile', v => { t.unlocked = v === 'unlocked'; })
    .arrange('the coin box', v => { t.coins = v; })
    .on('a visitor inserts a coin', () => { t.used.push({ view: 'Gate', id: 'coin-slot' }); t.coins += 1; t.unlocked = true; })
    .on('a visitor pushes the arm', async () => { t.used.push({ view: 'Gate', id: 'arm' }); t.unlocked = false; })
    .onControls(() => [
      { view: 'Gate', id: 'coin-slot', kind: 'slot', label: 'Coins', event: 'a visitor inserts a coin' },
      { view: 'Gate', id: 'arm', kind: 'lever', event: 'a visitor pushes the arm' },
      { view: 'Gate', id: 'donate', kind: 'button', label: 'Donate' },
    ])
    .onUsed(() => t.used.splice(0))
    .onReset(() => { t.unlocked = false; t.coins = 0; });
  driver.start(`http://127.0.0.1:${port}`);

  const code = await new Promise(r => run.on('exit', r));
  driver.stop();
  assert.equal(code, 0, out);
  assert.match(out, /✓ A day at the zoo/);
  // The controls the app listed, and which the tests used, came through.
  const report = JSON.parse(fs.readFileSync(path.join(docs, 'interactions.json'), 'utf8'));
  assert.equal(report.total, 3);
  assert.equal(report.unused, 1);
  const byId = Object.fromEntries(report.controls.map(c => [c.id, c]));
  assert.equal(byId['coin-slot'].used, true);
  assert.equal(byId['coin-slot'].event, 'a visitor inserts a coin');
  assert.equal(byId.donate.used, false);
  assert.deepEqual(byId.donate.gaps.slice(0, 1), ['no-event-mapping']);
});
