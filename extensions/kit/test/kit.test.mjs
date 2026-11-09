import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { attachedFiles, line, parseResults } from '../src/index.js';

const RESULTS = `expectation-results 1
run\tr1\t100
plan\tgo\tpassed\tfp1
step\t0\tpassed\t1\t-\t-
step\t1\tpassed\t1\t-\t-
step\t2\tpassed\t1\t-\t-
window\t1\t10\t20
attachment\t1\ttext/x-lcov\tr1/go/01/lcov.info
attachment\t2\tapplication/json\t../outside.json
`;

test('a results file gives the run, the steps and the attachments', () => {
  const r = parseResults(RESULTS, 'results/app@iOS.tsv');
  assert.equal(r.platform, 'iOS');
  assert.equal(r.plans[0].run, 'r1');
  assert.equal(r.plans[0].steps, 2);
  assert.equal(r.plans[0].attachments.length, 2);
  assert.throws(() => parseResults('nope', 'x.tsv'), /not an Expectation results file/);
});

test('attached files are found, and none can leave the directory', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'expectation-kit-'));
  fs.mkdirSync(path.join(dir, 'r1/go/01'), { recursive: true });
  fs.writeFileSync(path.join(dir, 'r1/go/01/lcov.info'), 'x');
  fs.writeFileSync(path.join(path.dirname(dir), 'outside.json'), '{}');
  const r = parseResults(RESULTS, 'app.tsv');
  const found = attachedFiles(r, dir, (a) => a.mediaType.includes('lcov'));
  assert.equal(found.length, 1);
  assert.equal(found[0].step, 1);
  assert.equal(found[0].name, 'lcov.info');
  assert.throws(() => attachedFiles(r, dir, () => true), /outside/);
  // A link out of the directory is outside too.
  const link = path.join(dir, 'r1/go/01/link.info');
  fs.symlinkSync(path.join(path.dirname(dir), 'outside.json'), link);
  const sneaky = parseResults(RESULTS.replace('lcov.info', 'link.info').replace(/attachment\t2.*\n/, ''), 'app.tsv');
  assert.throws(() => attachedFiles(sneaky, dir, () => true), /outside/);
});

test('a record has no room for a tab or a newline', () => {
  assert.equal(line('k', 'a\tb', '', 'c\nd'), 'k\ta b\t-\tc d\n');
});
