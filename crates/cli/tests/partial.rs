//! `expectation docs --section`: a manual of just some sections, for a preview, whose links all
//! go somewhere.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VOCABULARY: &str = "
Suite: Shop

Vocabulary:
  State: the basket is empty | full
  State: the order is none | placed | returned
  Event: the user adds an item
  Event: the user checks out
  Event: the user sends it back

Background:
  Given the basket is empty
  And the order is none

Term: Basket
  What you are about to buy.
";

fn section(name: &str, feature: &str) -> String {
    format!("Section: {name}\n\n{feature}")
}

/// A suite of three sections, one inside another.
fn suite(root: &Path) -> PathBuf {
    let dir = root.join("suite");
    let write = |path: &str, text: &str| {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    write("vocabulary.expect", VOCABULARY);
    write(
        "01-basket/basket.expect",
        &section(
            "Basket",
            "Feature: Filling the basket\n  Expectation: Adding fills it\n    Given the basket is empty\n    When the user adds an item\n    Then the basket is full\n\nStory: Fill it\n  When the user adds an item\n  Then the basket is full\n",
        ),
    );
    write(
        "02-orders/orders.expect",
        &section(
            "Orders",
            "Feature: Placing an order\n  See the [[Basket]] and [[Filling the basket]].\n  Expectation: Checking out places it\n    Given the basket is full\n    When the user checks out\n    Then the order is placed\n",
        ),
    );
    write(
        "02-orders/returns/returns.expect",
        &section(
            "Returns",
            "Feature: Sending it back\n  Expectation: Sending back returns it\n    Given the order is placed\n    When the user sends it back\n    Then the order is returned\n",
        ),
    );
    dir
}

fn docs(suite: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_expectation"))
        .arg("docs")
        .arg(suite)
        .arg(out)
        .args(extra)
        .output()
        .unwrap()
}

/// Every file under `dir`, relative to it.
fn files(dir: &Path) -> BTreeSet<String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

/// The relative links of the pages under `dir` that lead nowhere, and the ones that go outside.
fn links(dir: &Path) -> (Vec<String>, Vec<String>) {
    let (mut broken, mut outside) = (vec![], vec![]);
    for f in files(dir) {
        let (html, md) = (f.ends_with(".html"), f.ends_with(".md"));
        if !html && !md && !f.ends_with("nav.js") {
            continue;
        }
        let text = std::fs::read_to_string(dir.join(&f))
            .unwrap()
            .replace("\\\"", "\"");
        let here = if f.ends_with("nav.js") {
            dir.join("site")
        } else {
            dir.join(&f).parent().unwrap().to_path_buf()
        };
        let mut targets: Vec<String> = text
            .split("href=\"")
            .skip(1)
            .filter_map(|t| t.split('"').next().map(String::from))
            .collect();
        if md {
            targets.extend(
                text.match_indices("](")
                    .filter(|(i, _)| {
                        !text[..*i].ends_with('!')
                            && !text[..*i].rsplit('[').next().unwrap_or("").contains('!')
                    })
                    .filter_map(|(i, _)| text[i + 2..].split(')').next().map(String::from)),
            );
        }
        for t in targets {
            let path = t.split(['#', '?']).next().unwrap_or("");
            if path.is_empty() || path.contains("${") {
                continue;
            }
            if path.contains(':') {
                outside.push(path.to_string());
                continue;
            }
            if !here.join(path).exists() {
                broken.push(format!("{f} → {t}"));
            }
        }
    }
    (broken, outside)
}

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_section_and_what_is_under_it_and_nothing_else() {
    let root = fresh("partial-one");
    let suite = suite(&root);
    let full = root.join("full");
    assert!(docs(&suite, &full, &[]).status.success());
    let out = root.join("orders");
    let ran = docs(&suite, &out, &["--section", "Orders"]);
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let have = files(&out);
    let all = files(&full);
    assert!(have.len() < all.len(), "{} of {}", have.len(), all.len());
    // Its pages and those of the section under it.
    for page in [
        "site/section-orders.html",
        "site/section-orders--returns.html",
        "features/orders--placing-an-order.md",
        "features/orders--returns--sending-it-back.md",
        "README.md",
        "site/assets/nav.js",
    ] {
        assert!(have.contains(page), "{page} in {have:?}");
    }
    // Not the other section's, and not the pages about the whole suite.
    for page in [
        "site/section-basket.html",
        "site/tests.html",
        "site/glossary.html",
        "site/map.html",
        "site/states.html",
        "tests.md",
        "glossary.md",
    ] {
        assert!(!have.contains(page), "{page}");
    }
    assert!(!have.iter().any(|p| p.contains("basket--")), "{have:?}");
    // Every page it has is one the full manual has too.
    assert!(have.is_subset(&all));

    // No link leads nowhere: the others read as text.
    let (broken, outside) = links(&out);
    assert!(broken.is_empty(), "{broken:#?}");
    assert!(outside.iter().all(|o| !o.contains("example.com")));
    let placing =
        std::fs::read_to_string(out.join("site/feature-orders--placing-an-order.html")).unwrap();
    assert!(
        placing.contains(
            r#"<span class="unbuilt" title="Not in this preview">Filling the basket</span>"#
        ),
        "{placing}"
    );
    let md = std::fs::read_to_string(out.join("features/orders--placing-an-order.md")).unwrap();
    assert!(!md.contains("basket--filling-the-basket.md"), "{md}");
    // The navigation still shows the whole tree: the other section as text, this one's as links.
    let page = std::fs::read_to_string(out.join("site/section-orders.html")).unwrap();
    let nav = page
        .split(r#"<nav class="side""#)
        .nth(1)
        .unwrap()
        .split("</nav>")
        .next()
        .unwrap();
    assert!(
        nav.contains(r#"<span class="unbuilt" title="Not in this preview">Basket</span>"#),
        "{nav}"
    );
    assert!(
        nav.contains(r#"<a href="section-orders--returns.html">Returns</a>"#),
        "{nav}"
    );
}

#[test]
fn with_a_base_url_the_rest_goes_to_the_full_manual() {
    let root = fresh("partial-base");
    let suite = suite(&root);
    let out = root.join("out");
    let ran = docs(
        &suite,
        &out,
        &[
            "--section",
            "02-orders/returns",
            "--base-url",
            "https://docs.example.com/shop",
        ],
    );
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let have = files(&out);
    assert!(have.contains("site/section-orders--returns.html"));
    assert!(
        !have.contains("site/section-orders.html"),
        "only what is under the section given"
    );
    let (broken, outside) = links(&out);
    assert!(broken.is_empty(), "{broken:#?}");
    assert!(
        outside
            .iter()
            .any(|o| o == "https://docs.example.com/shop/site/section-orders.html"),
        "{outside:?}"
    );
    assert!(
        outside
            .iter()
            .any(|o| o.starts_with("https://docs.example.com/shop/site/glossary.html")),
        "{outside:?}"
    );
    let html = std::fs::read_to_string(out.join("site/section-orders--returns.html")).unwrap();
    assert!(!html.contains("class=\"unbuilt\""));
}

#[test]
fn a_preview_written_over_a_full_manual_leaves_its_other_pages() {
    let root = fresh("partial-over");
    let suite = suite(&root);
    let out = root.join("out");
    assert!(docs(&suite, &out, &[]).status.success());
    let before = files(&out);
    assert!(
        docs(&suite, &out, &["--section", "Basket"])
            .status
            .success()
    );
    assert!(before.is_subset(&files(&out)), "nothing pruned");
}

#[test]
fn a_section_that_isnt_there_says_which_are() {
    let root = fresh("partial-errors");
    let suite = suite(&root);
    let out = root.join("out");
    let ran = docs(&suite, &out, &["--section", "Shipping"]);
    assert_eq!(ran.status.code(), Some(2));
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        said.contains("no section Shipping") && said.contains("Orders (02-orders)"),
        "{said}"
    );
    let ran = docs(&suite, &out, &["--base-url", "https://x.example"]);
    assert_eq!(ran.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&ran.stderr).contains("--section"));
}
