//! Documentation from a suite, written as a manual: stories become step-by-step tutorials,
//! features become topic pages, and sections say what to do before you begin. The test
//! output (each test's steps, checks and verdicts, and the map of the expectation DAG) goes
//! into collapsed "How this is tested" blocks and a separate test report. Two outputs from the
//! same facts:
//!
//! ```text
//! README.md, tutorials/, features/,      Markdown that GitHub renders
//!   glossary.md, tests.md, tests/
//! site/                                   the same as an HTML site with nested navigation
//! screenshots/…                           written by the harness while the tests run
//! ```
//!
//! The wording comes from the specs: `Say:` on an event or phrase tells the reader what to do
//! ("press Space"), `Set:` on a state tells them how to change it, and a story's text under its
//! narrative is the tutorial's introduction. `voice` turns steps into instructions with them.
//! A step without a screenshot (its test isn't supported yet, didn't run, or failed before it
//! got there) shows a placeholder instead.

mod html;
mod manual;
mod map;
mod markdown;
pub mod outline;
mod states;
mod voice;

use expectation_graph::Analysis;
use expectation_model::{ExpId, Feature, StepText, Suite, VarKind, Variable};
use expectation_story::{Because, Plan, Report, Status, Verdict, coverage};
use std::path::{Path, PathBuf};

pub struct Docs<'a> {
    pub suite: &'a Suite,
    pub analysis: &'a Analysis,
    pub plans: &'a [Plan],
    /// Results of running `plans`, if they ran.
    pub report: Option<&'a Report>,
    /// Where the suite's files are, so each page can link to the one to edit.
    pub suite_dir: Option<&'a Path>,
    /// The other platforms' docs (when the suite has several), so each page can offer the same
    /// page there. Their trees are `platforms/<name>/` (the default platform's is the root).
    pub siblings: &'a [Sibling],
    /// The manual's parts and hand-written pages. Set by the docs themselves from the suite's
    /// `outline.txt`; leave it `None`.
    pub outline: Option<&'a outline::Outline>,
    /// Controls an integration says exist, so one that no run reached still shows in the
    /// interaction coverage.
    pub inventory: &'a [expectation_story::Control],
    /// How the HTML site looks beyond its built-in theme.
    pub look: Look<'a>,
}

/// Choices about the HTML site's appearance. The default is the built-in look.
#[derive(Clone, Copy, Default)]
pub struct Look<'a> {
    /// CSS (see [`read_stylesheet`]) written to `site/assets/custom.css` and loaded by every
    /// page after the built-in styles. `None` uses `theme.css` in the suite's directory if
    /// there is one, else nothing.
    pub stylesheet: Option<&'a str>,
    /// Width over height of the screenshots on the state map's cards (see [`parse_ratio`]);
    /// `None` keeps the built-in shape.
    pub shot_ratio: Option<f32>,
}

/// Where a custom stylesheet is written, relative to the docs directory.
pub const CUSTOM_CSS: &str = "site/assets/custom.css";
/// The file in the suite's directory that is the custom stylesheet unless one is given.
pub const SUITE_STYLESHEET: &str = "theme.css";

/// Reads a custom stylesheet, refusing what can't be one: a file that's missing, isn't UTF-8,
/// is empty, or has unbalanced braces. The CSS is used as written; it isn't checked further.
pub fn read_stylesheet(path: &Path) -> Result<String, String> {
    let css = std::fs::read_to_string(path)
        .map_err(|e| format!("couldn't read the stylesheet {}: {e}", path.display()))?;
    if css.trim().is_empty() {
        return Err(format!("the stylesheet {} is empty", path.display()));
    }
    balanced(&css)
        .map_err(|e| format!("the stylesheet {} is not valid CSS: {e}", path.display()))?;
    Ok(css)
}

