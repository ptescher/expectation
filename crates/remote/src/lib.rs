//! Driving an app from outside: a small protocol between the runner and a driver embedded in
//! the app under test, so a phone app, a desktop app or a web view can be the system under
//! test without the runner knowing how it's built.
//!
//! The **runner** listens ([`RemoteDriver`]), and the **app** connects to it. Apps don't have to
//! host a server, which suits phones and sandboxes: the embedded driver long-polls the runner
//! for commands and posts back what happened. Clients exist for Rust ([`client`]), Swift and
//! JavaScript; anything that can make an HTTP request can be one.
//!
//! ## The protocol
//!
//! JSON over HTTP/1.1, to `EXPECTATION_URL` (by default `http://127.0.0.1:4477`):
//!
//! | Request | Body | Answer |
//! | --- | --- | --- |
//! | `POST /hello` | `{"platform", "app", "observe": [..], "set": [..], "arrange": [..], "events": [..], "screenshot": bool, "reset": bool}` | `{"session": n}`, or 409 when the runner is for another platform (try again later) |
//! | `GET /next?session=n` | | a command (200), nothing yet (204: ask again), or 410: say hello again |
//! | `POST /reply?session=n` | `{"id", "value"}`, `{"id", "png"}` (base64), `{"id", "svg"}`, `{"id"}` or `{"id", "error"}` | 204 |
//!
//! The terms in the hello are the vocabulary the app can handle, as the app spells them; the
//! runner matches them the way steps are matched. A command is one of:
//!
//! ```json
//! {"id": 1, "op": "reset"}
//! {"id": 2, "op": "arrange", "subject": "the kettle", "value": "off"}
//! {"id": 3, "op": "set", "subject": "the dial", "value": 80}
//! {"id": 4, "op": "perform", "event": "the user presses lock"}
//! {"id": 5, "op": "observe", "subject": "the kettle"}
//! {"id": 6, "op": "screenshot"}
//! ```
//!
//! A named value is a JSON string and a number is a JSON number. `observe` answers with
//! `value`; `screenshot` with `png` or `svg`; the rest with just the `id`, or an `error`.

pub mod client;
pub mod http;
pub mod json;

use expectation_harness::{Driver, Need, Progress, Screenshot};
use expectation_model::text::key;
use expectation_model::{Event, Value, Variable};
use json::Json;
use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Where clients connect when `EXPECTATION_URL` isn't set.
pub const DEFAULT_ADDR: &str = "127.0.0.1:4477";

/// How long a `GET /next` waits for a command before answering 204.
const LONG_POLL: Duration = Duration::from_secs(20);

/// The app on the other end, as it introduced itself.
#[derive(Clone, Debug, Default)]
pub struct App {
    pub platform: String,
    pub name: String,
    pub observe: Vec<String>,
    pub set: Vec<String>,
    pub arrange: Vec<String>,
    pub events: Vec<String>,
    pub screenshot: bool,
    pub reset: bool,
}

impl App {
    fn from_json(j: &Json) -> App {
        let text = |k: &str| j.get(k).and_then(Json::as_str).unwrap_or("").to_string();
        let list = |k: &str| j.get(k).map(Json::strings).unwrap_or_default();
        let flag = |k: &str| j.get(k).and_then(Json::as_bool).unwrap_or(false);
        App {
            platform: text("platform"),
            name: text("app"),
            observe: list("observe"),
            set: list("set"),
            arrange: list("arrange"),
            events: list("events"),
            screenshot: flag("screenshot"),
            reset: flag("reset"),
        }
    }

    pub fn to_json(&self) -> Json {
        Json::obj([
            ("platform", self.platform.as_str().into()),
            ("app", self.name.as_str().into()),
            ("observe", self.observe.clone().into()),
            ("set", self.set.clone().into()),
            ("arrange", self.arrange.clone().into()),
            ("events", self.events.clone().into()),
            ("screenshot", self.screenshot.into()),
            ("reset", self.reset.into()),
        ])
    }

    /// The app's own spelling of a term, if it handles it.
    fn term<'a>(list: &'a [String], wanted: &str) -> Option<&'a str> {
        list.iter().find(|t| key(t) == wanted).map(String::as_str)
    }
}

