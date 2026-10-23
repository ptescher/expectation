//! User stories as acceptance tests: [`plan`] compiles stories (and generated coverage
//! routes) into test plans, and [`report`] holds what happened when they ran.

pub mod plan;
pub mod report;

pub use plan::{
    Because, Check, Origin, Plan, PlanStep, Planned, coverage, coverage_plans, plan_stories,
    reaching_plans, untested, visit_plans,
};
pub use report::{
    CheckResult, PlanResult, Report, Status, StepResult, Verdict, fingerprint, load_results,
    load_results_for, results_file,
};

#[cfg(test)]
mod tests {
    use super::*;
    use expectation_model::Suite;

    const SUITE: &str = "
Vocabulary:
  State: the kettle is off | heating | dry
  State: the dial is 0..100 %
  State: the help screen is shown | hidden
  Event: the user presses reset
  Event: the user presses help
  Event: the water runs out

Background:
  Given the kettle is off
  And the dial is 0 %

Feature: Heating
  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

  Expectation: Running dry stops the kettle
    Given the kettle is heating
    When the water runs out
    Then the kettle is dry

  Expectation: Reset turns the kettle off
    When the user presses reset
    Then the kettle is off

  Expectation: Help opens the help screen
    Given the help screen is hidden
    When the user presses help
    Then the help screen is shown
";

