// The system under test: a shopping cart. Nothing here knows about Expectation.
'use strict';

class Cart {
  constructor() {
    this.items = [];
    this.checkout = 'closed';
  }

  add(name) {
    if (this.checkout === 'closed' && this.items.length < 20) this.items.push(name);
  }

  openCheckout() {
    if (this.checkout === 'closed' && this.items.length > 0) this.checkout = 'open';
  }

  pay() {
    if (this.checkout !== 'open') return;
    this.items = [];
    this.checkout = 'paid';
  }
}

module.exports = { Cart };
