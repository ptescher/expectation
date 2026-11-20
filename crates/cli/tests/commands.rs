//! The parts of the command a newcomer meets first: asking for help, the version, and a
//! starter suite.

use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_expectation"))
        .args(args)
        .output()
        .unwrap()
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("expectation-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    dir
}

#[test]
fn help_that_was_asked_for_is_not_an_error() {
    for flag in ["--help", "-h", "help"] {
        let out = run(&[flag]);
        assert!(out.status.success(), "{flag} failed");
        assert!(String::from_utf8_lossy(&out.stdout).contains("usage: expectation"));
        assert!(out.stderr.is_empty());
    }
    // Not asked for: the usage is the error.
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage: expectation"));
}

#[test]
fn version_names_the_command_and_its_version() {
    for flag in ["--version", "-V"] {
        let out = run(&[flag]);
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            format!("expectation {}", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn init_writes_a_suite_that_checks_clean_and_never_writes_over_one() {
    let dir = temp("init");
    let suite = dir.join("expectations");
    let path = suite.to_str().unwrap();

    let out = run(&["init", path]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(suite.join("turnstile.expect").exists());

    let check = run(&["check", path]);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(check.status.success(), "the starter doesn't check:\n{text}");
    assert!(
        !text.contains("warning"),
        "the starter has warnings:\n{text}"
    );

    // A second init leaves what is there alone, also when the suite is in a subdirectory.
    std::fs::write(suite.join("turnstile.expect"), "mine").unwrap();
    let again = run(&["init", path]);
    assert_eq!(again.status.code(), Some(2));
    assert_eq!(
        std::fs::read_to_string(suite.join("turnstile.expect")).unwrap(),
        "mine"
    );
    let above = run(&["init", dir.to_str().unwrap()]);
    assert_eq!(above.status.code(), Some(2));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_trailing_comment_is_named_as_the_cause() {
    let dir = temp("trailing");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a.expect"),
        "Vocabulary:\n  State: the lamp is on | off\n  Event: the user flips the switch\nBackground:\n  Given the lamp is off\nFeature: Light\n  Approval: approved  # agreed on Monday\n  Expectation: Flipping turns it on\n    Given the lamp is off\n    When the user flips the switch\n    Then the lamp is on\n",
    )
    .unwrap();
    let out = run(&["check", dir.to_str().unwrap()]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(
        text.contains("only starts a comment at the start of a line"),
        "{text}"
    );
}
