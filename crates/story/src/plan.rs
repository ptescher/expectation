//! User stories to acceptance-test plans.
//!
//! A story says what the user does ("When the dial is 80 %") and, optionally, what they
//! see ("Then the kettle is heating"). Planning walks the story through the expectations:
//! each step works out which expectations fire, and their Thens become the checks to run.
//! So a story only has to tell the journey; the expectations supply what must be true at each
//! step. Planning also refuses stories that rely on behaviour nobody has specified, or that
//! contradict the expectations.

use expectation_graph::semantics::perform_among;
use expectation_graph::{Analysis, Precedence};
use expectation_model::text;
use expectation_model::{
    Condition, Constraint, Diagnostic, ExpId, Fact, Keyword, SectionId, Span, StepText, StoryId, Suite, Trigger, VarId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A scenario someone wrote.
    Story { story: StoryId, scenario: usize },
    /// Generated to exercise an expectation no story reaches.
    Coverage { target: ExpId },
    /// Generated to walk the route into a section that follows a setup, and check that it gets
    /// to the section's start: the way in that its own tests skip by arranging it.
    Reaching { section: SectionId },
}

/// Why a check is there.
#[derive(Clone, Debug, PartialEq)]
pub enum Because {
    /// The plan arranged it before the first step.
    Arranged,
    /// An expectation's Then (or an invariant) promises it.
    Expectation(ExpId),
    /// The story says so in a Then.
    Story(Span),
    /// The section's setup, which the route into it gets to.
    Setup(SectionId),
}

#[derive(Clone, Debug)]
pub struct Check {
    pub var: VarId,
    pub expect: Constraint,
    pub because: Because,
}

#[derive(Clone, Debug)]
pub struct PlanStep {
    pub action: Trigger,
    pub text: Vec<StepText>,
    /// The written step it belongs to: docs show a group as one step.
    pub group: usize,
    /// The group's lines as written, on its first step.
    pub written: Vec<StepText>,
    /// Transitions that fire.
    pub fired: Vec<ExpId>,
    /// Invariants in force afterwards.
    pub invariants: Vec<ExpId>,
    pub checks: Vec<Check>,
    /// The predicted state afterwards.
    pub state: Condition,
}

impl PlanStep {
    pub fn exercises(&self) -> impl Iterator<Item = ExpId> + '_ {
        self.fired.iter().chain(&self.invariants).copied()
    }
}

/// One acceptance test: arrange `start`, check it, then perform each step and check it.
#[derive(Clone, Debug)]
pub struct Plan {
    pub origin: Origin,
    /// The section it belongs to, whose setup it starts from.
    pub section: SectionId,
    pub title: String,
    pub slug: String,
    pub start: Condition,
    pub given: Vec<StepText>,
    pub start_checks: Vec<Check>,
    /// Invariants in force at the start.
    pub start_invariants: Vec<ExpId>,
    pub steps: Vec<PlanStep>,
}

pub struct Planned {
    pub plans: Vec<Plan>,
    pub diagnostics: Vec<Diagnostic>,
}

/// One input step: what to do, what the story checks afterwards, and how it was written.
struct Input<'a> {
    action: Trigger,
    checks: &'a [Fact],
    text: Vec<StepText>,
    span: Span,
    group: usize,
    written: Vec<StepText>,
}

/// A plan for every scenario of every story.
pub fn plan_stories(suite: &Suite, analysis: &Analysis) -> Planned {
    let mut plans = vec![];
    let mut diagnostics = vec![];
    for story in &suite.stories {
        for (i, sc) in story.scenarios.iter().enumerate() {
            let title = if story.scenarios.len() == 1 && sc.name == story.name {
                story.name.clone()
            } else {
                format!("{} › {}", story.name, sc.name)
            };
            let slug = if title == story.name { story.slug.clone() } else { format!("{}--{}", story.slug, sc.slug) };
            let section = suite.section(story.section);
            let slug = prefixed(&section.slug, &slug);
            let inputs = sc.steps.iter().map(|s| Input {
                action: s.action.clone(),
                checks: &s.checks,
                text: s.steps.clone(),
                span: s.span.clone(),
                group: s.group,
                written: s.written.clone(),
            });
            let start = section.start.overlay(&sc.given);
            let mut given: Vec<StepText> =
                suite.chain(section.id).iter().flat_map(|s| s.setup_steps.iter().cloned()).collect();
            given.extend(sc.given_steps.iter().cloned());
            let origin = Origin::Story { story: story.id, scenario: i };
            let head = Head { origin, section: section.id, title, slug, start, given };
            plans.push(build(suite, &analysis.precedence, head, inputs, &mut diagnostics));
        }
    }
    Planned { plans, diagnostics }
}

