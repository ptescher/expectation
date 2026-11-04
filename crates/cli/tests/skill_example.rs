//! The overview skill's example is copied into suites as is, so it has to check clean with the
//! command it documents: no errors and no warnings.

use std::path::Path;
use std::process::Command;

#[test]
fn the_overview_skills_example_checks_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let skill = std::fs::read_to_string(root.join("site/skills/expectation/SKILL.md")).unwrap();
    let example: String = skill
        .split("```gherkin\n")
        .nth(1)
        .and_then(|rest| rest.split("\n```").next())
        .expect("the skill has a gherkin example")
        .into();

    let dir = std::env::temp_dir().join(format!("expectation-skill-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kettle.expect"), &example).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_expectation"))
        .arg("check")
        .arg(&dir)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).ok();

    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "the example doesn't check:\n{text}");
    assert!(!text.contains("warning"), "the example has warnings:\n{text}");
}