/// Braces that open and close in order, outside comments and strings.
fn balanced(css: &str) -> Result<(), String> {
    let (mut depth, mut chars) = (0usize, css.chars().peekable());
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                loop {
                    match chars.next() {
                        Some('/') if prev == '*' => break,
                        Some(c) => prev = c,
                        None => return Err("a comment is never closed".into()),
                    }
                }
            }
            q @ ('"' | '\'') => loop {
                match chars.next() {
                    Some('\\') => {
                        chars.next();
                    }
                    Some(c) if c == q => break,
                    Some('\n') | None => return Err("a string is never closed".into()),
                    Some(_) => {}
                }
            },
            '{' => depth += 1,
            '}' => depth = depth.checked_sub(1).ok_or("a } has no matching {")?,
            _ => {}
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err("a { is never closed".into())
    }
}

/// A screenshot shape as width over height: `9:19.5`, `9/19.5` or a plain number (`0.46`).
/// Between 1:5 and 5:1.
pub fn parse_ratio(text: &str) -> Result<f32, String> {
    let bad =
        || format!("«{text}» isn't a ratio: write it as WIDTH:HEIGHT, like 9:19.5, or as a number");
    let num = |t: &str| {
        t.trim()
            .parse::<f32>()
            .ok()
            .filter(|n| n.is_finite() && *n > 0.0)
    };
    let r = match text.split_once([':', '/']) {
        Some((w, h)) => num(w).zip(num(h)).map(|(w, h)| w / h),
        None => num(text),
    }
    .ok_or_else(bad)?;
    if (0.2..=5.0).contains(&r) {
        Ok(r)
    } else {
        Err(format!("the ratio {text} is outside 1:5 to 5:1"))
    }
}

/// The pages of the same manual for another platform.
pub struct Sibling {
    pub platform: String,
    /// Its HTML pages, relative to its tree: `site/feature-x.html`.
    pub pages: std::collections::HashSet<String>,
}

pub struct Page {
    /// Relative to the docs directory.
    pub path: String,
    pub content: String,
}

/// Where the placeholders live, relative to the docs directory.
pub const NOT_SUPPORTED: &str = "site/assets/not-supported.svg";
pub const NO_SCREENSHOT: &str = "site/assets/no-screenshot.svg";

