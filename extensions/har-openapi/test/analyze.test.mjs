import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { analyze, coverageFile } from '../src/analyze.js';
import { main } from '../src/cli.js';
import { parseHar } from '../src/har.js';
import { parseSpec, readSpec } from '../src/openapi.js';
import { parseResults } from 'expectation-extension-kit';

const FIXTURE = fileURLToPath(new URL('./fixtures/quote.yaml', import.meta.url));
const spec = readSpec(FIXTURE);

// Two tests of run r1: the quote story (start 1000–1100, step 1 1100–1300, step 2 1300–1400)
// and another test after it (1400–1600).
const RESULTS = `expectation-results 1
run\tr1\t1000
plan\tget-a-quote\tpassed\tfp-quote
step\t0\tpassed\t1\t-\t-
window\t0\t1000\t1100
step\t1\tpassed\t1\t-\t-
window\t1\t1100\t1300
step\t2\tpassed\t1\t-\t-
window\t2\t1300\t1400
plan\tbrowse\tpassed\tfp-browse
step\t0\tpassed\t1\t-\t-
window\t0\t1400\t1500
step\t1\tpassed\t1\t-\t-
window\t1\t1500\t1600
`;
const results = parseResults(RESULTS, 'results/app@iOS.tsv');

/** A HAR entry: `at` and `time` in milliseconds. */
function entry({ method = 'POST', url = 'https://api.example.com/v1/quote', at = 1150, time = 50, status = 200, mime = 'application/json', body, content = {}, extra = {} }) {
  const text = body === undefined ? undefined : typeof body === 'string' ? body : JSON.stringify(body);
  return {
    startedDateTime: new Date(at).toISOString(),
    time,
    request: {
      method,
      url,
      headers: [
        { name: 'Authorization', value: 'Bearer sk_live_SECRET' },
        { name: 'Cookie', value: 'session=SECRET' },
      ],
      postData: { mimeType: 'application/json', text: '{"password":"SECRET"}' },
    },
    response: {
      status,
      headers: [{ name: 'Set-Cookie', value: 'session=SECRET' }],
      content: { mimeType: mime, text, size: text === undefined ? 10 : Buffer.byteLength(text), ...content },
    },
    ...extra,
  };
}

function har(...entries) {
  return parseHar(JSON.stringify({ log: { version: '1.2', entries } }), 'capture.har');
}

const FIRM = { kind: 'firm', price: { amount: 3 }, note: 'SECRET note' };

function run(entries, opts = {}) {
  const a = analyze(results, spec, [har(...entries)], { complete: true, source: 'live', ...opts });
  return { a, file: coverageFile(a) };
}

function state(a, label) {
  const found = a.items.filter((i) => i.label === label);
  assert.equal(found.length, 1, `one item ${label}: ${a.items.map((i) => i.label).join(' | ')}`);
  return found[0].state;
}

function seenBy(a, label) {
  const i = a.items.find((x) => x.label === label);
  return a.seen.filter(([id]) => id === i.id).map(([, e]) => a.evidence.find((x) => x.id === e));
}

