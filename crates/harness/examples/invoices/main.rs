//! An API's manual, with a `curl` command where a screenshot would be. The system under test is
//! a stand-in invoices API on a loopback port; the driver calls it over HTTP and hands the
//! runner each request it sent with the response it got. The runner keeps them redacted (the
//! API keys become `$ADMIN_API_KEY` and `$MEMBER_API_KEY`, the loopback address `$BASE_URL`,
//! people's emails `REDACTED`), and the manual shows each step's request and response.
//!
//! ```text
//! cargo run -p expectation_harness --example invoices                  # docs in target/expectation-docs/invoices
//! cargo run -p expectation_harness --example invoices -- out --bug     # members can void: a failure
//! ```

mod api;

use expectation_harness::Acceptance;
use std::path::{Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bug = args.iter().any(|a| a == "--bug");
    let docs = args.iter().find(|a| !a.starts_with("--")).map_or_else(
        || PathBuf::from("target/expectation-docs/invoices"),
        PathBuf::from,
    );
    let suite = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/invoices");
    let acceptance = Acceptance::load(&suite)
        .unwrap_or_else(|e| {
            eprint!("{e}");
            exit(1)
        })
        .with_coverage();
    let report = acceptance.run(&mut api::driver(bug), &api::options());
    if let Err(e) = acceptance.publish(&docs, "api", &report) {
        eprintln!("couldn't write the docs to {}: {e}", docs.display());
        exit(1);
    }
    println!("docs: {}", docs.join("site/index.html").display());
    if !report.passed() {
        eprint!("{}", report.failures(&acceptance.suite, &acceptance.plans));
        exit(1);
    }
}
