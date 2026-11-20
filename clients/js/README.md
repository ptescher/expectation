# expectation-driver (JavaScript)

The [Expectation](https://github.com/ptescher/expectation) driver for JavaScript: React Native,
web, Node, and test processes that drive an app through Playwright, WebdriverIO or Appium. You
bind the suite's vocabulary to your app, and the runner (`expectation-run`) drives it.

No dependencies. It needs `fetch` (Node 18 or later, any current browser, React Native).

## Install

It is not on npm yet. Copy `index.js` and `index.d.ts` (and `package.json`) into your project,
or add this repository as a git submodule, and depend on the directory:

```json
{ "devDependencies": { "expectation-driver": "file:vendor/expectation-driver" } }
```

Use the files from the same commit as the `expectation-run` you installed.

## Use

```js
import { ExpectationDriver } from 'expectation-driver';

const driver = new ExpectationDriver({ platform: 'web', app: 'Shop' })
  .observe('the cart', () => cart.items.length)          // read a state: a string or a number
  .arrange('the cart', n => cart.fill(n))                // put a state in place before a test
  .set('the dial', v => kettle.setDial(v))               // a state the outside world changes
  .on('the shopper adds an item', () => cart.add('tea')) // make an event happen
  .onReset(() => cart.clear())                           // before each test
  .onScreenshot(() => page.screenshot().then(b => b.toString('base64')));  // optional

driver.start('http://127.0.0.1:4477');   // where expectation-run listens
// ...
driver.stop();
```

- The first argument is the word as the suite declares it: a state's subject (`State: the cart
  has 0..20 items` is `'the cart'`), or the whole event.
- Handlers may be `async`.
- `arrange` falls back to `set` when a state has no `arrange` handler.
- A word with no handler is not an error. Tests that need it are reported as "not supported
  yet", and `expectation-run --strict` fails the run for them.
- `start` returns at once. The driver says hello, long-polls the runner for commands, and
  retries quietly while no runner is listening, so it can stay in a development build.
- `onControls`, `onUsed` and `onAttachments` are described in [`index.d.ts`](index.d.ts).

## Running a suite

`expectation-run <suite-dir>` listens, waits for the driver, runs every test through it and
prints a line for each. See the [`cart-node` example](../../examples/cart-node) for a script
that starts both, and the guide:

- [Getting started without Rust](https://expectation.sim001.net/guide/getting-started-node.html)
- [Remote drivers](https://expectation.sim001.net/guide/remote.html): embedded or external,
  and the protocol

## Tests

```bash
cargo build -p expectation_remote    # from the repository's root
npm test
```
