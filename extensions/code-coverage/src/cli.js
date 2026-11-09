// expectation-code-coverage: the command.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { attachedFiles, line, ran, readResults } from 'expectation-extension-kit';
import { rowsFor } from './analyze.js';
import { CoverageError, coverageKind, merge, parseCoverage } from './formats.js';

export const USAGE = `usage: expectation-code-coverage --results <results.tsv> --attachments <dir>
         [--revision <commit>] [--scope <what is instrumented>] [--name <program>]
         [--report-url <address>] [--counters cumulative|per-step] [--id <name>] [--out <file>]

Associates the coverage an instrumented app handed over (LCOV, or Istanbul's coverage-final.json,
as step attachments) with the run, test and step that produced it, and writes the code coverage
that \`expectation docs\` shows next to the results:
<results dir>/<id>[@platform].code.tsv.

  --results      the run's results, as Expectation saved them
  --attachments  where the run kept what the driver captured (expectation-run --attachments)
  --revision     the source revision that was instrumented (say it: coverage is about one)
  --scope        which source is counted, e.g. "src/**, without tests"
  --name         the program's name in the manual (default the results' runner)
  --report-url   a separately hosted HTML report; {run} and {slug} stand for the run and the test
                 (the manual fills them in for each test)
  --counters     cumulative (default): each capture is the totals since the app started, so a
                 step's part is the difference. per-step: the app empties its counters when
                 handing them over, so each capture is that step's own
  --id           the file's name (default: the results' runner, so app.code.tsv)
  --out          write here instead

Coverage is observed execution. It changes no result and no verdict. A step with no capture, or
with one that can't be read, is shown as that, not as uncovered code.`;

export function parseArgs(argv) {
  const o = { counters: 'cumulative' };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const value = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} needs a value`);
      return v;
    };
    switch (a) {
      case '--results': o.results = value(); break;
      case '--attachments': o.attachments = value(); break;
      case '--revision': o.revision = value(); break;
      case '--scope': o.scope = value(); break;
      case '--name': o.name = value(); break;
      case '--report-url': o.reportUrl = value(); break;
      case '--counters': o.counters = value(); break;
      case '--id': o.id = value(); break;
      case '--out': o.out = value(); break;
      case '-h':
      case '--help': o.help = true; break;
      default: throw new Error(`unknown option ${a}`);
    }
  }
  if (!o.help) {
    if (!o.results || !o.attachments) throw new Error('--results and --attachments are needed');
    if (!['cumulative', 'per-step'].includes(o.counters)) throw new Error('--counters is cumulative or per-step');
    if (o.id !== undefined && !/^[A-Za-z0-9._-]+$/.test(o.id)) throw new Error('--id is letters, digits, ".", "_" and "-"');
    if (o.reportUrl && !/^https?:\/\//.test(o.reportUrl)) throw new Error('--report-url is an http(s) address');
  }
  return o;
}

/** Does this attachment look like coverage? */
export const isCoverage = (a) => coverageKind(a.path, a.mediaType) !== null;

/**
 * The coverage snapshots a test's steps attached, by step. A step's files are joined. A file that
 * can't be read makes the step an error entry, but only if it is surely coverage by its name or
 * type: a `.info` that isn't LCOV is somebody else's file. Every step must hold the same files
 * as the first (a snapshot of the frontend alone after one of both would read as a restart).
 * @param {number} steps how many steps the test has
 * @param {{ step: number, mediaType: string, name: string, path: string }[]} files
 */
export function snapshotsOf(steps, files) {
  const snapshots = new Map();
  const names = new Map();
  for (const f of files) {
    const { kind, sure } = coverageKind(f.name, f.mediaType);
    let one = null;
    try {
      one = parseCoverage(kind, readFileSync(f.path, 'utf8'));
    } catch (e) {
      // The message goes in the manual, so it is one of ours, never the file's own words.
      if (sure) {
        snapshots.set(f.step, { error: `${f.name}: ${e instanceof CoverageError ? e.message : 'could not be read'}` });
      }
      continue;
    }
    if (snapshots.get(f.step) && 'error' in snapshots.get(f.step)) continue;
    snapshots.set(f.step, snapshots.has(f.step) ? merge(snapshots.get(f.step), one) : one);
    names.set(f.step, [...(names.get(f.step) ?? []), f.name].sort());
  }
  const reference = [...names.keys()].sort((a, b) => a - b)[0];
  for (const [step, list] of names) {
    if (step !== reference && list.join('\n') !== names.get(reference).join('\n')) {
      snapshots.set(step, { error: 'holds different files from the test\'s first capture, so it can\'t be compared' });
    }
  }
  return { snapshots, steps: Math.max(steps, 0, ...snapshots.keys()) };
}

const num = (n) => String(n);

/** The code coverage file's text. */
export function coverageFile(results, tests, o) {
  let out = 'expectation-code-coverage 1\n';
  // The address may have {run} and {slug} in it; the manual fills them in for each test.
  out += line('source', o.name ?? results.runner, o.revision ?? '', o.scope ?? '', o.reportUrl ?? '');
  for (const { plan, rows } of tests) {
    for (const r of rows) {
      const t = r.totals;
      out += line(
        'capture',
        plan.run,
        plan.slug,
        plan.fingerprint,
        num(r.step),
        r.state,
        r.basis,
        r.note ?? '',
        ...(t ? [t.L.hit, t.L.found, t.B.hit, t.B.found, t.F.hit, t.F.found] : [0, 0, 0, 0, 0, 0]).map(num),
        ...(r.new ? r.new.map(num) : ['', '', '']),
      );
    }
  }
  return out;
}

export function main(argv, log = (s) => process.stderr.write(s + '\n')) {
  const o = parseArgs(argv);
  if (o.help) {
    process.stdout.write(USAGE + '\n');
    return null;
  }
  const results = readResults(o.results);
  const files = attachedFiles(results, o.attachments, isCoverage);
  const tests = [];
  for (const plan of results.plans) {
    if (!plan.run || !ran(plan)) continue;
    const mine = files.filter((f) => f.plan === plan);
    tests.push({ plan, rows: rowsFor(snapshotsOf(plan.steps, mine), o) });
  }
  const id = o.id ?? results.runner;
  const file = `${id}${results.platform ? `@${results.platform}` : ''}.code.tsv`;
  const out = o.out ?? join(dirname(o.results), file);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, coverageFile(results, tests, o));
  const rows = tests.flatMap((t) => t.rows);
  const n = (s) => rows.filter((r) => r.state === s).length;
  log(`${tests.length} tests: ${n('captured')} captures, ${n('missing')} missing, ${n('incomplete')} unusable -> ${out}`);
  if (!o.revision) log('note: without --revision the manual cannot say which source was instrumented');
  return out;
}
