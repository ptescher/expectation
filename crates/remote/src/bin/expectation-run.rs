//! `expectation-run`: run a suite against an app that embeds an Expectation driver.
//!
//! ```text
//! expectation-run <suite-dir> [--platform NAME] [--addr 127.0.0.1:4477] [--wait SECONDS]
//!                 [--docs DIR] [--runner NAME] [--settle MS] [--screenshot-cmd "COMMAND"]
//!                 [--attachments DIR] [--screenshot-store DIR] [--strict[=all]]
//!                 [--no-state-pictures]
//!                 [--app-revision REV] [--data-mode MODE] [--fixture NAME]
//! ```
//!
//! It listens, waits for the app to say hello, runs every story and a generated test for each
//! expectation no story reaches, and prints the results. With `--docs`, it saves the results
//! as `DIR/results/<runner>.tsv` (merged with what other runners saved), with screenshots, and
//! writes the manual. `--screenshot-cmd` takes screenshots from outside the app: the command
//! gets a path in `$1` and writes a PNG there (`xcrun simctl io booted screenshot "$1"`).
//! `--attachments` keeps what the app captures during each step (a network log) in `DIR`, as
//! `DIR/<run>/<plan>/<step>/<name>`; keep it outside `--docs`, which is for publishing.
//! `--screenshot-store` keeps screenshots by content in `DIR` (`<sha256>.png`), so identical
//! frames are stored once and unchanged tests write nothing new; the docs take them from there.
//! `--no-state-pictures` leaves out the tests that only visit a state for the state map's
//! picture of it: a quicker run with the same verdicts.
//! `--app-revision`, `--data-mode` and `--fixture` say what the run is against (a commit, `fixtures`,
//! the fixture's name): they are saved with the results and shown in the manual, which is
//! written for the same, so results other runs saved for another build are labelled as such.
//! Nothing is read from the environment; say only what may be published.
//! Exits 1 if anything failed. Tests that aren't supported yet, are pending or didn't run don't
//! fail it, unless it is `--strict`: then every test that isn't pending must have run and passed
//! in this run (`--strict=all`: every test, pending or not), and there must be at least one.
//! Results saved by earlier runs don't count towards it.

use expectation_harness::{Acceptance, Policy, RunOptions, Screenshot};
use expectation_remote::{DEFAULT_ADDR, RemoteDriver};
use expectation_story::Status;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

const USAGE: &str = "usage: expectation-run <suite-dir> [--platform NAME] [--addr HOST:PORT] [--wait SECONDS] \
                     [--command-timeout SECONDS] \
                     [--stylesheet CSS] [--reader] [--state-shot-ratio W:H] [--docs DIR] [--runner NAME] [--settle MS] [--screenshot-cmd COMMAND] [--attachments DIR] [--screenshot-store DIR] [--strict[=all]] [--no-state-pictures] [--app-revision REV] [--data-mode MODE] [--fixture NAME]";

fn main() {
    let mut args = std::env::args().skip(1);
    let (
        mut suite,
        mut platform,
        mut addr,
        mut wait,
        mut docs,
        mut runner,
        mut settle,
        mut shot,
        mut command_timeout,
    ) = (
        None,
        None,
        DEFAULT_ADDR.to_string(),
        120u64,
        None,
        None,
        1000u64,
        None,
        None,
    );
    let (mut stylesheet, mut ratio, mut attachments, mut store) = (None, None, None, None);
    let mut reader = false;
    let mut provenance = expectation_harness::Provenance::default();
    let mut policy = Policy::Lenient;
    let mut state_pictures = true;
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
            "--command-timeout" => {
                command_timeout = Some(
                    value()
                        .parse::<u64>()
                        .ok()
                        .filter(|&n| n > 0)
                        .and_then(|n| {
                            let d = Duration::from_secs(n);
                            std::time::Instant::now().checked_add(d).map(|_| d)
                        })
                        .unwrap_or_else(|| {
                            fail("--command-timeout is a positive whole number of seconds")
                        }),
                )
            }
            "--stylesheet" => stylesheet = Some(value()),
            "--state-shot-ratio" => {
                ratio =
                    Some(expectation_harness::parse_ratio(&value()).unwrap_or_else(|e| fail(&e)))
            }
            "--reader" => reader = true,
            "--no-state-pictures" => state_pictures = false,
            "--app-revision" => provenance.app_revision = Some(value()),
            "--data-mode" => provenance.data_mode = Some(value()),
            "--fixture" => provenance.fixture = Some(value()),
            "--strict" => policy = Policy::Required,
            _ if a.starts_with("--strict=") => {
                policy = Policy::parse(&a["--strict=".len()..]).unwrap_or_else(|| {
                    fail("--strict is --strict, --strict=required or --strict=all")
                })
            }
            "--docs" => docs = Some(PathBuf::from(value())),
            "--runner" => runner = Some(value()),
            "--settle" => {
                settle = value()
                    .parse()
                    .unwrap_or_else(|_| fail("--settle is in milliseconds"))
            }
            "--screenshot-cmd" => shot = Some(value()),
            "--attachments" => attachments = Some(PathBuf::from(value())),
            "--screenshot-store" => store = Some(value()),
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
    let mut acceptance = acceptance;
    if state_pictures {
        acceptance = acceptance.with_state_pictures();
    }
    if let Some(p) = &stylesheet {
        acceptance = acceptance.with_stylesheet(p).unwrap_or_else(|e| fail(&e));
    }
    if let Some(r) = ratio {
        acceptance = acceptance.with_shot_ratio(r);
    }
    if reader {
        acceptance = acceptance.with_reader();
    }
    acceptance = acceptance.with_expected(provenance.clone());
    if let Some(s) = &store {
        acceptance = acceptance.with_screenshots(s.clone());
    }

    let mut driver = RemoteDriver::listen(&addr)
        .unwrap_or_else(|e| fail(&format!("couldn't listen on {addr}: {e}")))
        .settle_for(Duration::from_millis(settle));
    if let Some(d) = command_timeout {
        driver = driver.command_timeout(d).unwrap_or_else(|e| fail(&e));
    }
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

    let report = acceptance.run(
        &mut driver,
        &RunOptions {
            // Every platform's screenshots are in the docs' own screenshots/, a later
            // platform's in a folder of its own there, so its results name its own pictures.
            docs_dir: docs.clone(),
            screenshots_under: acceptance.screenshots_under(),
            attachments_dir: attachments,
            provenance,
            screenshot_store: store.map(PathBuf::from),
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
    // This run's own report: what earlier runs saved doesn't stand in for a test that didn't run.
    if !report.meets(policy) {
        eprint!(
            "\n{}",
            report.failures(&acceptance.suite, &acceptance.plans)
        );
        let incomplete = report.incomplete(&acceptance.plans, policy);
        if !incomplete.is_empty() {
            eprint!("\nNot every test ran and passed (--strict):\n{incomplete}");
        }
        exit(1);
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    exit(2)
}
