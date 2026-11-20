# Getting started without Rust

[Getting started](getting-started.md) runs a suite from a Rust test. If your product is
JavaScript or TypeScript, you don't write any Rust: the suite is the same `.expect` files, the
driver is a JavaScript object, and a command called `expectation-run` runs the tests through
it. This chapter is the
[`cart-node` example](https://github.com/ptescher/expectation/tree/main/examples/cart-node)
in the repository.

## Install the commands

The two commands are built from source, so this step needs a
[Rust toolchain](https://rustup.rs) once. Nothing in your project depends on it afterwards.

```bash
cargo install --git https://github.com/ptescher/expectation expectation_cli      # expectation
cargo install --git https://github.com/ptescher/expectation expectation_remote   # expectation-run
```

For a build you can repeat, pin the commit: see [Pinning a version](cli.md#pinning-a-version).

The JavaScript client is not on npm yet. It is two files with no dependencies, in
[`clients/js`](https://github.com/ptescher/expectation/tree/main/clients/js): copy them into
your project (say `vendor/expectation-driver/`), or add the repository as a git submodule, and
depend on that directory:

```json
{ "devDependencies": { "expectation-driver": "file:vendor/expectation-driver" } }
```

## Write the suite

`expectation init` writes a starter suite to `expectations/` that you can edit. The example's
suite describes a shopping cart:

```gherkin
Suite: Cart

Vocabulary:
  State: the cart has 0..20 items
    Shows: the cart badge reads {value}
  State: the checkout is closed | open | paid
  Event: the shopper adds an item
    Say: press Add to cart on a product
  Event: the shopper opens the checkout
    Say: press Checkout
  Event: the shopper pays
    Say: enter a card and press Pay

Background:
  Given the cart has 0 items
  And the checkout is closed

Feature: Buying what is in the cart
  Expectation: Adding an item puts it in the cart
    Given the checkout is closed
    And the cart has below 20 items
    When the shopper adds an item
    Then the cart goes up by 1

  Expectation: A cart with items can be checked out
    Given the checkout is closed
    And the cart has at least 1 item
    When the shopper opens the checkout
    Then the checkout is open

  Expectation: An empty cart has nothing to check out
    Given the checkout is closed
    And the cart has 0 items
    When the shopper opens the checkout
    Then the checkout is closed

  Expectation: Paying empties the cart
    Given the checkout is open
    When the shopper pays
    Then the checkout is paid
    And the cart has 0 items

Story: Buy two things
  As a shopper
  I want to pay for everything in my cart at once
  So that it arrives together

  When the shopper adds an item
  Then the cart has 1 item
  When the shopper adds an item
  Then the cart has 2 items
  When the shopper opens the checkout
  Then the checkout is open
  When the shopper pays
  Then the checkout is paid
```

Check it. Nothing runs yet:

```bash
expectation check expectations
```

## Write the driver

The driver has one handler for each word of the vocabulary. The state's subject ("the cart",
"the checkout") names it; an event is named in full.

```js
// driver.mjs
import { ExpectationDriver } from 'expectation-driver';
import { Cart } from './cart.js';

export function cartDriver() {
  let cart = new Cart();
  return new ExpectationDriver({ platform: 'node', app: 'Cart' })
    // observe: read a state back. A named value is a string, a number is a number.
    .observe('the cart', () => cart.items.length)
    .observe('the checkout', () => cart.checkout)
    // arrange: put a state in place before a test starts, however is quickest.
    .arrange('the cart', n => { cart.items = Array.from({ length: n }, (_, i) => `item ${i + 1}`); })
    .arrange('the checkout', v => { cart.checkout = v; })
    // on: make an event happen.
    .on('the shopper adds an item', () => cart.add('tea'))
    .on('the shopper opens the checkout', () => cart.openCheckout())
    .on('the shopper pays', () => cart.pay())
    // reset: runs before each test.
    .onReset(() => { cart = new Cart(); });
}
```

Handlers may be `async`. A word with no handler is not an error: tests that need it are
reported as "not supported yet", so you can bind a suite a word at a time.

## Run it

Two processes take part. `expectation-run` reads the suite and listens on a port. The driver
connects out to it, and the runner sends it one command at a time: reset, arrange this, make
that happen, what is the cart now. The simplest arrangement is a script that starts both:

```js
// run.mjs
import { spawn } from 'node:child_process';
import { cartDriver } from './driver.mjs';

const addr = '127.0.0.1:4477';
const runner = spawn('expectation-run', [
  'expectations',
  '--addr', addr,
  '--docs', 'docs',       // save the results and write the manual here
  '--runner', 'node',     // the name these results are saved under
  '--settle', '0',        // this cart answers at once; a UI needs time after each action
  '--strict',             // every test must run and pass
], { stdio: 'inherit' });

const driver = cartDriver();
driver.start(`http://${addr}`);

runner.on('exit', code => {
  driver.stop();
  process.exit(code ?? 1);
});
```

```bash
node run.mjs
```

```text
waiting for an app at http://127.0.0.1:4477 ...
Cart (node) connected
✓ Buy two things
✓ Reaching «An empty cart has nothing to check out»
docs: /path/to/your/project/docs/README.md
```

The second test is a generated one: no story reaches that expectation, so the runner wrote a
test that does. `docs/README.md` is the manual in Markdown, and `docs/site/index.html` is the
same as a website, with "Buy two things" as a tutorial.

Read the lines, not just the exit code: `✓` passed, `✗` failed and `–` is anything else (not
supported yet, pending, not run). Without `--strict` a run where nothing was bound exits 0.

## A web app: the same driver, through the browser

For a web app, keep everything above and change what the handlers do. They run in your test
process and act on the page through a browser automation library, so an event is a real click
and an observation is what the page shows. With Playwright:

```js
import { chromium } from 'playwright';
import { ExpectationDriver } from 'expectation-driver';

const browser = await chromium.launch();
let page = await browser.newPage();

const driver = new ExpectationDriver({ platform: 'web', app: 'Shop' })
  .observe('the cart', async () => Number(await page.getByTestId('cart-badge').innerText()))
  .observe('the checkout', async () =>
    (await page.getByText('Thanks for your order').isVisible()) ? 'paid'
      : (await page.getByTestId('payment-form').isVisible()) ? 'open' : 'closed')
  .on('the shopper adds an item', () => page.getByRole('button', { name: 'Add to cart' }).first().click())
  .on('the shopper opens the checkout', () => page.getByRole('button', { name: 'Checkout' }).click())
  .on('the shopper pays', async () => {
    await page.getByLabel('Card number').fill('4242 4242 4242 4242');
    await page.getByRole('button', { name: 'Pay' }).click();
  })
  .onReset(async () => {
    await page.context().clearCookies();
    await page.goto('http://localhost:3000');
  })
  .onScreenshot(async () => (await page.screenshot()).toString('base64'));
```

This sketch is not one of the repository's tested examples: the selectors are your app's. Three
things differ from the embedded driver:

- **`arrange` needs a way in.** The Background here is the empty cart that `onReset` gives, so
  nothing is arranged. A generated test that starts from another state needs `arrange`
  handlers, and from outside the app those have to use a UI path, a seeded database or a test
  endpoint. Without them such a test is "not supported yet".
- **Give the page time.** Leave `--settle` at its default (one second) or set it to what your
  app needs; the runner also keeps looking until every check holds or its patience runs out.
- **Screenshots come for free.** With `onScreenshot`, every step of every tutorial in the
  manual has a picture.

Start your app's dev server before the run, or let the suite say how with a `Launch:` line
(see [The language](language.md#suites-and-sections)). Before a suite drives anything that
pays, posts or sends, read [Safety preflight](feature-docs.md#safety-preflight).

[Remote drivers](remote.md) has the rest: the Swift client, what an embedded driver shows and
what an external one shows, and the protocol, which any language that can make an HTTP request
can speak.

## Next

- [Spec first](spec-first.md): write a feature's expectations before it is built.
- [In CI](ci.md): check the suite on every pull request and gate a release on a run.
- [Modelling an app with data](modelling.md): lists, records and typed-in text as states.
