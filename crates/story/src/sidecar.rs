//! What every extension's file next to the results has in common.
//!
//! Extensions (HAR and OpenAPI, code coverage, and whatever comes next) read the run's results
//! and the evidence a driver attached, and write a tab-separated file in the results directory:
//! `<name>[@platform].<kind>.tsv`. Expectation reads those files here, the same way whatever
//! wrote them:
//!
//! - they are found by their kind, and a file for another platform is left out;
//! - a file over [`MAX_BYTES`] isn't read, and one without its header line is an error;
//! - fields are cut and stripped of control characters ([`clean`]), so a file can't carry a
//!   page's worth of text or break the page it is shown in;
//! - evidence counts only for the run and the test it came from ([`credit`]).
//!
//! Each kind owns its format and how it is presented; this is only the part they share.

use crate::coverage::Credit;
use crate::plan::Plan;
use crate::report::{Report, fingerprint};

/// The largest extension file read.
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;
/// The longest an id is kept.
pub const MAX_ID: usize = 128;
/// The longest a label, location, reason or note is kept.
pub const MAX_TEXT: usize = 300;

/// The kinds of file there are: `contract coverage` is `<name>.coverage.tsv`, and so on.
pub const KINDS: &[&str] = &["coverage", "code"];

/// A field as it is kept: control characters dropped, cut to `max` characters, and `-` (nothing)
/// as an empty string.
pub fn clean(s: &str, max: usize) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).take(max).collect();
    if s == "-" { String::new() } else { s }
}

/// Is this file in a results directory an extension's, of a kind (or of any kind, if `None`)?
pub fn is_sidecar(path: &std::path::Path, kind: Option<&str>) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    KINDS
        .iter()
        .filter(|k| kind.is_none_or(|want| want == **k))
        .any(|k| name.ends_with(&format!(".{k}.tsv")))
}

/// Every file of `kind` in `dir` for `platform` (and those for every platform), read and parsed by
/// `parse` (given the file's name and text), with an error for each that couldn't be. In name order.
pub fn load<T>(
    dir: &std::path::Path,
    kind: &str,
    platform: Option<&str>,
    parse: impl Fn(&str, &str) -> Result<T, String>,
) -> (Vec<T>, Vec<String>) {
    let suffix = format!(".{kind}.tsv");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_sidecar(p, Some(kind)))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            match name.trim_end_matches(&suffix).split_once('@') {
                Some((_, on)) => Some(on) == platform,
                None => true,
            }
        })
        .collect();
    files.sort();
    let (mut found, mut errors) = (vec![], vec![]);
    for f in files {
        let name = f
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        match std::fs::metadata(&f) {
            Ok(m) if m.len() > MAX_BYTES => {
                errors.push(format!("{name}: over {} MiB, so not read", MAX_BYTES >> 20));
                continue;
            }
            Err(e) => {
                errors.push(format!("{name}: {e}"));
                continue;
            }
            _ => {}
        }
        match std::fs::read_to_string(&f) {
            Ok(text) => match parse(&name, &text) {
                Ok(x) => found.push(x),
                Err(e) => errors.push(e),
            },
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    (found, errors)
}

/// Whether evidence from `run`, about step `step` of the test `slug` as it was fingerprinted,
/// counts for the results shown: it must be the run of the result shown for that test, the test
/// must be as it was, and it must have run.
pub fn credit(
    plans: &[Plan],
    report: &Report,
    slug: &str,
    print: &str,
    run: &str,
    step: usize,
) -> Credit {
    let Some(i) = plans.iter().position(|p| p.slug == slug) else {
        return Credit::NoSuchTest;
    };
    if fingerprint(&plans[i]) != print {
        return Credit::TestChanged;
    }
    if step > plans[i].steps.len() {
        return Credit::NoSuchStep;
    }
    match report.results.get(i) {
        Some(r) if !r.status.ran() => Credit::DidNotRun,
        Some(r) if r.run.as_ref().is_some_and(|x| x.id == run) => Credit::Counts { plan: i, step },
        _ => Credit::OtherRun,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn files_are_found_by_kind_and_platform() {
        let dir =
            std::env::temp_dir().join(format!("expectation-sidecar-kinds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in [
            "a.coverage.tsv",
            "a@iOS.coverage.tsv",
            "a@web.coverage.tsv",
            "b.code.tsv",
            "app.tsv",
        ] {
            std::fs::write(dir.join(n), "x").unwrap();
        }
        let names =
            |kind, platform| load(&dir, kind, platform, |n, _| Ok::<_, String>(n.to_string())).0;
        assert_eq!(
            names("coverage", Some("iOS")),
            ["a.coverage.tsv", "a@iOS.coverage.tsv"]
        );
        assert_eq!(names("coverage", None), ["a.coverage.tsv"]);
        assert_eq!(names("code", None), ["b.code.tsv"]);
        assert!(is_sidecar(Path::new("x/a@b.code.tsv"), None));
        assert!(!is_sidecar(Path::new("x/app@iOS.tsv"), None));
        assert!(!is_sidecar(Path::new("x/a.code.tsv"), Some("coverage")));
    }

    #[test]
    fn a_big_file_is_refused_and_a_parse_error_is_listed() {
        let dir =
            std::env::temp_dir().join(format!("expectation-sidecar-big-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.code.tsv"), "bad").unwrap();
        let (found, errors) = load(&dir, "code", None, |n, _| Err::<(), _>(format!("{n}: no")));
        assert!(found.is_empty());
        assert_eq!(errors, ["a.code.tsv: no"]);
        let big = std::fs::File::create(dir.join("b.code.tsv")).unwrap();
        big.set_len(MAX_BYTES + 1).unwrap();
        let (_, errors) = load(&dir, "code", None, |_, _| Ok::<_, String>(()));
        assert!(
            errors.iter().any(|e| e.contains("so not read")),
            "{errors:?}"
        );
    }

    #[test]
    fn clean_cuts_strips_and_reads_a_dash_as_nothing() {
        assert_eq!(clean("a\tb\nc", 10), "abc");
        assert_eq!(clean("abcdef", 3), "abc");
        assert_eq!(clean("-", 3), "");
    }
}
