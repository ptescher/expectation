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
mod links;
mod manual;
mod map;
mod markdown;
pub mod outline;
pub mod partial;
mod states;
mod thumbs;
mod voice;

/// What a test's page says when it began where another ended (`Began::Continued`).
pub(crate) const CONTINUED: &str = "This run started where the test that ran before it had ended: everything it is given was seen to hold there, so nothing was reset or arranged.";

use expectation_graph::Analysis;
use expectation_model::{ExpId, Feature, StepText, Suite, VarKind, Variable};
use expectation_story::{
    Because, Exchange, Plan, Report, Role, Status, StepResult, Verdict, coverage,
};
pub use links::broken_links;
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
    /// Contracts that extensions held the evidence of these results against (see
    /// [`expectation_story::coverage`](mod@expectation_story::coverage)), each judged against `report`. One page each.
    pub contracts: &'a [expectation_story::Coverage],
    /// Just these sections, for a preview (see [`partial`]); `None` writes the whole manual.
    pub partial: Option<&'a partial::Partial>,
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
    /// Where the screenshots the results name (`screenshots/…`) are.
    pub screenshots: Screenshots<'a>,
    /// The reader edition: no "How this is tested" blocks, no "Tested by" lists and no
    /// placeholders for missing screenshots on the tutorials and topic pages, and the test
    /// report is linked from one place. The outline can ask for it too (`Edition: reader`).
    pub reader: bool,
    /// For a platform's manual written under another's (`platforms/<name>/`): the docs
    /// directory that holds the shared `screenshots/`, and how many folders up it is from this
    /// manual. `None` is a manual that has its own.
    pub shared: Option<(&'a Path, usize)>,
    /// Code coverage files (see [`expectation_story::CodeCoverage`]): shown on the test pages
    /// as observed execution, apart from the results.
    pub code: &'a [expectation_story::CodeCoverage],
    /// What this manual is written for (a build, its data): each test page says whether the
    /// run its result is from was against the same. `None` compares nothing.
    pub expected: Option<&'a expectation_story::Provenance>,
}

/// Where the screenshots are that the results name as `screenshots/<rest>`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Screenshots<'a> {
    /// In the docs directory's own `screenshots/`, as a run writes them by default.
    #[default]
    InDocs,
    /// In a directory of their own (a content-addressed store, say): `screenshots/<rest>` is
    /// `<dir>/<rest>`. The ones the pages show are copied into the docs' `screenshots/`.
    Dir(&'a Path),
    /// Behind a URL prefix (a bucket, a CI artifact): `screenshots/<rest>` is `<prefix>/<rest>`,
    /// and the pages link there. Nothing is copied or checked, and there are no thumbnails.
    Url(&'a str),
}

impl Look<'_> {
    /// What a page's link to the docs' `screenshots/` goes up through first.
    pub(crate) fn up(&self) -> String {
        "../".repeat(self.shared.map_or(0, |(_, n)| n))
    }
}

impl<'a> Screenshots<'a> {
    /// A `--screenshots` value: a URL if it starts with `http://` or `https://`, else a directory.
    pub fn parse(s: &'a str) -> Self {
        if s.starts_with("http://") || s.starts_with("https://") {
            Screenshots::Url(s)
        } else {
            Screenshots::Dir(Path::new(s))
        }
    }

    /// Where a page one directory down links to the screenshot the results name `p`.
    pub(crate) fn src(&self, up: &str, p: &str) -> String {
        match (self, p.strip_prefix("screenshots/")) {
            (Screenshots::Url(prefix), Some(rest)) => {
                format!("{}/{rest}", prefix.trim_end_matches('/'))
            }
            _ => format!("../{up}{p}"),
        }
    }

