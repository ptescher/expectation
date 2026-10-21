//! The state map: the states the system can get into from each section's setup, and the
//! actions that move it from one to another. Where the flow DAG is about expectations (which
//! leads to which), this is about the app: its screens and modes, and how you get around them.
//!
//! The search is the flow DAG's (breadth first from every root, taking every transition that
//! can apply), but it keeps every move between states, loops and all, instead of only the
//! shortest. States are told apart by a few states of the vocabulary, `by`: a screen, a mode,
//! the kettle's phase. Everything else (a dial's exact reading, a counter) is folded in, so
//! the map has one node per screen and not one per number on it. Without a choice, it's
//! every enumerated state.

use crate::flow::{Limits, companions, in_scope};
use crate::precedence::Precedence;
use crate::semantics::{perform_among, settle};
use expectation_model::{Condition, ExpId, Expectation, SectionId, Suite, Trigger, VarId, VarKind};
use std::collections::HashMap;
use std::fmt::Write;

/// One state of the map: what the `by` states are in it.
#[derive(Clone, Debug)]
pub struct MapState {
    /// Only the `by` states; one it doesn't mention can be anything here.
    pub cond: Condition,
    /// The fewest moves from a start.
    pub level: u32,
    /// A section's setup or a story's arranged start is in it.
    pub start: bool,
}

/// Doing `action` in state `from` leaves the system in state `to`, by way of `fired`.
#[derive(Clone, Debug)]
pub struct Move {
    pub from: usize,
    pub to: usize,
    pub action: Trigger,
    /// The transitions that fire, in the order they were found.
    pub fired: Vec<ExpId>,
}

impl Move {
    /// It changes something, but nothing the map tells states apart by.
    pub fn stays(&self) -> bool {
        self.from == self.to
    }
}

#[derive(Clone, Debug, Default)]
pub struct StateMap {
    /// The states its states are told apart by, in declaration order.
    pub by: Vec<VarId>,
    pub states: Vec<MapState>,
    pub moves: Vec<Move>,
    /// The search hit its limits, so there may be states it didn't get to.
    pub truncated: bool,
}

/// The states a map tells apart when nobody says: every enumerated one. Numbers would give a
/// state per reading.
pub fn default_by(suite: &Suite) -> Vec<VarId> {
    suite.vocabulary.variables.iter().filter(|v| matches!(v.kind, VarKind::Enum(_))).map(|v| v.id).collect()
}

/// The map of `suite`, telling states apart by `by` (or [`default_by`] when it's empty).
pub fn explore(suite: &Suite, prec: &Precedence, by: &[VarId], limits: Limits) -> StateMap {
    let mut by: Vec<VarId> = if by.is_empty() { default_by(suite) } else { by.to_vec() };
    by.sort();
    by.dedup();
    let mut map = StateMap { by: by.clone(), ..StateMap::default() };
    let mut nodes: HashMap<Condition, usize> = HashMap::new();
    let mut node = |map: &mut StateMap, c: &Condition, level: u32, start: bool| -> usize {
        let key = c.restrict(by.iter().copied());
        let i = *nodes.entry(key.clone()).or_insert_with(|| {
            map.states.push(MapState { cond: key, level, start });
            map.states.len() - 1
        });
        let s = &mut map.states[i];
        s.level = s.level.min(level);
        s.start |= start;
        i
    };
    let mut moves: HashMap<(usize, usize, Trigger), usize> = HashMap::new();

    // (full state, its section, level)
    let mut queue: Vec<(Condition, SectionId, u32)> = vec![];
    let mut seen: HashMap<(Condition, SectionId), ()> = HashMap::new();
    let mut scopes: HashMap<SectionId, Vec<bool>> = HashMap::new();
    for (root, section) in crate::roots(suite) {
        scopes.entry(section).or_insert_with(|| in_scope(suite, section));
        let (cond, _) = settle(suite, prec, &root, &[]);
        if !cond.is_satisfiable() || seen.contains_key(&(cond.clone(), section)) {
            continue;
        }
        node(&mut map, &cond, 0, true);
        seen.insert((cond.clone(), section), ());
        queue.push((cond, section, 0));
    }

    let transitions: Vec<&Expectation> = suite.transitions().collect();
    let companions = companions(&transitions);
    let mut i = 0;
    while i < queue.len() {
        let (cond, section, level) = queue[i].clone();
        let scope = &scopes[&section];
        let from = node(&mut map, &cond, level, false);
        for (t, e) in transitions.iter().enumerate() {
            if !scope[e.id.0 as usize] || !cond.overlaps(&e.given) {
                continue;
            }
            let pre = cond.and(&e.given);
            let out = perform_among(suite, prec, &pre, &e.trigger, companions[t].iter().map(|&c| transitions[c]));
            if !out.fired.contains(&e.id) || !out.after.is_satisfiable() {
                continue;
            }
            let to = node(&mut map, &out.after, level + 1, false);
            let m = *moves.entry((from, to, e.trigger.clone())).or_insert_with(|| {
                map.moves.push(Move { from, to, action: e.trigger.clone(), fired: vec![] });
                map.moves.len() - 1
            });
            for f in out.fired.iter().filter(|f| scope[f.0 as usize]) {
                if !map.moves[m].fired.contains(f) {
                    map.moves[m].fired.push(*f);
                }
            }
            if seen.contains_key(&(out.after.clone(), section)) {
                continue;
            }
            if level + 1 > limits.depth || queue.len() >= limits.states {
                map.truncated = true;
                continue;
            }
            seen.insert((out.after.clone(), section), ());
            queue.push((out.after, section, level + 1));
        }
        i += 1;
    }
    map
}

