import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseLcov, parseIstanbul, coverageKind, merge, totals } from '../src/formats.js';
import { rowsFor } from '../src/analyze.js';

const lcov = (hits) =>
  `SF:src/a.js\n${hits.map((h, i) => `DA:${i + 1},${h}`).join('\n')}\nBRDA:1,0,0,${hits[0] ? 1 : '-'}\nFN:1,main\nFNDA:${hits[0]},main\nend_of_record\n`;
const snap = (hits) => parseLcov(lcov(hits));

test('LCOV: lines, branches (a dash is not taken) and functions', () => {
  const t = totals(snap([2, 0, 1]));
  assert.deepEqual(t.L, { hit: 2, found: 3 });
  assert.deepEqual(t.B, { hit: 1, found: 1 });
  assert.deepEqual(t.F, { hit: 1, found: 1 });
  assert.deepEqual(totals(snap([0, 0])).B, { hit: 0, found: 1 });
  assert.throws(() => parseLcov('nothing here'), /not LCOV data/);
});

test('Istanbul: a line ran as often as its most-run statement', () => {
  const s = parseIstanbul({
    '/x/a.js': {
      path: '/x/a.js',
      statementMap: { 0: { start: { line: 1 } }, 1: { start: { line: 1 } }, 2: { start: { line: 2 } } },
      s: { 0: 0, 1: 3, 2: 0 },
      branchMap: { 0: { line: 2 } },
      b: { 0: [1, 0] },
      fnMap: { 0: { name: 'go' } },
      f: { 0: 1 },
    },
  });
  const t = totals(s);
  assert.deepEqual(t.L, { hit: 1, found: 2 });
  assert.deepEqual(t.B, { hit: 1, found: 2 });
  assert.deepEqual(t.F, { hit: 1, found: 1 });
  assert.throws(() => parseIstanbul([]), /not an Istanbul/);
});

test('attachments are recognised by name or media type', () => {
  assert.deepEqual(coverageKind('r/t/00/x.lcov'), { kind: 'lcov', sure: true });
  assert.deepEqual(coverageKind('r/t/00/x', 'text/x-lcov'), { kind: 'lcov', sure: true });
  // Names other tools use too: coverage only if the content is.
  assert.deepEqual(coverageKind('r/t/00/lcov.info'), { kind: 'lcov', sure: false });
  assert.deepEqual(coverageKind('r/t/00/coverage-final.json'), { kind: 'istanbul', sure: false });
  assert.equal(coverageKind('r/t/00/network.har', 'application/json'), null);
});

const test3 = (snapshots, steps = 2) => ({ snapshots: new Map(snapshots), steps });

test('cumulative counters: the first capture is a baseline, later ones add what newly ran', () => {
  const rows = rowsFor(test3([[0, snap([1, 0, 0])], [1, snap([2, 1, 0])], [2, snap([3, 1, 1])]]));
  assert.deepEqual(rows.map((r) => r.basis), ['baseline', 'step', 'step']);
  assert.equal(rows[0].new, null);
  assert.deepEqual(rows[1].new, [1, 0, 0]);
  assert.deepEqual(rows[2].new, [1, 0, 0]);
  assert.deepEqual(rows[2].totals.L, { hit: 3, found: 3 });
});

test('a counter that goes down means the program restarted: nothing is attributed to that step', () => {
  const rows = rowsFor(test3([[0, snap([5, 5, 0])], [1, snap([1, 0, 0])], [2, snap([2, 1, 0])]]));
  assert.equal(rows[1].basis, 'restart');
  assert.equal(rows[1].new, null);
  // Later steps are held against the restarted program, not the one before.
  assert.equal(rows[2].basis, 'step');
  assert.deepEqual(rows[2].new, [1, 0, 0]);
});

test('a missing capture is missing, not zero, and the next difference says what step it is since', () => {
  const rows = rowsFor(test3([[0, snap([1, 0, 0])], [2, snap([1, 1, 1])]]));
  assert.equal(rows[1].state, 'missing');
  assert.equal(rows[1].totals, undefined);
  assert.equal(rows[2].basis, 'since:0');
  assert.deepEqual(rows[2].new, [2, 0, 0]);
});

test('an unreadable capture is incomplete, and a test with none is missing', () => {
  const rows = rowsFor(test3([[0, snap([1])], [1, { error: 'lcov.info: not an LCOV file' }]], 1));
  assert.equal(rows[1].state, 'incomplete');
  assert.match(rows[1].note, /not an LCOV/);
  const none = rowsFor(test3([]));
  assert.deepEqual(none.map((r) => r.state), ['missing']);
});

test('per-step counters: each capture is that step\'s own, and totals run on', () => {
  const rows = rowsFor(test3([[0, snap([1, 0, 0])], [1, snap([0, 1, 0])], [2, snap([0, 0, 0])]]), { counters: 'per-step' });
  assert.deepEqual(rows.map((r) => r.basis), ['baseline', 'step', 'step']);
  assert.deepEqual(rows[1].new, [1, 0, 0]);
  assert.deepEqual(rows[2].new, [0, 0, 0]);
  assert.deepEqual(rows[2].totals.L, { hit: 2, found: 3 });
});

test('something that had run and is no longer counted is a restart too', () => {
  const was = parseLcov('SF:a.js\nDA:1,3\nend_of_record\nSF:b.js\nDA:1,3\nend_of_record\n');
  const now = parseLcov('SF:a.js\nDA:1,9\nend_of_record\n');
  const rows = rowsFor(test3([[0, was], [1, now]], 1));
  assert.equal(rows[1].basis, 'restart');
});

test('the same thing in two files at one step is one thing, not two', () => {
  const a = parseLcov('SF:a.js\nDA:1,2\nend_of_record\n');
  const joined = merge(a, a);
  assert.equal(joined.counts.get('L|a.js|1'), 2);
  const other = merge(a, parseLcov('SF:b.js\nDA:1,1\nend_of_record\n'));
  assert.deepEqual(totals(other).L, { hit: 2, found: 2 });
});

test('Istanbul functions are told apart by id, not name', () => {
  const s = parseIstanbul({
    'a.js': { path: 'a.js', statementMap: { 0: { start: { line: 1 } } }, s: { 0: 1 }, fnMap: { 0: { name: 'render' }, 1: { name: 'render' } }, f: { 0: 1, 1: 0 } },
  });
  assert.deepEqual(totals(s).F, { hit: 1, found: 2 });
});
