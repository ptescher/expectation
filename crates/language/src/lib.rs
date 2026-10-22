//! The `.expect` file format: a Gherkin-flavoured language for a suite's vocabulary,
//! expectations and user stories.
//!
//! ```text
//! Suite: Kettle
//!
//! Vocabulary:
//!   State: the kettle is off | heating | dry
//!   State: the dial is 0..100 %
//!   Event: the user presses reset
//!
//! Background:
//!   Given the kettle is off
//!
//! Feature: Heating
//!   Expectation: Turning the dial up starts the kettle
//!     Given the kettle is off
//!     When the dial is above 55 %
//!     Then the kettle is heating
//!
//! Story: First cup
//!   As a tea drinker
//!   I want to start the kettle by turning the dial
//!
//!   Scenario: Turn it on
//!     When the dial is 80 %
//!     Then the kettle is heating
//! ```
//!
//! [`load_dir`] reads every `*.expect` file under a directory into one suite;
//! [`load_sources`] does the same from strings.

mod lens;
mod parse;
mod resolve;
pub mod step;

use expectation_model::{Diagnostic, Suite};
use std::path::{Path, PathBuf};

pub const EXTENSION: &str = "expect";

/// A suite and everything wrong with it. Check [`Loaded::has_errors`] before using the suite:
/// steps that didn't resolve are left out of it.
pub struct Loaded {
    pub suite: Suite,
    pub diagnostics: Vec<Diagnostic>,
}

impl Loaded {
    pub fn has_errors(&self) -> bool {
        expectation_model::diagnostic::has_errors(&self.diagnostics)
    }

    pub fn render_diagnostics(&self) -> String {
        expectation_model::diagnostic::render_all(&self.diagnostics, &self.suite.sources)
    }
}

/// Parse named sources, in order, as one suite (for its default platform).
pub fn load_sources(name: &str, sources: &[(&str, &str)]) -> Loaded {
    load_sources_for(name, sources, None)
}

/// As [`load_sources`], read for `platform` (see `lens`): its variants replace the plain lines
/// and what is `On:` elsewhere is left out. `None` is the default platform.
pub fn load_sources_for(name: &str, sources: &[(&str, &str)], platform: Option<&str>) -> Loaded {
    let mut diags = vec![];
    let mut suite = Suite { name: name.to_string(), ..Suite::default() };
    let mut files = vec![];
    for (file, text) in sources {
        let mut raw = parse::parse(file, text, &mut diags);
        raw.dir = file.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        files.push(raw);
        suite.sources.insert((*file).into(), text.to_string());
    }
    (suite.platforms, suite.platform) = lens::apply(&mut files, platform, &mut diags);
    resolve::resolve(&mut suite, &files, &mut diags);
    Loaded { suite, diagnostics: diags }
}

/// Every `*.expect` file under `dir` (recursively, in path order) as one suite, named by its
/// `Suite:` line or else after the directory. File names in diagnostics are relative to `dir`.
pub fn load_dir(dir: &Path) -> std::io::Result<Loaded> {
    load_dir_for(dir, None)
}

/// As [`load_dir`], read for `platform`.
pub fn load_dir_for(dir: &Path, platform: Option<&str>) -> std::io::Result<Loaded> {
    let mut paths = vec![];
    collect(dir, &mut paths)?;
    paths.sort();
    let mut texts = vec![];
    for p in &paths {
        let rel = p.strip_prefix(dir).unwrap_or(p).to_string_lossy().replace('\\', "/");
        texts.push((rel, std::fs::read_to_string(p)?));
    }
    let refs: Vec<(&str, &str)> = texts.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut loaded = load_sources_for(&suite_name(dir), &refs, platform);
    if paths.is_empty() {
        loaded
            .diagnostics
            .push(Diagnostic::error("empty-suite", format!("no .{EXTENSION} files in {}", dir.display())));
    }
    Ok(loaded)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let p = entry?.path();
        if p.is_dir() {
            collect(&p, out)?;
        } else if p.extension().is_some_and(|e| e == EXTENSION) {
            out.push(p);
        }
    }
    Ok(())
}

/// "child_lock" -> "Child lock".
fn suite_name(dir: &Path) -> String {
    let base = dir.canonicalize().ok().and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()));
    let base = base.unwrap_or_else(|| "Expectations".into()).replace(['_', '-'], " ");
    let mut c = base.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use expectation_model::{Constraint, ExpId, Keyword};

    const SUITE: &str = "
Suite: Kettle basics

Vocabulary:
  State: the kettle is off | heating | dry
  State: the dial is 0..100 %
  State: the child lock is on | off
  Event: the user presses reset

Background:
  Given the kettle is off
  And the dial is 0 %

