//! Contract coverage, end to end: a quote app whose driver captures the requests each step
//! makes (as HAR) and hands them to the runner, which keeps them per run, test and step.
//!
//! ```text
//! cargo run -p expectation_harness --example quote -- <docs-dir> <attachments-dir>
//! node extensions/har-openapi/bin/expectation-har-openapi.js \
//!   --results <docs-dir>/results/app.tsv --attachments <attachments-dir> \
//!   --openapi extensions/har-openapi/test/fixtures/quote.yaml --source stand-in --complete
//! cargo run -p expectation_cli -- docs crates/harness/examples/quote <docs-dir>
//! ```
//!
//! The quote service here is a stand-in that always answers with a firm price, so the manual's
//! contract page shows the rate limit and the indicative quote as never observed: candidates for
//! a story, which is the point.

use expectation_harness::{Acceptance, Bindings, Capture, RunOptions};
use std::path::{Path, PathBuf};
use std::process::exit;

/// The app, and the requests it made since its driver last handed them over.
#[derive(Default)]
struct App {
    shown: bool,
    requests: Vec<String>,
}

impl App {
    /// A request to the quote service, as a HAR entry. The token is there to show that the
    /// extension never copies a header into what it writes.
    fn call(&mut self, method: &str, path: &str, status: u16, body: &str) {
        let started = expectation_story::now_ms();
        self.requests.push(format!(
            r#"{{"startedDateTime": "{}", "time": 12,
  "request": {{"method": "{method}", "url": "https://api.example.com/v1{path}",
    "headers": [{{"name": "Authorization", "value": "Bearer stand-in-token"}}]}},
  "response": {{"status": {status}, "headers": [],
    "content": {{"mimeType": "application/json", "size": {}, "text": {}}}}}}}"#,
            iso(started),
            body.len(),
            json_string(body)
        ));
    }

    fn ask(&mut self) {
        self.call(
            "POST",
            "/quote",
            200,
            r#"{"kind": "firm", "price": {"amount": 4.2}}"#,
        );
        self.shown = true;
    }

    fn har(&mut self) -> Vec<Capture> {
        if self.requests.is_empty() {
            return vec![];
        }
        let entries = std::mem::take(&mut self.requests).join(",\n");
        vec![Capture {
            name: "network.har".into(),
            media_type: "application/har+json".into(),
            bytes: format!(
                r#"{{"log": {{"version": "1.2", "creator": {{"name": "quote example"}}, "entries": [{entries}]}}}}"#
            )
            .into_bytes(),
        }]
    }
}

fn json_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Milliseconds since the epoch as an ISO 8601 UTC time, as HAR wants.
fn iso(ms: u64) -> String {
    let (secs, ms) = (ms / 1000, ms % 1000);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn main() {
    let mut args = std::env::args().skip(1);
    let docs = PathBuf::from(
        args.next()
            .unwrap_or("target/expectation-docs/quote".into()),
    );
    let kept = PathBuf::from(
        args.next()
            .unwrap_or("target/expectation-attachments/quote".into()),
    );
    let suite = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/quote");
    let acceptance = Acceptance::load(&suite).unwrap_or_else(|e| {
        eprint!("{e}");
        exit(1)
    });
    let mut driver = Bindings::new(App::default())
        .observe(
            "the quote",
            |a: &App| if a.shown { "shown" } else { "none" },
        )
        .arrange("the quote", |a, v| a.shown = v.as_str() == Some("shown"))
        .on("the user asks for a quote", App::ask)
        .on("the user dismisses the quote", |a| a.shown = false)
        // Opening the app fetches the latest quote, while the test is being set up.
        .on_reset(|a| {
            a.shown = false;
            a.call("GET", "/quotes/latest", 200, "{}");
        })
        .on_attachments(App::har);
    let report = acceptance.run(
        &mut driver,
        &RunOptions {
            attachments_dir: Some(kept.clone()),
            ..Default::default()
        },
    );
    if let Err(e) = acceptance.publish(&docs, "app", &report) {
        eprintln!("couldn't write the docs to {}: {e}", docs.display());
        exit(1);
    }
    println!(
        "results: {}\nattachments: {}",
        docs.join("results/app.tsv").display(),
        kept.display()
    );
    if !report.passed() {
        eprint!("{}", report.failures(&acceptance.suite, &acceptance.plans));
        exit(1);
    }
}
