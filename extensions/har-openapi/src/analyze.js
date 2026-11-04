// Holding captured traffic against an OpenAPI document: which documented responses and body
// variants the tests' requests got, which they didn't, and which can't be told. The result is
// an Expectation coverage file; Expectation then checks it against the results it is shown with.

import { ran } from './results.js';
import {
  matchRequest,
  mediaFor,
  observe,
  operations,
  origins,
  readable,
  response,
  responseFor,
  safePath,
  variants,
} from './openapi.js';
import { line } from './tsv.js';

const SOURCES = new Set(['live', 'replay', 'stand-in', 'fault', 'unknown']);

/**
 * Where an entry belongs among the results: the test step whose window it falls in. A capture
 * the driver made for a step is declared; one that falls in two tests' windows (tests run in
 * parallel) belongs to neither.
 */
function attribute(results, e, offset, declared) {
  if (declared) return { ...declared, attribution: 'declared' };
  const d = e.declared;
  if (d?.run && d?.test && d?.step !== undefined) {
    const plan = results.plans.find((p) => p.slug === d.test && p.run === d.run);
    if (plan) return { run: d.run, plan, step: d.step, attribution: 'declared' };
  }
  const start = e.start + offset;
  const end = e.end + offset;
  const hits = [];
  for (const plan of results.plans) {
    if (!plan.run) continue;
    for (const [step, w] of plan.windows) {
      if (w.start <= start && start <= w.end) hits.push({ plan, step, w });
    }
  }
  const plans = new Set(hits.map((h) => h.plan));
  if (hits.length === 0 || plans.size > 1) {
    return { run: null, plan: null, step: null, attribution: 'unassigned', why: plans.size > 1 ? 'overlaps tests' : null };
  }
  const { plan, step, w } = hits[0];
  const attribution = step === 0 ? 'setup' : end <= w.end ? 'timed' : 'spans';
  return { run: plan.run, plan, step, attribution };
}

function count(list) {
  const by = new Map();
  for (const x of list) by.set(x, (by.get(x) ?? 0) + 1);
  return [...by].map(([k, n]) => `${n} ${k}`).join(', ');
}

/**
 * The coverage of `spec` by the traffic in `captures`, among `results`.
 *
 * @param {import('./results.js').Results} results
 * @param {ReturnType<import('./openapi.js').parseSpec>} spec
 * @param {Array<{ entries: import('./har.js').Entry[], unusable: number,
 *   declared?: { run: string, plan: any, step: number } }>} captures
 * @param {{ source?: string, complete?: boolean, clockOffset?: number, servers?: string[],
 *   hosts?: string[], operations?: string[], all?: boolean }} opts
 */