/// `slug` under a section: "app--drawing--first-sketch".
fn prefixed(section: &str, slug: &str) -> String {
    if section.is_empty() { slug.to_string() } else { format!("{section}--{slug}") }
}

/// What a plan is, before its steps are worked out.
struct Head {
    origin: Origin,
    section: SectionId,
    title: String,
    slug: String,
    start: Condition,
    given: Vec<StepText>,
}

fn build<'a>(
    suite: &Suite,
    prec: &Precedence,
    head: Head,
    inputs: impl Iterator<Item = Input<'a>>,
    diags: &mut Vec<Diagnostic>,
) -> Plan {
    let Head { origin, section, title, slug, start, given } = head;
    let v = &suite.vocabulary;
    let (mut state, start_invariants) = expectation_graph::semantics::settle(suite, prec, &start, &[]);
    let mut start_checks: Vec<Check> =
        start.iter().map(|(var, c)| Check { var, expect: *c, because: Because::Arranged }).collect();
    for &inv in &start_invariants {
        promise(&mut start_checks, suite, inv, &start);
    }

    // A top-level section is a system of its own: only its expectations apply to its stories.
    let scope = expectation_graph::flow::in_scope(suite, section);
    let mut steps = vec![];
    for input in inputs {
        let errors = diags.iter().filter(|d| d.is_error()).count();
        let candidates = suite.transitions().filter(|e| scope[e.id.0 as usize]);
        let out = perform_among(suite, prec, &state, &input.action, candidates);
        if let Some(&(a, b, var)) = out.clashes.first() {
            let (a, b) = (suite.exp(a), suite.exp(b));
            diags.push(
                Diagnostic::error(
                    "clash",
                    format!("«{}» and «{}» both apply here and disagree about {}", a.name, b.name, v.var(var).subject),
                )
                .at(&input.span, "")
                .help("settle the conflict between them first"),
            );
            break;
        }
        for &u in &out.uncertain {
            let e = suite.exp(u);
            let mut d = Diagnostic::error("unclear-step", format!("it isn't clear whether «{}» applies here", e.name));
            let given = expectation_graph::semantics::seen_by(&state, &input.action, &e.trigger).unproven(&e.given);
            if !given.is_unconstrained() {
                d = d.at(&input.span, format!("nothing establishes that {}", v.describe(&given)));
            }
            let trig = input.action.changes.unproven(&e.trigger.changes);
            if !trig.is_unconstrained() {
                d = d.at(&input.span, format!("the step doesn't say whether {}", v.describe(&trig)));
            }
            diags.push(
                d.at(&e.span, "").help("pin it down: arrange it with a `Given`, or give the When an exact value"),
            );
        }
        if let (true, true, Some(e)) = (out.fired.is_empty(), out.uncertain.is_empty(), input.action.event) {
            let event = v.event(e);
            let line = input.text.iter().find(|t| text::key(&t.text) == event.key).map_or(&input.span, |t| &t.span);
            diags.push(
                Diagnostic::error("unspecified", format!("nothing is expected to happen when {}", event.text))
                    .at(line, format!("at this point {}", v.describe(&state)))
                    .help("no expectation covers this; write one, or the story relies on unspecified behaviour"),
            );
        }

        let mut checks = vec![];
        let before = state.overlay(&input.action.changes);
        for &f in out.fired.iter().chain(&out.invariants) {
            promise(&mut checks, suite, f, &before);
        }
        let mut after = out.after;
        for fact in input.checks {
            let mut want = Condition::new();
            want.constrain(v.var(fact.var), fact.constraint);
            let stated = v.var(fact.var).describe(&fact.constraint);
            if !after.overlaps(&want) {
                let said = v.describe(&after.restrict([fact.var]));
                let mut d = Diagnostic::error(
                    "contradicts",
                    format!("the story expects {stated}, but the expectations say {said}"),
                )
                .at(&fact.span, "");
                if let Some(&by) = out.fired.iter().find(|&&f| suite.exp(f).then.get(fact.var).is_some()) {
                    d = d.at(&suite.exp(by).span, format!("«{}» says so", suite.exp(by).name));
                }
                diags.push(d);
                continue;
            }
            if !after.implies(&want) {
                diags.push(
                    Diagnostic::warning("unpromised", format!("no expectation promises that {stated}"))
                        .at(&fact.span, "")
                        .help("it's checked anyway; write an expectation so it's specified"),
                );
            }
            after = after.and(&want);
            if !checks.iter().any(|c: &Check| c.var == fact.var && c.expect == fact.constraint) {
                checks.push(Check {
                    var: fact.var,
                    expect: fact.constraint,
                    because: Because::Story(fact.span.clone()),
                });
            }
        }
        steps.push(PlanStep {
            action: input.action,
            text: input.text,
            group: input.group,
            written: input.written,
            fired: out.fired,
            invariants: out.invariants,
            checks,
            state: after.clone(),
        });
        state = after;
        // Stop at a scenario's first error: later steps would be judged from a state that's wrong.
        if diags.iter().filter(|d| d.is_error()).count() > errors {
            break;
        }
    }
    Plan { origin, section, title, slug, start, given, start_checks, start_invariants, steps }
}

