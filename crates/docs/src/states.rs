//! The state map drawn for the HTML site: the app's states as cards (with a screenshot of the
//! state where a test took one), and an arrow for every way from one to another. A column per
//! number of moves from the start, start on the left.
//!
//! It works as plain HTML: each card links to a panel below the map (shown with `:target`)
//! that says what the state is, what can be done there and where it leads. A small script,
//! the only one on the site, adds panning and zooming and highlights the picked state's
//! arrows without leaving the map.

use crate::html::esc;
use crate::{Docs, Facts};
use expectation_graph::StateMap;
use expectation_model::ExpId;
use expectation_story::{Status, Verdict};
use std::collections::BTreeMap;
use std::fmt::Write;

const W: f32 = 220.0;
const LINE: f32 = 16.0;
const THUMB: f32 = 124.0;
const GAP_X: f32 = 90.0;
const GAP_Y: f32 = 28.0;
const PAD: f32 = 24.0;
/// A fact longer than this is cut short on its card (it's whole in the panel).
const CHARS: usize = 30;

/// A screenshot of a map state, from a test step that was in it.
struct Shot {
    path: String,
    plan: usize,
    /// The step's checks passed.
    passed: bool,
}

/// Per state, the screenshots tests took in it, the ones whose checks passed first.
fn shots(d: &Docs, map: &StateMap) -> Vec<Vec<Shot>> {
    let mut out: Vec<Vec<Shot>> = (0..map.states.len()).map(|_| vec![]).collect();
    let Some(report) = d.report else { return out };
    for (i, (plan, result)) in d.plans.iter().zip(&report.results).enumerate() {
        let steps = std::iter::once((&plan.start, &result.start))
            .chain(plan.steps.iter().map(|s| &s.state).zip(&result.steps));
        for (state, r) in steps {
            let (Some(path), Some(at)) = (&r.screenshot, map.locate(state)) else { continue };
            let passed = r.status == Status::Passed && r.checks.iter().all(|c| c.passed);
            if !out[at].iter().any(|s| s.path == *path) {
                out[at].push(Shot { path: path.clone(), plan: i, passed });
            }
        }
    }
    for s in &mut out {
        s.sort_by_key(|s| !s.passed);
    }
    out
}