    fn planned(stories: &str) -> (Suite, expectation_graph::Analysis, Planned) {
        let src = format!("{SUITE}{stories}");
        let l = expectation_language::load_sources("t", &[("t.expect", &src)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let a = expectation_graph::analyze(&l.suite);
        let p = plan_stories(&l.suite, &a);
        (l.suite, a, p)
    }

    fn codes(p: &Planned) -> Vec<&'static str> {
        p.diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn checks_come_from_the_expectations() {
        let (s, _, p) = planned(
            "
Story: Boil dry
  When the dial is 80 %
  When the water runs out
  Then the kettle is dry
",
        );
        assert!(codes(&p).is_empty(), "{:?}", codes(&p));
        let plan = &p.plans[0];
        assert_eq!(plan.start_checks.len(), 2);
        // The story never says the kettle heats; the expectation does, so the test checks it.
        let first = &plan.steps[0];
        assert_eq!(first.fired.len(), 1);
        assert_eq!(
            s.vocabulary.describe(&first.state),
            "the kettle is heating and the dial is 80 %"
        );
        assert!(matches!(first.checks[0].because, Because::Expectation(_)));
        assert_eq!(
            plan.steps[1].checks.len(),
            1,
            "the story's Then duplicates the promise"
        );
    }

    #[test]
    fn stories_cant_rely_on_unspecified_or_contradicted_behaviour() {
        let (_, _, p) = planned("Story: S\n  When the water runs out\n");
        assert_eq!(codes(&p), vec!["unspecified"]);

        let (_, _, p) = planned("Story: S\n  When the dial is 80 %\n  Then the kettle is off\n");
        assert_eq!(codes(&p), vec!["contradicts"]);

        // 50..100 % might or might not be above 55 %.
        let (_, _, p) = planned("Story: S\n  When the dial is at least 50 %\n");
        assert_eq!(codes(&p), vec!["unclear-step"]);

        // The Background doesn't say whether help is showing.
        let (_, _, p) = planned("Story: S\n  When the user presses help\n");
        assert_eq!(codes(&p), vec!["unclear-step"]);
        let (_, _, p) = planned(
            "Story: S\n  Given the help screen is hidden\n  When the user presses help\n  Then the dial is 0 %\n",
        );
        assert!(codes(&p).is_empty());

        let (_, _, p) =
            planned("Story: S\n  When the dial is 80 %\n  Then the help screen is hidden\n");
        assert_eq!(codes(&p), vec!["unpromised"]);
    }

    #[test]
    fn coverage_fills_the_gaps() {
        let (s, a, p) = planned("Story: S\n  When the dial is 80 %\n");
        let cov = coverage(&s, &p.plans);
        assert_eq!(cov.iter().filter(|c| c.is_empty()).count(), 3);
        assert_eq!(untested(&s, &a, &p.plans).len(), 3);
        let generated = coverage_plans(&s, &a, &p.plans);
        let titles: Vec<&str> = generated.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(
            titles,
            vec![
                "Reaching «Running dry stops the kettle»",
                "Reaching «Reset turns the kettle off»",
                "Reaching «Help opens the help screen»"
            ]
        );
        // To reach running dry, the generated test starts the kettle first.
        assert_eq!(generated[0].steps.len(), 2);
        // Help needs the help screen hidden, which the Background leaves open: it's arranged.
        assert_eq!(
            generated[2].given.last().unwrap().text,
            "the help screen is hidden"
        );
    }

    #[test]
    fn coverage_arranges_what_nothing_leads_to() {
        // Nothing sets the clock, so no route from the Background reaches 5 s: the generated test
        // arranges the Given itself and acts once.
        let src = "
Vocabulary:
  State: the clock is 0..60 s
  State: the result is none | draw
  Event: the clock runs out

Background:
  Given the clock is 60 s
  And the result is none

Feature: End
  Expectation: A tie when the clock runs out is a draw
    Given the clock is 5 s
    When the clock runs out
    Then the result is draw
";
        let l = expectation_language::load_sources("t", &[("t.expect", src)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let (s, a) = (&l.suite, expectation_graph::analyze(&l.suite));
        assert!(a.flow.routes[0].is_none());
        let untested = untested(s, &a, &[]);
        assert_eq!(untested.len(), 1);
        let help = untested[0].help.as_deref().unwrap();
        assert!(help.contains("has to arrange its Given directly: Given the clock is 5 s / When the clock runs out"), "{help}");
        let generated = coverage_plans(s, &a, &[]);
        assert_eq!(generated.len(), 1);
        let plan = &generated[0];
        assert_eq!(
            plan.title,
            "Arranging «A tie when the clock runs out is a draw»"
        );
        assert_eq!(plan.given.last().unwrap().text, "the clock is 5 s");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].fired, vec![expectation_model::ExpId(0)]);
        assert!(coverage(s, &generated)[0].len() == 1);
    }

    #[test]
    fn every_state_a_move_from_the_start_gets_a_visit() {
        // From the kettle off, two moves lead out: switching it on and unplugging it. The story
        // switches it on, so only the unplugged kettle needs a test of its own.
        let src = "
Vocabulary:
  State: the kettle is off | heating | unplugged
  Event: the user switches it on
  Event: the user unplugs it

Background:
  Given the kettle is off

Feature: Kettle
  Expectation: On heats
    Given the kettle is off
    When the user switches it on
    Then the kettle is heating

  Expectation: Unplugged
    Given the kettle is off
    When the user unplugs it
    Then the kettle is unplugged

Story: Boiling
  As a tea drinker
  I want hot water
  So that I make tea

  Scenario: Switch on
    When the user switches it on
    Then the kettle is heating
";
        let l = expectation_language::load_sources("t", &[("t.expect", src)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let (s, a) = (&l.suite, expectation_graph::analyze(&l.suite));
        let stories = plan_stories(s, &a).plans;
        let visits = visit_plans(s, &a, &stories);
        let titles: Vec<&str> = visits.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, ["Visiting the state after «Unplugged»"]);
        assert_eq!(visits[0].steps.len(), 1);
        // With that test too, nothing is left to visit.
        let all: Vec<Plan> = stories.into_iter().chain(visits).collect();
        assert!(visit_plans(s, &a, &all).is_empty());
    }

    #[test]
    fn a_follows_gets_a_test_that_walks_the_way_in() {
        let root = "
Vocabulary:
  State: the page is home | billing | settings
  State: the banner is shown | hidden
  Event: the user clicks Billing
";
        let home = "
Section: Home
  The home page.
Background:
  Given the page is home
  And the banner is shown
Feature: Navigation
  Expectation: The Billing link opens billing
    Given the page is home
    When the user clicks Billing
    Then the page is billing
";
        let billing = "
Section: Billing
  Paying.
Background:
  Follows: the page is home
  Given the page is billing
  And the banner is shown
";
        let files = [
            ("root.expect", root),
            ("01-home/h.expect", home),
            ("02-billing/b.expect", billing),
        ];
        let l = expectation_language::load_sources("t", &files);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let (s, a) = (&l.suite, expectation_graph::analyze(&l.suite));
        let generated = coverage_plans(s, &a, &[]);
        let plan = generated
            .iter()
            .find(|p| matches!(p.origin, Origin::Reaching { .. }))
            .expect("a reaching test");
        assert_eq!(plan.title, "Reaching «Billing»");
        assert_eq!(
            s.section(plan.section).name,
            "Home",
            "it starts where the setup it follows is"
        );
        let given: Vec<&str> = plan.given.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(given, ["the page is home", "the banner is shown"]);
        assert_eq!(plan.steps.len(), 1);
        // The link promises the page; only the section's setup says the banner is shown, so
        // that's checked because of it.
        let billing = s.sections.iter().find(|x| x.name == "Billing").unwrap().id;
        let checks = &plan.steps[0].checks;
        assert_eq!(checks.len(), 2, "{checks:?}");
        assert!(matches!(checks[0].because, Because::Expectation(_)));
        assert_eq!(checks[1].because, Because::Setup(billing));
        assert_eq!(
            s.vocabulary.describe(&plan.steps[0].state),
            "the page is billing and the banner is shown"
        );
        // It exercises the link, so coverage doesn't generate another test for it.
        assert_eq!(
            generated.len(),
            1,
            "{:?}",
            generated.iter().map(|p| &p.title).collect::<Vec<_>>()
        );
    }
}