/// Add an expectation's Then to `checks`, skipping what's already checked.
fn promise(checks: &mut Vec<Check>, suite: &Suite, e: ExpId, before: &Condition) {
    for (var, c) in suite.exp(e).effect(&suite.vocabulary, before).iter() {
        if !checks.iter().any(|k| k.var == var && k.expect == *c) {
            checks.push(Check { var, expect: *c, because: Because::Expectation(e) });
        }
    }
}

/// For each expectation, the (plan, step) pairs that exercise it; step `None` is the start.
pub fn coverage(suite: &Suite, plans: &[Plan]) -> Vec<Vec<(usize, Option<usize>)>> {
    let mut out = vec![vec![]; suite.expectations.len()];
    for (p, plan) in plans.iter().enumerate() {
        for &inv in &plan.start_invariants {
            out[inv.0 as usize].push((p, None));
        }
        for (s, step) in plan.steps.iter().enumerate() {
            for e in step.exercises() {
                out[e.0 as usize].push((p, Some(s)));
            }
        }
    }
    out
}

/// Warnings for expectations no plan exercises, each with a route that would.
pub fn untested(suite: &Suite, analysis: &Analysis, plans: &[Plan]) -> Vec<Diagnostic> {
    let cov = coverage(suite, plans);
    let mut diags = vec![];
    for e in &suite.expectations {
        if !cov[e.id.0 as usize].is_empty() {
            continue;
        }
        let Some(route) = &analysis.flow.routes[e.id.0 as usize] else {
            // Nothing leads there, so a test has to arrange the Given and act (`coverage_plans` does).
            let section = suite.section_of(e.id);
            let mut lines = arranged(suite, &section.start, &section.start.overlay(&e.given));
            if !e.is_invariant() {
                lines.push(format!("When {}", suite.describe_trigger(&e.trigger)));
            }
            diags.push(Diagnostic::warning("untested", format!("no story exercises «{}»", e.name)).at(&e.span, "").help(
                format!(
                    "no chain of expectations gets there, so a test has to arrange its Given directly{}",
                    if lines.is_empty() { String::new() } else { format!(": {}", lines.join(" / ")) }
                ),
            ));
            continue;
        };
        let mut lines: Vec<String> = route
            .start
            .iter()
            .filter(|(v, c)| suite.background.get(*v) != Some(c))
            .map(|(v, c)| format!("Given {}", suite.vocabulary.var(v).describe(c)))
            .collect();
        lines.extend(route.steps.iter().map(|s| format!("When {}", suite.describe_trigger(&suite.exp(*s).trigger))));
        diags.push(Diagnostic::warning("untested", format!("no story exercises «{}»", e.name)).at(&e.span, "").help(
            if lines.is_empty() {
                "it holds from the start; any story checks it".into()
            } else {
                format!("a story that would: {}", lines.join(" / "))
            },
        ));
    }
    diags
}

/// The `Given …` lines that arrange `start` on top of `base`.
fn arranged(suite: &Suite, base: &Condition, start: &Condition) -> Vec<String> {
    start
        .iter()
        .filter(|(v, c)| base.get(*v) != Some(c))
        .map(|(v, c)| format!("Given {}", suite.vocabulary.var(v).describe(c)))
        .collect()
}

