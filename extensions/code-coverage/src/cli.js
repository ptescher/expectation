// expectation-code-coverage: the command.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve, sep } from 'node:path';
import { rowsFor } from './analyze.js';
import { coverageKind, merge, parseCoverage } from './formats.js';
import { readResults, ran } from './results.js';
import { line } from './tsv.js';

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
  --counters     cumulative (default): each capture is the totals since the app started, so a
                 step's part is the difference. per-step: the app empties its counters when
                 handing them over, so each capture is that step's own
  --id           the file's name (default code)
  --out          write here instead

Coverage is observed execution. It changes no result and no verdict. A step with no capture, or
with one that can't be read, is shown as that, not as uncovered code.`;

export function parseArgs(argv) {
  const o = { id: 'code', counters: 'cumulative' };
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
    if (!/^[A-Za-z0-9._-]+$/.test(o.id)) throw new Error('--id is letters, digits, ".", "_" and "-"');
    if (o.reportUrl && !/^https?:\/\//.test(o.reportUrl)) throw new Error('--report-url is an http(s) address');
  }
  return o;
}

/** The coverage snapshots a test's steps attached, by step; a file that can't be read is an error entry. */
export function snapshotsOf(plan, dir) {
  const root = resolve(dir);
  const snapshots = new Map();
  const files = new Map();
  for (const a of plan.attachments) {
    const kind = coverageKind(a.path, a.mediaType);
    if (!kind) continue;
    files.set(a.step, [...(files.get(a.step) ?? []), { kind, a }]);
  }
  for (const [step, list] of files) {
    let merged = null;
    let error = null;
    for (const { kind, a } of list) {
      const path = resolve(root, a.path);
      // A results file is input too: an attachment can't point outside the directory.
      if (!path.startsWith(root + sep)) throw new Error(`${a.path}: outside ${dir}`);
      try {
        const one = parseCoverage(kind, readFileSync(path, 'utf8'));
        merged = merged ? merge(merged, one) : one;
      } catch (e) {
        error = `${a.path.split('/').pop()}: ${e.message}`;
      }
    }
    snapshots.set(step, error ? { error } : merged);
  }
  const steps = Math.max(0, ...snapshots.keys(), ...plan.windows.keys(), ...plan.attachments.map((a) => a.step));
  return { snapshots, steps };
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
  const tests = [];
  for (const plan of results.plans) {
    if (!plan.run || !ran(plan)) continue;
    tests.push({ plan, rows: rowsFor(snapshotsOf(plan, o.attachments), o) });
  }
  const file = `${o.id}${results.platform ? `@${results.platform}` : ''}.code.tsv`;
  const out = o.out ?? join(dirname(o.results), file);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, coverageFile(results, tests, o));
  const rows = tests.flatMap((t) => t.rows);
  const n = (s) => rows.filter((r) => r.state === s).length;
  log(`${tests.length} tests: ${n('captured')} captures, ${n('missing')} missing, ${n('incomplete')} unusable -> ${out}`);
  if (!o.revision) log('note: without --revision the manual cannot say which source was instrumented');
  return out;
}
