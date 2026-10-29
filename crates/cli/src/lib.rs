//! `expectation`: check a suite, look at its graph and plans, and write docs, without
//! running anything. Running needs the system under test, so that happens through
//! `expectation_harness` in its own process, which saves its results for the docs.

use expectation_graph::Analysis;
use expectation_language::Loaded;
use expectation_model::diagnostic::{has_errors, render_all};
use expectation_model::{Diagnostic, Suite};
use expectation_story::{Because, Plan};
use std::path::Path;
use std::process::exit;

const USAGE: &str = "\
usage: expectation <command> <suite-dir> [options]

commands:
  check <dir>               check the vocabulary, conflicts, overrides and stories
  graph <dir> [--dot]       print the flow DAG as Mermaid, or Graphviz DOT
  states <dir> [--by <states>] [--dot]
                            print the map of the app's states and the moves between them, as
                            a Mermaid state diagram or Graphviz DOT; --by names the states that
                            tell them apart, comma separated (default: every enumerated one)
  plan  <dir> [--coverage]  print the acceptance tests the stories compile to
                            (--coverage adds generated tests for untested expectations)
  docs  <dir> <out-dir> [--approved-only] [--results <dir>] [--stylesheet <css>] [--state-shot-ratio <w:h>]
                            write the docs (Markdown and an HTML site), with the results runs
                            have saved in <out-dir>/results; --stylesheet restyles the site
                            with a CSS file, --state-shot-ratio shapes the state map's pictures;
                            --approved-only writes just what is marked `Approval: approved`
                            (--results <dir> reads results from there, not <out-dir>/results)
";

struct Checked {
    suite: Suite,
    analysis: Analysis,
    plans: Vec<Plan>,
    diagnostics: Vec<Diagnostic>,
}

fn load(dir: &str) -> Checked {
    load_platform(dir, None)
}

fn load_for(dir: &str, platform: &str) -> Checked {
    load_platform(dir, Some(platform))
}

fn load_platform(dir: &str, platform: Option<&str>) -> Checked {
    load_platform_with(dir, platform, false)
}

/// As `load_platform`; `approved_only` keeps just what the suite marks `Approval: approved`.
fn load_platform_with(dir: &str, platform: Option<&str>, approved_only: bool) -> Checked {
    let Loaded {
        suite,
        mut diagnostics,
    } = expectation_language::load_dir_with(Path::new(dir), platform, approved_only)
        .unwrap_or_else(|e| {
            eprintln!("error: couldn't read {dir}: {e}");
            exit(2)
        });
    if has_errors(&diagnostics) {
        eprint!("{}", render_all(&diagnostics, &suite.sources));
        if approved_only {
            eprintln!("note: --approved-only leaves out what isn't marked `Approval: approved`");
        }
        exit(1);
    }
    let analysis = expectation_graph::analyze(&suite);
    diagnostics.extend(analysis.diagnostics.iter().cloned());
    let planned = expectation_story::plan_stories(&suite, &analysis);
    diagnostics.extend(planned.diagnostics);
    diagnostics.extend(expectation_story::untested(
        &suite,
        &analysis,
        &planned.plans,
    ));
    Checked {
        suite,
        analysis,
        plans: planned.plans,
        diagnostics,
    }
}

