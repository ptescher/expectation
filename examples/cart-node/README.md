# A cart, in Node

The smallest whole run without Rust: a suite, a JavaScript driver and `expectation-run`. The
guide's [Getting started without Rust](https://expectation.sim001.net/guide/getting-started-node.html)
walks through it.

| File | What it is |
| --- | --- |
| [`expectations/cart.expect`](expectations/cart.expect) | The suite: four expectations and a story. |
| [`cart.js`](cart.js) | The system under test. It knows nothing about Expectation. |
| [`driver.mjs`](driver.mjs) | The driver: one handler per word of the vocabulary. |
| [`run.mjs`](run.mjs) | Starts the runner, connects the driver, exits as the runner does. |

From the repository's root:

```bash
cargo build -p expectation_cli -p expectation_remote
target/debug/expectation check examples/cart-node/expectations
EXPECTATION_RUN=target/debug/expectation-run node examples/cart-node/run.mjs
```

With `expectation-run` installed, `node run.mjs` is enough. The manual is written to `docs/`
(not committed): `docs/README.md`, and `docs/site/index.html` as a website.

The run is `--strict`, so it fails if a test didn't run. Take a handler out of `driver.mjs`
to see the difference between "failed" and "not supported yet".
