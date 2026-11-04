#!/usr/bin/env node
import { main, USAGE } from '../src/cli.js';

try {
  main(process.argv.slice(2));
} catch (err) {
  process.stderr.write(`${err.message}\n\n${USAGE}\n`);
  process.exit(2);
}