Feature: Heating
  The kettle starts off.

  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

  Expectation: The child lock holds the kettle
    Overrides: Turning the dial up starts the kettle
    Given the kettle is off
    And the child lock is on
    When the dial is above 55 %
    Then the kettle is off

  Expectation: Never dry while off
    Given the kettle is off
    Then the kettle is not dry

Story: First cup
  As a tea drinker
  I want to boil the kettle

  Given the dial is 0 %
  When the dial is 80 %
  Then the kettle is heating
  When the dial is 0 %
  And the user presses reset
  Then the kettle is off
";

    #[test]
    fn launch_is_inherited_and_overridden() {
        let l = load_sources(
            "t",
            &[
                ("t.expect", "Section: Root\nLaunch: run the app\n"),
                ("a/a.expect", "Section: A\n"),
                ("b/b.expect", "Section: B\nLaunch: run the other app\n"),
            ],
        );
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let s = &l.suite;
        let launch = |name: &str| s.launch_of(s.sections.iter().find(|x| x.name == name).unwrap().id);
        assert_eq!(launch("Root"), Some("run the app"));
        assert_eq!(launch("A"), Some("run the app"));
        assert_eq!(launch("B"), Some("run the other app"));
    }

    #[test]
    fn warns_about_a_command_in_a_say() {
        let l = load_sources(
            "t",
            &[("t.expect", "Vocabulary:\n  Event: the user starts it\n    Say: run `cargo run -p towers` in a terminal\n")],
        );
        assert!(l.render_diagnostics().contains("launch-in-step"), "{}", l.render_diagnostics());
        let l = load_sources("t", &[("t.expect", "Vocabulary:\n  Event: the user unlocks it\n    Say: press `UNLOCK`\n")]);
        assert!(!l.render_diagnostics().contains("launch-in-step"), "{}", l.render_diagnostics());
    }

    #[test]
    fn parses_a_suite() {
        let l = load_sources("t", &[("t.expect", SUITE)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let s = &l.suite;
        assert_eq!(s.name, "Kettle basics");
        assert_eq!(s.vocabulary.variables.len(), 3);
        assert_eq!(s.background.vars().count(), 2);
        assert_eq!(s.features[0].description, vec!["The kettle starts off."]);
        let [start, lock, never] = &s.expectations[..] else { panic!() };
        assert!(start.trigger.event.is_none() && start.trigger.changes.vars().count() == 1);
        assert_eq!(lock.overrides, vec![start.id]);
        assert!(never.is_invariant());
        assert_eq!(never.then.get(expectation_model::VarId(0)), Some(&Constraint::OneOf(0b011)));

        let story = &s.stories[0];
        assert_eq!(story.narrative.len(), 2);
        let sc = &story.scenarios[0];
        assert_eq!(sc.name, "First cup");
        assert_eq!(sc.steps.len(), 2);
        assert!(sc.steps[1].action.event.is_some() && sc.steps[1].action.changes.vars().count() == 1);
        assert_eq!(sc.steps[1].steps[1].keyword, Keyword::And);
    }

    #[test]
    fn sections_nest_by_directory_and_layer_their_setup() {
        let vocab = "Suite: Sketch\nVocabulary:\n State: the tool is none | PEN\n State: the drawing has 0..100 shapes\n Event: the user presses PEN\n Event: the user clicks the canvas\nBackground:\n Given the tool is none\n And the drawing has 0 shapes\n";
        let tools = "Feature: Tools\n Expectation: PEN picks the pen\n  Given the tool is none\n  When the user presses PEN\n  Then the tool is PEN\n";
        let drawing = "Section: Drawing\n Everything here starts with the pen.\nBackground:\n Given the tool is PEN\nFeature: Shapes\n Expectation: Clicking adds a shape\n  Given the tool is PEN\n  When the user clicks the canvas\n  Then the drawing goes up by 1\n";
        let l = load_sources(
            "x",
            &[("root.expect", vocab), ("tools.expect", tools), ("03-drawing/shapes.expect", drawing)],
        );
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let s = &l.suite;
        assert_eq!(s.sections.len(), 2);
        let (root, drawing) = (&s.sections[0], &s.sections[1]);
        assert_eq!((root.name.as_str(), drawing.name.as_str()), ("Sketch", "Drawing"));
        assert_eq!(drawing.slug, "drawing");
        assert_eq!(drawing.parent, Some(root.id));
        assert_eq!(drawing.description, vec!["Everything here starts with the pen."]);
        // The drawing section starts from the root's start with its own setup on top.
        assert_eq!(s.describe(&drawing.start), "the tool is PEN and the drawing has 0 shapes");
        assert_eq!(s.feature(s.exp(ExpId(1)).feature).section, drawing.id);
        assert_eq!(s.exp(ExpId(1)).shifts, vec![(expectation_model::VarId(1), expectation_model::Shift::By(1.0))]);
    }

    #[test]
    fn phrases_say_more_with_less() {
        let src = "Vocabulary:
 State: the tool is pan | PEN
   Set: pick the {value} tool
 State: the drawing has 0..100 shapes
 Event: the user presses PEN
   Say: press PEN
 Event: the user clicks the canvas
 Phrase: the user picks the pen
   Say: press PEN
   When the user presses PEN
 Phrase: the user draws two shapes
   Say: click the canvas twice
   When the user clicks the canvas
   When the user clicks the canvas
 Phrase: drawing has started
   Given the tool is PEN
   And the drawing has 0 shapes
Feature: F
 Expectation: A click adds a shape
  Given drawing has started
  When the user clicks the canvas
  Then the drawing has 1 shape
Story: S
 As an artist
 I want to draw
 Drawing starts on a blank canvas.
 When the user picks the pen
 When the user draws two shapes
 Then the drawing has 2 shapes
";
        let l = load_sources("t", &[("t.expect", src)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let s = &l.suite;
        assert_eq!(s.vocabulary.events[0].say.as_deref(), Some("press PEN"));
        assert_eq!(s.vocabulary.variables[0].set.as_deref(), Some("pick the {value} tool"));
        assert_eq!(s.vocabulary.phrases.len(), 3);
        // A setup phrase in a Given is its facts.
        assert_eq!(s.exp(ExpId(0)).given.vars().count(), 2);
        let story = &s.stories[0];
        assert_eq!(story.narrative.len(), 2);
        assert_eq!(story.description, vec!["Drawing starts on a blank canvas."]);
        // Two written steps make three actions: the two-click phrase is one group of two.
        let steps = &story.scenarios[0].steps;
        assert_eq!(steps.iter().map(|st| st.group).collect::<Vec<_>>(), vec![1, 2, 2]);
        assert_eq!(steps[1].written.len(), 2, "the When and the Then, as written");
        assert!(steps[2].written.is_empty());
        assert_eq!(steps[2].checks.len(), 1, "the Then checks after the last click");
        assert!(matches!(steps[1].written[0].meaning, Some(expectation_model::Said::Phrase(_))));
    }

    #[test]
    fn docs_lines_say_what_the_reader_sees() {
        let src = "Vocabulary:
 State: the kettle is off | dry
   Shows: dry: the display says DRY
 State: the drawing has 0..650 shapes
   Shows: the counter reads {value} SHAPES
 State: the ink is 0..100 %
   Shows: at least 99 %: the ink is full
 Event: the user starts the kettle
Term: display
  The panel on the front of the kettle.
Feature: F
  When the kettle is off, Space starts it.
  Expectation: E
    Docs: hidden
    Given the kettle is off
    When the user starts the kettle
    Then the kettle is dry
";
        let l = load_sources("t", &[("t.expect", src)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let s = &l.suite;
        let v = &s.vocabulary;
        let shown = |step: &str| match step::resolve(step, &expectation_model::Span::new("t", 1), v) {
            Ok(step::Meaning::Fact(f)) => v.var(f.var).shown(&f.constraint),
            _ => panic!("{step} isn't a state"),
        };
        assert_eq!(shown("the kettle is dry").as_deref(), Some("the display says DRY"));
        assert_eq!(shown("the kettle is off"), None);
        assert_eq!(shown("the drawing has 3 shapes").as_deref(), Some("the counter reads 3 SHAPES"));
        assert_eq!(shown("the ink is at least 99 %").as_deref(), Some("the ink is full"));
        assert_eq!(s.glossary[0].name, "display");
        // A "When" line in a feature's description is prose: features hold no steps.
        assert_eq!(s.features[0].description, vec!["When the kettle is off, Space starts it."]);
        assert!(s.expectations[0].hidden);
    }

    const PLATFORMS: &str = "Suite: Sketch
Vocabulary:
  Platform: desktop | iOS
  State: the door is locked | unlocked
    Shows: unlocked: the UNLOCKED light comes on
    Shows[iOS]: unlocked: the UNLOCKED badge turns green
  State: the tool is pan | PEN
  Event: the user unlocks the door
    Say: press UNLOCK
    Say[iOS]: tap UNLOCK
  Phrase: the app is open
    Say: open it on the Mac
    Given the door is locked
    And the tool is pan
  Phrase[iOS]: the app is open
    Say: open the app on the phone
    Given the door is locked
Background:
  Given the app is open

Feature: Unlocking
  Expectation: UNLOCK unlocks
    Given the door is locked
    When the user unlocks the door
    Then the door is unlocked

Feature: Drawing
  On: desktop
  Expectation: PEN picks the pen
    Given the tool is pan
    When the user unlocks the door
    Then the tool is PEN
";

    #[test]
    fn a_platform_reads_its_own_variants() {
        let sources = [("s.expect", PLATFORMS)];
        let desktop = load_sources_for("s", &sources, None);
        assert!(!desktop.has_errors(), "{}", desktop.render_diagnostics());
        let s = &desktop.suite;
        assert_eq!((s.platforms.clone(), s.platform.as_deref()), (vec!["desktop".into(), "iOS".into()], Some("desktop")));
        assert_eq!(s.vocabulary.events[0].say.as_deref(), Some("press UNLOCK"));
        assert_eq!(s.expectations.len(), 2);
        assert_eq!(s.background.vars().count(), 2, "the plain phrase sets the tool too");

        let ios = load_sources_for("s", &sources, Some("iOS"));
        assert!(!ios.has_errors(), "{}", ios.render_diagnostics());
        let s = &ios.suite;
        assert_eq!(s.platform.as_deref(), Some("iOS"));
        assert_eq!(s.vocabulary.events[0].say.as_deref(), Some("tap UNLOCK"));
        assert_eq!(s.vocabulary.phrases.len(), 1, "one `the app is open`, the iOS one");
        assert_eq!(s.vocabulary.phrases[0].say.as_deref(), Some("open the app on the phone"));
        assert_eq!(s.background.vars().count(), 1, "iOS has no tool in its start");
        assert_eq!(s.expectations.len(), 1, "Drawing is on desktop only");
        let shown = s.vocabulary.variables[0].shows.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>();
        assert_eq!(shown, vec!["the UNLOCKED badge turns green"]);
    }

    #[test]
    fn platform_mistakes_are_reported() {
        let vocab = "Vocabulary:\n Platform: desktop | iOS\n State: the door is locked | unlocked\n Event: the user unlocks the door\n";
        let with = |rest: &str| format!("{vocab}{rest}");
        assert_eq!(errors(&with("Event: x\n Say[watch]: tap\n")), vec!["unknown-platform"]);
        assert_eq!(errors(&with("Event: y\n Say[desktop]: tap\n")), vec!["default-platform"]);
        assert_eq!(errors(&with("Feature: F\n On: Android\n")), vec!["unknown-platform"]);
        assert_eq!(errors("Feature: F\n On: iOS\n"), vec!["unknown-platform"]);
        let l = load_sources_for("t", &[("t.expect", vocab)], Some("Android"));
        assert_eq!(l.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["unknown-platform"]);
    }

    fn errors(src: &str) -> Vec<&'static str> {
        let l = load_sources("t", &[("t.expect", src)]);
        l.diagnostics.iter().filter(|d| d.is_error()).map(|d| d.code).collect()
    }

    #[test]
    fn reports_mistakes() {
        let vocab = "Vocabulary:\n State: the kettle is off | heating\n Event: the user presses reset\n";
        let with = |rest: &str| format!("{vocab}{rest}");
        assert_eq!(errors(&with("Feature: F\n Expectation: E\n  Given the kettle is heating\n")), vec!["no-then"]);
        assert_eq!(errors(&with("Feature: F\n Expectation: E\n  Then the kettle is boiling\n")), vec!["bad-value"]);
        assert_eq!(errors(&with("Feature: F\n Expectation: E\n  Then the teapot is heating\n")), vec!["unknown-step"]);
        assert_eq!(
            errors(&with(
                "Feature: F\n Expectation: E\n  Given the kettle is heating\n  And the kettle is off\n  Then the kettle is heating\n"
            )),
            vec!["contradiction"]
        );
        assert_eq!(
            errors(&with("Feature: F\n Expectation: E\n  Then the user presses reset\n")),
            vec!["misplaced-event"]
        );
        assert_eq!(
            errors(&with("Feature: F\n Expectation: E\n  Overrides: Nope\n  Then the kettle is heating\n")),
            vec!["bad-override"]
        );
        assert_eq!(errors(&with("Expectation: E\n")), vec!["syntax"]);
        assert_eq!(errors(&with("Story: S\n Then the kettle is heating\n")), vec!["syntax", "empty-story"]);
        let vacuous = load_sources(
            "t",
            &[(
                "t.expect",
                &with(
                    "Feature: F\n Expectation: E\n  When the kettle is off or heating\n  Then the kettle is heating\n",
                ),
            )],
        );
        assert_eq!(vacuous.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["says-nothing"]);
        let typo =
            load_sources("t", &[("t.expect", &with("Feature: F\n Expectation: E\n  Gvien the kettle is heating\n"))]);
        let codes: Vec<_> = typo.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec!["maybe-step", "no-then"]);
    }
}