fn short(s: &str) -> String {
    if s.chars().count() <= CHARS {
        return s.to_string();
    }
    let cut: String = s.chars().take(CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Each state's column and row: a column per level, and within one, states near the ones
/// they're reached from (two sweeps of the average-neighbour heuristic), so arrows cross less.
fn place(map: &StateMap) -> Vec<(usize, usize)> {
    let n = map.states.len();
    let mut cols: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, s) in map.states.iter().enumerate() {
        cols.entry(s.level).or_default().push(i);
    }
    let mut row = vec![0f32; n];
    for col in cols.values() {
        for (r, &i) in col.iter().enumerate() {
            row[i] = r as f32;
        }
    }
    for _ in 0..2 {
        for col in cols.values_mut() {
            let mut keyed: Vec<(f32, usize)> = col
                .iter()
                .map(|&i| {
                    let near: Vec<f32> = map
                        .moves
                        .iter()
                        .filter(|m| !m.stays())
                        .filter_map(|m| match (m.from == i, m.to == i) {
                            (true, _) if map.states[m.to].level != map.states[i].level => Some(row[m.to]),
                            (_, true) if map.states[m.from].level != map.states[i].level => Some(row[m.from]),
                            _ => None,
                        })
                        .collect();
                    let key = if near.is_empty() { row[i] } else { near.iter().sum::<f32>() / near.len() as f32 };
                    (key, i)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            *col = keyed.into_iter().map(|(_, i)| i).collect();
            for (r, &i) in col.iter().enumerate() {
                row[i] = r as f32;
            }
        }
    }
    let mut at = vec![(0, 0); n];
    for (c, col) in cols.values().enumerate() {
        for (r, &i) in col.iter().enumerate() {
            at[i] = (c, r);
        }
    }
    at
}

/// The worst verdict among `exps`, as a class for their arrow.
fn verdict_class(f: &Facts, exps: &[ExpId]) -> &'static str {
    let v: Vec<Option<Verdict>> = exps.iter().map(|&e| f.verdict(e)).collect();
    if v.contains(&Some(Verdict::Failed)) {
        "v-bad"
    } else if !v.is_empty() && v.iter().all(|v| *v == Some(Verdict::Verified)) {
        "v-ok"
    } else {
        "v-idle"
    }
}

/// The map and its panels. `exp_link` is an expectation's link, `test_link` a test's.
pub fn html(
    d: &Docs,
    f: &Facts,
    map: &StateMap,
    exp_link: &dyn Fn(ExpId) -> String,
    test_link: &dyn Fn(usize) -> String,
) -> String {
    let s = d.suite;
    let n = map.states.len();
    if n == 0 {
        return "<p>Nothing to map: no section has a setup to start from.</p>".into();
    }
    let shots = shots(d, map);
    let with_shots = shots.iter().any(|s| !s.is_empty());
    let facts: Vec<Vec<String>> = (0..n).map(|i| map.facts(s, i)).collect();
    let lines = facts.iter().map(|f| f.len().max(1)).max().unwrap_or(1) as f32;
    let h = if with_shots { THUMB + 10.0 } else { 0.0 } + 14.0 + lines * LINE + 8.0;
    let at = place(map);
    let cols = at.iter().map(|a| a.0).max().unwrap_or(0) + 1;
    let tallest = at.iter().map(|a| a.1).max().unwrap_or(0) + 1;
    let xy = |i: usize| (PAD + at[i].0 as f32 * (W + GAP_X), PAD + at[i].1 as f32 * (h + GAP_Y));
    // Room above and below for the arrows that go back.
    let width = PAD * 2.0 + cols as f32 * (W + GAP_X) - GAP_X;
    let height = PAD * 2.0 + tallest as f32 * (h + GAP_Y) - GAP_Y + 60.0;

    let mut o = String::new();
    let _ = write!(
        o,
        r#"<div class="states" id="states"><div class="states-view"><div class="states-tools" hidden><button type="button" data-zoom="1.25" aria-label="Zoom in">+</button><button type="button" data-zoom="0.8" aria-label="Zoom out">−</button><button type="button" data-zoom="fit">Fit</button><button type="button" data-fold="all">Show all</button><button type="button" data-fold="near">Fold</button></div><svg class="statemap" viewBox="0 0 {width:.0} {height:.0}" width="{width:.0}" height="{height:.0}" data-geom="{W} {h:.1} {GAP_X} {GAP_Y} {PAD}" role="img" aria-label="Map of the states"><defs><marker id="st-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z"/></marker></defs><g class="moves">"#
    );

    // One arrow per pair of states, whatever the actions; the panel lists them.
    let mut pairs: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (k, m) in map.moves.iter().enumerate() {
        if !m.stays() {
            pairs.entry((m.from, m.to)).or_default().push(k);
        }
    }
    for (&(a, b), ms) in &pairs {
        let ((ax, ay), (bx, by)) = (xy(a), xy(b));
        let path = if at[b].0 > at[a].0 {
            // Forward: right side to left side.
            let (x1, y1, x2, y2) = (ax + W, ay + h / 2.0, bx, by + h / 2.0);
            let mid = (x1 + x2) / 2.0;
            format!("M{x1:.1} {y1:.1}C{mid:.1} {y1:.1} {mid:.1} {y2:.1} {x2:.1} {y2:.1}")
        } else if at[b].0 < at[a].0 {
            // Back: bottom to bottom, dipping below.
            let (x1, y1, x2, y2) = (ax + W * 0.4, ay + h, bx + W * 0.6, by + h);
            let dip = y1.max(y2) + 30.0 + (x1 - x2).abs() * 0.08;
            format!("M{x1:.1} {y1:.1}C{x1:.1} {dip:.1} {x2:.1} {dip:.1} {x2:.1} {y2:.1}")
        } else {
            // Same column: out of the right side and back in, bulging right.
            let (x, y1, y2) = (ax + W, ay + h * 0.35, by + h * 0.65);
            let bulge = x + 36.0 + (y1 - y2).abs() * 0.12;
            format!("M{x:.1} {y1:.1}C{bulge:.1} {y1:.1} {bulge:.1} {y2:.1} {x:.1} {y2:.1}")
        };
        let fired: Vec<ExpId> = ms.iter().flat_map(|&k| map.moves[k].fired.clone()).collect();
        let actions: Vec<String> = ms.iter().map(|&k| s.describe_trigger(&map.moves[k].action)).collect();
        let _ = write!(
            o,
            r#"<path class="move {}" data-from="{a}" data-to="{b}" d="{path}" marker-end="url(#st-arrow)"><title>{}</title></path>"#,
            verdict_class(f, &fired),
            esc(&actions.join("; "))
        );
    }
    o.push_str("</g>");

    for i in 0..n {
        let (x, y) = xy(i);
        let st = &map.states[i];
        let mut class = String::from("state");
        if st.start {
            class.push_str(" start");
        }
        let label = map.label(s, i);
        let _ = write!(
            o,
            r##"<a href="#state-{i}" data-state="{i}" data-cell="{} {} {x:.1} {y:.1}" data-level="{}"><g class="{class}"><title>{}</title><rect x="{x:.1}" y="{y:.1}" width="{W}" height="{h:.1}" rx="10"/>"##,
            at[i].0,
            at[i].1,
            st.level,
            esc(&facts[i].join("\n"))
        );
        let mut ty = y + 8.0;
        if with_shots {
            match shots[i].first() {
                Some(shot) => {
                    let _ = write!(
                        o,
                        r#"<image href="../{}" x="{:.1}" y="{ty:.1}" width="{:.1}" height="{THUMB}" preserveAspectRatio="xMidYMid slice"/>"#,
                        esc(&shot.path),
                        x + 8.0,
                        W - 16.0
                    );
                }
                None => {
                    let _ = write!(
                        o,
                        r#"<rect class="noshot" x="{:.1}" y="{ty:.1}" width="{:.1}" height="{THUMB}" rx="6"/><text class="hint" x="{:.1}" y="{:.1}">no screenshot</text>"#,
                        x + 8.0,
                        W - 16.0,
                        x + W / 2.0,
                        ty + THUMB / 2.0 + 4.0
                    );
                }
            }
            ty += THUMB + 10.0;
        }
        let shown: Vec<String> = if facts[i].is_empty() { vec![label] } else { facts[i].iter().map(|f| short(f)).collect() };
        for (k, line) in shown.iter().enumerate() {
            let _ = write!(o, r#"<text x="{:.1}" y="{:.1}">{}</text>"#, x + 12.0, ty + 12.0 + k as f32 * LINE, esc(line));
        }
        o.push_str("</g></a>");
    }
    o.push_str("</svg></div>");

    // The panels.
    o.push_str(r#"<div class="states-panels"><p class="states-hint">Pick a state on the map to see what can happen there.</p>"#);
    let state_link = |j: usize| format!(r##"<a href="#state-{j}" data-state="{j}">{}</a>"##, esc(&map.label(s, j)));
    let by_exps = |fired: &[ExpId]| -> String {
        let links: Vec<String> = fired.iter().map(|&e| exp_link(e)).collect();
        if links.is_empty() { String::new() } else { format!(r#"<small>{}</small>"#, links.join(", ")) }
    };
    for i in 0..n {
        let st = &map.states[i];
        let _ = write!(o, r#"<section class="state-panel" id="state-{i}"><h3>{}</h3>"#, esc(&map.label(s, i)));
        let mut tags = vec![];
        if st.start {
            tags.push(r#"<span class="badge ok">start</span>"#.to_string());
        } else {
            let moves = if st.level == 1 { "1 move".to_string() } else { format!("{} moves", st.level) };
            tags.push(format!(r#"<span class="badge idle">{moves} from the start</span>"#));
        }
        let _ = write!(o, "<p>{}</p>", tags.join(" "));
        if !facts[i].is_empty() {
            let items: Vec<String> = facts[i].iter().map(|f| format!("<li>{}</li>", esc(f))).collect();
            let _ = write!(o, r#"<ul class="facts">{}</ul>"#, items.join(""));
        }
        if let Some(shot) = shots[i].first() {
            let _ = write!(
                o,
                r#"<figure><a href="../{0}"><img src="../{0}" alt="{1}" loading="lazy"></a><figcaption>From {2}</figcaption></figure>"#,
                esc(&shot.path),
                esc(&map.label(s, i)),
                test_link(shot.plan)
            );
        }
        let list = |o: &mut String, title: &str, items: Vec<String>| {
            if !items.is_empty() {
                let _ = write!(o, r#"<h4>{title}</h4><ul class="moves">{}</ul>"#, items.join(""));
            }
        };
        let out: Vec<String> = map
            .moves_from(i)
            .filter(|m| !m.stays())
            .map(|m| {
                format!(
                    r#"<li><span class="act">{}</span> → {} {}</li>"#,
                    esc(&s.describe_trigger(&m.action)),
                    state_link(m.to),
                    by_exps(&m.fired)
                )
            })
            .collect();
        list(&mut o, "From here", out);
        let stays: Vec<String> = map
            .moves_from(i)
            .filter(|m| m.stays())
            .map(|m| format!(r#"<li><span class="act">{}</span> {}</li>"#, esc(&s.describe_trigger(&m.action)), by_exps(&m.fired)))
            .collect();
        list(&mut o, "Stays here", stays);
        let into: Vec<String> = map
            .moves_into(i)
            .map(|m| format!(r#"<li>{} → <span class="act">{}</span></li>"#, state_link(m.from), esc(&s.describe_trigger(&m.action))))
            .collect();
        list(&mut o, "Ways in", into);
        o.push_str("</section>");
    }
    o.push_str("</div></div>");
    o.push_str(SCRIPT);
    o
}

/// Pan, zoom, pick, and fold: only the states a move from the start show at first, and
/// picking one opens the states it leads to. Cards are laid out again for what's showing.
/// Without the script, every state shows, and the links and `:target` still work.
const SCRIPT: &str = r#"<script>
(() => {
  const root = document.getElementById("states");
  const svg = root.querySelector("svg.statemap");
  const [W, H, GX, GY, PAD] = svg.dataset.geom.split(" ").map(Number);
  const NS = "http://www.w3.org/2000/svg";
  root.classList.add("js");
  root.querySelector(".states-tools").hidden = false;
  svg.removeAttribute("width");
  svg.removeAttribute("height");

  // The cards, where the page drew them, and the arrows between them.
  const nodes = new Map();
  svg.querySelectorAll("a[data-state]").forEach((el) => {
    const [col, row, x0, y0] = el.dataset.cell.split(" ").map(Number);
    const more = document.createElementNS(NS, "g");
    more.setAttribute("class", "more");
    more.innerHTML = `<rect x="${x0 + W - 30}" y="${y0 - 10}" width="40" height="20" rx="10"/><text x="${x0 + W - 10}" y="${y0 + 4}"></text>`;
    el.querySelector("g").appendChild(more);
    nodes.set(el.dataset.state, { el, col, row, x0, y0, level: Number(el.dataset.level), x: x0, y: y0, more });
  });
  const moves = [...svg.querySelectorAll(".move")];
  const outs = (id) => [...new Set(moves.filter((m) => m.dataset.from === id).map((m) => m.dataset.to))];
  const near = () => new Set([...nodes].filter(([, n]) => n.level <= 1).map(([id]) => id));
  let open = near();
  if (open.size === nodes.size) root.querySelectorAll("[data-fold]").forEach((b) => b.hidden = true);

  // Open `id`, and a shortest way to it from what's open, so it isn't left on its own.
  const reveal = (id) => {
    while (id !== undefined && !open.has(id)) {
      open.add(id);
      const level = nodes.get(id).level;
      id = moves.find((m) => m.dataset.to === id && nodes.get(m.dataset.from).level === level - 1)?.dataset.from;
    }
  };

  const path = (a, b) => {
    if (b.col > a.col) {
      const [x1, y1, x2, y2] = [a.x + W, a.y + H / 2, b.x, b.y + H / 2];
      const mid = (x1 + x2) / 2;
      return `M${x1} ${y1}C${mid} ${y1} ${mid} ${y2} ${x2} ${y2}`;
    }
    if (b.col < a.col) {
      const [x1, y1, x2, y2] = [a.x + W * 0.4, a.y + H, b.x + W * 0.6, b.y + H];
      const dip = Math.max(y1, y2) + 30 + Math.abs(x1 - x2) * 0.08;
      return `M${x1} ${y1}C${x1} ${dip} ${x2} ${dip} ${x2} ${y2}`;
    }
    const [x, y1, y2] = [a.x + W, a.y + H * 0.35, b.y + H * 0.65];
    const bulge = x + 36 + Math.abs(y1 - y2) * 0.12;
    return `M${x} ${y1}C${bulge} ${y1} ${bulge} ${y2} ${x} ${y2}`;
  };

  // What's open, packed into columns by level with the page's order kept.
  let home = { x: 0, y: 0, w: 1, h: 1 };
  const layout = () => {
    const cols = new Map();
    nodes.forEach((n, id) => {
      if (open.has(id)) (cols.get(n.col) ?? cols.set(n.col, []).get(n.col)).push(n);
    });
    const order = [...cols.keys()].sort((a, b) => a - b);
    let rows = 1;
    order.forEach((c, ci) => {
      const col = cols.get(c).sort((a, b) => a.row - b.row);
      rows = Math.max(rows, col.length);
      col.forEach((n, ri) => { n.x = PAD + ci * (W + GX); n.y = PAD + ri * (H + GY); });
    });
    nodes.forEach((n, id) => {
      const shown = open.has(id);
      n.el.style.display = shown ? "" : "none";
      n.el.setAttribute("transform", `translate(${n.x - n.x0} ${n.y - n.y0})`);
      const folded = outs(id).filter((t) => !open.has(t)).length;
      n.more.style.display = shown && folded ? "" : "none";
      n.more.querySelector("text").textContent = `+${folded}`;
      n.el.setAttribute("aria-label", folded ? `${n.el.querySelector("title").textContent} (${folded} more beyond)` : "");
    });
    moves.forEach((m) => {
      const shown = open.has(m.dataset.from) && open.has(m.dataset.to);
      m.style.display = shown ? "" : "none";
      if (shown) m.setAttribute("d", path(nodes.get(m.dataset.from), nodes.get(m.dataset.to)));
    });
    home = { x: 0, y: 0, w: PAD * 2 + order.length * (W + GX) - GX + 40, h: PAD * 2 + rows * (H + GY) - GY + 60 };
  };

  let view = { x: 0, y: 0, w: 1, h: 1 };
  const draw = () => svg.setAttribute("viewBox", `${view.x} ${view.y} ${view.w} ${view.h}`);
  const fit = () => { view = { ...home }; draw(); };
  // After opening more, fit again if any of it is off screen.
  const fitIfNeeded = () => {
    const out = [...nodes].some(([id, n]) => open.has(id) &&
      (n.x < view.x || n.y < view.y || n.x + W > view.x + view.w || n.y + H > view.y + view.h));
    if (out) fit();
  };
  const zoom = (k, cx = view.x + view.w / 2, cy = view.y + view.h / 2) => {
    const w = Math.min(Math.max(view.w / k, 120), home.w * 4);
    const s = w / view.w;
    view = { x: cx - (cx - view.x) * s, y: cy - (cy - view.y) * s, w, h: view.h * s };
    draw();
  };
  const point = (e) => {
    const r = svg.getBoundingClientRect();
    return { x: view.x + (e.clientX - r.left) / r.width * view.w, y: view.y + (e.clientY - r.top) / r.height * view.h };
  };
  svg.addEventListener("wheel", (e) => {
    e.preventDefault();
    const p = point(e);
    zoom(Math.exp(-e.deltaY / 300), p.x, p.y);
  }, { passive: false });
  let drag = null;
  svg.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    drag = { x: e.clientX, y: e.clientY, view: { ...view }, moved: false };
  });
  window.addEventListener("pointermove", (e) => {
    if (!drag) return;
    const r = svg.getBoundingClientRect();
    const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
    if (Math.abs(dx) + Math.abs(dy) > 4) drag.moved = true;
    if (!drag.moved) return;
    svg.classList.add("dragging");
    view = { ...view, x: drag.view.x - dx / r.width * view.w, y: drag.view.y - dy / r.height * view.h };
    draw();
  });
  window.addEventListener("pointerup", () => {
    svg.classList.remove("dragging");
    setTimeout(() => { drag = null; });
  });

  let picked = null;
  const pick = (i) => {
    picked = i;
    root.querySelectorAll(".state-panel.shown").forEach((p) => p.classList.remove("shown"));
    svg.classList.toggle("picked", i !== null);
    svg.querySelectorAll(".sel,.near,.in,.out").forEach((el) => el.classList.remove("sel", "near", "in", "out"));
    if (i === null || !nodes.has(i)) return;
    reveal(i);
    outs(i).forEach((t) => open.add(t));
    layout();
    fitIfNeeded();
    document.getElementById(`state-${i}`)?.classList.add("shown");
    nodes.get(i).el.classList.add("sel");
    svg.querySelectorAll(`.move[data-from="${i}"]`).forEach((m) => {
      m.classList.add("out");
      nodes.get(m.dataset.to).el.classList.add("near");
    });
    svg.querySelectorAll(`.move[data-to="${i}"]`).forEach((m) => {
      m.classList.add("in");
      nodes.get(m.dataset.from).el.classList.add("near");
    });
  };
  root.querySelectorAll("[data-zoom]").forEach((b) => b.addEventListener("click", () => {
    if (b.dataset.zoom === "fit") fit(); else zoom(Number(b.dataset.zoom));
  }));
  root.querySelectorAll("[data-fold]").forEach((b) => b.addEventListener("click", () => {
    open = b.dataset.fold === "all" ? new Set(nodes.keys()) : near();
    if (picked !== null) reveal(picked);
    layout();
    fit();
  }));
  root.addEventListener("click", (e) => {
    const a = e.target.closest("a[data-state]");
    if (!a) return;
    e.preventDefault();
    if (drag && drag.moved) return;
    history.replaceState(null, "", `#state-${a.dataset.state}`);
    pick(a.dataset.state);
  });
  layout();
  fit();
  const m = location.hash.match(/^#state-(\d+)$/);
  pick(m ? m[1] : null);
})();
</script>"#;

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn long_facts_are_cut_short() {
        assert_eq!(short("the kettle is off"), "the kettle is off");
        let s = short("the selected item in the inspector is the first one");
        assert!(s.ends_with('…') && s.chars().count() <= super::CHARS);
    }
}
