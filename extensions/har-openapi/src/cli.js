// expectation-har-openapi: the command.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { analyze, coverageFile } from './analyze.js';
import { readHar } from './har.js';
import { readSpec } from './openapi.js';
import { attachedFiles, readResults } from 'expectation-extension-kit';

export const USAGE = `usage: expectation-har-openapi --results <results.tsv> --openapi <api.yaml|json>
         [--har <capture.har>]... [--attachments <dir>]
         [--source live|replay|stand-in|fault|unknown] [--complete]
         [--server <url>]... [--host <host>]... [--operations "POST /quote, GET /prices"] [--all]
         [--clock-offset <ms>] [--id <name>] [--name <contract name>] [--out <file>]

Holds the HTTP traffic a test run captured against an OpenAPI document, and writes the contract
coverage that \`expectation docs\` shows next to the results:
<results dir>/<id>[@platform].coverage.tsv.

  --results      the run's results, as Expectation saved them
  --openapi      the contract, OpenAPI 3.0 or 3.1, JSON or YAML
  --har          a capture made by any tool; its requests are placed in test steps by time
  --attachments  where the run kept what the driver captured (RunOptions::attachments_dir,
                 expectation-run --attachments); its HAR files are already placed in steps
  --source       where the traffic came from (default unknown)
  --complete     say the capture holds all of each run's traffic, with every body; without it
                 nothing is reported as not observed
  --server       a base URL to match requests against, as well as the document's servers
  --host         only hold requests to this host to the contract (default: the servers' hosts)
  --operations   also report these operations, even if no test called them
  --all          report every operation in the document
  --clock-offset add this many milliseconds to the capture's times, if its clock was off
  --id           the coverage file's name (default api)
  --name         the contract's name in the manual (default the document's title)
  --out          write here instead`;

export function parseArgs(argv) {
  const o = { har: [], servers: [], hosts: [], operations: [], id: 'api' };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const value = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} needs a value`);
      return v;
    };
    switch (a) {
      case '--results': o.results = value(); break;
      case '--openapi': o.openapi = value(); break;
      case '--har': o.har.push(value()); break;
      case '--attachments': o.attachments = value(); break;
      case '--source': o.source = value(); break;
      case '--complete': o.complete = true; break;
      case '--server': o.servers.push(value()); break;
      case '--host': o.hosts.push(value()); break;
      case '--operations': o.operations.push(...value().split(',').map((s) => s.trim()).filter(Boolean)); break;
      case '--all': o.all = true; break;
      case '--clock-offset': {
        const n = Number(value());
        if (!Number.isFinite(n)) throw new Error('--clock-offset is a number of milliseconds');
        o.clockOffset = n;
        break;
      }
      case '--id': o.id = value(); break;
      case '--name': o.name = value(); break;
      case '--out': o.out = value(); break;
      case '-h':
      case '--help': o.help = true; break;
      default: throw new Error(`unknown option ${a}`);
    }
  }
  if (!o.help) {
    if (!o.results || !o.openapi) throw new Error('--results and --openapi are needed');
    if (!o.har.length && !o.attachments) throw new Error('give the traffic with --har or --attachments');
    if (!/^[A-Za-z0-9._-]+$/.test(o.id)) throw new Error('--id is letters, digits, ".", "_" and "-"');
  }
  return o;
}

/** The HAR files the driver attached, each placed in its test step. */
function attached(results, dir) {
  return attachedFiles(results, dir, (a) => /har/i.test(a.mediaType) || a.path.endsWith('.har')).map((f) => ({
    ...readHar(f.path),
    declared: { run: f.plan.run, plan: f.plan, step: f.step },
  }));
}

export function main(argv, log = (s) => process.stderr.write(s + '\n')) {
  const o = parseArgs(argv);
  if (o.help) {
    process.stdout.write(USAGE + '\n');
    return null;
  }
  const results = readResults(o.results);
  const spec = readSpec(o.openapi);
  const captures = [...o.har.map((p) => readHar(p)), ...(o.attachments ? attached(results, o.attachments) : [])];
  const a = analyze(results, spec, captures, o);
  const file = `${o.id}${results.platform ? `@${results.platform}` : ''}.coverage.tsv`;
  const out = o.out ?? join(dirname(o.results), file);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, coverageFile(a, o.name));
  const n = (s) => a.items.filter((i) => i.state === s).length;
  log(
    `${a.evidence.length} requests, ${a.unmatched.length} unmatched; ${a.items.length} outcomes: ` +
      `${n('observed')} observed, ${n('not-observed')} not observed, ${n('unknown')} unknown -> ${out}`,
  );
  if (!o.complete) log('note: without --complete, nothing is reported as not observed');
  return out;
}
