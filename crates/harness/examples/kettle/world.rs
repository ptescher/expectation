//! A toy kettle: the dial has to be held above 55 % for 0.6 s before the kettle starts, starting
//! takes 0.22 s, and while the child lock is on the dial and the buttons don't reach it.
//! Screenshots are SVG.

use expectation_harness::{Bindings, Control, Progress, Screenshot};
use std::fmt::Write;

pub const PRESETS: [&str; 3] = ["WARM", "HOT", "BOIL"];
const TARGET: [f64; 3] = [70.0, 85.0, 100.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Off,
    Starting { t: f64 },
    Heating,
    Dry,
}

#[derive(Clone, Debug)]
pub struct World {
    pub phase: Phase,
    /// The dial, in %.
    pub dial: f64,
    pub preset: usize,
    pub lock: bool,
    /// The water, in °C.
    pub temperature: f64,
    pub time: f64,
    start_hold: f64,
    /// A seeded defect for demos: the child lock lets the dial through.
    pub lock_leaks: bool,
    /// The controls the last actions used, as `Control::key`s.
    pub used: Vec<String>,
}

impl World {
    pub fn new(lock_leaks: bool) -> Self {
        World {
            phase: Phase::Off,
            dial: 0.0,
            preset: 0,
            lock: false,
            temperature: 20.0,
            time: 0.0,
            start_hold: 0.0,
            lock_leaks,
            used: vec![],
        }
    }

    pub fn tick(&mut self, dt: f64) {
        self.time += dt;
        // The dial doesn't reach the element while the child lock is on.
        let dial = if self.lock && !self.lock_leaks {
            0.0
        } else {
            self.dial
        };
        match self.phase {
            Phase::Off if dial > 55.0 => {
                self.start_hold += dt;
                if self.start_hold > 0.6 {
                    self.phase = Phase::Starting { t: 0.0 };
                    self.start_hold = 0.0;
                }
            }
            Phase::Off => self.start_hold = 0.0,
            Phase::Starting { t } => {
                self.phase = if t + dt >= 0.22 {
                    Phase::Heating
                } else {
                    Phase::Starting { t: t + dt }
                };
            }
            Phase::Heating => {
                self.temperature = (self.temperature + dt * 8.0).min(TARGET[self.preset])
            }
            Phase::Dry => {}
        }
    }

    pub fn start(&mut self) {
        if !self.lock && self.phase == Phase::Off {
            self.phase = Phase::Starting { t: 0.0 };
        }
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Off;
        self.start_hold = 0.0;
        self.temperature = 20.0;
    }

    pub fn press_preset(&mut self) {
        if !self.lock {
            self.preset = (self.preset + 1) % PRESETS.len();
        }
    }

    pub fn press_lock(&mut self) {
        self.lock = !self.lock;
    }

    pub fn run_out(&mut self) {
        if matches!(self.phase, Phase::Heating | Phase::Starting { .. }) {
            self.phase = Phase::Dry;
        }
    }