/// Removes the pages an earlier run wrote that this one did not: a renamed feature or story
/// would otherwise leave its old page in the manual's folders.
fn prune(dir: &Path, written: &[PathBuf]) -> std::io::Result<()> {
    let kinds: [(&str, &str, &[&str]); 4] = [
        ("features", "md", &[""]),
        ("tutorials", "md", &[""]),
        ("tests", "md", &[""]),
        (
            "site",
            "html",
            &["feature-", "section-", "test-", "tutorial-"],
        ),
    ];
    for (sub, ext, prefixes) in kinds {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let ours = path.is_file()
                && path.extension().is_some_and(|e| e == ext)
                && prefixes.iter().any(|p| name.starts_with(p));
            if ours && !written.contains(&path) {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

impl Docs<'_> {
    /// The controls the last run found and the tests that used them; none if the tests haven't
    /// run, or the driver can't list them.
    pub fn control_rows(&self) -> Vec<expectation_story::ControlRow> {
        match self.report {
            Some(r) => {
                expectation_story::control_coverage(self.suite, self.plans, r, self.inventory)
            }
            None => vec![],
        }
    }

    /// Write the Markdown pages and the HTML site under `dir`. Pages of stories or features
    /// that no longer exist are removed (only the generated kinds, in the folders this writes
    /// to); other files in the directory are left alone.
    pub fn write(&self, dir: &Path) -> std::io::Result<Vec<PathBuf>> {
        // A screenshot the results name but the directory doesn't have (an interrupted run
        // cleared it) is a placeholder, not a broken image.
        let checked = self.report.map(|r| {
            let mut r = r.clone();
            for result in &mut r.results {
                for step in std::iter::once(&mut result.start).chain(&mut result.steps) {
                    if step
                        .screenshot
                        .as_ref()
                        .is_some_and(|p| !dir.join(p).exists())
                    {
                        step.screenshot = None;
                    }
                }
            }
            r
        });
        // The given stylesheet, else the suite's own `theme.css`, so regenerating keeps the theme.
        let found = match (self.look.stylesheet, self.suite_dir) {
            (None, Some(d)) if d.join(SUITE_STYLESHEET).is_file() => Some(
                read_stylesheet(&d.join(SUITE_STYLESHEET))
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
            ),
            _ => None,
        };
        let docs = Docs {
            report: checked.as_ref(),
            look: Look {
                stylesheet: self.look.stylesheet.or(found.as_deref()),
                ..self.look
            },
            ..*self
        };
        std::fs::create_dir_all(dir)?;
        let source = self.suite_dir.and_then(|s| relative(dir, s));
        let mut written = vec![];
        for page in docs.pages_with(source) {
            let path = dir.join(&page.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, page.content)?;
            written.push(path);
        }
        prune(dir, &written)?;
        if docs.look.stylesheet.is_none() {
            // An earlier run's custom stylesheet would otherwise sit there unused.
            let _ = std::fs::remove_file(dir.join(CUSTOM_CSS));
        }
        Ok(written)
    }

    pub fn pages(&self) -> Vec<Page> {
        self.pages_with(None)
    }

    /// This manual's HTML pages, for the other platforms' dropdowns.
    pub fn sibling(&self) -> Sibling {
        Sibling {
            platform: self.suite.platform.clone().unwrap_or_default(),
            pages: self
                .pages()
                .into_iter()
                .map(|p| p.path)
                .filter(|p| p.ends_with(".html"))
                .collect(),
        }
    }

    /// The pages, with `source` the suite's directory relative to the docs directory.
    fn pages_with(&self, source: Option<String>) -> Vec<Page> {
        let outline = outline::of(self);
        let docs = Docs {
            outline: outline.as_ref(),
            ..*self
        };
        let mut facts = Facts::new(&docs);
        facts.source = source;
        let mut pages = markdown::pages(&docs, &facts);
        pages.extend(html::pages(&docs, &facts));
        pages
    }
}

/// What the pages say about each expectation and test, worked out once.
struct Facts {
    /// Per expectation, if anything ran.
    verdicts: Option<Vec<Verdict>>,
    /// Per expectation, the (plan, step) pairs that exercise it; step `None` is the start.
    coverage: Vec<Vec<(usize, Option<usize>)>>,
    /// Per plan, its status (`NotRun` without a report).
    status: Vec<Status>,
    /// The suite's directory relative to the docs directory, for "Edit this page" links.
    source: Option<String>,
}

impl Facts {
    fn new(d: &Docs) -> Self {
        let status = (0..d.plans.len())
            .map(|i| {
                d.report
                    .and_then(|r| r.results.get(i))
                    .map_or(Status::NotRun, |r| r.status)
            })
            .collect();
        Facts {
            verdicts: d.report.map(|r| r.verdicts(d.suite, d.plans)),
            coverage: coverage(d.suite, d.plans),
            status,
            source: None,
        }
    }

    fn verdict(&self, e: ExpId) -> Option<Verdict> {
        self.verdicts
            .as_ref()
            .map(|v| expectation_story::report::verdict_of(v, e))
    }

    /// The tests that exercise `e`, each once.
    fn tested_by(&self, e: ExpId) -> Vec<usize> {
        let mut by: Vec<usize> = self.coverage[e.0 as usize]
            .iter()
            .map(|(p, _)| *p)
            .collect();
        by.dedup();
        by
    }

    /// The best picture of `e` in action: the screenshot after a step whose checks for it
    /// passed, else after any step that exercises it. Returns (plan, step) with the path.
    fn shot_of(&self, d: &Docs, e: ExpId) -> Option<(usize, Option<usize>, String)> {
        let report = d.report?;
        let mut fallback = None;
        for &(p, step) in &self.coverage[e.0 as usize] {
            let result = &report.results[p];
            let (r, checks) = match step {
                None => (&result.start, &d.plans[p].start_checks),
                Some(s) => (&result.steps[s], &d.plans[p].steps[s].checks),
            };
            let Some(shot) = &r.screenshot else { continue };
            let verified = checks
                .iter()
                .zip(&r.checks)
                .any(|(c, cr)| c.because == Because::Expectation(e) && cr.passed);
            if verified {
                return Some((p, step, shot.clone()));
            }
            fallback.get_or_insert((p, step, shot.clone()));
        }
        fallback
    }
}

/// `to` relative to `from`, both directories: "../expectations".
fn relative(from: &Path, to: &Path) -> Option<String> {
    let (from, to) = (from.canonicalize().ok()?, to.canonicalize().ok()?);
    let common = from
        .components()
        .zip(to.components())
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<String> = vec!["..".into(); from.components().count() - common];
    parts.extend(
        to.components()
            .skip(common)
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    })
}

/// A feature's slug, unique in the suite: under its section's.
fn feature_slug(suite: &Suite, f: &Feature) -> String {
    let section = &suite.section(f.section).slug;
    if section.is_empty() {
        f.slug.clone()
    } else {
        format!("{section}--{}", f.slug)
    }
}

/// "As a tea drinker / I want … / So that …" as one sentence.
fn narrative(lines: &[String]) -> String {
    let parts: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let l = l.trim_end_matches(['.', ',']);
            let mut c = l.chars();
            match c.next() {
                Some(f) if i > 0 && !l.starts_with("I ") => f.to_lowercase().chain(c).collect(),
                _ => l.to_string(),
            }
        })
        .collect();
    format!("{}.", parts.join(", "))
}