export function analyze(results, spec, captures, opts = {}) {
  const source = opts.source ?? 'unknown';
  if (!SOURCES.has(source)) throw new Error(`a source is one of ${[...SOURCES].join(', ')}`);
  const ops = operations(spec, opts.servers ?? []);
  const named = origins(ops);
  const anyRelative = ops.some((o) => o.bases.some((b) => !b.origin));
  const hosts = (opts.hosts ?? []).map((h) => h.toLowerCase());
  const inScope = (url) => {
    const host = url.host.toLowerCase();
    if (hosts.length) return hosts.includes(host);
    return anyRelative || named.includes(host);
  };

  const evidence = [];
  const unmatched = [];
  const undocumented = [];
  /** Per matched evidence: what it shows. */
  const shows = [];
  let otherHosts = 0;
  const others = new Set();
  let unusable = 0;
  for (const cap of captures) {
    unusable += cap.unusable;
    for (const e of cap.entries) {
      if (!inScope(e.url)) {
        otherHosts += 1;
        others.add(e.url.host.toLowerCase());
        continue;
      }
      const where = attribute(results, e, opts.clockOffset ?? 0, cap.declared);
      const ev = {
        id: `e${evidence.length + 1}`,
        run: where.run,
        plan: where.plan,
        step: where.step,
        attribution: where.attribution,
        source: SOURCES.has(e.declared?.source) ? e.declared.source : source,
        label: '',
      };
      evidence.push(ev);
      const m = matchRequest(ops, e.method, e.url);
      if (!m.op) {
        ev.label = `${e.method} ${safePath(e.url, ops)} → ${e.status}`;
        unmatched.push({
          label: `${e.method} ${safePath(e.url, ops)}`,
          evidence: ev.id,
          reason: m.ambiguous ? `matches ${m.ambiguous.length} operations: ${m.ambiguous.join(', ')}` : m.none,
        });
        continue;
      }
      const op = m.op;
      const r = responseFor(op, e.status);
      const show = { ev, op, status: e.status, key: r?.key ?? null, media: null, body: e.body.state, stream: e.stream, seen: new Set(), unclear: new Set() };
      shows.push(show);
      ev.label = `${op.label} → ${e.status}`;
      if (!r) {
        undocumented.push({ group: op.label, label: `status ${e.status}`, evidence: ev.id });
        continue;
      }
      const res = response(spec, op, r.key);
      if (!('node' in res)) continue;
      const content = res.node?.content;
      if (content && e.body.state !== 'empty' && e.mime) {
        show.media = mediaFor(content, e.mime);
        if (!show.media) {
          undocumented.push({ group: op.label, label: `${r.key}: a ${e.mime} body`, evidence: ev.id });
        }
      }
      if (e.body.state !== 'ok' && e.body.state !== 'empty') ev.label += ` (body ${e.body.state})`;
      if (show.media && e.body.state === 'ok' && readable(show.media) && !e.stream) {
        const schema = content[show.media]?.schema;
        if (schema !== undefined) {
          const o = observe(spec, schema, `${res.loc}/content/${show.media.replace(/~/g, '~0').replace(/\//g, '~1')}/schema`, e.body.value);
          show.seen = o.seen;
          show.unclear = o.unclear;
          for (const u of o.undocumented) undocumented.push({ group: op.label, label: `${r.key} ${u}`, evidence: ev.id });
        }
      }
    }
  }

  // The operations to report: those the tests called, and any asked for.
  const called = new Set(shows.map((s) => s.op));
  const asked = new Set();
  for (const label of opts.operations ?? []) {
    const op = ops.find((o) => o.label.toLowerCase() === label.trim().toLowerCase());
    if (!op) throw new Error(`no operation ${label} in the contract (they are like "POST /quote")`);
    asked.add(op);
  }
  const scope = ops.filter((o) => opts.all || called.has(o) || asked.has(o));

  const items = [];
  const seen = [];
  const item = (op, label, loc, state, reason, by) => {
    const id = `i${items.length + 1}`;
    items.push({ id, group: op.label, label, loc, state, reason });
    for (const ev of by) seen.push([id, ev.id]);
  };
  for (const op of scope) {
    const mine = shows.filter((s) => s.op === op);
    const responses = op.op.responses && typeof op.op.responses === 'object' ? Object.keys(op.op.responses) : [];
    if (responses.length === 0) {
      item(op, 'responses', `${op.loc}/responses`, 'unknown', 'the operation documents no responses', []);
    }
    for (const key of responses) {
      const res = response(spec, op, key);
      const loc = `${op.loc}/responses/${key.replace(/~/g, '~0').replace(/\//g, '~1')}`;
      const desc = 'node' in res && typeof res.node?.description === 'string' ? res.node.description.split('\n')[0].slice(0, 60) : '';
      const label = key === 'default' ? `default${desc ? `: ${desc}` : ''}` : `${key}${desc ? ` ${desc}` : ''}`;
      const got = mine.filter((s) => s.key === key);
      item(op, label, loc, got.length ? 'observed' : 'not-observed', '', got.map((s) => s.ev));
      if (!('node' in res)) {
        item(op, `${key}: its body`, res.loc, 'unknown', `the response uses ${res.unsupported}`, []);
        continue;
      }
      const content = res.node?.content && typeof res.node.content === 'object' ? res.node.content : {};
      const medias = Object.keys(content);
      for (const media of medias) {
        const tag = medias.length > 1 ? ` (${media})` : '';
        const schema = content[media]?.schema;
        if (/event-stream/i.test(media)) {
          item(op, `${key}${tag}: stream messages`, `${loc}/content/${media.replace(/\//g, '~1')}`, 'unknown', 'messages on a stream are not assessed, only the connection', []);
          continue;
        }
        if (!readable(media) || schema === undefined) continue;
        const schemaLoc = `${res.loc}/content/${media.replace(/~/g, '~0').replace(/\//g, '~1')}/schema`;
        const these = got.filter((s) => s.media === media);
        for (const v of variants(spec, schema, schemaLoc)) {
          const label = `${key}${tag} ${v.label}`;
          if (v.kind === 'unsupported') {
            item(op, label, v.loc, 'unknown', "this extension doesn't assess that", []);
            continue;
          }
          const by = these.filter((s) => s.seen.has(v.key));
          if (by.length) {
            item(op, label, v.loc, 'observed', '', by.map((s) => s.ev));
            continue;
          }
          const unusableBodies = these.filter((s) => s.body !== 'ok' || s.stream).map((s) => (s.stream ? 'a stream' : s.body));
          const unclear = these.filter((s) => s.body === 'ok' && s.unclear.has(v.key)).length;
          if (unusableBodies.length || unclear) {
            const parts = [];
            if (unusableBodies.length) parts.push(`${unusableBodies.length} of ${these.length} had no usable body (${count(unusableBodies)})`);
            if (unclear) parts.push(`${unclear} couldn't be placed in the schema here`);
            item(op, label, v.loc, 'unknown', `of the ${key} responses, ${parts.join('; ')}`, []);
          } else {
            item(op, label, v.loc, 'not-observed', '', []);
          }
        }
      }
    }
  }

  // How complete the capture of each run is.
  const complete = [];
  const runs = [...new Set(results.plans.filter((p) => ran(p) && p.run).map((p) => p.run))];
  const fromAttachments = captures.some((c) => c.declared);
  for (const run of runs) {
    let how = opts.complete ? 'yes' : 'partial';
    let note = opts.complete ? '' : 'the capture was not said to be complete (--complete)';
    if (unusable > 0) {
      how = 'partial';
      note = `${unusable} HAR entries couldn't be read`;
    }
    if (fromAttachments) {
      const bare = results.plans.filter(
        (p) => p.run === run && ran(p) && !captures.some((c) => c.declared?.plan === p),
      );
      if (bare.length) {
        how = 'partial';
        note = `${bare.length} tests that ran have no capture`;
      }
    }
    complete.push([run, how, note]);
  }

  const excluded = [];
  const left = ops.length - scope.length;
  if (left > 0) {
    excluded.push([`${left} operations no test called`, 'pass --operations or --all to hold them to the capture too']);
  }
  if (otherHosts > 0) {
    excluded.push([`requests to other hosts`, `${otherHosts} requests to ${others.size} hosts the contract's servers don't name`]);
  }

  return { spec, results, evidence, items, seen, complete, undocumented, unmatched, excluded };
}

/** The analysis as an Expectation coverage file. */
export function coverageFile(a, name) {
  let out = 'expectation-coverage 1\n';
  out += line('contract', name ?? a.spec.title, 'API contract', a.spec.revision);
  for (const ev of a.evidence) {
    out += line(
      'evidence',
      ev.id,
      ev.run ?? '-',
      ev.plan?.slug ?? '-',
      ev.plan?.fingerprint ?? '-',
      ev.step ?? '-',
      ev.attribution,
      ev.source,
      ev.label,
    );
  }
  for (const i of a.items) out += line('item', i.id, i.group, i.label, i.loc, i.state, i.reason);
  for (const [i, e] of a.seen) out += line('seen', i, e);
  for (const [run, how, note] of a.complete) out += line('complete', run, how, note);
  for (const u of a.undocumented) out += line('undocumented', u.group, u.label, u.evidence);
  for (const u of a.unmatched) out += line('unmatched', u.label, u.evidence, u.reason);
  for (const [g, why] of a.excluded) out += line('excluded', g, why);
  return out;
}