    pub fn phase_name(&self) -> &'static str {
        match self.phase {
            Phase::Off => "off",
            Phase::Starting { .. } => "starting",
            Phase::Heating => "heating",
            Phase::Dry => "dry",
        }
    }

    /// The kettle on a counter, with its display and dial.
    pub fn svg(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 360" width="640" height="360" font-family="Helvetica, Arial, sans-serif">
<rect width="640" height="360" fill="#e9e4da"/>
<rect y="268" width="640" height="92" fill="#b8a283"/>
<rect y="268" width="640" height="8" fill="#9c8669"/>
<rect x="200" y="252" width="200" height="18" rx="4" fill="#3b3b3b"/>
<path d="M226 252 L240 120 Q300 96 360 120 L374 252 Z" fill="#d7dde2" stroke="#55606a" stroke-width="3"/>
<path d="M360 140 Q420 150 404 222 L372 216 Q384 170 356 166 Z" fill="#55606a"/>
<path d="M240 140 Q206 120 190 96 L204 92 Q222 116 246 124 Z" fill="#d7dde2" stroke="#55606a" stroke-width="3"/>
<rect x="282" y="100" width="36" height="10" rx="4" fill="#55606a"/>
"##
        );

        // The light: red while heating, off otherwise.
        let light = match self.phase {
            Phase::Starting { .. } | Phase::Heating => "#ff5a4f",
            _ => "#6b6b6b",
        };
        let _ = write!(
            s,
            r##"<circle cx="300" cy="232" r="7" fill="{light}" stroke="#2c2c2c"/>"##
        );

        // Steam while heating, more as it gets hot.
        if self.phase == Phase::Heating {
            let puffs = ((self.temperature - 20.0) / 20.0).clamp(1.0, 4.0) as usize;
            for i in 0..puffs {
                let (x, y) = (180.0 - 14.0 * i as f64, 78.0 - 16.0 * i as f64);
                let _ = write!(
                    s,
                    r##"<circle cx="{x}" cy="{y}" r="{}" fill="#fff" fill-opacity="0.7"/>"##,
                    8 + 3 * i
                );
            }
        }

        // The display: preset and state, and the dial as a bar.
        let status = match self.phase {
            Phase::Off => "OFF",
            Phase::Starting { .. } => "STARTING",
            Phase::Heating => "HEATING",
            Phase::Dry => "DRY",
        };
        let status_color = if self.phase == Phase::Dry {
            "#ff6b5b"
        } else {
            "#ffffff"
        };
        let _ = write!(
            s,
            r##"<rect x="12" y="12" width="196" height="74" rx="8" fill="#000" fill-opacity="0.7"/>
<text x="24" y="34" font-size="13" fill="#b8c7d9">PRESET</text><text x="90" y="34" font-size="15" font-weight="700" fill="#fff">{}</text>
<text x="24" y="55" font-size="13" fill="#b8c7d9">STATE</text><text x="90" y="55" font-size="15" font-weight="700" fill="{status_color}">{status}</text>
<text x="24" y="76" font-size="13" fill="#b8c7d9">WATER</text><text x="90" y="76" font-size="15" font-weight="700" fill="#fff">{:.0} °C</text>
<rect x="18" y="102" width="16" height="150" rx="3" fill="#000" fill-opacity="0.35" stroke="#555"/>
<rect x="18" y="{:.1}" width="16" height="{:.1}" rx="3" fill="#ffb21f"/>
<line x1="14" x2="38" y1="{:.1}" y2="{:.1}" stroke="#333" stroke-dasharray="3 2"/>
<text x="44" y="252" font-size="12" fill="#333">DIAL {:.0}%</text>
"##,
            PRESETS[self.preset],
            self.temperature,
            252.0 - 1.5 * self.dial,
            1.5 * self.dial,
            252.0 - 1.5 * 55.0,
            252.0 - 1.5 * 55.0,
            self.dial
        );

        if self.lock {
            let _ = write!(
                s,
                r##"<rect x="420" y="40" width="200" height="72" rx="10" fill="#101820" fill-opacity="0.85" stroke="#ffb21f"/>
<text x="520" y="70" font-size="16" font-weight="700" fill="#ffb21f" text-anchor="middle">CHILD LOCK</text>
<text x="520" y="94" font-size="12" fill="#e8eef4" text-anchor="middle">The dial and buttons do nothing</text>
"##
            );
        }
        if self.phase == Phase::Dry {
            let _ = write!(
                s,
                r##"<text x="300" y="320" font-size="15" fill="#5a0d0a" text-anchor="middle" font-weight="700">Refill, then press reset</text>"##
            );
        }
        let _ = write!(
            s,
            r##"<text x="628" y="350" font-size="11" fill="#333" text-anchor="end">t = {:.1} s</text></svg>"##,
            self.time
        );
        s
    }
}

/// The vocabulary bound to the world.
/// What is on the kettle's face: four buttons and the dial, and an export button that
/// nothing in the suite uses.
pub fn controls() -> Vec<Control> {
    vec![
        Control::new("Kettle", "start", "button", "Start"),
        Control::new("Kettle", "reset", "button", "Reset"),
        Control::new("Kettle", "preset", "button", "Preset"),
        Control::new("Kettle", "lock", "button", "Lock"),
        Control::new("Kettle", "dial", "dial", ""),
        Control::new("Kettle", "export-log", "button", "Export log"),
    ]
}

impl World {
    fn used(&mut self, id: &str) {
        self.used.push(Control::new("Kettle", id, "", "").key());
    }
}

pub fn driver(lock_leaks: bool) -> Bindings<World> {
    Bindings::new(World::new(lock_leaks))
        .observe("the kettle", |w| w.phase_name())
        .observe("the dial", |w| w.dial)
        .observe("the preset", |w| PRESETS[w.preset])
        .observe("the child lock", |w| if w.lock { "on" } else { "off" })
        .set("the dial", |w, v| {
            w.used("dial");
            w.dial = v.as_f64().unwrap()
        })
        .arrange("the kettle", |w, v| match v.as_str() {
            Some("off") => w.reset(),
            Some("starting") => w.phase = Phase::Starting { t: 0.0 },
            Some("heating") => {
                w.phase = Phase::Heating;
                w.temperature = 60.0;
            }
            _ => w.phase = Phase::Dry,
        })
        .arrange("the preset", |w, v| {
            w.preset = PRESETS.iter().position(|p| Some(*p) == v.as_str()).unwrap()
        })
        .arrange("the child lock", |w, v| w.lock = v.as_str() == Some("on"))
        .on("the user presses start", |w| {
            w.used("start");
            w.start()
        })
        .on("the user presses reset", |w| {
            w.used("reset");
            w.reset()
        })
        .on("the user presses preset", |w| {
            w.used("preset");
            w.press_preset()
        })
        .on("the user presses lock", |w| {
            w.used("lock");
            w.press_lock()
        })
        .on_controls(|_| controls())
        .on_used(|w| std::mem::take(&mut w.used))
        .on("the water runs out", World::run_out)
        .on_reset(|w| *w = World::new(w.lock_leaks))
        // A second after each action, then 0.1 s per extra look.
        .on_settle(|w| (0..20).for_each(|_| w.tick(0.05)))
        .on_advance(|w| {
            w.tick(0.1);
            Progress::Advanced
        })
        .on_screenshot(|w| Screenshot::svg(w.svg()))
}
