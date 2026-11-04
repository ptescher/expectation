// Reading an Expectation results file (`results/<runner>[@platform].tsv`): which tests ran in
// which run, their fingerprints, when each step ran, and what the driver attached.

import { readFileSync } from 'node:fs';
import { basename } from 'node:path';

/**
 * @typedef {{ start: number, end: number }} Window
 * @typedef {{ step: number, mediaType: string, path: string }} Attachment
 * @typedef {{ slug: string, status: string, fingerprint: string, run: string | null,
 *             windows: Map<number, Window>, attachments: Attachment[] }} PlanResult
 * @typedef {{ file: string, runner: string, platform: string | null,
 *             runs: Map<string, number>, plans: PlanResult[] }} Results
 */

/** The results in `text`, saved by Expectation; `file` is its name, for the platform. */
export function parseResults(text, file) {
  const lines = text.split('\n');
  if (lines[0]?.trim() !== 'expectation-results 1') {
    throw new Error(`${file}: not an Expectation results file (no "expectation-results 1" line)`);
  }
  const name = basename(file).replace(/\.tsv$/, '');
  const [runner, platform] = name.includes('@') ? name.split('@', 2) : [name, null];
  /** @type {Results} */
  const out = { file, runner, platform, runs: new Map(), plans: [] };
  let run = null;
  let plan = null;
  for (const l of lines.slice(1)) {
    const f = l.split('\t');
    switch (f[0]) {
      case 'run':
        if (f.length >= 3 && f[1] !== '-' && /^\d+$/.test(f[2])) {
          run = f[1];
          out.runs.set(run, Number(f[2]));
        } else {
          run = null;
        }
        break;
      case 'plan':
        if (f.length < 3) break;
        plan = {
          slug: f[1],
          status: f[2],
          fingerprint: f[3] ?? '',
          run,
          windows: new Map(),
          attachments: [],
        };
        out.plans.push(plan);
        break;
      case 'window':
        if (plan && f.length === 4 && [1, 2, 3].every((i) => /^\d+$/.test(f[i]))) {
          plan.windows.set(Number(f[1]), { start: Number(f[2]), end: Number(f[3]) });
        }
        break;
      case 'attachment':
        if (plan && f.length === 4 && /^\d+$/.test(f[1])) {
          plan.attachments.push({ step: Number(f[1]), mediaType: f[2], path: f[3] });
        }
        break;
      default:
        break;
    }
  }
  return out;
}

export function readResults(path) {
  return parseResults(readFileSync(path, 'utf8'), path);
}

/** Did this result run (pass or fail), so that it could have made traffic? */
export function ran(plan) {
  return plan.status === 'passed' || plan.status === 'failed';
}