/// An expectation's When lines, as one written step.
fn when_text(x: &expectation_model::Expectation) -> Vec<StepText> {
    x.when_steps()
        .cloned()
        .enumerate()
        .map(|(i, mut t)| {
            t.keyword = if i == 0 { Keyword::When } else { Keyword::And };
            t
        })
        .collect()
}

/// A plan for each section that follows a setup the expectations get from: arrange that setup,
/// take the route, and check everything the section's setup says. Its own tests arrange the
/// section's setup directly, so this is what shows the way in gets there too.
pub fn reaching_plans(suite: &Suite, analysis: &Analysis) -> Vec<Plan> {
    let mut out = vec![];
    for s in suite.sections.iter().filter(|s| s.follows.is_some()) {
        let Some(route) = analysis.section_routes[s.id.0 as usize].as_ref().filter(|r| !r.steps.is_empty()) else {
            continue;
        };
        // It runs where the setup it follows is: that section's launch and expectations.
        let from = expectation_graph::followed(suite, s);
        let mut given: Vec<StepText> = match from {
            Some(f) => suite.chain(f).iter().flat_map(|x| x.setup_steps.iter().cloned()).collect(),
            None => s.follows_steps.clone(),
        };
        given.iter_mut().for_each(|t| t.keyword = Keyword::Given);
        let inputs = route.steps.iter().enumerate().map(|(n, &id)| {
            let x = suite.exp(id);
            let text = when_text(x);
            let (action, span) = (x.trigger.clone(), x.span.clone());
            Input { action, checks: &[], written: text.clone(), text, span, group: n + 1 }
        });
        let head = Head {
            origin: Origin::Reaching { section: s.id },
            section: from.unwrap_or(s.id),
            title: format!("Reaching «{}»", s.name),
            slug: prefixed(&s.slug, "reaching"),
            start: route.start.clone(),
            given,
        };
        let mut diags = vec![];
        let mut plan = build(suite, &analysis.precedence, head, inputs, &mut diags);
        if diags.iter().any(|d| d.is_error()) || plan.steps.len() < route.steps.len() {
            continue;
        }
        // What the expectations promise is checked already; the rest of the setup isn't.
        let last = plan.steps.last_mut().expect("a route has steps");
        for (var, c) in s.start.iter() {
            if !last.checks.iter().any(|k| k.var == var && k.expect == *c) {
                last.checks.push(Check { var, expect: *c, because: Because::Setup(s.id) });
            }
        }
        out.push(plan);
    }
    out
}

/// Generated plans: [`reaching_plans`], then plans for the expectations that no plan in `plans`
/// (or those) exercises, following each one's shortest route from the flow graph. One the flow
/// graph has no route to is tested on its own: its Given arranged on its section's start, then
/// its When. Plans that can't be made clean are skipped.
pub fn coverage_plans(suite: &Suite, analysis: &Analysis, plans: &[Plan]) -> Vec<Plan> {
    let mut out = reaching_plans(suite, analysis);
    let cov = coverage(suite, plans);
    let mut covered: Vec<bool> = cov.iter().map(|c| !c.is_empty()).collect();
    for (e, by) in coverage(suite, &out).iter().enumerate() {
        covered[e] |= !by.is_empty();
    }
    // Routes from each section's own start, so a generated test starts where the section does.
    let mut flows: Vec<Option<expectation_graph::Flow>> = suite.sections.iter().map(|_| None).collect();
    for e in &suite.expectations {
        if covered[e.id.0 as usize] {
            continue;
        }
        let section = suite.section_of(e.id);
        let flow = flows[section.id.0 as usize].get_or_insert_with(|| {
            let roots = [(section.start.clone(), section.id)];
            expectation_graph::flow::explore(suite, &analysis.precedence, &roots, expectation_graph::Limits::default())
        });
        let route = flow.routes[e.id.0 as usize].as_ref().or(analysis.flow.routes[e.id.0 as usize].as_ref());
        if route.is_some_and(|r| r.steps.is_empty()) {
            continue;
        }
        // Without a route, the expectation on its own: its Given arranged on the section's
        // start, then its When (an invariant needs no step: it's checked at the start).
        let (start, steps, title) = match route {
            Some(r) => (r.start.clone(), r.steps.clone(), format!("Reaching «{}»", e.name)),
            None if e.is_invariant() => (section.start.overlay(&e.given), vec![], format!("Arranging «{}»", e.name)),
            None => (section.start.overlay(&e.given), vec![e.id], format!("Arranging «{}»", e.name)),
        };
        let inputs = steps.iter().enumerate().map(|(n, &s)| {
            let x = suite.exp(s);
            let text = when_text(x);
            Input {
                action: x.trigger.clone(),
                checks: &[],
                written: text.clone(),
                text,
                span: x.span.clone(),
                group: n + 1,
            }
        });
        let mut given: Vec<StepText> =
            suite.chain(section.id).iter().flat_map(|s| s.setup_steps.iter().cloned()).collect();
        for (var, c) in start.iter().filter(|(v, c)| section.start.get(*v) != Some(c)) {
            let span = e.span.clone();
            let (text, meaning) = (suite.vocabulary.var(var).describe(c), Some(expectation_model::Said::Fact(var, *c)));
            given.push(StepText { keyword: Keyword::Given, text, span, meaning });
        }
        let mut diags = vec![];
        let head = Head {
            origin: Origin::Coverage { target: e.id },
            section: section.id,
            title,
            slug: prefixed(&section.slug, &format!("coverage--{}", text::slug(&e.name))),
            start,
            given,
        };
        let plan = build(suite, &analysis.precedence, head, inputs, &mut diags);
        if diags.iter().any(|d| d.is_error()) {
            continue;
        }
        for &x in &plan.start_invariants {
            covered[x.0 as usize] = true;
        }
        for step in &plan.steps {
            for x in step.exercises() {
                covered[x.0 as usize] = true;
            }
        }
        out.push(plan);
    }
    out
}

