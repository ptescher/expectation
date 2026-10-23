//! Warnings about how the docs will be sized: each scenario is a tutorial page, each feature a
//! topic page and each section a page of cards. Pages that are too short, too long or too many
//! read badly, however good the specs behind them are.

use expectation_model::{Diagnostic, FeatureId, Suite};

/// A story has one scenario: each scenario is a tutorial page of its own.
pub const MAX_SCENARIOS: usize = 1;
/// The fewest numbered steps a tutorial needs.
pub const MIN_STEPS: usize = 3;
/// The most numbered steps a tutorial reads well with.
pub const MAX_STEPS: usize = 8;
/// The most behaviours (shown expectations) a topic page reads well with.
pub const MAX_BEHAVIOURS: usize = 8;
/// A topic with this many shown behaviours, among other topics, is too small to stand alone.
pub const LONE_BEHAVIOUR: usize = 1;
/// The most tutorials a section page holds before it needs subsections.
pub const MAX_TUTORIALS: usize = 8;
/// The most topics a section page holds before it needs subsections.
pub const MAX_TOPICS: usize = 8;
/// A section with this many topics needs a tutorial.
pub const TOPICS_NEEDING_A_TUTORIAL: usize = 2;

pub fn check(suite: &Suite, diags: &mut Vec<Diagnostic>) {
    for story in &suite.stories {
        if story.scenarios.len() > MAX_SCENARIOS {
            diags.push(
                Diagnostic::warning(
                    "many-scenarios",
                    format!("story «{}» has {} scenarios", story.name, story.scenarios.len()),
                )
                .at(&story.span, "")
                .help("each scenario is a tutorial page of its own; write one story per goal, or keep the variants as expectations"),
            );
        }
        for sc in &story.scenarios {
            // Each `When` starts a numbered step; a phrase of several actions stays one.
            let mut steps = sc.steps.iter().map(|s| s.group).collect::<Vec<_>>();
            steps.dedup();
            let n = steps.len();
            if n < MIN_STEPS {
                diags.push(
                    Diagnostic::warning("short-story", format!("scenario «{}» has {} step{}", sc.name, n, plural(n)))
                        .at(&sc.span, "")
                        .help(format!(
                            "a tutorial needs {MIN_STEPS} to {MAX_STEPS} steps; with fewer it is a topic, so let the expectations say it, or add the steps a reader takes before and after"
                        )),
                );
            } else if n > MAX_STEPS {
                diags.push(
                    Diagnostic::warning(
                        "long-story",
                        format!("scenario «{}» has {} steps", sc.name, n),
                    )
                    .at(&sc.span, "")
                    .help("split it, or fold repeated actions into a phrase"),
                );
            }
        }
    }

    for section in &suite.sections {
        let shown = |f: &FeatureId| {
            suite
                .feature(*f)
                .expectations
                .iter()
                .filter(|&&e| !suite.expectations[e.0 as usize].hidden)
                .count()
        };
        for f in &section.features {
            let feature = suite.feature(*f);
            let n = shown(f);
            if n > MAX_BEHAVIOURS {
                diags.push(
                    Diagnostic::warning("big-feature", format!("feature «{}» has {} expectations in the docs", feature.name, n))
                        .at(&feature.span, "")
                        .help(format!(
                            "a topic page reads best with 2 to {MAX_BEHAVIOURS} behaviours; split it into topics a reader would scan for"
                        )),
                );
            } else if n == LONE_BEHAVIOUR && section.features.len() > 1 {
                diags.push(
                    Diagnostic::warning(
                        "small-feature",
                        format!(
                            "feature «{}» has only one expectation in the docs",
                            feature.name
                        ),
                    )
                    .at(&feature.span, "")
                    .help("fold it into the closest topic, or add the behaviours around it"),
                );
            }
        }

        // A section without a `Section:` line is pointed at by what is in it.
        let feature = section.features.first().map(|f| &suite.feature(*f).span);
        let story = section.stories.first().map(|s| &suite.story(*s).span);
        let Some(span) = section.span.as_ref().or(feature).or(story) else {
            continue;
        };
        let tutorials: usize = section
            .stories
            .iter()
            .map(|s| suite.story(*s).scenarios.len())
            .sum();
        let topics = section.features.len();
        if tutorials > MAX_TUTORIALS || topics > MAX_TOPICS {
            let mut too_many = vec![];
            if tutorials > MAX_TUTORIALS {
                too_many.push(format!("{tutorials} tutorials"));
            }
            if topics > MAX_TOPICS {
                too_many.push(format!("{topics} topics"));
            }
            diags.push(
                Diagnostic::warning(
                    "crowded-section",
                    format!("section «{}» has {}", section.name, too_many.join(" and ")),
                )
                .at(span, "")
                .help("split it into subsections"),
            );
        }
        if topics >= TOPICS_NEEDING_A_TUTORIAL && section.stories.is_empty() {
            diags.push(
                Diagnostic::warning(
                    "no-tutorial",
                    format!(
                        "section «{}» has {} topics and no tutorial",
                        section.name, topics
                    ),
                )
                .at(span, "")
                .help("add a story for the reader's main goal in the section"),
            );
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
