// Runs the suite: starts the runner, connects the driver to it, and exits as the runner does.
//
//   node run.mjs            # needs `expectation-run` on the PATH, or $EXPECTATION_RUN
//
// The runner listens, the driver connects out to it, the runner drives every test through the
// driver and writes the manual to docs/.
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { cartDriver } from './driver.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const addr = process.env.EXPECTATION_ADDR || '127.0.0.1:4477';

const runner = spawn(
  process.env.EXPECTATION_RUN || 'expectation-run',
  [
    path.join(here, 'expectations'),
    '--addr', addr,
    '--docs', path.join(here, 'docs'),
    '--runner', 'node',
    // The cart answers at once, so there is nothing to wait for after each action.
    '--settle', '0',
    // A gate: every test must have run and passed, so an unbound word fails the run.
    '--strict',
  ],
  { stdio: 'inherit' },
);
runner.on('error', e => {
  console.error(`couldn't start expectation-run: ${e.message}`);
  console.error('install it: cargo install --git https://github.com/ptescher/expectation expectation_remote');
  process.exit(2);
});

const driver = cartDriver();
driver.start(`http://${addr}`);

runner.on('exit', code => {
  driver.stop();
  process.exit(code ?? 1);
});