/// "When the dial is 0 % and the user presses reset": the action, without its Thens.
fn action_heading(steps: &[StepText]) -> String {
    use expectation_model::Keyword;
    let mut out = String::new();
    for t in steps.iter().take_while(|t| t.keyword != Keyword::Then) {
        match t.keyword {
            Keyword::And => out.push_str(" and "),
            Keyword::But => out.push_str(" but "),
            k => out.push_str(&format!("{} ", k.as_str())),
        }
        out.push_str(&t.text);
    }
    out
}

fn values(v: &Variable) -> String {
    match &v.kind {
        VarKind::Enum(names) => names.join(" · "),
        VarKind::Number { min, max, unit } => {
            let u = unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
            format!(
                "a number from {} to {}{u}",
                expectation_model::text::number(*min),
                expectation_model::text::number(*max)
            )
        }
    }
}

fn verdict_label(v: Option<Verdict>) -> &'static str {
    match v {
        Some(Verdict::Verified) => "verified",
        Some(Verdict::Failed) => "failed",
        Some(Verdict::Untested) => "untested",
        Some(Verdict::Unsupported) => "not supported yet",
        Some(Verdict::Pending) => "pending",
        Some(Verdict::NotRun) | None => "not run",
    }
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Passed => "passed",
        Status::Failed => "failed",
        Status::Skipped => "skipped",
        Status::Unsupported => "not supported yet",
        Status::Pending => "pending: not built yet",
        Status::NotRun => "not run",
    }
}

