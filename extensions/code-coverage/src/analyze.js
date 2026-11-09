// Associating coverage snapshots with the run and step that produced them.
//
// A coverage counter is cumulative: it counts from when the app started, and an app that
// hands it over after each step gives running totals. What a step added is the difference
// between its snapshot and the one before. That is only a claim when:
//   - the counters never went down (they did if the app restarted), and
//   - the snapshot before is the previous step's (else the difference spans several steps).
// Nothing here can tell activity the step caused from activity that merely happened during it,
// so the page says what the number is. Execution is observed, never judged: a verdict is not
// touched.

import { totals } from './formats.js';

const newByKind = (counts, before) => {
  const n = { L: 0, B: 0, F: 0 };
  for (const [k, c] of counts) if (c > 0 && !((before.get(k) ?? 0) > 0)) n[k[0]] += 1;
  return [n.L, n.B, n.F];
};

/**
 * One row for each step of one test of one run.
 * @param {{ snapshots: Map<number, {counts: Map<string, number>} | {error: string}>, steps: number }} test
 * @param {{ counters?: 'cumulative' | 'per-step' }} options
 */
export function rowsFor(test, options = {}) {
  const perStep = options.counters === 'per-step';
  if (!test.snapshots.size) {
    return [{ step: 0, state: 'missing', basis: 'none', note: 'no coverage was attached to this test' }];
  }
  const rows = [];
  let prev = null; // { step, counts } of the last usable snapshot
  for (let step = 0; step <= test.steps; step++) {
    const s = test.snapshots.get(step);
    if (!s) {
      rows.push({ step, state: 'missing', basis: 'none', note: 'no capture at this step' });
      continue;
    }
    if ('error' in s) {
      rows.push({ step, state: 'incomplete', basis: 'none', note: s.error });
      continue;
    }
    let counts = s.counts;
    let row;
    if (perStep) {
      // Each capture is what ran since the one before: the totals are the running sum.
      const before = prev ? prev.counts : new Map();
      counts = new Map(before);
      for (const [k, n] of s.counts) counts.set(k, (counts.get(k) ?? 0) + n);
      row = prev
        ? { basis: prev.step === step - 1 ? 'step' : `spans:${prev.step}`, new: newByKind(counts, before), note: '' }
        : { basis: 'baseline', new: null, note: '' };
    } else if (!prev) {
      row = { basis: 'baseline', new: null, note: '' };
    } else if ([...prev.counts].some(([k, n]) => s.counts.has(k) && s.counts.get(k) < n)) {
      row = { basis: 'restart', new: null, note: 'a counter went down, so the program restarted' };
    } else {
      row = {
        basis: prev.step === step - 1 ? 'step' : `spans:${prev.step}`,
        new: newByKind(counts, prev.counts),
        note: '',
      };
    }
    prev = { step, counts };
    rows.push({ step, state: 'captured', totals: totals({ counts }), ...row });
  }
  return rows;
}
