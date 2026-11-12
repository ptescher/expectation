//! `expectation-run` on two platforms of one suite, into one docs directory: each platform's
//! manual shows its own screenshots, on the run itself and when `expectation docs` writes the
//! manuals again, and never another platform's.

use expectation_remote::client::{Client, Handlers, Reply};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const SUITE: &str = "
Vocabulary:
  Platform: iOS | Android
  State: the lamp is off | on
  Event: the user flips the switch
Background:
  Given the lamp is off
Feature: Lamp
  Expectation: Flipping turns it on
    Given the lamp is off
    When the user flips the switch
    Then the lamp is on
Story: Light
  Given the lamp is off
  When the user flips the switch
  Then the lamp is on
";

/// Run the suite for `platform` with the real runner, against a lamp that draws itself with
/// the platform's name.
fn run(suite: &Path, docs: &Path, platform: &'static str) {
    let mut runner = Command::new(env!("CARGO_BIN_EXE_expectation-run"))
        .arg(suite)
        .args(["--platform", platform, "--addr", "127.0.0.1:0"])
        .args(["--settle", "0", "--wait", "20", "--runner", "app", "--docs"])
        .arg(docs)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // "waiting for an app at http://127.0.0.1:PORT ..."
    let mut said = BufReader::new(runner.stderr.take().unwrap());
    let mut line = String::new();
    said.read_line(&mut line).unwrap();
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or_else(|| panic!("no address in {line:?}"))
        .to_string();
    let h: Handlers<bool> = Handlers::new()
        .observe("the lamp", |on: &bool| if *on { "on" } else { "off" })
        .arrange("the lamp", |on, v| *on = v.to_string() == "on")
        .on("the user flips the switch", |on| *on = !*on)
        .on_reset(|on| *on = false)
        .on_screenshot(move |on| {
            Ok(Reply::Svg(format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\"><text>{platform} {on}</text></svg>"
            )))
        });
    let client = Client::connect(&addr, h.hello(platform, "Lamp"));
    std::thread::spawn(move || {
        let mut on = false;
        while let Some(r) = client.recv_timeout(Duration::from_secs(20)) {
            h.handle(&mut on, r);
        }
    });
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut said, &mut rest).unwrap();
    assert!(runner.wait().unwrap().success(), "{line}{rest}");
}

/// Every generated page under `dir`, as one text.
fn pages(dir: &Path) -> String {
    let mut out = String::new();
    for sub in ["site", "tests", "tutorials"] {
        for e in std::fs::read_dir(dir.join(sub))
            .into_iter()
            .flatten()
            .flatten()
        {
            if e.path().is_file() {
                out.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
            }
        }
    }
    out
}

/// Each platform's manual shows its own pictures and none of the other's.
fn each_shows_its_own(docs: &Path) {
    let android = pages(&docs.join("platforms/android"));
    assert!(android.contains("../../../screenshots/platforms/android/light/"));
    assert!(!android.contains("No screenshot"), "a picture is missing");
    assert!(!android.contains("screenshots/light/"), "an iOS picture");
    let ios = pages(docs);
    assert!(ios.contains("../screenshots/light/"));
    assert!(!ios.contains("No screenshot"), "a picture is missing");
    assert!(
        !ios.contains("screenshots/platforms/"),
        "an Android picture"
    );
}

#[test]
fn each_platforms_manual_shows_its_own_screenshots() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("platform-screenshots");
    let _ = std::fs::remove_dir_all(&root);
    let (suite, docs) = (root.join("suite"), root.join("docs"));
    std::fs::create_dir_all(&suite).unwrap();
    std::fs::write(suite.join("lamp.expect"), SUITE).unwrap();

    run(&suite, &docs, "iOS");
    run(&suite, &docs, "Android");

    // The same test on both, each with its own pictures, in the docs' one screenshots/.
    let read = |p: &str| std::fs::read_to_string(docs.join(p)).unwrap();
    assert!(read("screenshots/light/00.svg").contains("iOS false"));
    assert!(read("screenshots/light/01.svg").contains("iOS true"));
    assert!(read("screenshots/platforms/android/light/00.svg").contains("Android false"));
    assert!(read("screenshots/platforms/android/light/01.svg").contains("Android true"));
    assert!(
        !docs.join("platforms/android/screenshots").exists(),
        "a platform's tree has screenshots of its own"
    );
    // The results say whose they are.
    assert!(
        read("results/app@Android.tsv").contains("\tscreenshots/platforms/android/light/00.svg\t")
    );
    assert!(read("results/app@iOS.tsv").contains("\tscreenshots/light/00.svg\t"));
    each_shows_its_own(&docs);

    // Written again from the saved results, by the command a docs build runs.
    for tree in [docs.join("site"), docs.join("platforms")] {
        std::fs::remove_dir_all(tree).unwrap();
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cli/Cargo.toml");
    let written = Command::new(env!("CARGO"))
        .args(["run", "-q", "--manifest-path"])
        .arg(&manifest)
        .args(["--bin", "expectation", "--", "docs"])
        .arg(&suite)
        .arg(&docs)
        .output()
        .unwrap();
    assert!(
        written.status.success(),
        "{}",
        String::from_utf8_lossy(&written.stderr)
    );
    each_shows_its_own(&docs);

    // Results from before platforms had folders of their own name the default's pictures:
    // Android's manual shows a placeholder for them, not an iOS screenshot.
    let old =
        read("results/app@Android.tsv").replace("screenshots/platforms/android/", "screenshots/");
    std::fs::write(docs.join("results/app@Android.tsv"), old).unwrap();
    let written = Command::new(env!("CARGO"))
        .args(["run", "-q", "--manifest-path"])
        .arg(&manifest)
        .args(["--bin", "expectation", "--", "docs"])
        .arg(&suite)
        .arg(&docs)
        .output()
        .unwrap();
    assert!(written.status.success());
    let android = pages(&docs.join("platforms/android"));
    assert!(android.contains("No screenshot"));
    assert!(!android.contains("screenshots/light/"), "an iOS picture");
}
