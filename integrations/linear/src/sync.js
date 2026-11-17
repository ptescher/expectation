import { PARENT_LABEL, plan } from './plan.js';

/**
 * Bring the board in line with `status` (the JSON `expectation status` prints). With `dryRun`
 * nothing is written. Returns what was (or would be) done, one line each.
 */
export async function sync(status, linear, { team, dryRun = false, ...config }) {
  const t = await linear.team(team);
  const issues = await linear.issues(t.id, PARENT_LABEL);
  const actions = plan(status.features, issues, config);
  const log = [];
  const writes = actions.filter((a) => a.kind === 'create' || a.kind === 'update');
  const names = [...new Set(writes.flatMap((a) => a.labels))];
  const labelIds = writes.length ? await linear.labels(t.id, names, !dryRun) : new Map();
  for (const a of actions) {
    if (a.kind === 'orphan') {
      log.push(`orphan   ${a.identifier} «${a.title}»: no feature has this key any more (${a.key})`);
      continue;
    }
    const stateId = a.state ? t.states.get(a.state) : undefined;
    if (a.state && !stateId) throw new Error(`the team has no workflow state called «${a.state}»`);
    if (a.kind === 'create') {
      log.push(`create   «${a.title}» [${a.labels.slice(1).join(', ')}]`);
      if (dryRun) continue;
      await linear.create({
        teamId: t.id,
        title: a.title,
        description: a.description,
        labelIds: a.labels.map((n) => labelIds.get(n)),
        ...(stateId ? { stateId } : {}),
      });
    } else {
      log.push(`update   ${a.identifier} «${a.title}»${a.comment ? `: ${a.comment}` : ''}`);
      if (dryRun) continue;
      const keep = a.keep.map((n) => labelIds.get(n)).filter(Boolean);
      await linear.update(a.id, {
        title: a.title,
        description: a.description,
        labelIds: [...a.labels.map((n) => labelIds.get(n)), ...keep],
        ...(stateId ? { stateId } : {}),
      });
      if (a.comment) await linear.comment(a.id, `Expectation: ${a.comment}`);
    }
  }
  if (!log.length) log.push('nothing to change');
  return log;
}