#[derive(Default)]
struct State {
    /// Only an app for this platform may connect.
    platform: Option<String>,
    session: u64,
    app: Option<App>,
    queue: VecDeque<Json>,
    replies: HashMap<u64, Json>,
    next_id: u64,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

type Shooter = Box<dyn FnMut() -> Result<Option<Screenshot>, String>>;

/// A [`Driver`] for an app that embeds an Expectation client: it listens for the app, sends it
/// commands and waits for the answers. Time is real: [`Driver::settle`] and
/// [`Driver::advance`] sleep.
pub struct RemoteDriver {
    shared: Arc<Shared>,
    addr: SocketAddr,
    settle: Duration,
    poll: Duration,
    answer: Duration,
    screenshot: Option<Shooter>,
    needs: Vec<String>,
    fresh: Vec<String>,
    shot_error: Option<String>,
}

impl RemoteDriver {
    /// Listens on `addr` ("127.0.0.1:4477", or port 0 for any free port).
    pub fn listen(addr: &str) -> std::io::Result<RemoteDriver> {
        let listener = TcpListener::bind(addr)?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared::default());
        let s = shared.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let s = s.clone();
                std::thread::spawn(move || serve(&s, stream));
            }
        });
        Ok(RemoteDriver {
            shared,
            addr,
            settle: Duration::from_millis(1000),
            poll: Duration::from_millis(100),
            answer: Duration::from_secs(10),
            screenshot: None,
            needs: vec![],
            fresh: vec![],
            shot_error: None,
        })
    }

    /// Where it's listening, for the app's `EXPECTATION_URL`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// How long to let the app settle after each action (1 s by default).
    pub fn settle_for(mut self, d: Duration) -> Self {
        self.settle = d;
        self
    }

    /// How long to wait between looks while a step's checks don't hold yet (0.1 s by default).
    pub fn poll_every(mut self, d: Duration) -> Self {
        self.poll = d;
        self
    }

    /// Screenshots from outside the app (a simulator's or the window's), for apps that can't
    /// take their own.
    pub fn screenshot_with(mut self, f: impl FnMut() -> Result<Option<Screenshot>, String> + 'static) -> Self {
        self.screenshot = Some(Box::new(f));
        self
    }

    /// Accepts only an app that says it's for `platform`; others are turned away (409) and
    /// keep trying, so apps for several platforms can wait for their turn on one machine.
    pub fn for_platform(self, platform: &str) -> Self {
        self.shared.lock().platform = Some(platform.to_string());
        self
    }

    /// The external services the tests need: see [`Driver::needs`].
    pub fn needs(mut self, services: &[&str]) -> Self {
        self.needs = services.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn fresh(mut self, services: &[&str]) -> Self {
        self.fresh = services.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Why the last screenshot failed, if one did. The runner carries on without the picture.
    pub fn screenshot_error(&self) -> Option<&str> {
        self.shot_error.as_deref()
    }

    /// The app that's connected, if one is.
    pub fn app(&self) -> Option<App> {
        self.shared.lock().app.clone()
    }

    /// Waits until an app says hello.
    pub fn wait_for_app(&self, timeout: Duration) -> Result<App, String> {
        let deadline = Instant::now() + timeout;
        let mut st = self.shared.lock();
        loop {
            if let Some(app) = &st.app {
                return Ok(app.clone());
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(format!("no app connected to {} within {:?}", self.url(), timeout));
            }
            st = self.shared.changed.wait_timeout(st, left).unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    fn call(&self, mut command: Vec<(&'static str, Json)>) -> Result<Json, String> {
        let mut st = self.shared.lock();
        if st.app.is_none() {
            return Err("no app is connected".into());
        }
        st.next_id += 1;
        let (id, session) = (st.next_id, st.session);
        command.insert(0, ("id", Json::Num(id as f64)));
        st.queue.push_back(Json::obj(command));
        self.shared.changed.notify_all();
        let deadline = Instant::now() + self.answer;
        loop {
            if let Some(reply) = st.replies.remove(&id) {
                return match reply.get("error").and_then(Json::as_str) {
                    Some(e) => Err(e.to_string()),
                    None => Ok(reply),
                };
            }
            if st.session != session {
                return Err("the app restarted".into());
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(format!("the app didn't answer within {:?}", self.answer));
            }
            st = self.shared.changed.wait_timeout(st, left).unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    fn term(&self, list: fn(&App) -> &Vec<String>, wanted: &str) -> Option<String> {
        self.shared.lock().app.as_ref().and_then(|a| App::term(list(a), wanted).map(String::from))
    }

    fn change(&mut self, op: &'static str, subject: String, value: &Value) -> Result<(), String> {
        self.call(vec![("op", op.into()), ("subject", subject.into()), ("value", to_json(value))]).map(|_| ())
    }
}

fn to_json(v: &Value) -> Json {
    match v {
        Value::Enum(s) => Json::Str(s.clone()),
        Value::Number(n) => Json::Num(*n),
    }
}

impl Driver for RemoteDriver {
    fn needs(&self) -> Vec<String> {
        self.needs.clone()
    }

    fn fresh(&self) -> Vec<String> {
        self.fresh.clone()
    }

    fn reset(&mut self) -> Result<(), String> {
        let resets = self.shared.lock().app.as_ref().is_some_and(|a| a.reset);
        if resets { self.call(vec![("op", "reset".into())]).map(|_| ()) } else { Ok(()) }
    }

    fn arrange(&mut self, var: &Variable, value: &Value) -> Result<(), String> {
        match self.term(|a| &a.arrange, &var.key) {
            Some(subject) => self.change("arrange", subject, value),
            None => self.set(var, value),
        }
    }

    fn set(&mut self, var: &Variable, value: &Value) -> Result<(), String> {
        let subject = self.term(|a| &a.set, &var.key).ok_or_else(|| format!("the app can't set {}", var.subject))?;
        self.change("set", subject, value)
    }

    fn perform(&mut self, event: &Event) -> Result<(), String> {
        let name =
            self.term(|a| &a.events, &event.key).ok_or_else(|| format!("the app can't make {} happen", event.text))?;
        self.call(vec![("op", "perform".into()), ("event", name.into())]).map(|_| ())
    }

    fn observe(&mut self, var: &Variable) -> Result<Value, String> {
        let subject =
            self.term(|a| &a.observe, &var.key).ok_or_else(|| format!("the app can't observe {}", var.subject))?;
        let reply = self.call(vec![("op", "observe".into()), ("subject", subject.into())])?;
        match reply.get("value") {
            Some(Json::Str(s)) => Ok(Value::Enum(s.clone())),
            Some(Json::Num(n)) => Ok(Value::Number(*n)),
            Some(Json::Bool(b)) => Ok(Value::Enum(b.to_string())),
            other => Err(format!("the app answered {} with {other:?}", var.subject)),
        }
    }

    fn settle(&mut self) {
        std::thread::sleep(self.settle);
    }

    fn advance(&mut self) -> Progress {
        std::thread::sleep(self.poll);
        Progress::Advanced
    }

    fn screenshot(&mut self) -> Result<Option<Screenshot>, String> {
        let shot = self.take_screenshot();
        if let Err(e) = &shot {
            self.shot_error = Some(e.clone());
        }
        shot
    }

    fn supports(&self, need: Need<'_>) -> bool {
        self.supports_need(need)
    }
}

impl RemoteDriver {
    fn take_screenshot(&mut self) -> Result<Option<Screenshot>, String> {
        let own = self.shared.lock().app.as_ref().is_some_and(|a| a.screenshot);
        if own {
            let reply = self.call(vec![("op", "screenshot".into())])?;
            if let Some(png) = reply.get("png").and_then(Json::as_str) {
                return Ok(Some(Screenshot::png(json::base64_decode(png)?)));
            }
            if let Some(svg) = reply.get("svg").and_then(Json::as_str) {
                return Ok(Some(Screenshot::svg(svg.to_string())));
            }
        }
        match &mut self.screenshot {
            Some(f) => f(),
            None => Ok(None),
        }
    }

    fn supports_need(&self, need: Need<'_>) -> bool {
        let st = self.shared.lock();
        let Some(app) = &st.app else { return false };
        match need {
            Need::Arrange(v) => App::term(&app.arrange, &v.key).or(App::term(&app.set, &v.key)).is_some(),
            Need::Set(v) => App::term(&app.set, &v.key).is_some(),
            Need::Perform(e) => App::term(&app.events, &e.key).is_some(),
            Need::Observe(v) => App::term(&app.observe, &v.key).is_some(),
        }
    }
}

fn serve(shared: &Shared, stream: std::net::TcpStream) {
    let Ok(req) = http::read_request(&stream) else {
        return http::respond(&stream, 400, "");
    };
    let session = || req.param("session").and_then(|s| s.parse::<u64>().ok());
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/hello") => {
            let Ok(hello) = Json::parse(&req.body) else {
                return http::respond(&stream, 400, r#"{"error":"bad hello"}"#);
            };
            let app = App::from_json(&hello);
            let mut st = shared.lock();
            if let Some(p) = &st.platform {
                if key(p) != key(&app.platform) {
                    let body = Json::obj([("error", format!("this runner is for {p}, not {}", app.platform).into())]);
                    drop(st);
                    return http::respond(&stream, 409, &body.to_string());
                }
            }
            st.session += 1;
            st.app = Some(app);
            st.queue.clear();
            st.replies.clear();
            let body = Json::obj([("session", Json::Num(st.session as f64))]).to_string();
            shared.changed.notify_all();
            drop(st);
            http::respond(&stream, 200, &body);
        }
        ("GET", "/next") => {
            let deadline = Instant::now() + LONG_POLL;
            let mut st = shared.lock();
            loop {
                if session() != Some(st.session) {
                    drop(st);
                    return http::respond(&stream, 410, "");
                }
                if let Some(cmd) = st.queue.pop_front() {
                    drop(st);
                    return http::respond(&stream, 200, &cmd.to_string());
                }
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    drop(st);
                    return http::respond(&stream, 204, "");
                }
                st = shared.changed.wait_timeout(st, left).unwrap_or_else(|e| e.into_inner()).0;
            }
        }
        ("POST", "/reply") => {
            let mut st = shared.lock();
            if session() != Some(st.session) {
                drop(st);
                return http::respond(&stream, 410, "");
            }
            match Json::parse(&req.body) {
                Ok(reply) => {
                    if let Some(id) = reply.get("id").and_then(Json::as_f64) {
                        st.replies.insert(id as u64, reply);
                        shared.changed.notify_all();
                    }
                    drop(st);
                    http::respond(&stream, 204, "");
                }
                Err(_) => {
                    drop(st);
                    http::respond(&stream, 400, "");
                }
            }
        }
        _ => http::respond(&stream, 404, ""),
    }
}
