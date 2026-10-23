//! Analysis of a suite as a graph of expectations:
//!
//! - [`precedence`]: the override DAG (exceptions and what they override),
//! - [`conflict`]: pairs of expectations that can't both hold, settled or not,
//! - [`flow`]: the DAG of what leads to what, from the Background,
//! - [`semantics`]: stepping a state through an action, shared with story planning.

pub mod conflict;
pub mod export;
pub mod flow;
pub mod precedence;
pub mod semantics;
pub mod states;

pub use conflict::{Conflict, Kind as ConflictKind};
pub use flow::{Flow, Limits, Node, Route};
pub use precedence::Precedence;
pub use states::StateMap;

use expectation_model::{Condition, Diagnostic, ExpId, FeatureId, Section, SectionId, Suite, VarId};

pub struct Analysis {
    pub precedence: Precedence,
    pub conflicts: Vec<Conflict>,
    pub flow: Flow,
    /// Per section: the expectations that lead into its start, from the setup it follows or its
    /// parent's start.
    pub section_routes: Vec<Option<Route>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    pub fn has_errors(&self) -> bool {
        expectation_model::diagnostic::has_errors(&self.diagnostics)
    }

    /// Conflicts an override settles: the suite's exceptions.
    pub fn exceptions(&self) -> impl Iterator<Item = &Conflict> {
        self.conflicts.iter().filter(|c| c.winner.is_some())
    }

    /// Features in dependency order: after the features their expectations follow on from
    /// (flow edges) or make exceptions to (overrides). Among those ready, the earliest in the
    /// flow first, then the most depended on, then declaration order. Cycles fall back to
    /// declaration order.
    pub fn feature_order(&self, suite: &Suite) -> Vec<FeatureId> {
        let n = suite.features.len();
        let feature = |e: ExpId| suite.exp(e).feature.0 as usize;
        let mut deps = vec![std::collections::BTreeSet::new(); n];
        for (p, c) in &self.flow.edges {
            if let Node::Exp(p) = p {
                deps[feature(*c)].insert(feature(*p));
            }
        }
        for (w, l) in &self.precedence.declared {
            deps[feature(*w)].insert(feature(*l));
        }
        for (f, d) in deps.iter_mut().enumerate() {
            d.remove(&f);
        }
        let dependents: Vec<usize> = (0..n).map(|f| deps.iter().filter(|d| d.contains(&f)).count()).collect();
        let earliest: Vec<u32> = suite
            .features
            .iter()
            .map(|f| f.expectations.iter().filter_map(|e| self.flow.level[e.0 as usize]).min().unwrap_or(u32::MAX))
            .collect();
        let key = |f: usize| (earliest[f], std::cmp::Reverse(dependents[f]), f);
        let mut placed = vec![false; n];
        let mut order = vec![];
        while order.len() < n {
            let ready = (0..n).filter(|&f| !placed[f] && deps[f].iter().all(|&d| placed[d])).min_by_key(|&f| key(f));
            let next = ready.unwrap_or_else(|| (0..n).find(|&f| !placed[f]).unwrap());
            placed[next] = true;
            order.push(FeatureId(next as u32));
        }
        order
    }
}

/// The states searches start from: the start of each section with features or stories in
/// it, and each scenario's arranged start. (A section that only groups others has nothing that
/// starts from it; searching from an empty root would mean searching every state there is.)
pub fn roots(suite: &Suite) -> Vec<(Condition, SectionId)> {
    let mut roots: Vec<(Condition, SectionId)> = vec![];
    let starts = suite
        .sections
        .iter()
        .filter(|s| !s.features.is_empty() || !s.stories.is_empty())
        .map(|s| (s.start.clone(), s.id));
    let scenarios = suite.stories.iter().flat_map(|story| {
        let start = &suite.section(story.section).start;
        story.scenarios.iter().map(move |sc| (start.overlay(&sc.given), story.section))
    });
    for r in starts.chain(scenarios) {
        if !roots.contains(&r) {
            roots.push(r);
        }
    }
    roots
}

