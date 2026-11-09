// Finding the files a driver attached to a run: what the results say was kept, in the directory
// `expectation-run --attachments` (or `RunOptions::attachments_dir`) wrote them to.

import { realpathSync } from 'node:fs';
import { resolve, sep } from 'node:path';

/**
 * The attachments of the tests that ran, for which `wanted(attachment)` says yes, each with the
 * file's full path. A results file is input too, so an attachment can't point outside `dir`,
 * not even through a link.
 *
 * @param {import('./results.js').Results} results
 * @param {string} dir
 * @param {(a: import('./results.js').Attachment) => boolean} wanted
 * @returns {{ plan: import('./results.js').PlanResult, step: number, mediaType: string,
 *             name: string, path: string }[]}
 */
export function attachedFiles(results, dir, wanted) {
  const root = realpathSync(resolve(dir));
  const out = [];
  for (const plan of results.plans) {
    if (!plan.run) continue;
    for (const a of plan.attachments) {
      if (!wanted(a)) continue;
      const lexical = resolve(root, a.path);
      if (!lexical.startsWith(root + sep)) throw new Error(`${a.path}: outside ${dir}`);
      let path;
      try {
        path = realpathSync(lexical);
      } catch {
        throw new Error(`${a.path}: not in ${dir}`);
      }
      if (!path.startsWith(root + sep)) throw new Error(`${a.path}: outside ${dir}`);
      out.push({ plan, step: a.step, mediaType: a.mediaType, name: a.path.split('/').pop(), path });
    }
  }
  return out;
}
