//! The state map drawn for the HTML site: the app's states as cards (with a screenshot of the
//! state where a test took one), and an arrow for every way from one to another. A column per
//! number of moves from the start, start on the left.
//!
//! The page holds the states and the moves as data, each string written once, and a small
//! script (the only one on the site) draws a state when it is opened: at first the start
//! states and the states a move from them, then the states that a picked state leads to.
//! Without the script, the panels of the start states are there as plain HTML.

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
            let (Some(path), Some(at)) = (&r.screenshot, map.locate(state)) else {
                continue;
            };
            let passed = r.status == Status::Passed && r.checks.iter().all(|c| c.passed);
            if !out[at].iter().any(|s| s.path == *path) {
                out[at].push(Shot {
                    path: path.clone(),
                    plan: i,
                    passed,
                });
            }
        }
    }
    for s in &mut out {
        s.sort_by_key(|s| !s.passed);
    }
    out
}

/// Each state's column and row: a column per level, and within one, states near the ones
/// they're reached from (two sweeps of the average-neighbour heuristic), so arrows cross less.
fn place(map: &StateMap) -> Vec<(usize, usize)> {
    let n = map.states.len();
    let mut cols: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, s) in map.states.iter().enumerate() {
        cols.entry(s.level).or_default().push(i);
    }
    // Each state's neighbours in other columns, found once.
    let mut near: Vec<Vec<usize>> = vec![vec![]; n];
    for m in map.moves.iter().filter(|m| !m.stays()) {
        if map.states[m.to].level != map.states[m.from].level {
            near[m.from].push(m.to);
            near[m.to].push(m.from);
        }
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
                    let key = if near[i].is_empty() {
                        row[i]
                    } else {
                        near[i].iter().map(|&j| row[j]).sum::<f32>() / near[i].len() as f32
                    };
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

/// A verdict as the data says it: 0 verified, 1 failed, 2 not verified.
fn verdict_code(v: Option<Verdict>) -> u8 {
    match v {
        Some(Verdict::Verified) => 0,
        Some(Verdict::Failed) => 1,
        _ => 2,
    }
}

/// A JSON string, safe inside a `<script>`.
fn js(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '<' => o.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Strings written once and referred to by number.
#[derive(Default)]
struct Table {
    index: std::collections::HashMap<String, usize>,
    items: Vec<String>,
}

impl Table {
    fn id(&mut self, item: String) -> usize {
        if let Some(&i) = self.index.get(&item) {
            return i;
        }
        self.items.push(item.clone());
        self.index.insert(item, self.items.len() - 1);
        self.items.len() - 1
    }

    fn json(&self) -> String {
        format!("[{}]", self.items.join(","))
    }
}

fn list(items: impl Iterator<Item = usize>) -> String {
    let v: Vec<String> = items.map(|i| i.to_string()).collect();
    format!("[{}]", v.join(","))
}

/// The map. The page holds the states and the moves as data: each fact, action, rule and test written
/// once, and states and moves as numbers. The script draws a state's card, arrows and panel
/// when it is opened, so a map of thousands of states stays a small page. `exp_link` is an
/// expectation's link, `test_link` a test's.
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
    let cond_facts: Vec<Vec<(String, String)>> = (0..n)
        .map(|i| {
            let c = &map.states[i].cond;
            c.iter()
                .map(|(v, k)| {
                    let var = s.vocabulary.var(v);
                    let p = var.predicate(k);
                    let part = ["is ", "are ", "has ", "have "]
                        .iter()
                        .find_map(|w| p.strip_prefix(w))
                        .unwrap_or(&p);
                    (var.describe(k), part.to_string())
                })
                .collect()
        })
        .collect();
    let lines = cond_facts.iter().map(|f| f.len().max(1)).max().unwrap_or(1) as f32;
    // The picture is as wide as the card less its margins; its height follows the ratio.
    let thumb = d
        .look
        .shot_ratio
        .map_or(THUMB, |r| ((W - 16.0) / r).round());
    let h = if with_shots { thumb + 10.0 } else { 0.0 } + 14.0 + lines * LINE + 8.0;
    let at = place(map);

    // Moves from each state, found once.
    let mut from: Vec<Vec<usize>> = vec![vec![]; n];
    for (k, m) in map.moves.iter().enumerate() {
        from[m.from].push(k);
    }
    let label = |i: usize| -> String {
        if cond_facts[i].is_empty() {
            "anything".into()
        } else {
            cond_facts[i]
                .iter()
                .map(|(_, p)| p.as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        }
    };
    let action = |k: usize| map.describe_move(s, &map.moves[k]);

    // The data.
    let (mut facts, mut actions, mut exps, mut tests, mut paths) = (
        Table::default(),
        Table::default(),
        Table::default(),
        Table::default(),
        Table::default(),
    );
    let mut states_json = Vec::with_capacity(n);
    for i in 0..n {
        let st = &map.states[i];
        let fs = list(
            cond_facts[i]
                .iter()
                .map(|(fact, part)| facts.id(format!("[{},{}]", js(fact), js(part)))),
        );
        let (shot, plan) = match shots[i].first() {
            Some(sh) => (
                paths.id(js(&sh.path)) as i64,
                tests.id(js(&test_link(sh.plan))) as i64,
            ),
            None => (-1, -1),
        };
        states_json.push(format!(
            "[{},{},{},{},{fs},{shot},{plan}]",
            st.level, st.start as u8, at[i].0, at[i].1
        ));
    }
    let mut moves_json = Vec::with_capacity(map.moves.len());
    for (k, m) in map.moves.iter().enumerate() {
        let a = actions.id(js(&action(k)));
        let es = list(m.fired.iter().map(|&e| {
            exps.id(format!(
                "[{},{}]",
                js(&exp_link(e)),
                verdict_code(f.verdict(e))
            ))
        }));
        moves_json.push(format!("[{},{},{a},{es}]", m.from, m.to));
    }

    let mut o = String::new();
    let _ = write!(
        o,
        r#"<div class="states" id="states"><div class="states-view"><div class="states-tools" hidden><button type="button" data-zoom="1.25" aria-label="Zoom in">+</button><button type="button" data-zoom="0.8" aria-label="Zoom out">−</button><button type="button" data-zoom="fit">Fit</button><button type="button" data-fold="near">Fold</button></div><svg class="statemap" role="img" aria-label="Map of the states"><defs><marker id="st-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z"/></marker></defs><g class="moves"></g><g class="cards"></g></svg></div>"#
    );

    // Without the script: the states the map shows at first, as panels.
    o.push_str(r#"<div class="states-panels"><p class="states-hint">Pick a state on the map to see what can happen there.</p><noscript><p>The map needs JavaScript. Here are the start states and where they lead.</p>"#);
    let state_link = |j: usize| {
        if map.states[j].start {
            format!(r##"<a href="#state-{j}">{}</a>"##, esc(&label(j)))
        } else {
            esc(&label(j))
        }
    };
    let by_exps = |fired: &[ExpId]| -> String {
        let links: Vec<String> = fired.iter().map(|&e| exp_link(e)).collect();
        if links.is_empty() {
            String::new()
        } else {
            format!(r#"<small>{}</small>"#, links.join(", "))
        }
    };
    for i in (0..n).filter(|&i| map.states[i].start) {
        let _ = write!(
            o,
            r#"<section class="state-panel static" id="state-{i}"><h3>{}</h3>"#,
            esc(&label(i))
        );
        o.push_str(r#"<p><span class="badge ok">start</span></p>"#);
        if !cond_facts[i].is_empty() {
            let items: Vec<String> = cond_facts[i]
                .iter()
                .map(|(f, _)| format!("<li>{}</li>", esc(f)))
                .collect();
            let _ = write!(o, r#"<ul class="facts">{}</ul>"#, items.join(""));
        }
        let mut out = vec![];
        let mut stays = vec![];
        for &k in &from[i] {
            let m = &map.moves[k];
            if m.stays() {
                stays.push(format!(
                    r#"<li><span class="act">{}</span> {}</li>"#,
                    esc(&action(k)),
                    by_exps(&m.fired)
                ));
            } else {
                out.push(format!(
                    r#"<li><span class="act">{}</span> → {} {}</li>"#,
                    esc(&action(k)),
                    state_link(m.to),
                    by_exps(&m.fired)
                ));
            }
        }
        for (title, items) in [("From here", out), ("Stays here", stays)] {
            if !items.is_empty() {
                let _ = write!(
                    o,
                    r#"<h4>{title}</h4><ul class="moves">{}</ul>"#,
                    items.join("")
                );
            }
        }
        o.push_str("</section>");
    }
    o.push_str(r#"</noscript><section class="state-panel live" hidden></section></div></div>"#);

    let _ = write!(
        o,
        r#"<script type="application/json" id="states-data">{{"g":[{W},{h:.1},{GAP_X},{GAP_Y},{PAD},{thumb},{},{CHARS}],"f":{},"a":{},"e":{},"t":{},"p":{},"s":[{}],"m":[{}]}}</script>"#,
        with_shots as u8,
        facts.json(),
        actions.json(),
        exps.json(),
        tests.json(),
        paths.json(),
        states_json.join(","),
        moves_json.join(",")
    );
    o.push_str(SCRIPT);
    o
}

/// Draws the map from the data: the start states and the states a move from them at first;
/// picking a state opens the states it leads to. Cards are laid out again for what's open.
const SCRIPT: &str = r##"<script>
(() => {
  const root = document.getElementById("states");
  const D = JSON.parse(document.getElementById("states-data").textContent);
  const [W, H, GX, GY, PAD, THUMB, SHOTS, CHARS] = D.g;
  const NS = "http://www.w3.org/2000/svg";
  const svg = root.querySelector("svg.statemap");
  const cards = svg.querySelector(".cards"), arrows = svg.querySelector(".moves");
  const panel = root.querySelector(".state-panel.live");
  root.classList.add("js");
  root.querySelector(".states-tools").hidden = false;

  const N = D.s.length;
  const level = (i) => D.s[i][0];
  const outs = Array.from({ length: N }, () => []), ins = Array.from({ length: N }, () => []);
  D.m.forEach((m, k) => { outs[m[0]].push(k); if (m[1] !== m[0]) ins[m[1]].push(k); });
  const label = (i) => D.s[i][4].length ? D.s[i][4].map((f) => D.f[f][1]).join(" · ") : "anything";
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const short = (s) => [...s].length <= CHARS ? s : [...s].slice(0, CHARS - 1).join("").trimEnd() + "…";
  const el = (tag, attrs, html) => {
    const e = document.createElementNS(NS, tag);
    for (const k in attrs) e.setAttribute(k, attrs[k]);
    if (html) e.innerHTML = html;
    return e;
  };

  // Cards, drawn the first time they open, at 0 0 and moved into place.
  const drawn = new Map();
  const card = (i) => {
    if (drawn.has(i)) return drawn.get(i);
    const st = D.s[i];
    const facts = st[4].map((f) => D.f[f][0]);
    let inner = `<title>${esc(facts.join("\n") || "anything")}</title><rect x="0" y="0" width="${W}" height="${H}" rx="10"/>`;
    let ty = 8;
    if (SHOTS) {
      inner += st[5] >= 0
        ? `<image href="../${esc(D.p[st[5]])}" x="8" y="${ty}" width="${W - 16}" height="${THUMB}" preserveAspectRatio="xMidYMid slice"/>`
        : `<rect class="noshot" x="8" y="${ty}" width="${W - 16}" height="${THUMB}" rx="6"/><text class="hint" x="${W / 2}" y="${ty + THUMB / 2 + 4}">no screenshot</text>`;
      ty += THUMB + 10;
    }
    (facts.length ? facts.map(short) : ["anything"]).forEach((line, k) => {
      inner += `<text x="12" y="${ty + 12 + k * 16}">${esc(line)}</text>`;
    });
    inner += `<g class="more"><rect x="${W - 30}" y="-10" width="40" height="20" rx="10"/><text x="${W - 10}" y="4"></text></g>`;
    const a = el("a", { href: `#state-${i}`, "data-state": i }, `<g class="state${st[1] ? " start" : ""}">${inner}</g>`);
    cards.appendChild(a);
    const n = { a, more: a.querySelector(".more"), x: 0, y: 0 };
    drawn.set(i, n);
    return n;
  };

  const near = () => new Set(D.s.map((s, i) => i).filter((i) => level(i) <= 1));
  let open = near();
  const reveal = (i) => {
    while (i !== undefined && !open.has(i)) {
      open.add(i);
      const k = ins[i].find((k) => level(D.m[k][0]) === level(i) - 1);
      i = k === undefined ? undefined : D.m[k][0];
    }
  };

  const path = (a, b, ca, cb) => {
    if (cb > ca) {
      const [x1, y1, x2, y2] = [a.x + W, a.y + H / 2, b.x, b.y + H / 2];
      const mid = (x1 + x2) / 2;
      return `M${x1} ${y1}C${mid} ${y1} ${mid} ${y2} ${x2} ${y2}`;
    }
    if (cb < ca) {
      const [x1, y1, x2, y2] = [a.x + W * 0.4, a.y + H, b.x + W * 0.6, b.y + H];
      const dip = Math.max(y1, y2) + 30 + Math.abs(x1 - x2) * 0.08;
      return `M${x1} ${y1}C${x1} ${dip} ${x2} ${dip} ${x2} ${y2}`;
    }
    const [x, y1, y2] = [a.x + W, a.y + H * 0.35, b.y + H * 0.65];
    const bulge = x + 36 + Math.abs(y1 - y2) * 0.12;
    return `M${x} ${y1}C${bulge} ${y1} ${bulge} ${y2} ${x} ${y2}`;
  };

  // What's open, in columns by level with the order the page worked out kept.
  let home = { x: 0, y: 0, w: 1, h: 1 };
  const layout = () => {
    const cols = new Map();
    open.forEach((i) => { const c = D.s[i][2]; (cols.get(c) ?? cols.set(c, []).get(c)).push(i); });
    const order = [...cols.keys()].sort((a, b) => a - b);
    let rows = 1;
    order.forEach((c, ci) => {
      const col = cols.get(c).sort((a, b) => D.s[a][3] - D.s[b][3]);
      rows = Math.max(rows, col.length);
      col.forEach((i, ri) => { const n = card(i); n.x = PAD + ci * (W + GX); n.y = PAD + ri * (H + GY); });
    });
    drawn.forEach((n, i) => {
      const shown = open.has(i);
      n.a.style.display = shown ? "" : "none";
      if (!shown) return;
      n.a.setAttribute("transform", `translate(${n.x} ${n.y})`);
      const folded = new Set(outs[i].map((k) => D.m[k][1]).filter((t) => t !== i && !open.has(t))).size;
      n.more.style.display = folded ? "" : "none";
      n.more.querySelector("text").textContent = `+${folded}`;
    });
    // One arrow for each pair of open states, coloured by the worst verdict behind it.
    const pairs = new Map();
    open.forEach((i) => outs[i].forEach((k) => {
      const t = D.m[k][1];
      if (t === i || !open.has(t)) return;
      const key = `${i} ${t}`;
      (pairs.get(key) ?? pairs.set(key, []).get(key)).push(k);
    }));
    arrows.replaceChildren();
    pairs.forEach((ks, key) => {
      const [a, b] = key.split(" ").map(Number);
      const v = ks.flatMap((k) => D.m[k][3].map((e) => D.e[e][1]));
      const cls = v.includes(1) ? "v-bad" : v.length && v.every((x) => x === 0) ? "v-ok" : "v-idle";
      const p = el("path", { class: `move ${cls}`, "data-from": a, "data-to": b, d: path(drawn.get(a), drawn.get(b), D.s[a][2], D.s[b][2]), "marker-end": "url(#st-arrow)" },
        `<title>${esc(ks.map((k) => D.a[D.m[k][2]]).join("; "))}</title>`);
      arrows.appendChild(p);
    });
    home = { x: 0, y: 0, w: PAD * 2 + order.length * (W + GX) - GX + 40, h: PAD * 2 + rows * (H + GY) - GY + 60 };
  };

  // The view keeps the shape of the map's box, so a tall map is never squeezed into a strip.
  let view = { x: 0, y: 0, w: 1, h: 1 };
  const aspect = () => { const r = svg.getBoundingClientRect(); return r.width ? r.height / r.width : 0.6; };
  const draw = () => { view.h = view.w * aspect(); svg.setAttribute("viewBox", `${view.x} ${view.y} ${view.w} ${view.h}`); };
  const fit = () => { view = { x: 0, y: 0, w: Math.max(home.w, home.h / aspect()) }; draw(); };
  // Readable: about three columns across, from the top left.
  const start = () => { view = { x: 0, y: 0, w: Math.min(Math.max(home.w, 2 * (W + GX)), 3 * (W + GX) + PAD) }; draw(); };
  // Bring state `i` into view, keeping the zoom.
  const show = (i) => {
    const n = drawn.get(i);
    if (n.x >= view.x && n.y >= view.y && n.x + W <= view.x + view.w && n.y + H <= view.y + view.h) return;
    view = { ...view, x: n.x + W / 2 - view.w / 2, y: n.y + H / 2 - view.h / 2 };
    draw();
  };
  const zoom = (k, cx = view.x + view.w / 2, cy = view.y + view.h / 2) => {
    const w = Math.min(Math.max(view.w / k, 120), Math.max(home.w, home.h / aspect()) * 2);
    const s = w / view.w;
    view = { x: cx - (cx - view.x) * s, y: cy - (cy - view.y) * s, w };
    draw();
  };
  window.addEventListener("resize", draw);
  const point = (e) => {
    const r = svg.getBoundingClientRect();
    return { x: view.x + (e.clientX - r.left) / r.width * view.w, y: view.y + (e.clientY - r.top) / r.height * view.h };
  };
  svg.addEventListener("wheel", (e) => { e.preventDefault(); const p = point(e); zoom(Math.exp(-e.deltaY / 300), p.x, p.y); }, { passive: false });
  let drag = null;
  svg.addEventListener("pointerdown", (e) => { if (e.button === 0) drag = { x: e.clientX, y: e.clientY, view: { ...view }, moved: false }; });
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
  window.addEventListener("pointerup", () => { svg.classList.remove("dragging"); setTimeout(() => { drag = null; }); });

  // The picked state's panel, written when it's picked.
  const link = (j) => `<a href="#state-${j}" data-state="${j}">${esc(label(j))}</a>`;
  const rules = (k) => D.m[k][3].length ? `<small>${D.m[k][3].map((e) => D.e[e][0]).join(", ")}</small>` : "";
  const section = (title, items) => items.length ? `<h4>${title}</h4><ul class="moves">${items.join("")}</ul>` : "";
  const showPanel = (i) => {
    const st = D.s[i];
    const lv = st[0] === 1 ? "1 move" : `${st[0]} moves`;
    let html = `<h3>${esc(label(i))}</h3><p>${st[1] ? '<span class="badge ok">start</span>' : `<span class="badge idle">${lv} from the start</span>`}</p>`;
    if (st[4].length) html += `<ul class="facts">${st[4].map((f) => `<li>${esc(D.f[f][0])}</li>`).join("")}</ul>`;
    if (st[5] >= 0) {
      const p = esc(D.p[st[5]]);
      html += `<figure><a href="../${p}"><img src="../${p}" alt="${esc(label(i))}" loading="lazy"></a><figcaption>From ${D.t[st[6]]}</figcaption></figure>`;
    }
    const act = (k) => `<span class="act">${esc(D.a[D.m[k][2]])}</span>`;
    html += section("From here", outs[i].filter((k) => D.m[k][1] !== i).map((k) => `<li>${act(k)} → ${link(D.m[k][1])} ${rules(k)}</li>`));
    html += section("Stays here", outs[i].filter((k) => D.m[k][1] === i).map((k) => `<li>${act(k)} ${rules(k)}</li>`));
    html += section("Ways in", ins[i].map((k) => `<li>${link(D.m[k][0])} → ${act(k)}</li>`));
    panel.innerHTML = html;
    panel.hidden = false;
    panel.classList.add("shown");
  };

  let picked = null;
  const pick = (i) => {
    picked = i;
    svg.classList.toggle("picked", i !== null);
    if (i === null || !(i >= 0 && i < N)) { panel.hidden = true; panel.classList.remove("shown"); layout(); return; }
    reveal(i);
    outs[i].forEach((k) => open.add(D.m[k][1]));
    layout();
    show(i);
    showPanel(i);
    drawn.forEach((n) => n.a.classList.remove("sel", "near"));
    drawn.get(i).a.classList.add("sel");
    arrows.querySelectorAll(".move").forEach((m) => {
      if (Number(m.dataset.from) === i) { m.classList.add("out"); drawn.get(Number(m.dataset.to)).a.classList.add("near"); }
      if (Number(m.dataset.to) === i) { m.classList.add("in"); drawn.get(Number(m.dataset.from)).a.classList.add("near"); }
    });
  };
  root.querySelectorAll("[data-zoom]").forEach((b) => b.addEventListener("click", () => {
    if (b.dataset.zoom === "fit") fit(); else zoom(Number(b.dataset.zoom));
  }));
  root.querySelector("[data-fold]").addEventListener("click", () => {
    open = near();
    if (picked !== null) reveal(picked);
    layout();
    start();
    if (picked !== null) pick(picked);
  });
  root.addEventListener("click", (e) => {
    const a = e.target.closest("a[data-state]");
    if (!a) return;
    e.preventDefault();
    if (drag && drag.moved) return;
    history.replaceState(null, "", `#state-${a.dataset.state}`);
    pick(Number(a.dataset.state));
  });
  layout();
  start();
  const fromHash = () => { const m = location.hash.match(/^#state-(\d+)$/); pick(m ? Number(m[1]) : null); };
  window.addEventListener("hashchange", fromHash);
  fromHash();
})();
</script>"##;

#[cfg(test)]
mod tests {
    use super::{Table, js};

    #[test]
    fn strings_are_safe_inside_a_script() {
        assert_eq!(
            js(r#"say "hi" </script>"#),
            r#""say \"hi\" \u003c/script>""#
        );
        assert_eq!(js("a\\b\nc"), r#""a\\b\nc""#);
    }

    #[test]
    fn a_string_is_written_once() {
        let mut t = Table::default();
        assert_eq!(t.id(js("on")), 0);
        assert_eq!(t.id(js("off")), 1);
        assert_eq!(t.id(js("on")), 0);
        assert_eq!(t.json(), r#"["on","off"]"#);
    }
}