test('the quote fixture reports what was seen, what was not, and links it to the step', () => {
  const { a, file } = run([entry({ body: FIRM })]);
  assert.equal(state(a, '200 A quote'), 'observed');
  assert.equal(state(a, '429 Rate limited'), 'not-observed');
  assert.equal(state(a, 'default: Something went wrong'), 'not-observed');
  assert.equal(state(a, '200 kind = "firm"'), 'observed');
  assert.equal(state(a, '200 kind = "indicative"'), 'not-observed');
  assert.equal(state(a, '200 price is Fixed (oneOf)'), 'observed');
  assert.equal(state(a, '200 price is Range (oneOf)'), 'not-observed');
  // The observation is tied to the test, its fingerprint, and the step it happened in.
  const [ev] = seenBy(a, '200 A quote');
  assert.equal(ev.plan.slug, 'get-a-quote');
  assert.equal(ev.step, 1);
  assert.equal(ev.attribution, 'timed');
  assert.match(file, /^evidence\te1\tr1\tget-a-quote\tfp-quote\t1\ttimed\tlive\tPOST \/quote → 200$/m);
  assert.match(file, /^contract\tQuote API\tAPI contract\tsha256:[0-9a-f]{16} \(version 1\.2\.0\)$/m);
  assert.match(file, /^complete\tr1\tyes\t-$/m);
  // Only the operation the tests called is reported; the rest are left out, and say so.
  assert.ok(a.items.every((i) => i.group === 'POST /quote'));
  assert.match(file, /^excluded\t4 operations no test called\t/m);
  // The locations are where the contract defines each outcome.
  const loc = (label) => a.items.find((i) => i.label === label).loc;
  assert.equal(loc('429 Rate limited'), '#/paths/~1quote/post/responses/429');
  assert.equal(loc('200 kind = "firm"'), '#/components/schemas/Quote/properties/kind');
});

