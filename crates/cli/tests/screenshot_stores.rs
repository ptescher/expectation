//! Where a store of screenshots can live besides the repository: a bucket, through the tool its
//! provider ships, or Git LFS. Stand-in tools record what they were asked; Git LFS is the real one
//! when it is installed.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A directory of stand-in tools: each answers `--version` (or doesn't, if it isn't "installed")
/// and otherwise writes its name and arguments to `log`.
fn tools(dir: &Path, installed: &[&str], missing: &[&str]) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = dir.join("log");
    for (names, version) in [(installed, 0), (missing, 1)] {
        for name in names {
            let path = bin.join(name);
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\nif [ \"$1\" = --version ]; then exit {version}; fi\necho \"{name} $*\" >> '{}'\n",
                    log.display()
                ),
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
    }
    bin
}

fn run(bin: &Path, args: &[&str]) -> Output {
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(env!("CARGO_BIN_EXE_expectation"))
        .args(args)
        .env("PATH", path)
        .output()
        .unwrap()
}

fn logged(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("log")).unwrap_or_default()
}

#[test]
#[cfg(unix)]
fn buckets_are_reached_through_their_own_tools_adding_and_never_deleting() {
    let dir = fresh("stores-buckets");
    let store = dir.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let s = store.to_str().unwrap();
    let bin = tools(&dir, &["aws", "gcloud", "rclone", "gsutil"], &[]);

    assert!(
        run(&bin, &["screenshots", "push", s, "s3://shots/app"])
            .status
            .success()
    );
    assert!(
        run(
            &bin,
            &["screenshots", "pull", "s3://shots/app", s, "--dry-run"]
        )
        .status
        .success()
    );
    assert!(
        run(&bin, &["screenshots", "push", s, "gs://shots/app"])
            .status
            .success()
    );
    assert!(
        run(&bin, &["screenshots", "push", s, "r2:shots/app"])
            .status
            .success()
    );
    let log = logged(&dir);
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(
        lines[0],
        format!("aws s3 sync {s} s3://shots/app --size-only --exclude .* --exclude */.*")
    );
    assert_eq!(
        lines[1],
        format!("aws s3 sync s3://shots/app {s} --size-only --exclude .* --exclude */.* --dryrun")
    );
    assert!(
        lines[2].starts_with(&format!(
            "gcloud storage rsync {s} gs://shots/app --recursive --exclude"
        )),
        "{}",
        lines[2]
    );
    assert_eq!(
        lines[3],
        format!("rclone copy {s} r2:shots/app --ignore-existing --exclude .* --exclude **/.*")
    );
    // Nothing anywhere deletes.
    assert!(!log.contains("delete"), "{log}");

    // Without gcloud, gsutil.
    let dir = fresh("stores-gsutil");
    let bin = tools(&dir, &["gsutil"], &["gcloud"]);
    assert!(
        run(&bin, &["screenshots", "push", s, "gs://shots/app"])
            .status
            .success()
    );
    assert!(
        logged(&dir).starts_with("gsutil -m rsync -r -x"),
        "{}",
        logged(&dir)
    );

    // A tool that isn't there, or a place that isn't a bucket, is an error that says what to do.
    let dir = fresh("stores-missing");
    let bin = tools(&dir, &[], &["aws"]);
    let out = run(&bin, &["screenshots", "push", s, "s3://shots/app"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("AWS CLI"));
    let out = run(&bin, &["screenshots", "push", s, "/somewhere/else"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("a bucket is s3://"));
    assert!(logged(&dir).is_empty());
}

#[test]
fn docs_given_a_bucket_say_to_pull_it_or_give_its_address() {
    let dir = fresh("stores-docs");
    let suite = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/smart-kettle");
    let out = Command::new(env!("CARGO_BIN_EXE_expectation"))
        .args([
            "docs",
            suite.to_str().unwrap(),
            dir.join("out").to_str().unwrap(),
        ])
        .args(["--screenshots", "s3://shots/app"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("expectation screenshots pull s3://shots/app"),
        "{said}"
    );
}

#[test]
fn a_store_can_keep_its_images_in_git_lfs() {
    let dir = fresh("stores-lfs");
    let store = dir.join("docs/screenshots");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_expectation"))
            .args(args)
            .output()
            .unwrap()
    };
    let s = store.to_str().unwrap();
    assert!(run(&["screenshots", "lfs", s]).status.success());
    let attributes = std::fs::read_to_string(store.join(".gitattributes")).unwrap();
    assert!(attributes.contains("*.png filter=lfs diff=lfs merge=lfs -text"));
    assert!(!attributes.contains("svg"), "SVGs stay in Git");
    // Again: nothing changes, and lines of one's own are kept.
    std::fs::write(
        store.join(".gitattributes"),
        format!("*.gif binary\n{attributes}"),
    )
    .unwrap();
    assert!(run(&["screenshots", "lfs", s]).status.success());
    assert_eq!(
        std::fs::read_to_string(store.join(".gitattributes")).unwrap(),
        format!("*.gif binary\n{attributes}")
    );

    // With Git LFS installed, a screenshot added to the store goes to LFS, and an SVG doesn't.
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if !git(&["lfs", "version"]) {
        eprintln!("Git LFS isn't installed: the attributes are all that can be tested");
        return;
    }
    assert!(git(&["init", "-q"]));
    assert!(git(&["lfs", "install", "--local"]));
    let png = format!("{}.png", "a".repeat(64));
    let svg = format!("{}.svg", "b".repeat(64));
    std::fs::write(store.join(&png), b"\x89PNG\r\n\x1a\nnot really").unwrap();
    std::fs::write(store.join(&svg), b"<svg/>").unwrap();
    assert!(git(&["add", "docs/screenshots"]));
    // What Git stores for each: an LFS pointer for the PNG, the SVG itself.
    let staged = |name: &str| {
        let out = Command::new("git")
            .args(["show", &format!(":docs/screenshots/{name}")])
            .current_dir(&dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert!(
        staged(&png).starts_with("version https://git-lfs"),
        "{}",
        staged(&png)
    );
    assert_eq!(staged(&svg), "<svg/>");
}