/// Why a step has no screenshot, for the placeholder's caption.
fn missing_shot(status: Status) -> &'static str {
    match status {
        Status::Unsupported => "Screenshot to come: the tests can't run this step yet.",
        Status::Pending => "No screenshot: this behaviour is pending, so the test isn't run.",
        Status::Skipped => "No screenshot: the test stopped at an earlier step.",
        Status::NotRun => "No screenshot yet: the tests haven't run.",
        Status::Failed => "No screenshot: the test failed before this step.",
        Status::Passed => "No screenshot of this step.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expectation_model::{Keyword, Span};

    fn glossary_pages(reference: Option<&str>) -> (String, String) {
        let reference = reference
            .map(|r| format!("  Reference: {r}\n"))
            .unwrap_or_default();
        let src = format!("Suite: S\nTerm: Account\n  The customer's account.\n{reference}");
        let loaded = expectation_language::load_sources("S", &[("s.expect", &src)]);
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let docs = Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &[],
            report: None,
            suite_dir: None,
            siblings: &[],
            outline: None,
            inventory: &[],
            look: Look::default(),
        };
        let pages = docs.pages();
        let get = |path: &str| {
            pages
                .iter()
                .find(|p| p.path == path)
                .unwrap_or_else(|| panic!("no {path}"))
                .content
                .clone()
        };
        (get("glossary.md"), get("site/glossary.html"))
    }

    #[test]
    fn a_term_links_to_its_outside_reference_in_both_formats() {
        let (md, html) = glossary_pages(Some(
            "AccountResponse | https://api.example.com/docs/models/AccountResponse",
        ));
        assert!(md.contains("**Account**"));
        assert!(md.contains(
            "Reference: [AccountResponse](https://api.example.com/docs/models/AccountResponse)"
        ));
        assert!(html.contains(r#"<dt id="account">Account</dt>"#));
        assert!(html.contains(
            r#"<a href="https://api.example.com/docs/models/AccountResponse" rel="noopener noreferrer">AccountResponse</a>"#
        ));
    }

    #[test]
    fn a_reference_cannot_break_out_of_its_link() {
        let (md, html) = glossary_pages(Some("A]<b>\"x\" & (y) | https://e.com/a?b=1&c=2(d)"));
        assert!(
            md.contains(r#"[A\]\<b\>"x" \& (y)](https://e.com/a?b=1&c=2%28d%29)"#),
            "{md}"
        );
        assert!(
            html.contains("A]&lt;b&gt;&quot;x&quot; &amp; (y)</a>"),
            "{html}"
        );
        assert!(
            html.contains(r#"href="https://e.com/a?b=1&amp;c=2(d)""#),
            "{html}"
        );
        assert!(!html.contains("<b>"));
    }

    #[test]
    fn a_term_without_a_reference_has_no_reference_line() {
        let (md, html) = glossary_pages(None);
        assert!(!md.contains("Reference:"));
        assert!(!html.contains("Reference:"));
    }

    #[test]
    fn narratives_read_as_one_sentence() {
        let lines = [
            "As a tea drinker",
            "I want to boil the kettle.",
            "So that I can make tea",
        ]
        .map(String::from);
        assert_eq!(
            narrative(&lines),
            "As a tea drinker, I want to boil the kettle, so that I can make tea."
        );
    }

    #[test]
    fn headings_show_the_whole_action() {
        let step = |keyword, text: &str| StepText {
            keyword,
            text: text.into(),
            span: Span::new("t", 1),
            meaning: None,
        };
        let steps = [
            step(Keyword::When, "the dial is 0 %"),
            step(Keyword::And, "the user presses reset"),
            step(Keyword::Then, "the kettle is off"),
        ];
        assert_eq!(
            action_heading(&steps),
            "When the dial is 0 % and the user presses reset"
        );
    }

    const TURNSTILE: &str = "
Vocabulary:
  State: the turnstile is locked | unlocked
  Event: a visitor inserts a coin
Background:
  Given the turnstile is locked
Feature: Paying
  Expectation: A coin unlocks it
    Given the turnstile is locked
    When a visitor inserts a coin
    Then the turnstile is unlocked
";

    fn styled(look: Look, suite_dir: Option<&Path>, out: &Path) -> Vec<Page> {
        let loaded = expectation_language::load_sources("S", &[("s.expect", TURNSTILE)]);
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let docs = Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &[],
            report: None,
            suite_dir,
            siblings: &[],
            outline: None,
            inventory: &[],
            look,
        };
        docs.write(out).unwrap();
        docs.pages_with(None)
    }

    fn temp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("expectation-docs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn without_a_stylesheet_the_site_is_as_before() {
        let out = temp("plain");
        let pages = styled(Look::default(), None, &out);
        assert!(pages.iter().all(|p| p.path != CUSTOM_CSS));
        assert!(pages.iter().all(|p| {
            !p.content
                .contains(r#"<link rel="stylesheet" href="assets/custom.css">"#)
        }));
        assert!(!out.join(CUSTOM_CSS).exists());
    }

    #[test]
    fn a_stylesheet_is_loaded_after_the_built_in_styles_by_every_page() {
        let out = temp("custom");
        let css = ":root { --canvas: #fffdf7; }";
        let look = Look {
            stylesheet: Some(css),
            ..Look::default()
        };
        let pages = styled(look, None, &out);
        assert_eq!(std::fs::read_to_string(out.join(CUSTOM_CSS)).unwrap(), css);
        let html: Vec<_> = pages
            .iter()
            .filter(|p| p.path.ends_with(".html") && p.content.contains("assets/style.css"))
            .collect();
        for name in ["index", "map", "states", "tests", "theme"] {
            assert!(
                html.iter().any(|p| p.path == format!("site/{name}.html")),
                "{name}"
            );
        }
        for p in html {
            let (style, custom) = (
                p.content.find("assets/style.css"),
                p.content.find("assets/custom.css"),
            );
            assert!(style < custom && custom.is_some(), "{}", p.path);
        }
        let theme = &pages
            .iter()
            .find(|p| p.path == "site/theme.html")
            .unwrap()
            .content;
        assert!(theme.contains("A custom stylesheet is applied"));
        assert!(theme.contains("Built-in Night palette"));
    }

    #[test]
    fn regenerating_keeps_the_suites_theme_and_drops_a_stale_one() {
        let (suite, out) = (temp("suite"), temp("regen"));
        std::fs::write(suite.join(SUITE_STYLESHEET), "body { color: red; }").unwrap();
        let pages = styled(Look::default(), Some(&suite), &out);
        assert!(
            std::fs::read_to_string(out.join(CUSTOM_CSS))
                .unwrap()
                .contains("red")
        );
        let _ = pages;
        std::fs::remove_file(suite.join(SUITE_STYLESHEET)).unwrap();
        styled(Look::default(), Some(&suite), &out);
        assert!(!out.join(CUSTOM_CSS).exists());
        // A broken theme.css is an error, not a silent fallback.
        std::fs::write(suite.join(SUITE_STYLESHEET), "body {").unwrap();
        let loaded = expectation_language::load_sources("S", &[("s.expect", TURNSTILE)]);
        let analysis = expectation_graph::analyze(&loaded.suite);
        let docs = Docs {
            suite: &loaded.suite,
            analysis: &analysis,
            plans: &[],
            report: None,
            suite_dir: Some(&suite),
            siblings: &[],
            outline: None,
            inventory: &[],
            look: Look::default(),
        };
        assert!(
            docs.write(&out)
                .unwrap_err()
                .to_string()
                .contains("never closed")
        );
    }

    #[test]
    fn a_bad_stylesheet_is_refused_with_a_reason() {
        let dir = temp("bad");
        let err = |name: &str, css: &str| {
            let p = dir.join(name);
            std::fs::write(&p, css).unwrap();
            read_stylesheet(&p).unwrap_err()
        };
        assert!(
            read_stylesheet(&dir.join("missing.css"))
                .unwrap_err()
                .contains("couldn't read")
        );
        assert!(err("empty.css", "  \n").contains("empty"));
        assert!(err("open.css", "a { color: red;").contains("never closed"));
        assert!(err("close.css", "a { } }").contains("no matching"));
        assert!(err("comment.css", "/* a { }").contains("comment"));
        std::fs::write(dir.join("ok.css"), "/* } */ a::after { content: \"}\"; }").unwrap();
        assert!(read_stylesheet(&dir.join("ok.css")).is_ok());
    }

    #[test]
    fn ratios_are_read_and_bounded() {
        assert_eq!(parse_ratio("16:9"), Ok(16.0 / 9.0));
        assert_eq!(parse_ratio("1/2"), Ok(0.5));
        assert_eq!(parse_ratio("0.5"), Ok(0.5));
        for bad in [
            "", "abc", "0", "-1", "1:0", "9:", "nan", "inf", "1:6", "6:1", "1:2:3",
        ] {
            assert!(parse_ratio(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_state_maps_screenshots_follow_the_ratio() {
        let thumb = |ratio: Option<f32>| {
            let page = styled(
                Look {
                    shot_ratio: ratio,
                    ..Look::default()
                },
                None,
                &temp("ratio"),
            )
            .into_iter()
            .find(|p| p.path == "site/states.html")
            .unwrap()
            .content;
            let g = page.split(r#""g":["#).nth(1).unwrap();
            g.split(']').next().unwrap().to_string()
        };
        // Cards are 204 wide inside their margins: 9:19.5 is 442 tall; the default stays 124.
        assert!(thumb(None).contains(",124,"), "{}", thumb(None));
        let tall = thumb(Some(9.0 / 19.5));
        assert!(tall.contains(",442,"), "{tall}");
    }
}
