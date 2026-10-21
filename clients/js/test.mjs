// The JS driver against the real runner: `npm test` after `cargo build -p expectation_remote`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const require = createRequire(import.meta.url);
const { ExpectationDriver } = require('./index.js');
const here = path.dirname(fileURLToPath(import.meta.url));
const runner = path.join(here, '../../target/debug/expectation-run');

test('a suite runs through the JS driver', async () => {
  const port = 4000 + Math.floor(Math.random() * 1000);
  const run = spawn(runner, [path.join(here, 'fixture'), '--addr', `127.0.0.1:${port}`, '--settle', '0', '--wait', '20']);
  let out = '';
  run.stdout.on('data', d => (out += d));
  run.stderr.on('data', d => (out += d));

  const t = { unlocked: false, coins: 0 };
  const driver = new ExpectationDriver({ platform: 'node', app: 'Turnstile' })
    .observe('the turnstile', () => (t.unlocked ? 'unlocked' : 'locked'))
    .observe('the coin box', () => t.coins)
    .arrange('the turnstile', v => { t.unlocked = v === 'unlocked'; })
    .arrange('the coin box', v => { t.coins = v; })
    .on('a visitor inserts a coin', () => { t.coins += 1; t.unlocked = true; })
    .on('a visitor pushes the arm', async () => { t.unlocked = false; })
    .onReset(() => { t.unlocked = false; t.coins = 0; });
  driver.start(`http://127.0.0.1:${port}`);

  const code = await new Promise(r => run.on('exit', r));
  driver.stop();
  assert.equal(code, 0, out);
  assert.match(out, /✓ A day at the zoo/);
});
