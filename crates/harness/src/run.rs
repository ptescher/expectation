//! Running plans against a driver.

use crate::driver::{Driver, Need, Progress};
use crate::service::Services;
use expectation_model::{Suite, Trigger};
use expectation_story::{
    Attachment, Began, Check, CheckResult, Control, Plan, PlanResult, Report, Run, Status,
    StepResult, Window, now_ms,
};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Screenshots go to `<docs_dir>/screenshots/<plan>/<step>.<ext>`; `None` takes none.
    pub docs_dir: Option<PathBuf>,
    /// Keep screenshots by content instead: each is named by its SHA-256 (`<store>/<hash>.png`)
    /// and written only if no screenshot with the same content is there yet, so identical frames
    /// are stored once and an unchanged test writes nothing new. The results name it
    /// `screenshots/<hash>.png`; give the docs the store as their screenshots (`expectation docs
    /// --screenshots <store>`), or make it the docs' own `screenshots/` directory. Screenshots
    /// are taken when this or `docs_dir` is set.
    pub screenshot_store: Option<PathBuf>,
    /// A folder under `screenshots/` for this run's per-test screenshots, so that runs which
    /// share a docs directory don't write over each other's: with `platforms/android`, they go
    /// to `<docs_dir>/screenshots/platforms/android/<plan>/<step>.<ext>` and the results name
    /// them so. [`Acceptance::run_options`](crate::Acceptance::run_options) sets it for a
    /// platform other than the suite's first. A store has no use for it: its names are the
    /// content's.
    pub screenshots_under: Option<String>,
    /// How many times to [`Driver::advance`] while waiting for a step's checks to hold.
    pub patience: u32,
    /// Ask the driver for its controls and which ones each test used (see
    /// [`Driver::controls`]). Cheap for a driver that has none; a driver that walks a UI tree can
    /// turn it off.
    pub controls: bool,
    /// Where to keep what the driver captures ([`Driver::take_attachments`]), as
    /// `<dir>/<run>/<plan>/<step>/<name>`; `None` keeps nothing. Keep it out of the docs
    /// directory: a capture can hold what a manual must not publish.
    pub attachments_dir: Option<PathBuf>,
    /// What this run is against (the program's revision, its data), kept with every result so
    /// a manual can say which build a saved pass was about. Say only what may be published;
    /// nothing is filled in for you.
    pub provenance: expectation_story::Provenance,
    /// Let a test start where the last one ended, when that test passed and everything this one
    /// is given is observed to hold there already: no reset, nothing arranged. Tests are then
    /// run in an order that makes the most of it (the results stay in the plans' order), and
    /// each result says how it began ([`PlanResult::began`]). On by default; a driver can
    /// refuse it for itself ([`Driver::may_continue`]), and one that wants services afresh
    /// for each test ([`Driver::fresh`]) never continues.
    pub share_starts: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            docs_dir: None,
            screenshot_store: None,
            screenshots_under: None,
            patience: 100,
            controls: true,
            attachments_dir: None,
            provenance: Default::default(),
            share_starts: true,
        }
    }
}

/// What a driver has done so far in a run, as far as its next test is concerned.
#[derive(Default)]
struct Session {
    /// How the program was last started (a section's `Launch:`), so it is started once for the
    /// plans that share it and again when a section says to start it differently.
    launched: Option<String>,
    /// The last test it ran, if that test passed: the system is where that test ends.
    last: Option<usize>,
}

impl Session {
    /// Would `plan` be started the way the program is running now?
    fn launched_for(&self, suite: &Suite, plan: &Plan) -> bool {
        suite
            .launch_of(plan.section)
            .is_none_or(|how| self.launched.as_deref() == Some(how))
    }

    /// After `plans[at]` gave `result`: a pass leaves the system where that test ends, a test
    /// that wasn't run leaves it where it was, and after anything else it could be anywhere.
    fn after(&mut self, at: usize, result: &PlanResult) {
        match result.status {
            Status::Passed => self.last = Some(at),
            Status::Pending | Status::Unsupported => {}
            _ => self.last = None,
        }
    }
}

