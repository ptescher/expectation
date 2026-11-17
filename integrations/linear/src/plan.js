// What the board should say, from `expectation status` JSON, and what has to change to make it so.
// Pure: no network. `sync.js` does the asking and the writing.

import { createHash } from 'node:crypto';

export const PARENT_LABEL = 'expectation';
const KEY = 'expectation-key:';
const HASH = 'expectation-hash:';

export const specLabel = (approval) => `spec:${approval}`;
export const testsLabel = (verdict) => `tests:${verdict}`;
const isOurs = (name) => name.startsWith('spec:') || name.startsWith('tests:') || name === PARENT_LABEL;

/** A feature's identity on the board: its section and slug, stable until it is renamed. */
export const keyOf = (f) => `${f.section || '.'}/${f.slug}`;

const sourceLink = (f, sourceUrl) => {
  const where = `${f.file}:${f.line}`;
  return sourceUrl ? `[${where}](${sourceUrl.replace(/\/$/, '')}/${f.file}#L${f.line})` : where;
};

/** The issue body: what the feature is, then its expectations, then the two lines that find it again. */
export function describe(f, { sourceUrl } = {}) {
  const rows = f.expectations.map((e) => {
    const marks = [e.approval, e.pending ? 'pending' : null, e.verdict].filter(Boolean).join(', ');
    return `- ${e.name} (${marks})`;
  });
  return [
    `**Spec:** ${f.approval} · **Tests:** ${f.verdict}${f.pending ? ` · ${f.pending} not built yet` : ''}`,
    `Source: ${sourceLink(f, sourceUrl)}`,
    '',
    ...rows,
  ].join('\n');
}

const sha = (s) => createHash('sha256').update(s).digest('hex').slice(0, 12);

/** What one feature should look like on the board. */
export function wanted(f, config = {}) {
  const body = describe(f, config);
  const labels = [PARENT_LABEL, specLabel(f.approval), testsLabel(f.verdict)];
  const state = config.states?.[f.verdict] ?? null;
  const hash = sha(JSON.stringify([f.name, body, labels, state]));
  return {
    key: keyOf(f),
    title: f.name,
    labels,
    state,
    hash,
    description: `${body}\n\n${KEY} ${keyOf(f)}\n${HASH} ${hash}`,
    feature: f,
  };
}

/** The key and hash an issue's description carries, if it was made by this tool. */
export function identify(description) {
  const line = (tag) =>
    (description ?? '').split('\n').find((l) => l.startsWith(tag))?.slice(tag.length).trim() ?? null;
  return { key: line(KEY), hash: line(HASH) };
}

/**
 * The changes that bring `issues` (what the board has) in line with `features` (what the spec
 * says): create, update, comment on a change of standing, and report what the spec no longer has.
 */
export function plan(features, issues, config = {}) {
  const byKey = new Map();
  for (const issue of issues) {
    const { key, hash } = identify(issue.description);
    if (key) byKey.set(key, { issue, hash });
  }
  const actions = [];
  const seen = new Set();
  for (const f of features) {
    const w = wanted(f, config);
    seen.add(w.key);
    const have = byKey.get(w.key);
    if (!have) {
      actions.push({ kind: 'create', ...w });
      continue;
    }
    const labelNames = have.issue.labels.map((l) => l.name);
    const standing = (names, p) => names.find((n) => n.startsWith(p)) ?? null;
    const before = {
      spec: standing(labelNames, 'spec:'),
      tests: standing(labelNames, 'tests:'),
    };
    const stateWrong = w.state && have.issue.state?.name !== w.state;
    if (have.hash === w.hash && !stateWrong && before.spec === w.labels[1] && before.tests === w.labels[2]) {
      continue;
    }
    const changed = [];
    if (before.spec !== w.labels[1]) changed.push(`spec: ${before.spec?.slice(5) ?? 'none'} → ${f.approval}`);
    if (before.tests !== w.labels[2]) changed.push(`tests: ${before.tests?.slice(6) ?? 'none'} → ${f.verdict}`);
    actions.push({
      kind: 'update',
      id: have.issue.id,
      identifier: have.issue.identifier,
      ...w,
      keep: labelNames.filter((n) => !isOurs(n)),
      comment: changed.length ? changed.join('; ') : null,
    });
  }
  for (const [key, { issue }] of byKey) {
    if (!seen.has(key)) actions.push({ kind: 'orphan', key, identifier: issue.identifier, title: issue.title });
  }
  return actions;
}
