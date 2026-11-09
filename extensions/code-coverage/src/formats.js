// Reading coverage data: LCOV (`.info`, `.lcov`) and Istanbul's JSON (`coverage-final.json`).
// Both become a Map from the key of one thing in the source (a line, a branch, a function) to
// how many times it ran, so two snapshots can be compared.

/**
 * @typedef {{ counts: Map<string, number> }} Snapshot
 *   keys: `L|file|line`, `B|file|line|block|branch`, `F|file|name`
 */

const add = (m, key, n) => m.set(key, (m.get(key) ?? 0) + n);

/**
 * A problem with a capture, in words that are safe to print in the manual. The manual shows
 * these, so they never quote the file: a parser's own message can repeat its first characters.
 */
export class CoverageError extends Error {}

/** An LCOV tracefile's text. Throws if it holds nothing of the kind. */
export function parseLcov(text) {
  const counts = new Map();
  let file = null;
  for (const raw of text.split(/\r?\n/)) {
    const l = raw.trim();
    if (l.startsWith('SF:')) file = l.slice(3);
    else if (l === 'end_of_record') file = null;
    else if (file === null) continue;
    else if (l.startsWith('DA:')) {
      const [line, hits] = l.slice(3).split(',');
      if (/^\d+$/.test(line) && /^\d+$/.test(hits)) add(counts, `L|${file}|${line}`, Number(hits));
    } else if (l.startsWith('BRDA:')) {
      const [line, block, branch, taken] = l.slice(5).split(',');
      if (line !== undefined && taken !== undefined) {
        add(counts, `B|${file}|${line}|${block}|${branch}`, /^\d+$/.test(taken) ? Number(taken) : 0);
      }
    } else if (l.startsWith('FN:')) {
      const name = l.slice(3).split(',').slice(1).join(',');
      if (name) add(counts, `F|${file}|${name}`, 0);
    } else if (l.startsWith('FNDA:')) {
      const [hits, ...name] = l.slice(5).split(',');
      if (/^\d+$/.test(hits) && name.length) add(counts, `F|${file}|${name.join(',')}`, Number(hits));
    }
  }
  if (!counts.size) throw new CoverageError('not LCOV data, or nothing was instrumented');
  return { counts };
}

/** Istanbul's coverage map (`coverage-final.json`) as parsed JSON. */
export function parseIstanbul(json) {
  const counts = new Map();
  if (!json || typeof json !== 'object' || Array.isArray(json)) throw new CoverageError('not an Istanbul coverage map');
  for (const [key, f] of Object.entries(json)) {
    if (!f || typeof f !== 'object' || !f.statementMap) continue;
    const file = f.path ?? key;
    // A line ran as often as its most-run statement that starts there, as Istanbul reports it.
    const lines = new Map();
    for (const [id, loc] of Object.entries(f.statementMap)) {
      const line = loc?.start?.line;
      if (!Number.isInteger(line)) continue;
      lines.set(line, Math.max(lines.get(line) ?? 0, Number(f.s?.[id] ?? 0)));
    }
    for (const [line, n] of lines) add(counts, `L|${file}|${line}`, n);
    for (const [id, taken] of Object.entries(f.b ?? {})) {
      const line = f.branchMap?.[id]?.line ?? f.branchMap?.[id]?.loc?.start?.line ?? 0;
      (Array.isArray(taken) ? taken : []).forEach((n, i) => add(counts, `B|${file}|${line}|${id}|${i}`, Number(n) || 0));
    }
    for (const [id, n] of Object.entries(f.f ?? {})) {
      // By id: two functions can share a name.
      add(counts, `F|${file}|${id}:${f.fnMap?.[id]?.name ?? ''}`, Number(n) || 0);
    }
  }
  if (!counts.size) throw new CoverageError('no statements in the Istanbul coverage map');
  return { counts };
}

/**
 * Is this attachment coverage data, going by its name and media type? `sure` is whether the name
 * or type says so for certain: `.info` and `coverage.json` are also what other tools write, so
 * a file like that which isn't coverage is just something else, not a broken capture.
 * @returns {{ kind: 'lcov' | 'istanbul', sure: boolean } | null}
 */
export function coverageKind(path, mediaType = '') {
  const name = path.toLowerCase();
  if (name.endsWith('.lcov') || /lcov/.test(mediaType)) return { kind: 'lcov', sure: true };
  if (name.endsWith('.info')) return { kind: 'lcov', sure: false };
  if (/istanbul/.test(mediaType) || name.endsWith('.istanbul.json')) return { kind: 'istanbul', sure: true };
  if (/(^|[/-])coverage(-final)?\.json$/.test(name)) return { kind: 'istanbul', sure: false };
  return null;
}

/** The snapshot in `text`, whichever format `kind` says it is. */
export function parseCoverage(kind, text) {
  if (kind === 'lcov') return parseLcov(text);
  let json;
  try {
    json = JSON.parse(text);
  } catch {
    throw new CoverageError('not valid JSON');
  }
  return parseIstanbul(json);
}

/**
 * Join two files captured at the same step. Counts are cumulative, so the same thing in two files
 * is one thing seen twice (a duplicate), not two: the larger count stands.
 */
export function merge(a, b) {
  const counts = new Map(a.counts);
  for (const [k, n] of b.counts) counts.set(k, Math.max(counts.get(k) ?? 0, n));
  return { counts };
}

/** Hit and found for lines, branches and functions. */
export function totals(snapshot) {
  const t = { L: { hit: 0, found: 0 }, B: { hit: 0, found: 0 }, F: { hit: 0, found: 0 } };
  for (const [k, n] of snapshot.counts) {
    const c = t[k[0]];
    c.found += 1;
    if (n > 0) c.hit += 1;
  }
  return t;
}
