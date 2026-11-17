import test from 'node:test';
import assert from 'node:assert/strict';
import { plan, wanted, identify } from '../src/plan.js';
import { Linear } from '../src/linear.js';
import { sync } from '../src/sync.js';

const feature = (over = {}) => ({
  name: 'Heating',
  slug: 'heating',
  section: '',
  file: 'heating.expect',
  line: 3,
  approval: 'approved',
  verdict: 'verified',
  pending: 0,
  expectations: [
    { name: 'Turning the dial starts it', slug: 'a', approval: 'approved', pending: false, verdict: 'verified' },
  ],
  ...over,
});

const issueFor = (f, over = {}, config = {}) => {
  const w = wanted(f, config);
  return {
    id: 'i1',
    identifier: 'ENG-1',
    title: w.title,
    description: w.description,
    state: { id: 's1', name: 'Todo' },
    labels: w.labels.map((name) => ({ id: name, name })),
    ...over,
  };
};

test('a feature with no issue is created, labelled with its approval and verdict', () => {
  const [a] = plan([feature()], []);
  assert.equal(a.kind, 'create');
  assert.deepEqual(a.labels, ['expectation', 'spec:approved', 'tests:verified']);
  assert.deepEqual(identify(a.description).key, './heating');
});

test('an issue that already says it is left alone', () => {
  const f = feature();
  assert.deepEqual(plan([f], [issueFor(f)]), []);
});

test('a change of standing updates the issue, keeps other labels, and says what changed', () => {
  const was = feature();
  const now = feature({ verdict: 'failed' });
  const issue = issueFor(was);
  issue.labels.push({ id: 'x', name: 'urgent' });
  const [a] = plan([now], [issue]);
  assert.equal(a.kind, 'update');
  assert.equal(a.comment, 'tests: verified → failed');
  assert.deepEqual(a.keep, ['urgent']);
});

test('a feature the spec no longer has is reported, not touched', () => {
  const gone = issueFor(feature({ slug: 'gone', name: 'Gone' }));
  const [a] = plan([], [gone]);
  assert.equal(a.kind, 'orphan');
});

test('an issue in the wrong workflow state is moved when the config says where', () => {
  const f = feature();
  const config = { states: { verified: 'Done' } };
  const [a] = plan([f], [issueFor(f, {}, config)], config);
  assert.equal(a.kind, 'update');
  assert.equal(a.state, 'Done');
});

const fakeLinear = (existing = []) => {
  const calls = [];
  const fetch = async (_url, { body }) => {
    const { query, variables } = JSON.parse(body);
    calls.push({ query, variables });
    const reply = (data) => ({ ok: true, json: async () => ({ data }) });
    if (query.includes('teams(')) return reply({ teams: { nodes: [{ id: 't', key: 'ENG', states: { nodes: [{ id: 'sd', name: 'Done' }] } }] } });
    if (query.includes('issues(')) return reply({ issues: { nodes: existing.map((i) => ({ ...i, labels: { nodes: i.labels } })), pageInfo: { hasNextPage: false } } });
    if (query.includes('issueLabels(')) return reply({ issueLabels: { nodes: [] } });
    if (query.includes('issueLabelCreate')) return reply({ issueLabelCreate: { issueLabel: { id: `id:${variables.input.name}` } } });
    if (query.includes('issueCreate')) return reply({ issueCreate: { issue: { id: 'new', identifier: 'ENG-2' } } });
    return reply({});
  };
  return { linear: new Linear('key', fetch), calls };
};

test('a dry run asks and writes nothing', async () => {
  const { linear, calls } = fakeLinear();
  const log = await sync({ features: [feature()] }, linear, { team: 'ENG', dryRun: true });
  assert.match(log[0], /^create/);
  assert.ok(!calls.some((c) => c.query.includes('mutation')));
});

test('a real run creates the labels it needs, then the issue with their ids', async () => {
  const { linear, calls } = fakeLinear();
  await sync({ features: [feature()] }, linear, { team: 'ENG' });
  const create = calls.find((c) => c.query.includes('issueCreate'));
  assert.deepEqual(create.variables.input.labelIds, ['id:expectation', 'id:spec:approved', 'id:tests:verified']);
  assert.equal(create.variables.input.teamId, 't');
});

test('a second run against what the first made changes nothing', async () => {
  const f = feature();
  const { linear } = fakeLinear([issueFor(f)]);
  assert.deepEqual(await sync({ features: [f] }, linear, { team: 'ENG' }), ['nothing to change']);
});
