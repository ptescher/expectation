# External services

Tests sometimes need something that isn't part of the system under test: a database, a
server, a container. The harness treats that as a **service**, so no test, driver or script
has to start it.

```rust
use expectation_harness::service::{Command, Docker, Ready, Services};

let mut services = Services::new()
    .with(Command::new("server", "./target/debug/server").ready(Ready::Tcp(8080)))
    .with(Docker::new("database", "postgres:16").port(5432, 55432)   // a database is a container
        .env("POSTGRES_PASSWORD", "test").ready(Ready::Tcp(55432)));

let mut driver = Bindings::new(world).needs(&["server", "database"]).fresh(&["database"]);
let report = acceptance.run_with(&mut driver, &RunOptions::default(), &mut services);
```

## Needs

A driver says what it needs (`Driver::needs`, or `Bindings::needs`). Before each test, the
runner makes sure each one is up: one that is up stays up, and one that died is started
again. They all stop when `services` is dropped, the last started first.

A test the driver can't run anyway starts nothing.

## Fresh

What a test leaves behind (rows the last test wrote, a user it signed in) is cleared by naming
the service in `Driver::fresh`. It's started afresh before each test that runs.

## Not supported yet, not failed

A service that can't run on this machine (no Docker, no build) makes the tests that need it
"not supported yet", and the reason says how to get it:

```text
database: install PostgreSQL with `brew install postgresql`
```

A test suite on a laptop without Docker still runs everything else, and the gap is visible in
the docs.

## Providers

| Provider | Starts |
| --- | --- |
| `Command::new(name, program)` | A program, with `.arg`, `.env` and `.hint` (how to install it). Ready when `.ready(…)` says so. |
| `Docker::new(name, image)` | A container, with `.port(inside, outside)`, `.env`, `.container(name)`. `.build_from(dir)` builds the image when it isn't there. On a Mac, it wakes Docker Desktop. |
| `Present::new(name, path)` | Nothing: a build or a file that must exist. |
| `Fallback::new(name, ways)` | The first of several ways that's available: a native build, else a container. |

`Ready::Tcp(port)` waits until the port answers. A port that something else already holds is
an error, not a ready service: whatever answers there is probably left over from an earlier
run.

Anything else implements the `Service` trait: a name, whether it can run here
(`unavailable`), and `start`, which returns a `Running` that stops when it's dropped.
