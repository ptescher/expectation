//! The system under test, as the runner sees it.

use expectation_model::{Event, Value, Variable};

/// Whether waiting could change anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Time passed (a frame, a tick, a sleep); observe again.
    Advanced,
    /// Nothing will change by waiting; the observations so far are final.
    Static,
}

pub struct Screenshot {
    pub bytes: Vec<u8>,
    /// File extension, e.g. "png" or "svg".
    pub extension: &'static str,
}

impl Screenshot {
    pub fn png(bytes: Vec<u8>) -> Self {
        Screenshot { bytes, extension: "png" }
    }

    pub fn jpeg(bytes: Vec<u8>) -> Self {
        Screenshot { bytes, extension: "jpg" }
    }

    pub fn svg(svg: String) -> Self {
        Screenshot { bytes: svg.into_bytes(), extension: "svg" }
    }
}

/// Something a plan needs the driver to do.
#[derive(Clone, Copy, Debug)]
pub enum Need<'a> {
    Arrange(&'a Variable),
    Set(&'a Variable),
    Perform(&'a Event),
    Observe(&'a Variable),
}

impl std::fmt::Display for Need<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Need::Arrange(v) => write!(f, "arrange {}", v.subject),
            Need::Set(v) => write!(f, "set {}", v.subject),
            Need::Perform(e) => write!(f, "make it happen that {}", e.text),
            Need::Observe(v) => write!(f, "observe {}", v.subject),
        }
    }
}

/// Drives the system under test in the vocabulary's terms.
///
/// Calls may block. A real-time system (a game, a simulation) can let time pass in [`Driver::advance`]
/// and the runner keeps observing until a step's checks hold or its patience runs out, so
/// "Then the kettle is heating" means *eventually*, as it would for a person watching.
pub trait Driver {
    /// The external services this driver needs for its tests (see [`crate::service`]): a
    /// controller, a database. The runner starts them before a plan, and when one cannot be had
    /// here the plans are "not supported yet", with what to do about it.
    fn needs(&self) -> Vec<String> {
        vec![]
    }

    /// The services that must be started afresh before each test (a database the last
    /// test wrote to): a subset of [`Driver::needs`].
    fn fresh(&self) -> Vec<String> {
        vec![]
    }

    /// Start the program under test, as the section's `Launch:` says (`how` is its text). The
    /// runner calls it before the first test that needs it and again only when another section
    /// launches differently, so a driver doesn't start its own server. Drivers that are always
    /// attached to a running program leave it alone.
    fn launch(&mut self, how: &str) -> Result<(), String> {
        let _ = how;
        Ok(())
    }

    /// Put the system in its starting state (what the Background describes), before each test.
    fn reset(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Make `var` hold `value` before the first step ("Given …").
    fn arrange(&mut self, var: &Variable, value: &Value) -> Result<(), String> {
        self.set(var, value)
    }

    /// Make `var` hold `value` as an action ("When the dial is 80 %").
    fn set(&mut self, var: &Variable, value: &Value) -> Result<(), String>;

    /// Make `event` happen ("When the user presses reset").
    fn perform(&mut self, event: &Event) -> Result<(), String>;

    /// What `var` is now. Enumerated values are matched to the vocabulary ignoring case.
    fn observe(&mut self, var: &Variable) -> Result<Value, String>;

    /// Can it put `var` in `value` to start a test? [`Driver::supports`] only knows the state; a
    /// driver that can arrange some of its values and not others (a test account that can't
    /// be emptied can't start with no messages) says so here, and the tests that need the rest
    /// are "not supported yet" instead of failing.
    fn can_arrange(&self, var: &Variable, value: &Value) -> bool {
        let _ = (var, value);
        true
    }

    /// Let the system settle after an action, before anything is observed: long enough for
    /// what should happen to happen, and for what shouldn't to show. Without it, "Then the
    /// kettle is off" could pass in the instant before it switches on.
    fn settle(&mut self) {}

    /// Let a little time pass before the runner observes again.
    fn advance(&mut self) -> Progress {
        Progress::Static
    }

    fn screenshot(&mut self) -> Result<Option<Screenshot>, String> {
        Ok(None)
    }

    /// Can it do `need`? The runner checks every plan up front, so a missing binding fails
    /// the test before it starts instead of halfway through.
    fn supports(&self, need: Need<'_>) -> bool {
        let _ = need;
        true
    }
}