    /// The same for a small picture: its thumbnail if one was made, else the screenshot.
    pub(crate) fn thumb(
        &self,
        up: &str,
        thumbs: &std::collections::HashSet<String>,
        p: &str,
    ) -> String {
        match p.strip_prefix("screenshots/") {
            Some(rest) if thumbs.contains(rest) => format!("../{up}{}{rest}", thumbs::DIR),
            _ => self.src(up, p),
        }
    }
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
fn prune(dir: &Path, written: &std::collections::HashSet<&Path>) -> std::io::Result<()> {
    let kinds: [(&str, &str, &[&str]); 6] = [
        ("features", "md", &[""]),
        ("tutorials", "md", &[""]),
        ("tests", "md", &[""]),
        ("contracts", "md", &[""]),
        ("contracts", "json", &[""]),
        (
            "site",
            "html",
            &[
                "feature-",
                "section-",
                "test-",
                "tutorial-",
                "contract-",
                "tests-",
                "map-",
            ],
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
            if ours && !written.contains(path.as_path()) {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

/// What a test page says about what its result was run against.
pub(crate) struct Against {
    /// The parts the run said, by what the manual calls them.
    pub said: Vec<(&'static str, String)>,
    /// How that compares with what the manual is for, in a sentence, if it is for something.
    pub verdict: Option<(expectation_story::Agreement, String)>,
}

impl Docs<'_> {
    /// What test `i`'s result was run against, and whether that is what this manual is for.
    /// `None` when it didn't run, or there is nothing to say: no part was given and the manual
    /// isn't for anything in particular.
    pub(crate) fn against(&self, i: usize) -> Option<Against> {
        use expectation_story::Agreement;
        let result = self.report?.results.get(i)?;
        if !result.status.ran() {
            return None;
        }
        // A result from before runs said what they were against says nothing: unknown.
        let none = expectation_story::Provenance::default();
        let run = result.run.as_ref().map_or(&none, |r| &r.provenance);
        let said: Vec<(&'static str, String)> = run
            .parts()
            .into_iter()
            .filter_map(|(_, label, v)| Some((label, v?.to_string())))
            .collect();
        let verdict = self.look.expected.and_then(|want| {
            let agreement = run.agreement(want)?;
            let pairs = run.parts().into_iter().zip(want.parts());
            let text = match agreement {
                Agreement::Matching => "This is what this manual is for.".to_string(),
                Agreement::Mismatched => {
                    let differs: Vec<String> = pairs
                        .filter_map(|((_, label, mine), (_, _, want))| {
                            let (mine, want) = (mine?, want?);
                            (mine != want).then(|| format!("{label} {want}"))
                        })
                        .collect();
                    format!(
                        "This manual is for {}, so this is what an earlier or other run saw. It \
                         is not a result for this one, and not a failure of it.",
                        expectation_model::text::join_and(&differs)
                    )
                }
                Agreement::Unknown => {
                    let missing: Vec<String> = pairs
                        .filter(|((_, _, mine), (_, _, want))| mine.is_none() && want.is_some())
                        .map(|((_, label, _), _)| label.to_string())
                        .collect();
                    format!(
                        "The run didn't say its {}, so it isn't known whether this is about \
                         what this manual is for.",
                        expectation_model::text::join_and(&missing)
                    )
                }
            };
            Some((agreement, text))
        });
        (!said.is_empty() || verdict.is_some()).then_some(Against { said, verdict })
    }

    /// For the test report's first page, when the manual is for something: how many of the
    /// results that ran are about it.
    pub(crate) fn against_summary(&self) -> Option<String> {
        use expectation_story::Agreement;
        let want = self.look.expected.filter(|w| !w.is_empty())?;
        let (mut same, mut other, mut unknown) = (0, 0, 0);
        for i in 0..self.plans.len() {
            match self.against(i).and_then(|a| a.verdict) {
                Some((Agreement::Matching, _)) => same += 1,
                Some((Agreement::Mismatched, _)) => other += 1,
                Some((Agreement::Unknown, _)) => unknown += 1,
                None => {}
            }
        }
        let for_what: Vec<String> = want
            .parts()
            .into_iter()
            .filter_map(|(_, label, v)| Some(format!("{label} {}", v?)))
            .collect();
        Some(format!(
            "This manual is for {}. Of the tests that ran, {same} ran against it, {other} against \
             something else, and {unknown} didn't say. Each test's page says which; a result from \
             another run is what that run saw, and is never counted as a failure.",
            expectation_model::text::join_and(&for_what)
        ))
    }
    /// Is this the reader edition?
    pub fn reader(&self) -> bool {
        self.look.reader || self.outline.is_some_and(|o| o.reader)
    }

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

    /// Does this build write the pages of section `id`? All of them, unless it is partial.
    pub fn includes(&self, id: expectation_model::SectionId) -> bool {
        self.partial.is_none_or(|p| p.sections.contains(&id))
    }

    /// Each contract's slug, for its pages: from its name, with a number when two share one.
    pub fn contract_slugs(&self) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        for c in self.contracts {
            let base = match expectation_model::text::slug(&c.contract.name) {
                s if s.is_empty() => "contract".to_string(),
                s => s,
            };
            let mut slug = base.clone();
            let mut n = 1;
            while out.contains(&slug) {
                n += 1;
                slug = format!("{base}-{n}");
            }
            out.push(slug);
        }
        out
    }

    /// The evidence that counts for step `step` of plan `plan` (0 is its start), as
    /// (contract, evidence) indices.
    pub fn evidence_at(&self, plan: usize, step: usize) -> Vec<(usize, usize)> {
        let mut out = vec![];
        for (c, cov) in self.contracts.iter().enumerate() {
            for (e, credit) in cov.credit.iter().enumerate() {
                if *credit == (expectation_story::Credit::Counts { plan, step }) {
                    out.push((c, e));
                }
            }
        }
        out
    }

    /// The code coverage that counts for test `plan`: per file, its captures of this run of the
    /// test as it is now. A file with none that count isn't listed.
    pub fn code_coverage_of(
        &self,
        plan: usize,
    ) -> Vec<(
        &expectation_story::CodeCoverage,
        Vec<&expectation_story::Capture>,
    )> {
        let Some(report) = self.report else {
            return vec![];
        };
        self.look
            .code
            .iter()
            .map(|c| (c, c.for_plan(self.plans, report, plan)))
            .filter(|(_, found)| !found.is_empty())
            .collect()
    }

    /// Write the Markdown pages and the HTML site under `dir`. Pages of stories or features
    /// that no longer exist are removed (only the generated kinds, in the folders this writes
    /// to); other files in the directory are left alone.
    pub fn write(&self, dir: &Path) -> std::io::Result<Vec<PathBuf>> {
        // A screenshot the results name but nobody has (an interrupted run cleared it) is a
        // placeholder, not a broken image. From a store, the ones the pages show are copied in.
        let shots = self.look.shared.map_or(dir, |(root, _)| root);
        let checked = self.report.map(|r| {
            let mut r = r.clone();
            for result in &mut r.results {
                for step in std::iter::once(&mut result.start).chain(&mut result.steps) {
                    if let Some(p) = &step.screenshot
                        && !self.have_shot(shots, p)
                    {
                        step.screenshot = None;
                    }
                }
            }
            r
        });
        // Small copies for the state map's cards and the story cards, when a tool can make them.
        let shown: Vec<&str> = checked
            .iter()
            .flat_map(|r| &r.results)
            .flat_map(|r| std::iter::once(&r.start).chain(&r.steps))
            .filter_map(|s| s.screenshot.as_deref())
            .collect();
        let thumbs = match self.look.screenshots {
            Screenshots::Url(_) => Default::default(),
            _ => thumbs::make(shots, &shown),
        };
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
        for page in docs.pages_with(source, &thumbs) {
            let path = dir.join(&page.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, page.content)?;
            written.push(path);
        }
        // A set: the folders can hold thousands of pages, and each is looked up.
        // A preview writes some pages: the others in the directory are not stale.
        if self.partial.is_none() {
            prune(dir, &written.iter().map(PathBuf::as_path).collect())?;
        }
        if docs.look.stylesheet.is_none() {
            // An earlier run's custom stylesheet would otherwise sit there unused.
            let _ = std::fs::remove_file(dir.join(CUSTOM_CSS));
        }
        Ok(written)
    }

    pub fn pages(&self) -> Vec<Page> {
        self.pages_with(None, &Default::default())
    }

    /// Is the screenshot the results name `p` there to show? Taken from the store into `dir`
    /// if it is in one.
    fn have_shot(&self, dir: &Path, p: &str) -> bool {
        let here = dir.join(p);
        match self.look.screenshots {
            Screenshots::InDocs => here.exists(),
            Screenshots::Url(_) => true,
            Screenshots::Dir(store) => {
                let Some(rest) = p.strip_prefix("screenshots/") else {
                    return here.exists();
                };
                let from = store.join(rest);
                if !from.is_file() {
                    return here.exists();
                }
                // Already copied, and the same size: content-addressed names don't change.
                let same = std::fs::metadata(&here)
                    .ok()
                    .zip(std::fs::metadata(&from).ok())
                    .is_some_and(|(a, b)| {
                        a.len() == b.len() && a.modified().ok() >= b.modified().ok()
                    });
                same || here
                    .parent()
                    .is_some_and(|d| std::fs::create_dir_all(d).is_ok())
                    && std::fs::copy(&from, &here).is_ok()
            }
        }
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

    /// The pages, with `source` the suite's directory relative to the docs directory, and
    /// `thumbs` the screenshots (`<rest>` of `screenshots/<rest>`) that have a thumbnail.
    fn pages_with(
        &self,
        source: Option<String>,
        thumbs: &std::collections::HashSet<String>,
    ) -> Vec<Page> {
        let outline = outline::of(self);
        let docs = Docs {
            outline: outline.as_ref(),
            ..*self
        };
        let mut facts = Facts::new(&docs);
        facts.source = source;
        facts.thumbs = thumbs.clone();
        let mut pages = markdown::pages(&docs, &facts);
        pages.extend(html::pages(&docs, &facts));
        // In a partial build, a link to a page it doesn't write goes to the full manual or is text.
        if let Some(p) = self.partial {
            let built: std::collections::HashSet<String> =
                pages.iter().map(|p| p.path.clone()).collect();
            let base = p.base_url.as_deref();
            for page in &mut pages {
                let from = page
                    .path
                    .rsplit_once('/')
                    .map_or("", |(d, _)| d)
                    .to_string();
                if page.path.ends_with(".html") {
                    page.content = partial::html(&page.content, &from, &built, base);
                } else if page.path.ends_with(".md") {
                    page.content = partial::markdown(&page.content, &from, &built, base);
                }
            }
        }
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
    /// The screenshots (`<rest>` of `screenshots/<rest>`) that have a thumbnail.
    thumbs: std::collections::HashSet<String>,
}

impl Facts {
    /// A test that exercises `e`, if any does.
    fn first_test(&self, e: ExpId) -> Option<usize> {
        self.coverage
            .get(e.0 as usize)?
            .first()
            .map(|&(plan, _)| plan)
    }

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
            thumbs: Default::default(),
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
    ///
    /// For an API, a step's requests show it as a screenshot would: a step with action requests
    /// and no screenshot counts, and the screenshot is then `None`.
    fn shot_of(&self, d: &Docs, e: ExpId) -> Option<(usize, Option<usize>, Option<String>)> {
        let report = d.report?;
        let mut fallback = None;
        for &(p, step) in &self.coverage[e.0 as usize] {
            let result = &report.results[p];
            let (r, checks) = match step {
                None => (&result.start, &d.plans[p].start_checks),
                Some(s) => (&result.steps[s], &d.plans[p].steps[s].checks),
            };
            if r.screenshot.is_none() && actions(&r.exchanges).is_empty() {
                continue;
            }
            let shot = &r.screenshot;
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

/// The result of step `step` (`None` for the start) of plan `p`, if it ran.
fn step_result<'d>(d: &Docs<'d>, p: usize, step: Option<usize>) -> Option<&'d StepResult> {
    let r = d.report?.results.get(p)?;
    match step {
        None => Some(&r.start),
        Some(s) => r.steps.get(s),
    }
}

/// Requests with the same request sent again in a row counted instead of repeated: a check
/// that polled until it held asked the same thing many times.
fn collapse<'a>(xs: &[&'a Exchange]) -> Vec<(&'a Exchange, usize)> {
    let mut out: Vec<(&Exchange, usize)> = vec![];
    for &x in xs {
        match out.last_mut() {
            Some((last, n)) if **last == *x => *n += 1,
            _ => out.push((x, 1)),
        }
    }
    out
}

/// The requests that were a step's action: what a reading page shows of it.
fn actions(xs: &[Exchange]) -> Vec<&Exchange> {
    xs.iter().filter(|x| x.role == Role::Action).collect()
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
fn missing_shot(status: Status, stale: bool) -> &'static str {
    if stale && status == Status::NotRun {
        return "Not run since this page changed.";
    }
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
            contracts: &[],
            partial: None,
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
            contracts: &[],
            partial: None,
            look,
        };
        docs.write(out).unwrap();
        docs.pages_with(None, &Default::default())
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
            contracts: &[],
            partial: None,
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

    fn sections_site(n: usize) -> Vec<Page> {
        site_of(n, 1, false)
    }

    /// `n` sections of `k` expectations each; with `tests`, the generated test for each too.
    fn site_of(n: usize, k: usize, tests: bool) -> Vec<Page> {
        let values: Vec<String> = (0..=k).map(|j| format!("s{j}")).collect();
        let vocab = format!(
            "Vocabulary:\n  State: the lamp is off | on\n  State: the dial is {}\n  Event: the user flips the switch\n  Event: the user turns the dial\nBackground:\n  Given the lamp is off\n  And the dial is s0\n",
            values.join(" | ")
        );
        let feature = |i: usize| {
            let mut f = format!(
                "Feature: Topic {i}\n  What it does.\n  Expectation: Rule {i}\n    Given the lamp is off\n    When the user flips the switch\n    Then the lamp is on\n"
            );
            // More expectations, each from the dial's last value to its next.
            for j in 1..k {
                f.push_str(&format!(
                    "  Expectation: Rule {i}.{j}\n    Given the dial is s{}\n    When the user turns the dial\n    Then the dial is s{j}\n",
                    j - 1
                ));
            }
            f
        };
        let mut sources = vec![("vocab.expect".to_string(), vocab)];
        // Only the first section needs the expectation to be unique; the rest differ by number.
        for i in 0..n {
            sources.push((format!("{i:03}-area{i}/f.expect"), feature(i)));
        }
        let refs: Vec<(&str, &str)> = sources
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let loaded = expectation_language::load_sources("S", &refs);
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let plans = if tests {
            expectation_story::coverage_plans(&suite, &analysis, &[])
        } else {
            vec![]
        };
        Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &plans,
            report: None,
            suite_dir: None,
            siblings: &[],
            outline: None,
            inventory: &[],
            contracts: &[],
            partial: None,
            look: Look::default(),
        }
        .pages()
    }

    fn page<'a>(pages: &'a [Page], path: &str) -> &'a str {
        &pages
            .iter()
            .find(|p| p.path == path)
            .unwrap_or_else(|| {
                let paths: Vec<_> = pages.iter().map(|p| &p.path).collect();
                panic!("no {path} in {paths:?}")
            })
            .content
    }

    #[test]
    fn a_page_lists_its_own_surroundings_and_the_script_has_the_rest() {
        let pages = sections_site(3);
        let nav = page(&pages, "site/assets/nav.js");
        let here = page(&pages, "site/feature-area1--topic-1.html");
        // Its own section is open with its topic; the others are links with nothing under them.
        assert!(here.contains(r#"<details open><summary><a href="section-area1.html""#));
        assert!(here.contains(r#"<details data-nav="section-area0.html">"#));
        assert!(!here.contains("feature-area0--topic-0.html"));
        assert!(here.contains(r#"src="assets/nav.js""#));
        // The script knows what is under every section.
        assert!(nav.contains("feature-area0--topic-0.html"));
        assert!(nav.contains("feature-area2--topic-2.html"));
    }

    #[test]
    fn many_sibling_sections_are_folded_into_one_entry() {
        let pages = sections_site(40);
        let here = page(&pages, "site/feature-area7--topic-7.html");
        assert!(here.contains("39 more sections"), "{}", here.len());
        assert!(!here.contains("section-area20.html"));
        assert!(here.matches("<details").count() < 5);
        let nav = page(&pages, "site/assets/nav.js");
        assert!(nav.contains("index.html#sections"));
        assert!(nav.contains("section-area20.html"));
    }

    /// Every relative link and image in the site's pages names a page or file the site has.
    fn links_resolve(pages: &[Page]) {
        let have: std::collections::HashSet<&str> = pages.iter().map(|p| p.path.as_str()).collect();
        for p in pages
            .iter()
            .filter(|p| p.path.starts_with("site/") && p.path.ends_with(".html"))
        {
            for part in p.content.split("href=\"").skip(1) {
                let target = part.split(['"', '#', '?']).next().unwrap_or("");
                // Outside the site, or a template the page's script fills in.
                if target.is_empty()
                    || target.contains(':')
                    || target.starts_with("../")
                    || target.contains("${")
                {
                    continue;
                }
                assert!(
                    have.contains(format!("site/{target}").as_str()),
                    "{} links to {target}",
                    p.path
                );
            }
        }
    }

    #[test]
    fn a_large_test_report_and_map_are_an_index_of_section_pages() {
        let pages = site_of(html::SPLIT_OVER / 10 + 1, 10, true);
        let tests = page(&pages, "site/tests.html");
        // An index: a row per section, linking its page; no row per test.
        assert!(
            tests.contains(r#"<a href="tests-area7.html">"#),
            "{}",
            &tests[..2000.min(tests.len())]
        );
        assert!(
            !tests.contains("href=\"test-"),
            "no test is listed on the index"
        );
        assert!(tests.len() < 20_000, "{} bytes", tests.len());
        // A section's page lists its tests.
        let area = page(&pages, "site/tests-area7.html");
        assert!(area.contains("href=\"test-"));
        assert!(area.contains(r#"<a href="tests.html">Test report</a>"#));
        // The map too: an index without drawings, and a drawing per section.
        let map = page(&pages, "site/map.html");
        assert!(map.contains(r#"<a href="map-area7.html">"#) && !map.contains("<svg"));
        assert!(page(&pages, "site/map-area7.html").contains("<svg"));
        links_resolve(&pages);
    }

    #[test]
    fn a_small_test_report_and_map_stay_one_page() {
        let pages = site_of(3, 2, true);
        let tests = page(&pages, "site/tests.html");
        assert!(tests.contains("href=\"test-") && !tests.contains("tests-area"));
        assert!(page(&pages, "site/map.html").contains("<svg"));
        assert!(
            !pages
                .iter()
                .any(|p| p.path.starts_with("site/tests-") || p.path.starts_with("site/map-"))
        );
        links_resolve(&pages);
    }

    const HIDDEN: &str = "
Vocabulary:
  State: the turnstile is locked | unlocked
  Event: a visitor inserts a coin
  Event: a visitor pushes the arm
Background:
  Given the turnstile is locked
Feature: Paying
  What a coin does.
  Expectation: A coin unlocks it
    Given the turnstile is locked
    When a visitor inserts a coin
    Then the turnstile is unlocked
  Expectation: Pushing locks it
    Docs: hidden
    Given the turnstile is unlocked
    When a visitor pushes the arm
    Then the turnstile is locked
";

    #[test]
    fn a_hidden_expectation_is_linked_to_its_test_not_to_a_missing_anchor() {
        let loaded = expectation_language::load_sources("S", &[("s.expect", HIDDEN)]);
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let mut plans = expectation_story::plan_stories(&suite, &analysis).plans;
        let extra = expectation_story::coverage_plans(&suite, &analysis, &plans);
        plans.extend(extra);
        let docs = Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &plans,
            report: None,
            suite_dir: None,
            siblings: &[],
            outline: None,
            inventory: &[],
            contracts: &[],
            partial: None,
            look: Look::default(),
        };
        let out = temp("hidden");
        docs.write(&out).unwrap();
        let map = std::fs::read_to_string(out.join("site/map.html")).unwrap();
        assert!(
            !map.contains("#pushing-locks-it"),
            "linked to a missing anchor"
        );
        assert!(
            map.contains("test-coverage--pushing-locks-it.html"),
            "no link to its test"
        );
        assert_eq!(broken_links(&out), Vec::<String>::new());
        // A link to nothing is found, and so is a link to an anchor that isn't there.
        std::fs::write(
            out.join("site/extra.html"),
            r##"<a href="gone.html">x</a> <a href="index.html#nope">y</a> <a href="index.html">z</a> <a href="https://example.com/">w</a> <a href="#top" id="top">v</a>"##,
        )
        .unwrap();
        assert_eq!(
            broken_links(&out),
            vec!["extra.html: gone.html", "extra.html: index.html#nope"]
        );
    }

    #[test]
    fn platforms_are_explained_in_the_menu_and_on_the_first_page() {
        let src = "Vocabulary:\n  Platform: desktop | iOS\n    About: desktop: the app on a computer\n    About: iOS: the app on an iPad\n  State: the lamp is on | off\nBackground:\n  Given the lamp is on\n";
        let loaded = expectation_language::load_sources("S", &[("s.expect", src)]);
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let pages = Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &[],
            report: None,
            suite_dir: None,
            siblings: &[],
            outline: None,
            inventory: &[],
            contracts: &[],
            partial: None,
            look: Look::default(),
        }
        .pages();
        let home = page(&pages, "site/index.html");
        assert!(home.contains("Version of this manual"));
        assert!(home.contains(r#"title="the app on an iPad""#));
        assert!(home.contains("Versions of this manual</h2>"));
        assert!(home.contains("<strong>iOS</strong>: the app on an iPad"));
        assert!(page(&pages, "README.md").contains("- **desktop**: the app on a computer"));
    }

    #[test]
    fn next_can_follow_the_tutorials_and_ends_at_the_glossary() {
        let dir = temp("tutorial-chain");
        let area = |name: &str| {
            format!(
                "Section: {name}\n  About {name}.\nVocabulary:\n  State: the {name} lamp is on | off\n  Event: the user flips the {name} switch\nBackground:\n  Given the {name} lamp is off\nFeature: {name} lamp\n  Expectation: Flip {name}\n    Given the {name} lamp is off\n    When the user flips the {name} switch\n    Then the {name} lamp is on\nStory: Using {name}\n  As a user\n  I want light\n  So that I see\n  Scenario: x\n    When the user flips the {name} switch\n    Then the {name} lamp is on\n"
            )
        };
        for name in ["Alpha", "Beta"] {
            std::fs::create_dir_all(dir.join(name)).unwrap();
            std::fs::write(dir.join(name).join("s.expect"), area(name)).unwrap();
        }
        std::fs::write(
            dir.join("outline.txt"),
            "Part: Start\n  Section: Alpha\n  Section: Beta\nNext: tutorials\n",
        )
        .unwrap();
        let loaded = expectation_language::load_dir(&dir).unwrap();
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let plans = expectation_story::plan_stories(&suite, &analysis).plans;
        let pages = Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &plans,
            report: None,
            suite_dir: Some(&dir),
            siblings: &[],
            outline: None,
            inventory: &[],
            contracts: &[],
            partial: None,
            look: Look::default(),
        }
        .pages();
        let next = |path: &str| {
            let html = page(&pages, path);
            let i = html.find(r#"class="next" href=""#)?;
            let rest = &html[i + r#"class="next" href=""#.len()..];
            Some(rest[..rest.find('"').unwrap()].to_string())
        };
        // Alpha's page, its tutorial, then Beta's page: the topic page is skipped.
        assert_eq!(
            next("site/section-alpha.html").as_deref(),
            Some("tutorial-alpha--using-alpha--x.html")
        );
        assert_eq!(
            next("site/tutorial-alpha--using-alpha--x.html").as_deref(),
            Some("section-beta.html")
        );
        // The last tutorial leads to the glossary, which has a way back.
        assert_eq!(
            next("site/tutorial-beta--using-beta--x.html").as_deref(),
            Some("glossary.html")
        );
        assert!(page(&pages, "site/glossary.html").contains(r#"class="prev""#));
        // A topic page is not in the chain.
        assert_eq!(next("site/feature-alpha--alpha-lamp.html"), None);
    }

    #[test]
    fn a_section_page_lists_its_own_tutorials_before_its_subsections() {
        let dir = temp("own-first");
        let area = |name: &str, vocab: bool| {
            let v = if vocab {
                "Vocabulary:\n  State: the lamp is on | off\n  Event: the user flips the switch\nBackground:\n  Given the lamp is off\n"
            } else {
                ""
            };
            format!(
                "Section: {name}\n  About {name}.\n{v}Feature: {name} feature\n  Expectation: Flip {name}\n    Given the lamp is off\n    When the user flips the switch\n    Then the lamp is on\nStory: Using {name}\n  As a user\n  I want light\n  So that I see\n  Scenario: x\n    When the user flips the switch\n    Then the lamp is on\n"
            )
        };
        std::fs::create_dir_all(dir.join("Parent/Child")).unwrap();
        std::fs::write(dir.join("Parent/p.expect"), area("Parent", true)).unwrap();
        std::fs::write(dir.join("Parent/Child/c.expect"), area("Child", false)).unwrap();
        // The outline reads the subsection before the section that holds it.
        std::fs::write(
            dir.join("outline.txt"),
            "Part: Start\n  Section: Child\n  Section: Parent\n",
        )
        .unwrap();
        let loaded = expectation_language::load_dir(&dir).unwrap();
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let plans = expectation_story::plan_stories(&suite, &analysis).plans;
        let pages = Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &plans,
            report: None,
            suite_dir: Some(&dir),
            siblings: &[],
            outline: None,
            inventory: &[],
            contracts: &[],
            partial: None,
            look: Look::default(),
        }
        .pages();
        let parent = page(&pages, "site/section-parent.html");
        let own = parent
            .find("tutorial-parent--using-parent--x.html")
            .unwrap();
        let child = parent
            .find("tutorial-parent--child--using-child--x.html")
            .unwrap();
        assert!(own < child, "the section's own tutorial comes first");
    }

    #[test]
    fn the_reader_edition_leaves_out_the_test_machinery() {
        let src = "Vocabulary:\n  State: the lamp is on | off\n  Event: the user flips the switch\nBackground:\n  Given the lamp is off\nFeature: Lamp\n  What it does.\n  Expectation: Flip\n    Given the lamp is off\n    When the user flips the switch\n    Then the lamp is on\nStory: Using the lamp\n  As a user\n  I want light\n  So that I see\n  Scenario: x\n    When the user flips the switch\n    Then the lamp is on\n";
        let loaded = expectation_language::load_sources("S", &[("s.expect", src)]);
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let plans = expectation_story::plan_stories(&suite, &analysis).plans;
        let pages = |reader: bool| {
            Docs {
                suite: &suite,
                analysis: &analysis,
                plans: &plans,
                report: None,
                suite_dir: None,
                siblings: &[],
                outline: None,
                inventory: &[],
                contracts: &[],
                partial: None,
                look: Look {
                    reader,
                    ..Look::default()
                },
            }
            .pages()
        };
        let (full, reader) = (pages(false), pages(true));
        for path in [
            "site/tutorial-using-the-lamp--x.html",
            "site/feature-lamp.html",
            "tutorials/using-the-lamp--x.md",
            "features/lamp.md",
        ] {
            let (f, r) = (page(&full, path), page(&reader, path));
            assert!(f.contains("How this is tested"), "{path}");
            assert!(!r.contains("How this is tested"), "{path}");
            assert!(
                !r.contains("Tested by") && !r.contains("Its report"),
                "{path}"
            );
            assert!(
                !r.contains("Screenshot to come") && !r.contains("No screenshot"),
                "{path}"
            );
            assert!(
                !r.contains("placeholder") && !r.contains("no-screenshot.svg"),
                "{path}"
            );
        }
        // The user story stays, and the report is one entry in the sidebar.
        assert!(page(&reader, "site/tutorial-using-the-lamp--x.html").contains("The user story"));
        assert!(page(&reader, "site/feature-lamp.html").contains("For developers"));
        assert!(!page(&full, "site/feature-lamp.html").contains("For developers"));
        // The report, the map and the vocabulary are still written.
        for path in [
            "site/tests.html",
            "site/map.html",
            "site/states.html",
            "site/vocabulary.html",
        ] {
            assert!(reader.iter().any(|p| p.path == path), "{path}");
        }
    }

    #[test]
    fn a_section_that_skips_the_default_platform_shows_its_first_platforms_results() {
        let root = "Vocabulary:\n  Platform: desktop | iOS\n    About: sim: the game with simulated drones\n  State: the lamp is on | off\n  Event: the user flips the switch\nBackground:\n  Given the lamp is off\n";
        let story = |name: &str| {
            format!(
                "Story: {name}\n  As a user\n  I want light\n  So that I see\n  Scenario: x\n    When the user flips the switch\n    Then the lamp is on\n"
            )
        };
        let ground = format!("Section: Ground\n\n{}", story("Lighting the ground"));
        let towers = format!(
            "Section: Towers\n\nVocabulary:\n  Platform: sim | field\n\n{}",
            story("Lighting a tower")
        );
        let loaded = expectation_language::load_sources(
            "S",
            &[
                ("s.expect", root),
                ("01-ground/g.expect", &ground),
                ("02-towers/t.expect", &towers),
            ],
        );
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let plans = expectation_story::plan_stories(&suite, &analysis).plans;
        let sim_ran = Report {
            results: plans
                .iter()
                .map(|p| expectation_story::report::PlanResult::with(Status::Passed, p))
                .collect(),
        };
        let mut report = Report::not_run(&plans);
        report.borrow_from(&plans, &sim_ran, &plans, |p| {
            suite.shown_platform(p.section) == Some("sim")
        });
        let pages = Docs {
            suite: &suite,
            analysis: &analysis,
            plans: &plans,
            report: Some(&report),
            suite_dir: None,
            siblings: &[],
            outline: None,
            inventory: &[],
            contracts: &[],
            partial: None,
            look: Look::default(),
        }
        .pages();
        let page = |name: &str| {
            pages
                .iter()
                .find(|p| p.path == format!("site/{name}.html"))
                .unwrap_or_else(|| panic!("{name}"))
                .content
                .clone()
        };
        let towers = page("tutorial-towers--lighting-a-tower--x");
        assert!(
            towers.contains("Shown for <strong>sim</strong>: the game with simulated drones"),
            "{towers}"
        );
        assert!(towers.contains("selected>sim"), "{towers}");
        assert!(towers.contains("passed"));
        let ground = page("tutorial-ground--lighting-the-ground--x");
        assert!(!ground.contains("Shown for"));
        assert!(ground.contains("selected>desktop"));
    }

    #[test]
    fn code_coverage_is_shown_as_observed_execution_for_the_run_and_test_it_came_from() {
        let src = "Vocabulary:\n  State: the lamp is on | off\n  Event: the user flips the switch\n  Event: the user presses reset\nBackground:\n  Given the lamp is off\nFeature: Lamp\n  Expectation: Flip\n    Given the lamp is off\n    When the user flips the switch\n    Then the lamp is on\n  Expectation: Reset\n    Given the lamp is on\n    When the user presses reset\n    Then the lamp is off\nStory: Using the lamp\n  As a user\n  I want light\n  So that I see\n  Scenario: x\n    When the user flips the switch\n    Then the lamp is on\n    When the user presses reset\n    Then the lamp is off\n";
        let loaded = expectation_language::load_sources("S", &[("s.expect", src)]);
        assert!(!loaded.has_errors(), "{}", loaded.render_diagnostics());
        let suite = loaded.suite;
        let analysis = expectation_graph::analyze(&suite);
        let plans = expectation_story::plan_stories(&suite, &analysis).plans;
        let mut report = Report::not_run(&plans);
        for (r, p) in report.results.iter_mut().zip(&plans) {
            *r = expectation_story::PlanResult::with(Status::Passed, p);
            r.run = Some(expectation_story::Run {
                id: "r1".into(),
                started_ms: 1,
                provenance: Default::default(),
            });
        }
        let (slug, print) = (&plans[0].slug, expectation_story::fingerprint(&plans[0]));
        let file = |run: &str, print: &str| {
            format!(
                "expectation-code-coverage 1\nsource\tapp\tabc123\tsrc/**\thttps://cov.example/r\ncapture\t{run}\t{slug}\t{print}\t0\tcaptured\tbaseline\t-\t5\t10\t1\t4\t2\t3\t-\t-\t-\ncapture\t{run}\t{slug}\t{print}\t1\tcaptured\tstep\t-\t7\t10\t2\t4\t2\t3\t2\t1\t0\ncapture\t{run}\t{slug}\t{print}\t2\tmissing\tnone\tno snapshot\t0\t0\t0\t0\t0\t0\t-\t-\t-\n"
            )
        };
        let pages = |code: &[expectation_story::CodeCoverage]| {
            Docs {
                suite: &suite,
                analysis: &analysis,
                plans: &plans,
                report: Some(&report),
                suite_dir: None,
                siblings: &[],
                outline: None,
                inventory: &[],
                contracts: &[],
                partial: None,
                look: Look {
                    code,
                    ..Look::default()
                },
            }
            .pages()
        };
        let find = |pages: &[Page], path: &str| {
            pages
                .iter()
                .find(|p| p.path == path)
                .unwrap_or_else(|| panic!("{path}"))
                .content
                .clone()
        };
        let good = [
            expectation_story::CodeCoverage::parse("app.code.tsv", &file("r1", &print)).unwrap(),
        ];
        let shown = pages(&good);
        let (html, md) = (
            find(&shown, &format!("site/test-{slug}.html")),
            find(&shown, &format!("tests/{slug}.md")),
        );
        for page in [&html, &md] {
            assert!(page.contains("Code coverage"), "{page}");
            assert!(page.contains("observed execution"));
            assert!(page.contains("abc123") && page.contains("src/"));
            assert!(page.contains("7 of 10"));
            assert!(page.contains("2 lines, 1 branch, 0 functions"));
            assert!(page.contains("No capture at this step"));
            assert!(page.contains("isn't the same as nothing being covered"));
            assert!(page.contains("https://cov.example/r"));
        }
        // The result is what it was.
        assert!(html.contains("passed"));
        // Text from the file can't become markup, in either kind of page.
        let hostile = file("r1", &print)
            .replace(
                "source\tapp\tabc123\tsrc/**",
                "source\t<b>app</b>\t\"><script>x</script>\t[a](http://evil)",
            )
            .replace("no snapshot", "<img src=x onerror=alert(1)>");
        let bad = [expectation_story::CodeCoverage::parse("a.code.tsv", &hostile).unwrap()];
        let shown = pages(&bad);
        let html = find(&shown, &format!("site/test-{slug}.html"));
        let md = find(&shown, &format!("tests/{slug}.md"));
        for page in [&html, &md] {
            assert!(
                !page.contains("<script>x") && !page.contains("<img src=x"),
                "{page}"
            );
            assert!(!page.contains("<b>app"));
        }
        assert!(!md.contains("[a](http://evil)"), "{md}");
        // Another run's captures, or a changed test's, are not shown.
        for other in [file("r2", &print), file("r1", "0000")] {
            let bad = [expectation_story::CodeCoverage::parse("app.code.tsv", &other).unwrap()];
            let hidden = pages(&bad);
            assert!(!find(&hidden, &format!("site/test-{slug}.html")).contains("Code coverage"));
            assert!(!find(&hidden, &format!("tests/{slug}.md")).contains("Code coverage"));
        }
    }
}
