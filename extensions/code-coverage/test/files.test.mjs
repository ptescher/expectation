import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { snapshotsOf, coverageFile } from '../src/cli.js';
import { parseCoverage } from '../src/formats.js';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-covfiles-'));
const put = (name, text) => {
  const p = path.join(dir, name);
  fs.writeFileSync(p, text);
  return p;
};
const lcov = (n) => `SF:a.js\nDA:1,${n}\nend_of_record\n`;
const file = (step, name, text, mediaType = 'text/plain') => ({ step, name, mediaType, path: put(`${step}-${name}`, text) });

test('a step is its files joined; every step must hold the same files', () => {
  const { snapshots } = snapshotsOf(2, [
    file(0, 'front.lcov', lcov(1)),
    file(0, 'back.lcov', 'SF:b.js\nDA:1,1\nend_of_record\n'),
    file(1, 'front.lcov', lcov(2)),
  ]);
  assert.ok(snapshots.get(0).counts);
  assert.match(snapshots.get(1).error, /different files/);
});

test('a surely-coverage file that is broken makes the step unusable, without quoting it', () => {
  const secret = 'password=hunter2 and more';
  const { snapshots } = snapshotsOf(1, [file(0, 'x.istanbul.json', secret)]);
  assert.equal(snapshots.get(0).error, 'x.istanbul.json: not valid JSON');
  assert.ok(!JSON.stringify(snapshots.get(0)).includes('hunter2'));
  const lcovBad = snapshotsOf(1, [file(0, 'x.lcov', secret)]);
  assert.ok(!JSON.stringify(lcovBad.snapshots.get(0)).includes('hunter2'));
});

test('a .info or coverage.json that is not coverage is somebody else\'s file', () => {
  const { snapshots } = snapshotsOf(1, [
    file(0, 'build.info', 'version=3'),
    file(0, 'coverage.json', '{"a":1}'),
    file(0, 'real.lcov', lcov(1)),
  ]);
  assert.ok(snapshots.get(0).counts);
  assert.equal(snapshotsOf(1, [file(0, 'build.info', 'version=3')]).snapshots.size, 0);
});

test('the step count is at least the test\'s own', () => {
  assert.equal(snapshotsOf(3, [file(0, 'a.lcov', lcov(1))]).steps, 3);
});

test('the file has a source line and a row per capture, empty fields as dashes', () => {
  const results = { runner: 'app' };
  const plan = { run: 'r1', slug: 'go', fingerprint: 'fp' };
  const rows = [
    { step: 0, state: 'captured', basis: 'baseline', note: '', new: null, totals: { L: { hit: 1, found: 2 }, B: { hit: 0, found: 0 }, F: { hit: 0, found: 0 } } },
    { step: 1, state: 'missing', basis: 'none', note: 'no capture at this step' },
  ];
  const text = coverageFile(results, [{ plan, rows }], { revision: 'abc', reportUrl: 'https://c.example/{run}/{slug}' });
  const lines = text.trimEnd().split('\n').map((l) => l.split('\t'));
  assert.equal(lines[0][0], 'expectation-code-coverage 1');
  assert.deepEqual(lines[1], ['source', 'app', 'abc', '-', 'https://c.example/{run}/{slug}']);
  assert.equal(lines[2].length, 17);
  assert.deepEqual(lines[2].slice(14), ['-', '-', '-']);
  assert.equal(lines[3][5], 'missing');
  void parseCoverage;
});