/// The section whose start a `Follows:` names: of the sections with tests in them whose start
/// has the followed setup in place, the one that says least besides. The route starts there,
/// so it can use what else that setup says.
pub fn followed(suite: &Suite, section: &Section) -> Option<SectionId> {
    let follows = section.follows.as_ref()?;
    suite
        .sections
        .iter()
        .filter(|s| s.id != section.id && (!s.features.is_empty() || !s.stories.is_empty()) && s.start.implies(follows))
        .min_by_key(|s| s.start.iter().count())
        .map(|s| s.id)
}

/// Where a section's setup is reached from: the setup its `Follows:` names (as the section it
/// names has it, if one does), or else its parent's start.
pub fn reached_from<'a>(suite: &'a Suite, section: &'a Section) -> Option<&'a Condition> {
    match &section.follows {
        Some(f) => Some(followed(suite, section).map_or(f, |s| &suite.section(s).start)),
        None => section.parent.map(|p| &suite.section(p).start),
    }
}

/// How a section's setup is reached from where it is reached from ([`reached_from`]): the chain
/// of expectations that gets there. `None` for the root, for a section that adds no setup to
/// its parent's, and for one nothing leads into.
pub fn section_route(suite: &Suite, prec: &Precedence, section: &Section, limits: Limits) -> Option<Route> {
    let from = reached_from(suite, section)?;
    if (section.follows.is_none() && section.setup.is_unconstrained()) || from.implies(&section.start) {
        return None;
    }
    flow::route_to(suite, prec, from, &section.start, limits)
}

/// For each section, how its setup is reached: see [`section_route`].
pub fn section_routes(suite: &Suite, prec: &Precedence, limits: Limits) -> Vec<Option<Route>> {
    suite.sections.iter().map(|s| section_route(suite, prec, s, limits)).collect()
}

pub fn analyze(suite: &Suite) -> Analysis {
    analyze_with(suite, Limits::default())
}

pub fn analyze_with(suite: &Suite, limits: Limits) -> Analysis {
    let (precedence, mut diagnostics) = precedence::build(suite);
    let conflicts = conflict::detect(suite, &precedence);
    diagnostics.extend(conflict::diagnose(suite, &precedence, &conflicts));
    let flow = flow::explore(suite, &precedence, &roots(suite), limits);
    if !flow.truncated {
        for e in suite.expectations.iter().filter(|e| flow.level[e.id.0 as usize].is_none()) {
            let needs = if e.is_invariant() {
                suite.describe(&e.given)
            } else {
                format!("{}, and then {}", suite.describe(&e.given), suite.describe_trigger(&e.trigger))
            };
            diagnostics.push(
                Diagnostic::warning("unreachable", format!("«{}» can never happen, starting from any section's setup", e.name))
                    .at(&e.span, format!("it needs {needs}"))
                    .help("no chain of expectations leads there; check the Background, or add the expectation that gets there"),
            );
        }
    }
    let section_routes = section_routes(suite, &precedence, limits);
    for (s, route) in suite.sections.iter().zip(&section_routes) {
        if s.follows.is_some() {
            diagnostics.extend(follows(suite, &precedence, &flow, s, route.as_ref()));
            continue;
        }
        let Some(parent) = s.parent.map(|p| suite.section(p)) else { continue };
        // From a parent with no setup there's nothing to route from: getting there is starting
        // the program, which happens outside the suite.
        let from_somewhere = !parent.start.is_unconstrained();
        if from_somewhere && route.is_none() && !s.setup.is_unconstrained() && !parent.start.implies(&s.start) {
            let span = s.setup_steps.first().map(|t| &t.span).or(s.span.as_ref());
            let mut d =
                Diagnostic::warning("no-way-in", format!("nothing leads from «{}» into «{}»", parent.name, s.name))
                    .note_that(format!("«{}» starts with {}", s.name, suite.describe(&parent.start.unproven(&s.start))))
                    .help("add the expectation that gets there, so the docs can say how to get to this section");
            if let Some(span) = span {
                d = d.at(span, "");
            }
            diagnostics.push(d);
        }
    }
    Analysis { precedence, conflicts, flow, section_routes, diagnostics }
}

