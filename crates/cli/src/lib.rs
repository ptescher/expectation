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
        [--screenshots <dir|url>]
                            write the docs (Markdown and an HTML site), with the results runs
                            have saved in <out-dir>/results; --stylesheet restyles the site
                            with a CSS file, --state-shot-ratio shapes the state map's pictures;
                            --approved-only writes just what is marked `Approval: approved`
                            (--results <dir> reads results from there, not <out-dir>/results);
                            --screenshots says where the screenshots are: a store directory
                            (the ones shown are copied in) or a URL prefix the pages link to
  screenshots prune <store> <results-dir>... [--dry-run]
                            remove the screenshots in a content-addressed store that no
                            results file in the given directories names, and their thumbnails
  screenshots push <store> <bucket> [--dry-run]
  screenshots pull <bucket> <store> [--dry-run]
                            copy a store to or from a bucket, adding what the other side lacks
                            and deleting nothing: s3://… (aws), gs://… (gcloud or gsutil), or
                            an rclone remote (name:path)
  screenshots lfs <store>   keep the store's images in Git LFS: writes <store>/.gitattributes
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
    let look = (
        take("--stylesheet"),
        take("--state-shot-ratio"),
        take("--screenshots"),
    );
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
        ["screenshots", "prune", store, dirs @ ..] if !dirs.is_empty() => {
            prune_screenshots(store, dirs, flag("--dry-run"))
        }
        ["screenshots", "push", store, bucket] => {
            sync_bucket(store, bucket, true, flag("--dry-run"))
        }
        ["screenshots", "pull", bucket, store] => {
            sync_bucket(store, bucket, false, flag("--dry-run"))
        }
        ["screenshots", "lfs", store] => lfs_screenshots(store),
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
    (stylesheet, ratio, shots): (Option<String>, Option<String>, Option<String>),
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
        screenshots: shots
            .as_deref()
            .map_or_else(Default::default, expectation_docs::Screenshots::parse),
    };
    if let Some(expectation_docs::Screenshots::Dir(d)) =
        shots.as_deref().map(expectation_docs::Screenshots::parse)
        && !d.is_dir()
    {
        let given = d.display().to_string();
        if given.contains("://") || bucket_tool(&given).is_ok() {
            fail(format!(
                "--screenshots {given}: give the address it is served at (https://…), or pull it \
                 first: `expectation screenshots pull {given} <dir>` and `--screenshots <dir>`"
            ));
        }
        fail(format!("--screenshots {given}: no such directory"));
    }
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
        let results_dir = results.as_deref().map_or_else(
            || Path::new(out).join("results"),
            |r| Path::new(r).to_path_buf(),
        );
        let report = expectation_story::load_results_for(&results_dir, &c.plans, name.as_deref());
        // Coverage files that extensions wrote next to the results, held to these results.
        let (found, errors) = expectation_story::load_contracts_for(&results_dir, name.as_deref());
        for e in errors {
            eprintln!("warning: coverage file {e}");
        }
        let contracts: Vec<expectation_story::Coverage> = found
            .into_iter()
            .map(|contract| {
                for p in &contract.problems {
                    eprintln!("warning: {}: {p}", contract.file);
                }
                expectation_story::Coverage::judge(contract, &c.plans, &report)
            })
            .collect();
        manuals.push((c, report, contracts));
    }
    let docs_of = |i: usize, siblings: &[expectation_docs::Sibling]| {
        let (c, report, contracts) = &manuals[i];
        let docs = expectation_docs::Docs {
            suite: &c.suite,
            analysis: &c.analysis,
            plans: &c.plans,
            report: Some(report),
            suite_dir: Some(Path::new(dir)),
            siblings,
            outline: None,
            inventory: &[],
            contracts,
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
    // Only a suite with several platforms offers the same page on the others, so only then are
    // all the pages worked out ahead of writing them.
    let several = manuals.len() > 1;
    let all: Vec<expectation_docs::Sibling> = (0..if several { manuals.len() } else { 0 })
        .map(|i| {
            let (c, report, contracts) = &manuals[i];
            expectation_docs::Docs {
                suite: &c.suite,
                analysis: &c.analysis,
                plans: &c.plans,
                report: Some(report),
                suite_dir: None,
                siblings: &[],
                outline: None,
                inventory: &[],
                contracts,
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

/// `expectation screenshots prune`: delete what a content-addressed store holds that no results
/// file names. Only files named by a hash (`<64 hex>.<ext>`) are touched, in the store and its
/// `thumbs/`; with no results files at all, nothing is.
fn prune_screenshots(store: &str, dirs: &[&str], dry_run: bool) {
    let mut named = std::collections::HashSet::new();
    let mut files = 0;
    for d in dirs {
        for entry in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "tsv")
                || expectation_story::is_coverage_file(&path)
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !text.starts_with("expectation-results ") {
                continue;
            }
            files += 1;
            named.extend(expectation_story::screenshots_named(&text));
        }
    }
    if files == 0 {
        eprintln!(
            "error: no results files in {}, so nothing is pruned",
            dirs.join(", ")
        );
        exit(2);
    }
    let hashed = |name: &str| {
        name.split_once('.').is_some_and(|(stem, ext)| {
            stem.len() == 64
                && stem
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                && !ext.is_empty()
                && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        })
    };
    let (mut gone, mut kept, mut bytes) = (0, 0, 0u64);
    let store = Path::new(store);
    for dir in [store.to_path_buf(), store.join("thumbs")] {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.path().is_file() || !hashed(&name) {
                continue;
            }
            if named.contains(&name) {
                kept += 1;
                continue;
            }
            bytes += entry.metadata().map_or(0, |m| m.len());
            gone += 1;
            if dry_run {
                println!("would remove {}", entry.path().display());
            } else if let Err(e) = std::fs::remove_file(entry.path()) {
                eprintln!("error: couldn't remove {}: {e}", entry.path().display());
            }
        }
    }
    println!(
        "{} {gone} screenshots ({:.1} MB) that none of {files} results files names; kept {kept}",
        if dry_run { "would remove" } else { "removed" },
        bytes as f64 / 1_048_576.0
    );
}

/// How a bucket is reached: by the command-line tool its provider ships. Expectation doesn't
/// talk to clouds itself; the tool brings its own credentials and configuration.
#[derive(Debug, PartialEq)]
enum Bucket {
    /// `s3://bucket/prefix`, with the AWS CLI. An S3-compatible store (R2, MinIO) works with
    /// `AWS_ENDPOINT_URL` set, which the AWS CLI reads.
    S3,
    /// `gs://bucket/prefix`, with `gcloud storage`, or `gsutil` without it.
    Gcs,
    /// An rclone remote, `name:path`: any provider rclone knows.
    Rclone,
}

fn bucket_tool(dest: &str) -> Result<Bucket, String> {
    if dest.starts_with("s3://") {
        Ok(Bucket::S3)
    } else if dest.starts_with("gs://") {
        Ok(Bucket::Gcs)
    } else if dest
        .split_once(':')
        .is_some_and(|(name, _)| name.len() > 1 && !name.contains(['/', '\\', '.']))
    {
        Ok(Bucket::Rclone)
    } else {
        Err(format!(
            "{dest}: a bucket is s3://bucket/prefix, gs://bucket/prefix, or an rclone remote (name:path)"
        ))
    }
}

fn installed(program: &str) -> bool {
    std::process::Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The command that copies `from` to `to` for this kind of bucket, adding what `to` lacks and
/// deleting nothing. In a content-addressed store a name never changes content, so a file that
/// is there already is skipped. Hidden files (a write in progress) stay behind.
fn sync_command(
    kind: &Bucket,
    from: &str,
    to: &str,
    dry_run: bool,
) -> Result<std::process::Command, String> {
    let hidden = r"(^|.*/)\.[^/]*$";
    let mut cmd;
    match kind {
        Bucket::S3 => {
            if !installed("aws") {
                return Err("s3:// needs the AWS CLI (`aws`): https://aws.amazon.com/cli/".into());
            }
            cmd = std::process::Command::new("aws");
            cmd.args(["s3", "sync", from, to, "--size-only"]).args([
                "--exclude",
                ".*",
                "--exclude",
                "*/.*",
            ]);
            if dry_run {
                cmd.arg("--dryrun");
            }
        }
        Bucket::Gcs if installed("gcloud") => {
            cmd = std::process::Command::new("gcloud");
            cmd.args([
                "storage",
                "rsync",
                from,
                to,
                "--recursive",
                "--exclude",
                hidden,
            ]);
            if dry_run {
                cmd.arg("--dry-run");
            }
        }
        Bucket::Gcs => {
            if !installed("gsutil") {
                return Err(
                    "gs:// needs `gcloud` or `gsutil`: https://cloud.google.com/sdk".into(),
                );
            }
            cmd = std::process::Command::new("gsutil");
            cmd.args(["-m", "rsync", "-r", "-x", hidden]);
            if dry_run {
                cmd.arg("-n");
            }
            cmd.args([from, to]);
        }
        Bucket::Rclone => {
            if !installed("rclone") {
                return Err("an rclone remote needs `rclone`: https://rclone.org".into());
            }
            cmd = std::process::Command::new("rclone");
            cmd.args(["copy", from, to, "--ignore-existing"]).args([
                "--exclude",
                ".*",
                "--exclude",
                "**/.*",
            ]);
            if dry_run {
                cmd.arg("--dry-run");
            }
        }
    }
    Ok(cmd)
}

/// `expectation screenshots push` and `pull`: a store to a bucket, or a bucket to a store.
fn sync_bucket(store: &str, bucket: &str, push: bool, dry_run: bool) {
    let fail = |e: String| -> ! {
        eprintln!("error: {e}");
        exit(2)
    };
    let kind = bucket_tool(bucket).unwrap_or_else(|e| fail(e));
    if push && !Path::new(store).is_dir() {
        fail(format!("{store}: no such directory"));
    }
    if !push && !dry_run {
        std::fs::create_dir_all(store).unwrap_or_else(|e| fail(format!("{store}: {e}")));
    }
    let (from, to) = if push {
        (store, bucket)
    } else {
        (bucket, store)
    };
    let mut cmd = sync_command(&kind, from, to, dry_run).unwrap_or_else(|e| fail(e));
    eprintln!(
        "{} {from} to {to}",
        if dry_run { "would copy" } else { "copying" }
    );
    let tool = cmd.get_program().to_string_lossy().into_owned();
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => fail(format!("{tool} exited with {s}")),
        Err(e) => fail(format!("couldn't run {tool}: {e}")),
    }
}

/// The lines that keep a store's images in Git LFS. SVGs stay in Git: they are text, and diff.
const LFS_ATTRIBUTES: &[&str] = &[
    "*.png filter=lfs diff=lfs merge=lfs -text",
    "*.jpg filter=lfs diff=lfs merge=lfs -text",
    "*.jpeg filter=lfs diff=lfs merge=lfs -text",
    "*.webp filter=lfs diff=lfs merge=lfs -text",
];

/// `expectation screenshots lfs`: write the `.gitattributes` that keeps a store's images in Git
/// LFS. Lines already there are kept, and running it again changes nothing.
fn lfs_screenshots(store: &str) {
    let dir = Path::new(store);
    let path = dir.join(".gitattributes");
    let had = std::fs::read_to_string(&path).unwrap_or_default();
    let missing: Vec<&str> = LFS_ATTRIBUTES
        .iter()
        .copied()
        .filter(|l| !had.lines().any(|h| h.trim() == *l))
        .collect();
    if missing.is_empty() {
        println!(
            "{} already keeps the store's images in Git LFS",
            path.display()
        );
    } else {
        let mut text = had.clone();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        if had.is_empty() {
            text.push_str(
                "# Screenshots in Git LFS (`expectation screenshots lfs`). SVGs stay in Git.\n",
            );
        }
        for l in &missing {
            text.push_str(l);
            text.push('\n');
        }
        if let Err(e) = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, text)) {
            eprintln!("error: couldn't write {}: {e}", path.display());
            exit(2);
        }
        println!("wrote {}", path.display());
    }
    let lfs = std::process::Command::new("git")
        .args(["lfs", "version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !lfs {
        eprintln!(
            "warning: Git LFS isn't installed here. Install it (https://git-lfs.com) and run \
             `git lfs install`, or the images are committed to Git as they are"
        );
    }
}