test('nothing secret reaches the coverage file', () => {
  const { file } = run([
    entry({ body: FIRM, url: 'https://api.example.com/v1/quote?token=SECRET&account=SECRET' }),
    entry({ body: { kind: 'SECRETVALUE', price: { amount: 1 } }, at: 1160 }),
    entry({ method: 'GET', url: 'https://api.example.com/v1/accounts/8f14e45fceea167a5a36dedd4bea2543/SECRET-token-xyz', at: 1170 }),
  ]);
  for (const bad of ['SECRET', 'Bearer', 'password', 'token=', 'session=', '8f14e45f']) {
    assert.ok(!file.includes(bad), `${bad} in:\n${file}`);
  }
  // An undocumented value is described, not shown.
  assert.match(file, /^undocumented\tPOST \/quote\t200 kind: a string \(11 characters\) that isn't one of its values\te2$/m);
  assert.match(file, /^unmatched\tGET \/v1\/accounts\/\{…\}\/\{…\}\te3\t/m);
});

test("a body that can't be read makes what it might have shown unknown, not unseen", () => {
  for (const [what, e] of [
    ['truncated', entry({ body: '{"kind": "indic', content: { size: 400 }, at: 1160 })],
    ['missing', entry({ at: 1160, content: { text: undefined } })],
    ['redacted', entry({ body: '{}', content: { comment: 'body redacted by the capture' }, at: 1160 })],
    ['not-json', entry({ body: '<html/>', mime: 'application/json', content: {}, at: 1160, extra: {} })],
  ]) {
    const entries = [entry({ body: FIRM }), e];
    if (what === 'not-json') entries[1].response.content.mimeType = 'application/json';
    const { a } = run(entries);
    // What the readable body showed is still observed.
    assert.equal(state(a, '200 kind = "firm"'), 'observed', what);
    // What the unreadable one might have shown can't be told.
    const indicative = a.items.find((i) => i.label === '200 kind = "indicative"');
    assert.equal(indicative.state, 'unknown', what);
    assert.match(indicative.reason, /of 2 had no usable body/, what);
    // The status is still seen.
    assert.equal(state(a, '200 A quote'), 'observed', what);
  }
});

test('requests are matched by server, path template and method, concrete paths first', () => {
  const get = (path, status = 200) => entry({ method: 'GET', url: `https://api.example.com/v1${path}`, status, body: undefined, mime: '' });
  const { a, file } = run([get('/quotes/latest'), get('/quotes/123', 404), get('/quotes/abc', 418), get('/health'), entry({ method: 'DELETE', body: undefined })]);
  const groups = (label) => a.evidence.filter((e) => e.label.startsWith(label)).length;
  assert.equal(groups('GET /quotes/latest → 200'), 1);
  assert.equal(groups('GET /quotes/{id} → 404'), 1);
  // 418 falls under 4XX, as the contract allows.
  assert.equal(state(a, '4XX A bad request'), 'observed');
  assert.equal(state(a, '404 No such quote'), 'observed');
  assert.match(file, /^unmatched\tGET \/v1\/health\te4\tno operation in the contract matches its path$/m);
  assert.match(file, /^unmatched\tDELETE \/v1\/quote\te5\tthe contract has no DELETE there \(only POST\)$/m);
});

test('a status the contract does not describe is undocumented; default and ranges are not claimed as exact', () => {
  const { a, file } = run([entry({ status: 503, body: undefined, mime: '' }), entry({ method: 'GET', url: 'https://api.example.com/v1/quotes/1', status: 500, body: undefined, mime: '', at: 1160 })]);
  // 503 on POST /quote falls under default, which is what the contract says about it.
  assert.equal(state(a, 'default: Something went wrong'), 'observed');
  assert.equal(state(a, '200 A quote'), 'not-observed');
  // GET /quotes/{id} has no default and no 5XX: undocumented.
  assert.match(file, /^undocumented\tGET \/quotes\/\{id\}\tstatus 500\te2$/m);
});

test('media types: a problem+json error is read, an undocumented media type is reported', () => {
  const { a, file } = run([
    entry({ status: 429, mime: 'application/problem+json; charset=utf-8', body: { type: 'rate-limit' } }),
    entry({ status: 200, mime: 'text/html', body: '<p>quote</p>', at: 1160 }),
  ]);
  assert.equal(state(a, '429 Rate limited'), 'observed');
  assert.match(file, /^undocumented\tPOST \/quote\t200: a text\/html body\te2$/m);
});

test('requests are placed by time: within a step, across steps, during setup, nowhere, or in two tests at once', () => {
  const { a } = run([
    entry({ body: FIRM, at: 1150, time: 10 }),
    entry({ body: FIRM, at: 1250, time: 200 }),
    entry({ body: FIRM, at: 1050, time: 10 }),
    entry({ body: FIRM, at: 5000, time: 10 }),
  ]);
  assert.deepEqual(
    a.evidence.map((e) => [e.attribution, e.step]),
    [
      ['timed', 1],
      ['spans', 1],
      ['setup', 0],
      ['unassigned', null],
    ],
  );
  // Tests that ran at the same time (in parallel) can't be told apart by time.
  const parallel = parseResults(RESULTS.replace('window\t0\t1400\t1500', 'window\t0\t1100\t1500'), 'app.tsv');
  const b = analyze(parallel, spec, [har(entry({ body: FIRM, at: 1150 }))], { complete: true });
  assert.equal(b.evidence[0].attribution, 'unassigned');
  // A clock that was off is corrected.
  const c = analyze(results, spec, [har(entry({ body: FIRM, at: 50 }))], { complete: true, clockOffset: 1000 });
  assert.equal(c.evidence[0].attribution, 'setup');
});

test('a capture can declare its test and step, and its source', () => {
  const declared = entry({ body: FIRM, at: 99999, extra: { _expectation: { run: 'r1', test: 'browse', step: 1, source: 'fault' } } });
  const { a, file } = run([declared]);
  assert.equal(a.evidence[0].attribution, 'declared');
  assert.match(file, /^evidence\te1\tr1\tbrowse\tfp-browse\t1\tdeclared\tfault\t/m);
});

test('streams are classified as unknown, not as covered messages', () => {
  const { a } = run([entry({ method: 'GET', url: 'https://api.example.com/v1/prices', mime: 'text/event-stream', body: 'data: 1\n\n' })]);
  assert.equal(state(a, '200 Prices as they change'), 'observed');
  const stream = a.items.find((i) => i.label === '200: stream messages');
  assert.equal(stream.state, 'unknown');
  assert.match(stream.reason, /not assessed/);
});

test("schema forms this extension can't assess are listed as unknown", () => {
  const { a } = run([entry({ method: 'GET', url: 'https://api.example.com/v1/odd', body: { shape: 1, remote: {} } })]);
  const unknown = a.items.filter((i) => i.state === 'unknown').map((i) => i.label);
  assert.ok(unknown.includes('200 shape: uses not'), unknown.join(' | '));
  assert.ok(unknown.some((l) => l.startsWith('200 remote: uses an external $ref')), unknown.join(' | '));
});

test('without --complete, the coverage file says the capture is partial', () => {
  const { file } = run([entry({ body: FIRM })], { complete: false });
  assert.match(file, /^complete\tr1\tpartial\tthe capture was not said to be complete \(--complete\)$/m);
  // HAR entries that can't be read make it partial too, whatever --complete says.
  const broken = parseHar(JSON.stringify({ log: { entries: [entry({ body: FIRM }), { request: {} }] } }), 'x');
  const a = analyze(results, spec, [broken], { complete: true });
  assert.deepEqual(a.complete[0], ['r1', 'partial', "1 HAR entries couldn't be read"]);
});

test('traffic to hosts the contract does not name is left out, and counted', () => {
  const { a, file } = run([entry({ body: FIRM }), entry({ method: 'GET', url: 'https://analytics.example.net/collect', at: 1160 })]);
  assert.equal(a.evidence.length, 1);
  assert.match(file, /^excluded\trequests to other hosts\t1 requests to 1 hosts the contract's servers don't name$/m);
  // --server adds a base, so a capture of staging matches.
  const b = analyze(results, spec, [har(entry({ url: 'https://staging.example.com/api/quote', body: FIRM }))], { complete: true, servers: ['https://staging.example.com/api'] });
  assert.equal(b.evidence[0].label, 'POST /quote → 200');
});

test('--operations and --all report operations no test called', () => {
  const { a } = run([entry({ body: FIRM })], { operations: ['GET /quotes/{id}'] });
  assert.equal(state(a, '404 No such quote'), 'not-observed');
  assert.throws(() => run([entry({ body: FIRM })], { operations: ['GET /nope'] }), /no operation GET \/nope/);
  const all = run([entry({ body: FIRM })], { all: true }).a;
  assert.ok(all.items.some((i) => i.group === 'GET /prices'));
});

test('OpenAPI 3.1: type lists with null, and $ref with siblings', () => {
  const doc = {
    openapi: '3.1.0',
    info: { title: 'T', version: '1' },
    paths: {
      '/x': {
        get: {
          responses: {
            200: {
              description: 'ok',
              content: {
                'application/json': {
                  schema: {
                    type: 'object',
                    properties: {
                      level: { $ref: '#/components/schemas/Level', description: 'a sibling that changes nothing' },
                      maybe: { type: ['string', 'null'], enum: ['a', 'b', null] },
                    },
                  },
                },
              },
            },
          },
        },
      },
    },
    components: { schemas: { Level: { enum: ['low', 'high'] } } },
  };
  const s = parseSpec(JSON.stringify(doc), 'x.json');
  const a = analyze(results, s, [har(entry({ method: 'GET', url: 'https://any.example.org/x', body: { level: 'high', maybe: null } }))], { complete: true });
  assert.equal(state(a, '200 level = "high"'), 'observed');
  assert.equal(state(a, '200 level = "low"'), 'not-observed');
  assert.equal(state(a, '200 maybe = null'), 'observed');
});

test('the contract is read defensively', () => {
  assert.throws(() => parseSpec('swagger: "2.0"\n', 's.yaml'), /not an OpenAPI 3\.0 or 3\.1 document/);
  assert.throws(() => parseSpec('{', 's.json'), /not JSON or YAML/);
  // Aliases can't multiply into a huge document.
  const bomb = 'a: &a [x,x,x,x,x,x,x,x,x]\n' + Array.from({ length: 12 }, (_, i) => `${String.fromCharCode(98 + i)}: &${String.fromCharCode(98 + i)} [${Array(9).fill('*' + String.fromCharCode(97 + i)).join(',')}]`).join('\n');
  assert.throws(() => parseSpec(bomb, 'bomb.yaml'));
  // References that lead nowhere, back to themselves, or into the prototype are unknown.
  const loops = {
    openapi: '3.0.0',
    info: { title: 'L', version: '1' },
    paths: { '/l': { get: { responses: { 200: { description: 'ok', content: { 'application/json': { schema: { $ref: '#/components/schemas/A' } } } } } } } },
    components: { schemas: { A: { $ref: '#/components/schemas/B' }, B: { $ref: '#/components/schemas/A' } } },
  };
  const a = analyze(results, parseSpec(JSON.stringify(loops), 'l.json'), [har(entry({ method: 'GET', url: 'https://x.org/l', body: {} }))], { complete: true });
  assert.ok(a.items.some((i) => i.state === 'unknown' && /leads back to itself/.test(i.label)));
  const proto = JSON.parse(JSON.stringify(loops));
  proto.components.schemas.A = { $ref: '#/__proto__/polluted' };
  const b = analyze(results, parseSpec(JSON.stringify(proto), 'p.json'), [har(entry({ method: 'GET', url: 'https://x.org/l', body: {} }))], { complete: true });
  assert.ok(b.items.some((i) => i.state === 'unknown' && /a \$ref to nothing/.test(i.label)));
  assert.equal({}.polluted, undefined);
  // The revision names the content: a change makes another one.
  const r1 = parseSpec(JSON.stringify(loops), 'l.json').revision;
  loops.info.version = '2';
  assert.notEqual(parseSpec(JSON.stringify(loops), 'l.json').revision, r1);
});

test('the command reads attachments, refuses paths outside their directory, and writes next to the results', () => {
  const dir = mkdtempSync(join(tmpdir(), 'har-openapi-'));
  const kept = join(dir, 'attachments');
  mkdirSync(join(kept, 'r1', 'get-a-quote', '01'), { recursive: true });
  writeFileSync(join(kept, 'r1', 'get-a-quote', '01', 'network.har'), JSON.stringify({ log: { entries: [entry({ body: FIRM, at: 99999 })] } }));
  mkdirSync(join(dir, 'results'));
  const withAttachment = RESULTS.replace('window\t1\t1100\t1300\n', 'window\t1\t1100\t1300\nattachment\t1\tapplication/har+json\tr1/get-a-quote/01/network.har\n');
  writeFileSync(join(dir, 'results', 'app@iOS.tsv'), withAttachment);
  const logs = [];
  const out = main(['--results', join(dir, 'results', 'app@iOS.tsv'), '--openapi', FIXTURE, '--attachments', kept, '--complete', '--source', 'stand-in'], (s) => logs.push(s));
  assert.equal(out, join(dir, 'results', 'api@iOS.coverage.tsv'));
  const file = readFileSync(out, 'utf8');
  // Declared by where the driver kept it, whatever its clock said.
  assert.match(file, /^evidence\te1\tr1\tget-a-quote\tfp-quote\t1\tdeclared\tstand-in\tPOST \/quote → 200$/m);
  // The other test that ran has no capture, so the run's capture isn't complete.
  assert.match(file, /^complete\tr1\tpartial\t1 tests that ran have no capture$/m);
  assert.match(logs[0], /1 requests/);

  const escape = RESULTS.replace('window\t1\t1100\t1300\n', 'window\t1\t1100\t1300\nattachment\t1\tapplication/har+json\t../../etc/passwd.har\n');
  writeFileSync(join(dir, 'results', 'app@iOS.tsv'), escape);
  assert.throws(() => main(['--results', join(dir, 'results', 'app@iOS.tsv'), '--openapi', FIXTURE, '--attachments', kept], () => {}), /outside/);
  assert.throws(() => main(['--results', 'x', '--openapi', FIXTURE], () => {}), /--har or --attachments/);
  assert.throws(() => main(['--id', '../x', '--results', 'x', '--openapi', FIXTURE, '--har', 'h'], () => {}), /--id is/);
});
