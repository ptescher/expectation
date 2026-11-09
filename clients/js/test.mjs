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

/** Runs the turnstile suite through a JS driver that `extra` adds handlers to, keeping attachments in `keep`. */
async function runTurnstile(extra, keep) {
  const port = 5000 + Math.floor(Math.random() * 1000);
  const docs = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-js-docs-'));
  const args = [path.join(here, 'fixture'), '--addr', `127.0.0.1:${port}`, '--settle', '0', '--wait', '20', '--docs', docs];
  if (keep) args.push('--attachments', keep);
  const run = spawn(runner, args);
  let out = '';
  run.stdout.on('data', d => (out += d));
  run.stderr.on('data', d => (out += d));
  const t = { unlocked: false, coins: 0 };
  const driver = extra(new ExpectationDriver({ platform: 'node', app: 'Turnstile' })
    .observe('the turnstile', () => (t.unlocked ? 'unlocked' : 'locked'))
    .observe('the coin box', () => t.coins)
    .arrange('the turnstile', v => { t.unlocked = v === 'unlocked'; })
    .arrange('the coin box', v => { t.coins = v; })
    .on('a visitor inserts a coin', () => { t.coins += 1; t.unlocked = true; })
    .on('a visitor pushes the arm', () => { t.unlocked = false; })
    .onReset(() => { t.unlocked = false; t.coins = 0; }));
  driver.start(`http://127.0.0.1:${port}`);
  const code = await new Promise(r => run.on('exit', r));
  driver.stop();
  return { code, out, docs, hello: driver.hello() };
}

/** Every file under `dir`, as its path below `dir` and its bytes. */
function filesUnder(dir) {
  const found = {};
  const walk = d => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else found[path.relative(dir, p)] = fs.readFileSync(p);
    }
  };
  if (fs.existsSync(dir)) walk(dir);
  return found;
}

test('attachments reach the runner per step, as text or binary, and never the manual', async () => {
  const keep = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-js-keep-'));
  let n = 0;
  const bytes = Buffer.from([0, 1, 2, 253, 254, 255]);
  const { code, out, docs, hello } = await runTurnstile(
    d => d.onAttachments(async () => {
      n += 1;
      return [
        { name: 'cov.json', type: 'application/json', text: `{"call":${n}}` },
        { name: 'frame.bin', base64: bytes.toString('base64') },
      ];
    }),
    keep,
  );
  assert.equal(code, 0, out);
  assert.equal(hello.attachments, true);
  const kept = filesUnder(keep);
  const texts = Object.entries(kept).filter(([p]) => p.endsWith('cov.json'));
  // One drain after each test's start and each step, each in its own step's folder.
  assert.ok(texts.length >= 3, Object.keys(kept).join('\n'));
  const calls = texts.map(([, b]) => JSON.parse(b).call).sort((a, b) => a - b);
  assert.equal(new Set(calls).size, calls.length, 'each drain is kept once, under its own step');
  const folders = new Set(texts.map(([p]) => path.dirname(p)));
  assert.equal(folders.size, texts.length);
  // Binary is decoded, byte for byte.
  for (const [p, b] of Object.entries(kept)) if (p.endsWith('frame.bin')) assert.deepEqual(b, bytes);
  // Never in the generated manual.
  assert.deepEqual(Object.keys(filesUnder(docs)).filter(p => /cov\.json|frame\.bin/.test(p)), []);
});

test('without a handler the client does not advertise attachments, and the run is unaffected', async () => {
  const keep = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-js-keep-'));
  const { code, out, hello } = await runTurnstile(d => d, keep);
  assert.equal(code, 0, out);
  assert.equal(hello.attachments, false);
  assert.deepEqual(Object.keys(filesUnder(keep)), []);
});

test('captures are drained and dropped when the runner is not asked to keep them', async () => {
  let drains = 0;
  const { code, out } = await runTurnstile(d => d.onAttachments(() => { drains += 1; return [{ name: 'a.txt', text: 'x' }]; }), null);
  assert.equal(code, 0, out);
  assert.ok(drains > 0, 'the runner still empties the app\'s captures');
});

test('a handler that throws, or returns something malformed, costs only those captures', async () => {
  const keep = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-js-keep-'));
  let n = 0;
  const { code, out } = await runTurnstile(
    d => d.onAttachments(() => {
      n += 1;
      if (n === 1) throw new Error('the coverage snapshot failed');
      if (n === 2) return [{ name: 'bad.txt' }];
      return [{ name: 'good.txt', text: String(n) }];
    }),
    keep,
  );
  assert.equal(code, 0, out);
  const names = Object.keys(filesUnder(keep)).map(p => path.basename(p));
  assert.ok(names.includes('good.txt'), names.join(','));
  assert.ok(!names.includes('bad.txt'));
});

test('a handler returning something that is not a list is an error reply', async () => {
  const d = new ExpectationDriver({ platform: 'node', app: 'x' }).onAttachments(() => 'nope');
  assert.match((await d.handle({ op: 'attachments' })).error, /list of captures/);
  assert.deepEqual((await new ExpectationDriver({ platform: 'node', app: 'x' }).handle({ op: 'attachments' })), { error: 'no attachments' });
  const ok = new ExpectationDriver({ platform: 'node', app: 'x' }).onAttachments(() => null);
  assert.deepEqual(await ok.handle({ op: 'attachments' }), { attachments: [] });
});