/// What's wrong with a section's `Follows:`. Nothing gets to the setup it follows (a warning,
/// and nothing more to say), or the expectations change every state the section's setup needs
/// and still nothing gets there from it (an error: the route it claims is broken). Where some
/// of those states nothing changes, the Background's steps are a claim about how to set up
/// that the expectations can't check, so there's nothing to say.
fn follows(suite: &Suite, prec: &Precedence, flow: &Flow, s: &Section, route: Option<&Route>) -> Vec<Diagnostic> {
    let (Some(f), Some(from)) = (s.follows.as_ref(), reached_from(suite, s)) else { return vec![] };
    let span = s.follows_steps.first().map(|t| &t.span).or(s.span.as_ref());
    let at = |d: Diagnostic| match span {
        Some(span) => d.at(span, ""),
        None => d,
    };
    if !flow.truncated && !flow.followed[s.id.0 as usize] {
        let what = format!("nothing gets to {}, which «{}» follows", suite.describe(f), s.name);
        let d = Diagnostic::warning("follows-unreached", what)
            .help("no section's setup has it in place and no chain of expectations gets there; check the `Follows:`");
        return vec![at(d)];
    }
    if route.is_some() || from.implies(&s.start) {
        return vec![];
    }
    let (settled, _) = semantics::settle(suite, prec, from, &[]);
    let needed: Vec<VarId> =
        s.start.iter().filter(|(v, c)| !settled.get(*v).is_some_and(|x| x.within(c))).map(|(v, _)| v).collect();
    if !needed.iter().all(|v| suite.transitions().any(|e| e.sets().any(|x| x == *v))) {
        return vec![];
    }
    let what = format!("nothing leads from {} into «{}»", suite.describe(f), s.name);
    let d = Diagnostic::error("follows-no-way-in", what)
        .note_that(format!(
            "«{}» starts with {}, and expectations change all of that, but no chain of them gets there",
            s.name,
            suite.describe(&settled.unproven(&s.start))
        ))
        .help("the route it follows is broken: fix the expectations that get there, or the `Follows:`");
    vec![at(d)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOCAB: &str = "
Vocabulary:
  State: the kettle is off | heating | dry
  State: the dial is 0..100 %
  State: the child lock is on | off
  State: the help screen is shown | hidden
  Event: the user presses reset
  Event: the user presses help
  Event: the water runs out

Background:
  Given the kettle is off
  And the dial is 0 %
  And the child lock is off
  And the help screen is hidden
";

    const HEATING: &str = "
Feature: Heating
  Expectation: Turning the dial up starts the kettle
    Given the kettle is off
    When the dial is above 55 %
    Then the kettle is heating

  Expectation: The child lock holds the kettle
    Given the kettle is off
    And the child lock is on
    When the dial is above 70 %
    Then the kettle is off

  Expectation: Running dry stops the kettle
    Given the kettle is heating
    When the water runs out
    Then the kettle is dry

  Expectation: Reset turns the kettle off
    When the user presses reset
    Then the kettle is off
";

    /// A website: the home page links to billing, and the billing section follows the home page.
    const SITE_ROOT: &str = "
Suite: Site
Vocabulary:
  State: the page is home | billing | settings
  State: the session is in | out
  Event: the user clicks Billing
  Event: the user clicks Settings
";

    const SITE_HOME: &str = "
Section: Home
  The home page.
Background:
  Given the page is home
  And the session is in
Feature: Navigation
  Expectation: The Billing link opens billing
    Given the page is home
    When the user clicks Billing
    Then the page is billing
";

    const SITE_BILLING: &str = "
Section: Billing
  Paying.
Background:
  Follows: the page is home
  Given the page is billing
Feature: Billing
  Expectation: Settings from billing
    Given the page is billing
    When the user clicks Settings
    Then the page is settings
";

    fn suite(extra: &str) -> Suite {
        let src = format!("{VOCAB}{HEATING}{extra}");
        let l = expectation_language::load_sources("t", &[("t.expect", &src)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        l.suite
    }

    fn named(s: &Suite, name: &str) -> ExpId {
        s.expectations.iter().find(|e| e.name == name).unwrap().id
    }

    #[test]
    fn finds_an_outcome_conflict_and_where_it_happens() {
        let s = suite("");
        let a = analyze(&s);
        let [c] = &a.conflicts[..] else { panic!("{:?}", a.conflicts) };
        assert_eq!((c.a, c.b), (named(&s, "Turning the dial up starts the kettle"), named(&s, "The child lock holds the kettle")));
        assert_eq!(c.winner, None);
        // The triggers overlap above 70 %, and both Givens hold with the child lock on.
        assert_eq!(s.describe_trigger(&c.trigger), "the dial is above 70 %");
        assert!(s.describe(&c.situation).contains("the child lock is on"));
        let d = a.diagnostics.iter().find(|d| d.code == "conflict").unwrap();
        assert!(d.help.as_ref().unwrap().contains("«The child lock holds the kettle» is the more specific"), "{:?}", d.help);
    }

    #[test]
    fn an_override_settles_it() {
        let s = suite("");
        let src = format!("{VOCAB}{}", HEATING.replace("    Given the kettle is off\n    And the child", "    Overrides: Turning the dial up starts the kettle\n    Given the kettle is off\n    And the child"));
        let l = expectation_language::load_sources("t", &[("t.expect", &src)]);
        let a = analyze(&l.suite);
        assert!(!a.has_errors(), "{}", expectation_model::diagnostic::render_all(&a.diagnostics, &l.suite.sources));
        assert_eq!(a.exceptions().count(), 1);
        assert_eq!(a.exceptions().next().unwrap().winner, Some(named(&s, "The child lock holds the kettle")));
    }

    #[test]
    fn disjoint_givens_or_different_triggers_dont_conflict() {
        let s = suite(
            "
Feature: More
  Expectation: Help while heating
    Given the kettle is heating
    When the user presses help
    Then the help screen is shown
  Expectation: Help is ignored while off
    Given the kettle is off
    When the user presses help
    Then the help screen is hidden
",
        );
        let a = analyze(&s);
        assert_eq!(a.conflicts.len(), 1, "only the child lock conflict: {:?}", a.conflicts);
    }

    #[test]
    fn invariant_conflicts() {
        let s = suite(
            "
Feature: Help
  Expectation: Help toggles on
    Given the help screen is hidden
    When the user presses help
    Then the help screen is shown
  Expectation: No help while heating
    Given the kettle is heating
    Then the help screen is hidden
",
        );
        let a = analyze(&s);
        let c = a.conflicts.iter().find(|c| c.kind == ConflictKind::Invariant).expect("help toggles while heating");
        assert_eq!((c.a, c.b), (named(&s, "Help toggles on"), named(&s, "No help while heating")));
    }

    #[test]
    fn override_cycles_are_errors() {
        let src = format!(
            "{VOCAB}
Feature: F
  Expectation: A
    Overrides: B
    When the user presses help
    Then the help screen is shown
  Expectation: B
    Overrides: A
    When the user presses help
    Then the help screen is hidden
"
        );
        let l = expectation_language::load_sources("t", &[("t.expect", &src)]);
        let a = analyze(&l.suite);
        assert!(a.diagnostics.iter().any(|d| d.code == "override-cycle"));
    }

    #[test]
    fn flow_levels_edges_and_routes() {
        let s = suite("");
        let a = analyze(&s);
        let start = named(&s, "Turning the dial up starts the kettle");
        let dry = named(&s, "Running dry stops the kettle");
        let reset = named(&s, "Reset turns the kettle off");
        let lock = named(&s, "The child lock holds the kettle");
        let level = |e: ExpId| a.flow.level[e.0 as usize];
        assert_eq!(level(start), Some(0));
        assert_eq!(level(reset), Some(0));
        assert_eq!(level(dry), Some(1));
        assert!(a.flow.edges.contains(&(Node::Start, start)));
        assert!(a.flow.edges.contains(&(Node::Exp(start), dry)));
        // Nothing turns the child lock on, so its exception can't happen from the Background.
        assert_eq!(level(lock), None);
        assert!(a.diagnostics.iter().any(|d| d.code == "unreachable"));
        // Levels grow along every edge: a DAG, even though reset loops back to the start.
        for (p, c) in &a.flow.edges {
            if let Node::Exp(p) = p {
                assert!(level(*p) < level(*c));
            }
        }
        let route = a.flow.routes[dry.0 as usize].as_ref().unwrap();
        assert_eq!(route.steps, vec![start, dry]);
    }

    #[test]
    fn relative_thens_conflict_and_sections_have_a_way_in() {
        let root = "Vocabulary:\n State: the tool is none | PEN\n State: the drawing has 0..100 shapes\n Event: the user presses PEN\n Event: the user clicks the canvas\nBackground:\n Given the tool is none\n And the drawing has 0 shapes\nFeature: Tools\n Expectation: PEN picks the pen\n  Given the tool is none\n  When the user presses PEN\n  Then the tool is PEN\n";
        let drawing = "Background:\n Given the tool is PEN\nFeature: Shapes\n Expectation: A click adds a shape\n  Given the tool is PEN\n  When the user clicks the canvas\n  Then the drawing goes up by 1\n Expectation: A click adds two\n  Given the tool is PEN\n  When the user clicks the canvas\n  Then the drawing goes up by 2\n";
        let l = expectation_language::load_sources("t", &[("root.expect", root), ("drawing/w.expect", drawing)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let a = analyze(&l.suite);
        let [c] = &a.conflicts[..] else { panic!("{:?}", a.conflicts) };
        assert_eq!((c.a, c.b), (ExpId(1), ExpId(2)));
        // From the root's start, pressing PEN gets into the drawing section.
        let route = a.section_routes[1].as_ref().expect("a way into drawing");
        assert_eq!(route.steps, vec![ExpId(0)]);
        assert!(!a.diagnostics.iter().any(|d| d.code == "no-way-in"));
    }

    #[test]
    fn mermaid_mentions_everything() {
        let s = suite("");
        let a = analyze(&s);
        let m = export::mermaid(&s, &a, &|_| None);
        assert!(m.contains("start --> e0"));
        assert!(m.contains("<-. conflict .->"));
        assert!(m.contains("class e1 unreachable"));
        assert!(export::dot(&s, &a).contains("color=red"));
    }

    fn site(home: &str, billing: &str) -> (Suite, Analysis) {
        let files = [("root.expect", SITE_ROOT), ("01-home/h.expect", home), ("02-billing/b.expect", billing)];
        let l = expectation_language::load_sources("t", &files);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        let a = analyze(&l.suite);
        (l.suite, a)
    }

    #[test]
    fn a_follows_takes_the_route_the_expectations_take() {
        let (s, a) = site(SITE_HOME, SITE_BILLING);
        assert!(!a.diagnostics.iter().any(|d| d.code.starts_with("follows")), "{:?}", a.diagnostics);
        let billing = s.sections.iter().find(|x| x.name == "Billing").unwrap();
        let route = a.section_routes[billing.id.0 as usize].as_ref().expect("a route from home");
        assert_eq!(route.steps, vec![named(&s, "The Billing link opens billing")]);
        // The map takes the link, not a made-up move named after the section.
        let map = states::explore(&s, &a.precedence, &[], Limits::default());
        assert!(map.moves.iter().all(|m| m.setup.is_none()), "{:?}", map.moves);
        let to_billing = map.moves.iter().find(|m| map.label(&s, m.to).contains("billing")).unwrap();
        assert_eq!(map.describe_move(&s, to_billing), "the user clicks Billing");
        assert!(!map.states[to_billing.to].start);
    }

    #[test]
    fn a_broken_route_into_a_follows_is_an_error() {
        // The link now goes to settings, and nothing else gets from home to billing.
        let home = SITE_HOME.replace("Then the page is billing", "Then the page is settings");
        let (_, a) = site(&home, SITE_BILLING);
        let d = a.diagnostics.iter().find(|d| d.code == "follows-no-way-in").expect("an error");
        assert!(d.is_error());
        assert!(d.message.contains("nothing leads from the page is home into «Billing»"), "{}", d.message);
    }

    #[test]
    fn a_follows_nothing_gets_to_is_a_warning() {
        let billing = SITE_BILLING.replace("Follows: the page is home", "Follows: the session is out");
        let (_, a) = site(SITE_HOME, &billing);
        let d = a.diagnostics.iter().find(|d| d.code == "follows-unreached").expect("a warning");
        assert!(!d.is_error());
        // A route from a setup nobody gets to isn't worth an error of its own.
        assert!(!a.diagnostics.iter().any(|d| d.code == "follows-no-way-in"), "{:?}", a.diagnostics);
    }
}