/// The test to run next, taken out of `left`: one that starts where the last one ended, so it
/// may need no reset, or else the first that is left.
fn take(
    suite: &Suite,
    plans: &[Plan],
    left: &mut Vec<usize>,
    session: &Session,
    share: bool,
) -> Option<usize> {
    let follows = session.last.filter(|_| share).and_then(|last| {
        let last = &plans[last];
        let end = last.steps.last().map_or(&last.start, |s| &s.state);
        left.iter().position(|&i| {
            let p = &plans[i];
            !p.pending
                && !p.start_checks.is_empty()
                && session.launched_for(suite, p)
                && end.implies(&p.start)
        })
    });
    (!left.is_empty()).then(|| left.remove(follows.unwrap_or(0)))
}

/// Run every plan, each chosen by [`take`], and give the results in the plans' order.
fn run_each(
    suite: &Suite,
    plans: &[Plan],
    driver: &mut dyn Driver,
    opts: &RunOptions,
    mut one: impl FnMut(&Plan, &mut dyn Driver, &mut Session) -> PlanResult,
) -> Report {
    let mut session = Session::default();
    let mut left: Vec<usize> = (0..plans.len()).collect();
    let mut results: Vec<Option<PlanResult>> = plans.iter().map(|_| None).collect();
    while let Some(i) = {
        let share = opts.share_starts && driver.may_continue();
        take(suite, plans, &mut left, &session, share)
    } {
        let result = one(&plans[i], driver, &mut session);
        session.after(i, &result);
        results[i] = Some(result);
    }
    Report {
        results: results.into_iter().flatten().collect(),
    }
}

pub fn run(suite: &Suite, plans: &[Plan], driver: &mut dyn Driver, opts: &RunOptions) -> Report {
    let run = Run::start().against(opts.provenance.clone());
    run_each(suite, plans, driver, opts, |plan, driver, session| {
        run_plan(suite, plan, driver, opts, session, &run)
    })
}

/// As [`run`], with what the driver needs from outside started before each plan: a service that
/// is up stays, one that died is started again, and when one cannot be had the plan is not
/// supported yet, with what to do about it.
pub fn run_with(
    suite: &Suite,
    plans: &[Plan],
    driver: &mut dyn Driver,
    opts: &RunOptions,
    services: &mut Services,
) -> Report {
    let run = Run::start().against(opts.provenance.clone());
    run_each(suite, plans, driver, opts, |plan, driver, session| {
        run_served(suite, plan, driver, opts, services, session, &run)
    })
}

/// What one worker of [`run_parallel`] runs with: a driver of its own, and the services that
/// driver needs, which no other worker shares.
pub struct Worker {
    pub driver: Box<dyn Driver>,
    pub services: Services,
}

impl Worker {
    pub fn new(driver: impl Driver + 'static) -> Self {
        Worker {
            driver: Box::new(driver),
            services: Services::new(),
        }
    }

    pub fn with_services(mut self, services: Services) -> Self {
        self.services = services;
        self
    }
}

