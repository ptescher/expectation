// The driver: one handler per word of the suite's vocabulary. This one is embedded: it calls
// the cart's own methods. For a web app, the same handlers would click and read the page
// through Playwright or WebdriverIO (see the guide's "Getting started without Rust").
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
// Until `expectation-driver` is on npm, it is used from the repository.
const { ExpectationDriver } = require('../../clients/js/index.js');
const { Cart } = require('./cart.js');

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