/// Runs the command with the process's arguments, and exits with its status: 0, 1 for errors in
/// the suite, 2 for usage. A library, so that a project can ship the command under its own
/// package.
pub fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--by <states>` takes a value; the other options are flags.
    let mut take = |name: &str| {
        args.iter().position(|a| a == name).map(|i| {
            let v = args.get(i + 1).cloned().unwrap_or_else(|| {
                eprintln!("error: {name} needs a value");
                exit(2)
            });
            args.drain(i..i + 2);
            v
        })
    };
    let by = take("--by");
    let look = (take("--stylesheet"), take("--state-shot-ratio"));
    let results = take("--results");
    let approved_only = args.iter().any(|a| a == "--approved-only");
    let flag = |f: &str| args.iter().any(|a| a == f);
    let pos: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .collect();
    match pos.as_slice() {
        ["check", dir] => {
            // Every platform the suite declares reads as a suite of its own, and must check clean.
            let platforms = load(dir).suite.platforms;
            check(dir, None);
            for p in platforms.iter().skip(1) {
                check(dir, Some(p));
            }
        }
        ["graph", dir] => {
            let c = load(dir);
            if flag("--dot") {
                print!("{}", expectation_graph::export::dot(&c.suite, &c.analysis));
            } else {
                print!(
                    "{}",
                    expectation_graph::export::mermaid(&c.suite, &c.analysis, &|_| None)
                );
            }
        }
        ["states", dir] => {
            let c = load(dir);
            let by = states_by(&c.suite, by.as_deref());
            let map = expectation_graph::states::explore(
                &c.suite,
                &c.analysis.precedence,
                &by,
                Default::default(),
            );
            if flag("--dot") {
                print!("{}", expectation_graph::states::dot(&c.suite, &map));
            } else {
                print!("{}", expectation_graph::states::mermaid(&c.suite, &map));
            }
            if map.truncated {
                eprintln!("warning: the search hit its limits, so the map may be missing states");
            }
        }
        ["plan", dir] => {
            let mut c = load(dir);
            if has_errors(&c.diagnostics) {
                eprint!("{}", render_all(&c.diagnostics, &c.suite.sources));
                exit(1);
            }
            if flag("--coverage") {
                let extra = expectation_story::coverage_plans(&c.suite, &c.analysis, &c.plans);
                c.plans.extend(extra);
                let visits = expectation_story::visit_plans(&c.suite, &c.analysis, &c.plans);
                c.plans.extend(visits);
            }
            for p in &c.plans {
                print_plan(&c.suite, p);
            }
        }
        ["docs", dir, out] => docs(dir, out, look, approved_only, results),
        _ => {
            eprint!("{USAGE}");
            exit(2);
        }
    }
}

/// The manual, once for each platform the suite declares (the first at `out`, the others
/// under `out/platforms/`), each with the results its own runs saved under `out/results`.
fn docs(
    dir: &str,
    out: &str,
    (stylesheet, ratio): (Option<String>, Option<String>),
    approved_only: bool,
    results: Option<String>,
) {
    let fail = |e: String| -> ! {
        eprintln!("error: {e}");
        exit(2)
    };
    let css = stylesheet
        .map(|p| expectation_docs::read_stylesheet(Path::new(&p)).unwrap_or_else(|e| fail(e)));
    let ratio = ratio.map(|r| expectation_docs::parse_ratio(&r).unwrap_or_else(|e| fail(e)));
    let look = expectation_docs::Look {
        stylesheet: css.as_deref(),
        shot_ratio: ratio,
    };
    let first = load(dir);
    let platforms = first.suite.platforms.clone();
    let names: Vec<Option<String>> = if platforms.is_empty() {
        vec![None]
    } else {
        platforms.into_iter().map(Some).collect()
    };
    // Every platform's tests and results, then the pages: each one's dropdown knows the others' pages.
    let mut manuals = vec![];
    for name in &names {
        let platform = name
            .as_deref()
            .filter(|n| Some(*n) != first.suite.platforms.first().map(String::as_str));
        // The whole suite must check clean either way; a manual of the approved part is a view of it.
        let mut c = match platform {
            Some(n) => load_for(dir, n),
            None => load(dir),
        };
        if approved_only {
            c = load_platform_with(dir, platform, true);
            if c.suite.features.is_empty() && c.suite.stories.is_empty() {
                eprintln!(
                    "warning: nothing is marked `Approval: approved`, so the manual is empty"
                );
            }
        }
        if has_errors(&c.diagnostics) {
            eprint!("{}", render_all(&c.diagnostics, &c.suite.sources));
            if approved_only {
                eprintln!(
                    "note: --approved-only leaves out what isn't marked `Approval: approved`; an approved \
                     story or expectation can't rely on behaviour that isn't"
                );
            }
            exit(1);
        }
        // The same tests a run has (stories and generated ones), with whatever results
        // runs have saved under <out>/results.
        let extra = expectation_story::coverage_plans(&c.suite, &c.analysis, &c.plans);
        c.plans.extend(extra);
        let visits = expectation_story::visit_plans(&c.suite, &c.analysis, &c.plans);
        c.plans.extend(visits);
        let report = expectation_story::load_results_for(
            &results.as_deref().map_or_else(
                || Path::new(out).join("results"),
                |r| Path::new(r).to_path_buf(),
            ),
            &c.plans,
            name.as_deref(),
        );
        manuals.push((c, report));
    }
    let docs_of = |i: usize, siblings: &[expectation_docs::Sibling]| {
        let (c, report) = &manuals[i];
        let docs = expectation_docs::Docs {
            suite: &c.suite,
            analysis: &c.analysis,
            plans: &c.plans,
            report: Some(report),
            suite_dir: Some(Path::new(dir)),
            siblings,
            outline: None,
            inventory: &[],
            look,
        };
        let tree = match &names[i] {
            Some(n) if i > 0 => Path::new(out)
                .join("platforms")
                .join(expectation_model::text::slug(n)),
            _ => Path::new(out).to_path_buf(),
        };
        docs.write(&tree)
    };
    let all: Vec<expectation_docs::Sibling> = (0..manuals.len())
        .map(|i| {
            let (c, report) = &manuals[i];
            expectation_docs::Docs {
                suite: &c.suite,
                analysis: &c.analysis,
                plans: &c.plans,
                report: Some(report),
                suite_dir: None,
                siblings: &[],
                outline: None,
                inventory: &[],
                look,
            }
            .sibling()
        })
        .collect();
    for (i, name) in names.iter().enumerate().take(manuals.len()) {
        match docs_of(i, &all) {
            Ok(files) => println!(
                "wrote {} pages to {out}{}",
                files.len(),
                name.as_ref()
                    .filter(|_| i > 0)
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default()
            ),
            Err(e) => {
                eprintln!("error: couldn't write {out}: {e}");
                exit(2);
            }
        }
    }
}

/// The states `--by` names, comma separated, or none for the default.
fn states_by(suite: &Suite, by: Option<&str>) -> Vec<expectation_model::VarId> {
    let Some(by) = by else { return vec![] };
    by.split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| match suite.vocabulary.find_var(n) {
            Some(v) => v.id,
            None => {
                let known: Vec<&str> = suite
                    .vocabulary
                    .variables
                    .iter()
                    .map(|v| v.subject.as_str())
                    .collect();
                eprintln!(
                    "error: --by: no state called «{n}» (the states are: {})",
                    known.join(", ")
                );
                exit(2)
            }
        })
        .collect()
}

fn check(dir: &str, platform: Option<&str>) {
    let c = load_platform(dir, platform);
    if !c.diagnostics.is_empty() {
        eprint!("{}", render_all(&c.diagnostics, &c.suite.sources));
    }
    let s = &c.suite;
    let deepest = c
        .analysis
        .flow
        .level
        .iter()
        .flatten()
        .max()
        .copied()
        .unwrap_or(0);
    let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
    println!(
        "{}{}: {} in {}, {} ({}); {}; flow DAG {}, {} deep{}",
        s.name,
        platform.map(|p| format!(" on {p}")).unwrap_or_default(),
        n(s.expectations.len(), "expectation", "expectations"),
        n(s.features.len(), "feature", "features"),
        n(s.stories.len(), "story", "stories"),
        n(c.plans.len(), "test", "tests"),
        n(c.analysis.exceptions().count(), "exception", "exceptions"),
        n(c.analysis.flow.edges.len(), "edge", "edges"),
        n(deepest as usize + 1, "level", "levels"),
        if c.analysis.flow.truncated {
            " (search truncated)"
        } else {
            ""
        }
    );
    if has_errors(&c.diagnostics) {
        exit(1);
    }
}

fn print_plan(s: &Suite, p: &Plan) {
    let v = &s.vocabulary;
    println!("{}", p.title);
    println!("  arrange  {}", v.describe(&p.start));
    for (i, step) in p.steps.iter().enumerate() {
        let when: Vec<String> = step
            .text
            .iter()
            .filter(|t| !matches!(t.keyword, expectation_model::Keyword::Then))
            .map(|t| t.text.clone())
            .collect();
        println!("  {}. when   {}", i + 1, when.join(" and "));
        for &e in &step.fired {
            println!("       fires  «{}»", s.exp(e).name);
        }
        for c in &step.checks {
            let why = match &c.because {
                Because::Arranged => "arranged".to_string(),
                Because::Expectation(e) => format!("«{}»", s.exp(*e).name),
                Because::Story(span) => format!("story, {span}"),
                Because::Setup(x) => format!("setup of «{}»", s.section(*x).name),
            };
            println!(
                "       check  {}  ({why})",
                v.var(c.var).describe(&c.expect)
            );
        }
    }
    println!();
}