impl StateMap {
    /// The map's state that a full state `c` (a test step's, say) is in: the one that says
    /// exactly what `c` does about the `by` states, or else the most specific one that holds.
    pub fn locate(&self, c: &Condition) -> Option<usize> {
        let key = c.restrict(self.by.iter().copied());
        if let Some(i) = self.states.iter().position(|s| s.cond == key) {
            return Some(i);
        }
        (0..self.states.len())
            .filter(|&i| key.is_satisfiable() && key.implies(&self.states[i].cond))
            .max_by_key(|&i| (self.states[i].cond.vars().count(), std::cmp::Reverse(i)))
    }

    /// State `i` as facts, one per `by` state it pins down: "the kettle is heating".
    pub fn facts(&self, suite: &Suite, i: usize) -> Vec<String> {
        suite.vocabulary.facts(&self.states[i].cond)
    }

    /// Short names for each state's values only ("heating · off"), for a compact label.
    pub fn label(&self, suite: &Suite, i: usize) -> String {
        let c = &self.states[i].cond;
        if c.is_unconstrained() {
            return "anything".into();
        }
        c.iter()
            .map(|(v, k)| {
                let p = suite.vocabulary.var(v).predicate(k);
                ["is ", "are ", "has ", "have "].iter().find_map(|w| p.strip_prefix(w)).unwrap_or(&p).to_string()
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn moves_from(&self, i: usize) -> impl Iterator<Item = &Move> {
        self.moves.iter().filter(move |m| m.from == i)
    }

    pub fn moves_into(&self, i: usize) -> impl Iterator<Item = &Move> {
        self.moves.iter().filter(move |m| m.to == i && m.from != i)
    }
}

fn quoted(s: &str) -> String {
    s.replace('"', "#quot;").replace(':', "#58;")
}

/// The map as a Mermaid state diagram, which GitHub renders in Markdown.
pub fn mermaid(suite: &Suite, map: &StateMap) -> String {
    let mut s = String::from("stateDiagram-v2\n  direction LR\n");
    for i in 0..map.states.len() {
        let _ = writeln!(s, "  s{i} : {}", quoted(&map.facts(suite, i).join("<br/>")));
    }
    for (i, st) in map.states.iter().enumerate() {
        if st.start {
            let _ = writeln!(s, "  [*] --> s{i}");
        }
    }
    for m in &map.moves {
        let _ = writeln!(s, "  s{} --> s{} : {}", m.from, m.to, quoted(&suite.describe_trigger(&m.action)));
    }
    s
}

/// The map as Graphviz DOT.
pub fn dot(suite: &Suite, map: &StateMap) -> String {
    let q = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut s = String::from("digraph states {\n  rankdir=LR;\n  node [shape=box, style=rounded];\n");
    for (i, st) in map.states.iter().enumerate() {
        let periph = if st.start { ", peripheries=2" } else { "" };
        let _ = writeln!(s, "  s{i} [label=\"{}\"{periph}];", map.facts(suite, i).iter().map(|f| q(f)).collect::<Vec<_>>().join("\\n"));
    }
    for m in &map.moves {
        let _ = writeln!(s, "  s{} -> s{} [label=\"{}\"];", m.from, m.to, q(&suite.describe_trigger(&m.action)));
    }
    s.push_str("}\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUITE: &str = "
Vocabulary:
  State: the screen is home | settings | about
  State: the volume is 0..10
  Event: the user opens settings
  Event: the user opens about
  Event: the user goes back
  Event: the user turns it up

Background:
  Given the screen is home
  And the volume is 0

Feature: Navigation
  Expectation: Settings opens from home
    Given the screen is home
    When the user opens settings
    Then the screen is settings

  Expectation: About opens from settings
    Given the screen is settings
    When the user opens about
    Then the screen is about

  Expectation: Back from about goes to settings
    Given the screen is about
    When the user goes back
    Then the screen is settings

  Expectation: Back from settings goes home
    Given the screen is settings
    When the user goes back
    Then the screen is home

  Expectation: Turning it up in settings
    Given the screen is settings
    And the volume is below 10
    When the user turns it up
    Then the volume goes up by 1
";

    fn suite() -> Suite {
        let l = expectation_language::load_sources("t", &[("t.expect", SUITE)]);
        assert!(!l.has_errors(), "{}", l.render_diagnostics());
        l.suite
    }

    #[test]
    fn one_state_per_screen_with_every_way_between() {
        let s = suite();
        let a = crate::analyze(&s);
        let map = explore(&s, &a.precedence, &[], Limits::default());
        // The volume is a number, so it's folded in: three screens, not one per volume.
        assert_eq!(map.states.len(), 3, "{:?}", map.states);
        let at = |name: &str| (0..3).find(|&i| map.label(&s, i) == name).unwrap();
        let (home, settings, about) = (at("home"), at("settings"), at("about"));
        assert!(map.states[home].start && !map.states[about].start);
        assert_eq!(map.states[about].level, 2);
        let has = |a: usize, b: usize| map.moves.iter().any(|m| m.from == a && m.to == b);
        assert!(has(home, settings) && has(settings, about) && has(about, settings) && has(settings, home));
        assert!(!has(home, about));
        // Turning it up changes the volume, which the map doesn't tell apart: it stays in settings.
        let up = map.moves.iter().find(|m| s.describe_trigger(&m.action) == "the user turns it up").unwrap();
        assert!(up.stays() && up.from == settings);
        assert_eq!(map.moves_into(settings).count(), 2);
    }

    #[test]
    fn locates_a_full_state_and_exports() {
        let s = suite();
        let a = crate::analyze(&s);
        let map = explore(&s, &a.precedence, &[], Limits::default());
        let settings = map.locate(&s.background.overlay(&s.expectations[1].given)).unwrap();
        assert_eq!(map.label(&s, settings), "settings");
        let m = mermaid(&s, &map);
        assert!(m.starts_with("stateDiagram-v2") && m.contains("[*] --> s0") && m.contains("the user goes back"));
        assert!(dot(&s, &map).contains("peripheries=2"));
    }

    #[test]
    fn by_picks_what_tells_states_apart() {
        let s = suite();
        let a = crate::analyze(&s);
        let volume = s.vocabulary.find_var("the volume").unwrap().id;
        let map = explore(&s, &a.precedence, &[volume], Limits::default());
        // 0..10 in settings, and the volume never changes anywhere else.
        assert_eq!(map.states.len(), 11);
        assert!(map.states.iter().all(|st| st.cond.vars().all(|v| v == volume)));
    }
}
