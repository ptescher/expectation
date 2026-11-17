#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { Linear } from '../src/linear.js';
import { sync } from '../src/sync.js';

const usage = `usage: expectation status <suite> <out> > status.json
       LINEAR_API_KEY=… expectation-linear --status status.json --team <KEY> [options]

  --status <file>      the JSON expectation status printed ('-' for stdin)
  --team <KEY>         the Linear team's key (the prefix of its issue ids)
  --config <file>      JSON: { "states": { "verified": "Done", "failed": "Blocked" }, "sourceUrl": "https://github.com/o/r/blob/main" }
  --dry-run            say what would change and change nothing
`;

const args = process.argv.slice(2);
const take = (name) => {
  const i = args.indexOf(name);
  if (i < 0) return null;
  return args.splice(i, 2)[1] ?? null;
};
const status = take('--status');
const team = take('--team');
const configFile = take('--config');
const dryRun = args.includes('--dry-run');
const key = process.env.LINEAR_API_KEY;
if (!status || !team || !key) {
  process.stderr.write(usage + (key ? '' : '\nLINEAR_API_KEY is not set.\n'));
  process.exit(2);
}
try {
  const json = JSON.parse(readFileSync(status === '-' ? 0 : status, 'utf8'));
  const config = configFile ? JSON.parse(readFileSync(configFile, 'utf8')) : {};
  for (const line of await sync(json, new Linear(key), { team, dryRun, ...config })) console.log(line);
} catch (e) {
  process.stderr.write(`error: ${e.message}\n`);
  process.exit(1);
}
