//! `expectation-run`: run a suite against an app that embeds an Expectation driver.
//!
//! ```text
//! expectation-run <suite-dir> [--platform NAME] [--addr 127.0.0.1:4477] [--wait SECONDS]
//!                 [--docs DIR] [--runner NAME] [--settle MS] [--screenshot-cmd "COMMAND"]
//! ```
//!
//! It listens, waits for the app to say hello, runs every story and a generated test for each
//! expectation no story reaches, and prints the results. With `--docs`, it saves the results
//! as `DIR/results/<runner>.tsv` (merged with what other runners saved), with screenshots, and
//! writes the manual. `--screenshot-cmd` takes screenshots from outside the app: the command
//! gets a path in `$1` and writes a PNG there (`xcrun simctl io booted screenshot "$1"`).
//! Exits 1 if anything failed.

use expectation_harness::{Acceptance, RunOptions, Screenshot};
use expectation_remote::{DEFAULT_ADDR, RemoteDriver};
use expectation_story::Status;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

const USAGE: &str = "usage: expectation-run <suite-dir> [--platform NAME] [--addr HOST:PORT] [--wait SECONDS] \
                     [--docs DIR] [--runner NAME] [--settle MS] [--screenshot-cmd COMMAND]";

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut suite, mut platform, mut addr, mut wait, mut docs, mut runner, mut settle, mut shot) = (
        None,
        None,
        DEFAULT_ADDR.to_string(),
        120u64,
        None,
        None,
        1000u64,
        None,
    );
    while let Some(a) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| fail(&format!("{a} needs a value")))
        };
        match a.as_str() {
            "--platform" => platform = Some(value()),
            "--addr" => addr = value(),
            "--wait" => {
                wait = value()
                    .parse()
                    .unwrap_or_else(|_| fail("--wait is in seconds"))
            }
            "--docs" => docs = Some(PathBuf::from(value())),
            "--runner" => runner = Some(value()),
            "--settle" => {
                settle = value()
                    .parse()
                    .unwrap_or_else(|_| fail("--settle is in milliseconds"))
            }
            "--screenshot-cmd" => shot = Some(value()),
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            _ if a.starts_with("--") => fail(&format!("unknown option {a}")),
            _ => suite = Some(a),
        }
    }
    let suite = suite.unwrap_or_else(|| fail(USAGE));
    let acceptance = match &platform {
        Some(p) => Acceptance::load_for(&suite, p),
        None => Acceptance::load(&suite),
    }
    .unwrap_or_else(|e| {
        eprint!("{e}");
        exit(1)
    })
    .with_coverage();

    let mut driver = RemoteDriver::listen(&addr)
        .unwrap_or_else(|e| fail(&format!("couldn't listen on {addr}: {e}")))
        .settle_for(Duration::from_millis(settle));
    if let Some(p) = &platform {
        driver = driver.for_platform(p);
    }
    if let Some(cmd) = shot {
        driver = driver.screenshot_with(move || {
            let path =
                std::env::temp_dir().join(format!("expectation-shot-{}.png", std::process::id()));
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .arg("sh")
                .arg(&path)
                .status();
            match status {
                Ok(s) if s.success() => std::fs::read(&path)
                    .map(|b| Some(Screenshot::png(b)))
                    .map_err(|e| e.to_string()),
                Ok(s) => Err(format!("the screenshot command failed: {s}")),
                Err(e) => Err(format!("couldn't run the screenshot command: {e}")),
            }
        });
    }
    eprintln!("waiting for an app at {} ...", driver.url());
    let app = driver
        .wait_for_app(Duration::from_secs(wait))
        .unwrap_or_else(|e| fail(&e));
    eprintln!("{} ({}) connected", app.name, app.platform);

    let tree = docs.as_ref().map(|d| acceptance.tree(d));
    let report = acceptance.run(
        &mut driver,
        &RunOptions {
            docs_dir: tree,
            ..Default::default()
        },
    );
    for (plan, r) in acceptance.plans.iter().zip(&report.results) {
        let mark = match r.status {
            Status::Passed => "✓",
            Status::Failed => "✗",
            _ => "–",
        };
        println!("{mark} {}", plan.title);
    }
    if let Some(e) = driver.screenshot_error() {
        eprintln!("warning: some screenshots failed: {e}");
    }
    if let Some(dir) = &docs {
        let runner = runner.unwrap_or_else(|| app.name.to_lowercase().replace(' ', "-"));
        match acceptance.publish(dir, &runner, &report) {
            Ok(_) => println!("docs: {}", acceptance.tree(dir).join("README.md").display()),
            Err(e) => fail(&format!(
                "couldn't write the docs to {}: {e}",
                dir.display()
            )),
        }
    }
    if !report.passed() {
        eprint!(
            "\n{}",
            report.failures(&acceptance.suite, &acceptance.plans)
        );
        exit(1);
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    exit(2)
}
