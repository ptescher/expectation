//! The kettle itself, the same on every platform: the dial has to stay above 55 % for 0.6 s
//! before it starts, starting takes 0.22 s, and while the child lock is on the dial and the
//! buttons (all but LOCK and RESET) do nothing.

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
pub struct Kettle {
    pub phase: Phase,
    pub dial: f64,
    pub preset: usize,
    pub lock: bool,
    pub temperature: f64,
    hold: f64,
}

impl Default for Kettle {
    fn default() -> Self {
        Kettle { phase: Phase::Off, dial: 0.0, preset: 0, lock: false, temperature: 20.0, hold: 0.0 }
    }
}

impl Kettle {
    pub fn tick(&mut self, dt: f64) {
        let dial = if self.lock { 0.0 } else { self.dial };
        match self.phase {
            Phase::Off if dial > 55.0 => {
                self.hold += dt;
                if self.hold > 0.6 {
                    self.phase = Phase::Starting { t: 0.0 };
                    self.hold = 0.0;
                }
            }
            Phase::Off => self.hold = 0.0,
            Phase::Starting { t } => {
                self.phase = if t + dt >= 0.22 { Phase::Heating } else { Phase::Starting { t: t + dt } }
            }
            Phase::Heating => self.temperature = (self.temperature + dt * 8.0).min(TARGET[self.preset]),
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
        self.hold = 0.0;
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

    /// For tests: put the kettle in a phase directly.
    pub fn force(&mut self, phase: &str) {
        match phase {
            "starting" => self.phase = Phase::Starting { t: 0.0 },
            "heating" => {
                self.phase = Phase::Heating;
                self.temperature = 60.0;
            }
            "dry" => self.phase = Phase::Dry,
            _ => self.reset(),
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
}
