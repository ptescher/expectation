//! Screenshots kept by content: identical frames stored once, an unchanged run writing nothing
//! new, and docs built from the store, from a URL, with thumbnails where a tool can make them.
#[path = "../examples/kettle/world.rs"]
mod world;

use expectation_harness::{Acceptance, Bindings, RunOptions, Screenshot};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn kettle() -> Acceptance {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/kettle");
    Acceptance::load(dir)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_coverage()
}

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The files in `dir`, by name, with when they were last written.
fn files(dir: &Path) -> BTreeMap<String, SystemTime> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                e.metadata().unwrap().modified().unwrap(),
            )
        })
        .collect()
}

/// The results without the lines that are about the run itself (its id, when steps ran).
fn without_run(text: &str) -> String {
    text.lines()
        .filter(|l| !l.starts_with("run\t") && !l.starts_with("window\t"))
        .map(|l| format!("{l}\n"))
        .collect()
}

#[test]
fn a_store_keeps_each_frame_once_and_an_unchanged_run_writes_nothing() {
    let a = kettle();
    let root = fresh("shot-store");
    let (store, docs) = (root.join("store"), root.join("docs"));
    let opts = RunOptions {
        screenshot_store: Some(store.clone()),
        ..Default::default()
    };
    let first = a.run(&mut world::driver(false), &opts);
    assert!(first.passed());
    let a = a.with_screenshots(store.to_string_lossy());
    a.publish(&docs, "app", &first).unwrap();
    let saved = std::fs::read_to_string(docs.join("results/app.tsv")).unwrap();
    let stored = files(&store);

    // Named by content: every file is a hash, and the results name them as screenshots/<hash>.
    assert!(!stored.is_empty());
    for name in stored.keys() {
        let (stem, ext) = name.split_once('.').unwrap();
        assert!(
            stem.len() == 64 && stem.bytes().all(|b| b.is_ascii_hexdigit()),
            "{name}"
        );
        assert_eq!(ext, "svg");
    }
    let steps: usize = first
        .results
        .iter()
        .map(|r| 1 + r.steps.iter().filter(|s| s.screenshot.is_some()).count())
        .sum();
    assert!(
        stored.len() < steps,
        "identical frames are stored once: {} files for {steps} steps",
        stored.len()
    );
    for r in &first.results {
        for s in std::iter::once(&r.start).chain(&r.steps) {
            let shot = s.screenshot.as_deref().unwrap();
            let name = shot.strip_prefix("screenshots/").unwrap();
            assert!(stored.contains_key(name), "{shot}");
        }
    }

    // Run again, unchanged: no file is written, and the results are the same but for the run.
    let second = a.run(&mut world::driver(false), &opts);
    a.publish(&docs, "app", &second).unwrap();
    assert_eq!(files(&store), stored, "nothing new, nothing rewritten");
    let again = std::fs::read_to_string(docs.join("results/app.tsv")).unwrap();
    assert_ne!(again, saved, "the run's own lines change");
    assert_eq!(without_run(&again), without_run(&saved));

    // The docs show them, copied from the store: the ones the pages use, and nothing else.
    let copied = files(&docs.join("screenshots"));
    assert!(!copied.is_empty() && copied.keys().all(|k| stored.contains_key(k)));
    let tutorial = std::fs::read_dir(docs.join("site"))
        .unwrap()
        .flatten()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("tutorial-first-cup")
        })
        .unwrap();
    let html = std::fs::read_to_string(tutorial.path()).unwrap();
    let name = stored
        .keys()
        .find(|k| html.contains(k.as_str()))
        .expect("a stored screenshot");
    assert!(html.contains(&format!(r#"src="../screenshots/{name}""#)));
}

#[test]
fn docs_can_link_screenshots_where_they_are_served() {
    let a = kettle();
    let root = fresh("shot-url");
    let store = root.join("store");
    let report = a.run(
        &mut world::driver(false),
        &RunOptions {
            screenshot_store: Some(store.clone()),
            ..Default::default()
        },
    );
    let a = a.with_screenshots("https://cdn.example.com/shots/");
    let docs = root.join("docs");
    a.docs(Some(&report)).write(&docs).unwrap();
    let name = files(&store).into_keys().next().unwrap();
    let pages: String = std::fs::read_dir(docs.join("site"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "html"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(pages.contains(&format!("https://cdn.example.com/shots/{name}")));
    assert!(!pages.contains(&format!("../screenshots/{name}")));
    // Nothing is copied: they are served from there.
    assert!(!docs.join("screenshots").exists());
    let md = std::fs::read_dir(docs.join("tutorials"))
        .unwrap()
        .flatten()
        .next()
        .unwrap();
    assert!(
        std::fs::read_to_string(md.path())
            .unwrap()
            .contains("https://cdn.example.com/shots/")
    );
}

#[test]
fn without_a_store_screenshots_are_where_they_always_were() {
    let a = kettle();
    let docs = fresh("shot-legacy");
    let report = a.run(
        &mut world::driver(false),
        &RunOptions {
            docs_dir: Some(docs.clone()),
            ..Default::default()
        },
    );
    let shot = report.results[0].steps[0].screenshot.as_deref().unwrap();
    assert_eq!(shot, format!("screenshots/{}/01.svg", a.plans[0].slug));
    assert!(docs.join(shot).is_file());
}

/// A PNG, made by ImageMagick if it is installed.
fn png(dir: &Path, name: &str, colour: &str) -> Option<Vec<u8>> {
    let path = dir.join(name);
    let made = std::process::Command::new("convert")
        .args(["-size", "960x540", &format!("xc:{colour}")])
        .arg(&path)
        .status()
        .is_ok_and(|s| s.success());
    made.then(|| std::fs::read(&path).unwrap())
}

#[test]
fn story_cards_and_the_state_map_use_thumbnails_when_a_tool_can_make_them() {
    let root = fresh("shot-thumbs");
    let (Some(red), Some(blue)) = (png(&root, "red.png", "red"), png(&root, "blue.png", "blue"))
    else {
        eprintln!("ImageMagick isn't installed: no thumbnails to test");
        return;
    };
    let suite = "
Vocabulary:
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
  When the user flips the switch
  Then the lamp is on
";
    let loaded = expectation_language::load_sources("t", &[("t.expect", suite)]);
    let a = Acceptance::from_loaded(loaded).unwrap();
    let mut driver = Bindings::new(false)
        .observe("the lamp", |on: &bool| if *on { "on" } else { "off" })
        .arrange("the lamp", |on, v| *on = v.as_str() == Some("on"))
        .on("the user flips the switch", |on| *on = !*on)
        .on_screenshot(move |on| Screenshot::png(if *on { blue.clone() } else { red.clone() }));
    let docs = root.join("docs");
    let report = a.run(
        &mut driver,
        &RunOptions {
            screenshot_store: Some(docs.join("screenshots")),
            ..Default::default()
        },
    );
    a.docs(Some(&report)).write(&docs).unwrap();
    let shot = report.results[0].steps[0].screenshot.clone().unwrap();
    let name = shot.strip_prefix("screenshots/").unwrap();
    let thumb = docs.join("screenshots/thumbs").join(name);
    assert!(thumb.is_file(), "a thumbnail of {name}");
    // Shrunk: a PNG's width is in its header, after the signature and the IHDR chunk's start.
    let width = |p: &Path| {
        let b = std::fs::read(p).unwrap();
        u32::from_be_bytes([b[16], b[17], b[18], b[19]])
    };
    assert_eq!(width(&docs.join(&shot)), 960);
    assert!(width(&thumb) <= 480, "{}", width(&thumb));
    let index = std::fs::read_to_string(docs.join("site/index.html")).unwrap();
    assert!(
        index.contains(&format!("../screenshots/thumbs/{name}")),
        "the story card uses it"
    );
    let states = std::fs::read_to_string(docs.join("site/states.html")).unwrap();
    assert!(
        states.contains(&format!("screenshots/thumbs/{name}")),
        "the map's cards use it"
    );
    // The full screenshot is still what a step shows.
    let test = std::fs::read_dir(docs.join("site"))
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("test-light"))
        .unwrap();
    assert!(
        std::fs::read_to_string(test.path())
            .unwrap()
            .contains(&format!("../{shot}"))
    );
    // Made once: writing again leaves it be.
    let made = std::fs::metadata(&thumb).unwrap().modified().unwrap();
    a.docs(Some(&report)).write(&docs).unwrap();
    assert_eq!(std::fs::metadata(&thumb).unwrap().modified().unwrap(), made);
}