/// As [`run_with`], on `workers` threads that take the next plan as they finish one. `make` is
/// called once on each worker's thread, with the worker's number (from 0), to build its
/// [`Worker`], so a driver need not be `Send` and two workers never share one: give each its own
/// app, port, database and screen. A worker takes a plan that starts where its last one ended
/// when there is one (see [`RunOptions::share_starts`]). Results are in the order of `plans`. `workers` is at least 1
/// and at most the number of plans.
pub fn run_parallel<F>(
    suite: &Suite,
    plans: &[Plan],
    workers: usize,
    opts: &RunOptions,
    make: F,
) -> Report
where
    F: Fn(usize) -> Worker + Sync,
{
    let left: Mutex<Vec<usize>> = Mutex::new((0..plans.len()).collect());
    // One run, however many workers it has.
    let run = Run::start().against(opts.provenance.clone());
    let run = &run;
    let done: Mutex<Vec<(usize, PlanResult)>> = Mutex::new(Vec::with_capacity(plans.len()));
    std::thread::scope(|scope| {
        for n in 0..workers.clamp(1, plans.len().max(1)) {
            let (left, done, make) = (&left, &done, &make);
            scope.spawn(move || {
                let mut worker: Option<Worker> = None;
                let mut session = Session::default();
                loop {
                    let share = opts.share_starts
                        && worker.as_ref().is_some_and(|w| w.driver.may_continue());
                    let taken = take(suite, plans, &mut left.lock().unwrap(), &session, share);
                    let Some(i) = taken else { break };
                    let plan = &plans[i];
                    // Built when there is a plan to run, so a worker with nothing to do starts nothing.
                    let w = worker.get_or_insert_with(|| make(n));
                    let result = run_served(
                        suite,
                        plan,
                        &mut *w.driver,
                        opts,
                        &mut w.services,
                        &mut session,
                        run,
                    );
                    session.after(i, &result);
                    done.lock().unwrap().push((i, result));
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap();
    done.sort_by_key(|(i, _)| *i);
    Report {
        results: done.into_iter().map(|(_, r)| r).collect(),
    }
}

fn run_served(
    suite: &Suite,
    plan: &Plan,
    driver: &mut dyn Driver,
    opts: &RunOptions,
    services: &mut Services,
    session: &mut Session,
    run: &Run,
) -> PlanResult {
    if let Some(r) = pending(suite, plan) {
        return ran_in(r, run);
    }
    let needs = driver.needs();
    let names: Vec<&str> = needs.iter().map(String::as_str).collect();
    let fresh = driver.fresh();
    let fresh: Vec<&str> = fresh.iter().map(String::as_str).collect();
    match unsupported(suite, plan, driver).map_or_else(
        || {
            services
                .restart(&fresh)
                .and_then(|()| services.start(&names))
        },
        |_| Ok(()),
    ) {
        Ok(()) => run_plan(suite, plan, driver, opts, session, run),
        Err(why) => {
            // Whatever was started afresh before that has lost what the last test left.
            session.last = None;
            let mut r = PlanResult::with(Status::Unsupported, plan);
            r.start.error = Some(format!("It needs {why}."));
            ran_in(r, run)
        }
    }
}

fn needs<'a>(suite: &'a Suite, plan: &Plan) -> Vec<Need<'a>> {
    let v = &suite.vocabulary;
    let mut out: Vec<Need> = plan.start.vars().map(|x| Need::Arrange(v.var(x))).collect();
    let observe = |c: &Check| Need::Observe(v.var(c.var));
    out.extend(plan.start_checks.iter().map(observe));
    for step in &plan.steps {
        out.extend(step.action.changes.vars().map(|x| Need::Set(v.var(x))));
        out.extend(step.action.event.map(|e| Need::Perform(v.event(e))));
        out.extend(step.checks.iter().map(observe));
    }
    out
}

/// A plan that depends on an expectation marked `Status: pending` is not run, before anything
/// else is asked of the driver: it is neither a pass nor a failure, and it is not a driver gap
/// (that is [`unsupported`], which a pending plan never reaches), so a consumer can tell the
/// behaviour that is intentionally not built from a binding that is unexpectedly missing.
fn pending(suite: &Suite, plan: &Plan) -> Option<PlanResult> {
    if !plan.pending {
        return None;
    }
    let mut names: Vec<String> = plan
        .start_invariants
        .iter()
        .chain(
            plan.steps
                .iter()
                .flat_map(|s| s.fired.iter().chain(&s.invariants)),
        )
        .map(|&e| suite.exp(e))
        .filter(|e| e.pending)
        .map(|e| format!("«{}»", e.name))
        .collect();
    names.sort();
    names.dedup();
    let mut r = PlanResult::with(Status::Pending, plan);
    r.start.error = Some(format!(
        "Pending: it depends on {}, which isn't built yet.",
        expectation_model::text::join_and(&names)
    ));
    Some(r)
}

/// A plan the driver cannot run yet is not run and not failed: the result says what it lacks.
fn unsupported(suite: &Suite, plan: &Plan, driver: &dyn Driver) -> Option<PlanResult> {
    let mut missing: Vec<String> = needs(suite, plan)
        .into_iter()
        .filter(|n| !driver.supports(*n))
        .map(|n| n.to_string())
        .collect();
    for (x, c) in plan.start.iter() {
        let var = suite.vocabulary.var(x);
        let value = var
            .witness(c)
            .expect("plans only arrange satisfiable states");
        if !driver.can_arrange(var, &value) {
            missing.push(format!("arrange {} as {value}", var.subject));
        }
    }
    missing.sort();
    missing.dedup();
    if !missing.is_empty() {
        // Not a failure: the system (or its driver) just doesn't do this yet.
        let mut r = PlanResult::with(Status::Unsupported, plan);
        // A few of them, so the reason stays readable: another system's tests lack dozens.
        let mut shown: Vec<String> = missing.iter().take(3).cloned().collect();
        if missing.len() > 3 {
            shown.push(format!("{} more things", missing.len() - 3));
        }
        r.start.error = Some(format!(
            "The driver can't {} yet.",
            expectation_model::text::join_and(&shown)
        ));
        return Some(r);
    }
    None
}

fn run_plan(
    suite: &Suite,
    plan: &Plan,
    driver: &mut dyn Driver,
    opts: &RunOptions,
    session: &mut Session,
    run: &Run,
) -> PlanResult {
    ran_in(run_steps(suite, plan, driver, opts, session, run), run)
}

/// A result, marked with the run it came from.
fn ran_in(mut r: PlanResult, run: &Run) -> PlanResult {
    r.run = Some(run.clone());
    r
}

fn run_steps(
    suite: &Suite,
    plan: &Plan,
    driver: &mut dyn Driver,
    opts: &RunOptions,
    session: &mut Session,
    run: &Run,
) -> PlanResult {
    let failed = |error: String| StepResult {
        status: Status::Failed,
        error: Some(error),
        ..StepResult::with(Status::Failed)
    };

    if let Some(r) = pending(suite, plan) {
        return r;
    }
    if let Some(r) = unsupported(suite, plan, driver) {
        return r;
    }

    // Per test and step under the docs, replaced each run; or by content in a store.
    let named = match opts
        .screenshots_under
        .as_deref()
        .map(|u| u.trim_matches('/'))
    {
        Some(under) if !under.is_empty() => format!("screenshots/{under}/{}", plan.slug),
        _ => format!("screenshots/{}", plan.slug),
    };
    let shots = match (&opts.screenshot_store, &opts.docs_dir) {
        (Some(_), _) => None,
        (None, Some(d)) => {
            let dir = d.join(&named);
            let _ = std::fs::remove_dir_all(&dir);
            Some(dir)
        }
        (None, None) => None,
    };
    let shoot = |driver: &mut dyn Driver, n: usize| -> Option<String> {
        if let Some(store) = &opts.screenshot_store {
            let shot = driver.screenshot().ok()??;
            return store_shot(store, &shot.bytes, shot.extension);
        }
        let dir = shots.as_ref()?;
        let shot = driver.screenshot().ok()??;
        let name = format!("{n:02}.{}", shot.extension);
        std::fs::create_dir_all(dir).ok()?;
        std::fs::write(dir.join(&name), &shot.bytes).ok()?;
        Some(format!("{named}/{name}"))
    };
    let mut attachments = vec![];
    let mut keep = |driver: &mut dyn Driver, n: usize| {
        let taken = driver.take_attachments();
        if let Some(dir) = &opts.attachments_dir {
            attachments.extend(save_captures(dir, run, &plan.slug, n, taken));
        }
    };
    let skip_all = |start: StepResult, attachments: Vec<Attachment>| PlanResult {
        status: Status::Failed,
        start,
        steps: plan.steps.iter().map(|_| StepResult::skipped()).collect(),
        controls: vec![],
        used: vec![],
        run: None,
        began: None,
        attachments,
        stale: false,
    };

    if let Some(how) = suite.launch_of(plan.section)
        && !session.launched_for(suite, plan)
    {
        // Started again, it isn't where the last test left it.
        session.last = None;
        if let Err(e) = driver.launch(how) {
            return skip_all(failed(format!("couldn't launch: {e}")), vec![]);
        }
        session.launched = Some(how.to_string());
    }
    let began = now_ms();
    let setup_failed =
        |error: String, driver: &mut dyn Driver, keep: &mut dyn FnMut(&mut dyn Driver, usize)| {
            let mut start = failed(error);
            start.window = Some(Window {
                start_ms: began,
                end_ms: now_ms(),
            });
            // What it looked like when it went wrong, if the driver can still say. The failure
            // is the setup's whether or not there is a picture of it.
            start.screenshot = shoot(driver, 0);
            start.window = Some(Window {
                start_ms: began,
                end_ms: now_ms(),
            });
            keep(driver, 0);
            start
        };
    // Where the last test ended, if that is where this one starts: looked at once, as it is,
    // without waiting for it to become so. A test that is given nothing has nothing to tell
    // it by, and one whose driver wants services afresh must not find what the last one left.
    let continued = opts.share_starts
        && session.last.is_some()
        && !plan.start_checks.is_empty()
        && driver.may_continue()
        && driver.fresh().is_empty()
        && observe(suite, driver, &plan.start_checks, 0).status == Status::Passed;
    if !continued {
        if let Err(e) = driver.reset() {
            let start = setup_failed(format!("couldn't reset: {e}"), driver, &mut keep);
            return skip_all(start, attachments);
        }
        for (x, c) in plan.start.iter() {
            let var = suite.vocabulary.var(x);
            let value = var
                .witness(c)
                .expect("plans only arrange satisfiable states");
            if let Err(e) = driver.arrange(var, &value) {
                let error = format!("couldn't arrange {}: {e}", var.describe(c));
                let start = setup_failed(error, driver, &mut keep);
                return skip_all(start, attachments);
            }
        }
    }
    driver.settle();
    // Setting the test up is not using the controls.
    let mut seen = Seen::new(opts.controls);
    seen.take(driver);
    seen.used.clear();
    seen.look(driver);
    let mut start = observe(suite, driver, &plan.start_checks, opts.patience);
    start.screenshot = shoot(driver, 0);
    start.window = Some(Window {
        start_ms: began,
        end_ms: now_ms(),
    });
    keep(driver, 0);

    let mut ok = start.status == Status::Passed;
    let mut steps = vec![];
    for (n, step) in plan.steps.iter().enumerate() {
        if !ok {
            steps.push(StepResult::skipped());
            continue;
        }
        let began = now_ms();
        let acted = act(suite, driver, &step.action);
        seen.take(driver);
        let mut r = match acted {
            Ok(()) => {
                driver.settle();
                seen.look(driver);
                observe(suite, driver, &step.checks, opts.patience)
            }
            Err(e) => failed(e),
        };
        r.screenshot = shoot(driver, n + 1);
        r.window = Some(Window {
            start_ms: began,
            end_ms: now_ms(),
        });
        keep(driver, n + 1);
        ok = r.status == Status::Passed;
        steps.push(r);
    }
    PlanResult {
        status: if ok { Status::Passed } else { Status::Failed },
        start,
        steps,
        controls: seen.controls,
        used: seen.used,
        run: None,
        began: Some(if continued {
            Began::Continued
        } else {
            Began::Arranged
        }),
        attachments,
        stale: false,
    }
}

/// Keep a screenshot in a content-addressed store: `<store>/<sha256>.<ext>`, written only when
/// it isn't there already (through a temporary file, so a reader never sees half of one). Says
/// how the results name it.
fn store_shot(store: &std::path::Path, bytes: &[u8], ext: &str) -> Option<String> {
    let name = format!("{}.{ext}", expectation_story::sha256::sha256_hex(bytes));
    let path = store.join(&name);
    if !path.is_file() {
        std::fs::create_dir_all(store).ok()?;
        // Unique to this write: parallel workers can take the same frame at once.
        static WRITES: AtomicUsize = AtomicUsize::new(0);
        let n = WRITES.fetch_add(1, Ordering::Relaxed);
        let tmp = store.join(format!(".{name}.{}.{n}.tmp", std::process::id()));
        std::fs::write(&tmp, bytes).ok()?;
        std::fs::rename(&tmp, &path).ok()?;
    }
    Some(format!("screenshots/{name}"))
}

/// Save what the driver captured after step `n` of `plan`, under `dir/<run>/<plan>/<nn>/`, and
/// say where. A capture that can't be written is left out rather than failing the test: it is
/// evidence about the run, not part of it.
fn save_captures(
    dir: &std::path::Path,
    run: &Run,
    plan: &str,
    n: usize,
    captures: Vec<crate::driver::Capture>,
) -> Vec<Attachment> {
    let rel = format!("{}/{plan}/{n:02}", safe_name(&run.id));
    let mut names: Vec<String> = vec![];
    let mut out = vec![];
    for c in captures {
        let mut name = safe_name(&c.name);
        let stem = name.clone();
        let mut i = 1;
        while names.contains(&name) {
            i += 1;
            name = format!("{i}-{stem}");
        }
        let folder = dir.join(&rel);
        if std::fs::create_dir_all(&folder).is_err()
            || std::fs::write(folder.join(&name), &c.bytes).is_err()
        {
            continue;
        }
        out.push(Attachment {
            step: n,
            media_type: c.media_type.replace(['\t', '\n', '\r'], " "),
            path: format!("{rel}/{name}"),
        });
        names.push(name);
    }
    out
}

/// A file name that stays where it is put: the last part of a path, in letters, digits, `.`,
/// `_` and `-`, with no `..`, not starting with a dot, at most 100 characters.
fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let mut clean: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    while clean.contains("..") {
        clean = clean.replace("..", ".");
    }
    let clean = clean.trim_start_matches('.').to_string();
    if clean.is_empty() {
        "attachment".into()
    } else {
        clean
    }
}

/// The controls a test saw, and which of them its actions used.
struct Seen {
    on: bool,
    controls: Vec<Control>,
    used: Vec<String>,
}

impl Seen {
    fn new(on: bool) -> Self {
        Seen {
            on,
            controls: vec![],
            used: vec![],
        }
    }

    fn look(&mut self, driver: &mut dyn Driver) {
        if !self.on {
            return;
        }
        for c in driver.controls().unwrap_or_default() {
            if !self.controls.iter().any(|k| k.key() == c.key()) {
                self.controls.push(c);
            }
        }
    }

    fn take(&mut self, driver: &mut dyn Driver) {
        if !self.on {
            return;
        }
        for key in driver.take_used() {
            if !self.used.contains(&key) {
                self.used.push(key);
            }
        }
    }
}

fn act(suite: &Suite, driver: &mut dyn Driver, action: &Trigger) -> Result<(), String> {
    for (x, c) in action.changes.iter() {
        let var = suite.vocabulary.var(x);
        let value = var
            .witness(c)
            .ok_or_else(|| format!("no value satisfies {}", var.describe(c)))?;
        driver
            .set(var, &value)
            .map_err(|e| format!("couldn't set {}: {e}", var.describe(c)))?;
    }
    if let Some(e) = action.event {
        let event = suite.vocabulary.event(e);
        driver
            .perform(event)
            .map_err(|err| format!("couldn't make it happen that {}: {err}", event.text))?;
    }
    Ok(())
}

/// Observe until every check holds at once, or patience runs out.
fn observe(suite: &Suite, driver: &mut dyn Driver, checks: &[Check], patience: u32) -> StepResult {
    let mut polls = 0;
    loop {
        polls += 1;
        let results: Vec<CheckResult> = checks
            .iter()
            .map(|c| {
                let var = suite.vocabulary.var(c.var);
                match driver.observe(var) {
                    Ok(v) => CheckResult {
                        passed: var.admits(&c.expect, &v),
                        observed: Some(v),
                        error: None,
                    },
                    Err(e) => CheckResult {
                        observed: None,
                        passed: false,
                        error: Some(e),
                    },
                }
            })
            .collect();
        let passed = results.iter().all(|r| r.passed);
        if passed || polls > patience || driver.advance() == Progress::Static {
            let status = if passed {
                Status::Passed
            } else {
                Status::Failed
            };
            return StepResult {
                status,
                checks: results,
                polls,
                ..StepResult::with(status)
            };
        }
    }
}