/// A test for each state of the state map a move from a start that no other test passes
/// through: the start arranged with the rule's Given, then the rule's When. Each runner takes
/// a screenshot after the step, so the map's first view has a picture of every state. `plans`
/// are the tests there are already, generated ones included.
pub fn visit_plans(suite: &Suite, analysis: &Analysis, plans: &[Plan]) -> Vec<Plan> {
    let by = expectation_graph::states::default_by(suite);
    let key = |c: &Condition| c.restrict(by.iter().copied());
    let mut seen: std::collections::HashSet<Condition> = plans
        .iter()
        .flat_map(|p| std::iter::once(&p.start).chain(p.steps.iter().map(|s| &s.state)))
        .map(key)
        .collect();
    let mut slugs: std::collections::HashSet<String> = plans.iter().map(|p| p.slug.clone()).collect();
    let mut out = vec![];
    for m in expectation_graph::states::first_moves(suite, &analysis.precedence) {
        if seen.contains(&key(&m.after)) {
            continue;
        }
        let e = suite.exp(m.rule);
        let section = suite.sections.iter().find(|s| s.id == m.section).expect("a move's section");
        let text = when_text(e);
        let input = Input { action: e.trigger.clone(), checks: &[], written: text.clone(), text, span: e.span.clone(), group: 1 };
        let mut given: Vec<StepText> =
            suite.chain(section.id).iter().flat_map(|s| s.setup_steps.iter().cloned()).collect();
        for (var, c) in m.before.iter().filter(|(v, c)| section.start.get(*v) != Some(c)) {
            let (text, meaning) = (suite.vocabulary.var(var).describe(c), Some(expectation_model::Said::Fact(var, *c)));
            given.push(StepText { keyword: Keyword::Given, text, span: e.span.clone(), meaning });
        }
        let base = prefixed(&section.slug, &format!("state--{}", text::slug(&e.name)));
        let mut slug = base.clone();
        let mut n = 2;
        while slugs.contains(&slug) {
            slug = format!("{base}-{n}");
            n += 1;
        }
        let head = Head {
            origin: Origin::Coverage { target: e.id },
            section: section.id,
            title: format!("Visiting the state after «{}»", e.name),
            slug: slug.clone(),
            start: m.before.clone(),
            given,
        };
        let mut diags = vec![];
        let plan = build(suite, &analysis.precedence, head, std::iter::once(input), &mut diags);
        if diags.iter().any(|d| d.is_error()) {
            continue;
        }
        seen.insert(key(&m.after));
        for s in &plan.steps {
            seen.insert(key(&s.state));
        }
        slugs.insert(slug);
        out.push(plan);
    }
    out
}
