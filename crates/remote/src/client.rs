//! The embedded side, for apps written in Rust: bind the vocabulary to the app with
//! [`Handlers`], connect with [`Client::connect`], and hand each [`Request`] to the handlers on
//! whatever thread owns the app (a GUI's main thread, say):
//!
//! ```ignore
//! let handlers = Handlers::new()
//!     .observe("the kettle", |k: &Kettle| k.phase_name())
//!     .set("the dial", |k, v| k.dial = v.as_f64().unwrap())
//!     .on("the user presses lock", Kettle::press_lock);
//! let client = Client::connect_env(handlers.hello("desktop", "Kettle"));
//! // On the app's thread, every frame or so:
//! while let Some(request) = client.try_recv() {
//!     handlers.handle(&mut kettle, request);
//! }
//! ```
//!
//! The client runs on its own thread, says hello, long-polls for commands and reconnects when
//! the runner goes away, so an app can embed it and forget it. Without a runner it does nothing.

use crate::http;
use crate::json::{Json, base64_encode};
use crate::{App, DEFAULT_ADDR};
use expectation_model::Value;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// What the runner asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Reset,
    Arrange { subject: String, value: Value },
    Set { subject: String, value: Value },
    Perform { event: String },
    Observe { subject: String },
    Screenshot,
}

/// What the app answers.
#[derive(Clone, Debug)]
pub enum Reply {
    Done,
    Value(Value),
    Png(Vec<u8>),
    Svg(String),
}

/// A command waiting for its answer. Answer every one, or the runner times out.
pub struct Request {
    pub command: Command,
    answer: Sender<Result<Reply, String>>,
}

impl Request {
    pub fn reply(self, result: Result<Reply, String>) {
        let _ = self.answer.send(result);
    }
}

/// A connection to the runner, from inside the app.
pub struct Client {
    requests: Receiver<Request>,
}

impl Client {
    /// Connects to `EXPECTATION_URL` (`http://127.0.0.1:4477` when it isn't set).
    pub fn connect_env(hello: App) -> Client {
        let url = std::env::var("EXPECTATION_URL").unwrap_or_default();
        let addr = url.trim_start_matches("http://").trim_end_matches('/');
        Client::connect(if addr.is_empty() { DEFAULT_ADDR } else { addr }, hello)
    }

    /// Connects to the runner at `addr` ("127.0.0.1:4477").
    pub fn connect(addr: &str, hello: App) -> Client {
        let (tx, rx) = channel();
        let addr = addr.to_string();
        std::thread::spawn(move || run(&addr, &hello, &tx));
        Client { requests: rx }
    }

    /// The next command, if one has arrived.
    pub fn try_recv(&self) -> Option<Request> {
        self.requests.try_recv().ok()
    }

    /// The next command, waiting up to `timeout` for one.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Request> {
        self.requests.recv_timeout(timeout).ok()
    }
}

fn run(addr: &str, hello: &App, requests: &Sender<Request>) {
    let short = Duration::from_secs(5);
    loop {
        // Say hello until a runner answers.
        let session =
            match http::request(addr, "POST", "/hello", &hello.to_json().to_string(), short) {
                Ok((200, body)) => Json::parse(&body)
                    .ok()
                    .and_then(|j| j.get("session").and_then(Json::as_f64)),
                _ => None,
            };
        let Some(session) = session else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let session = session as u64;
        loop {
            let next = http::request(
                addr,
                "GET",
                &format!("/next?session={session}"),
                "",
                Duration::from_secs(30),
            );
            let command = match next {
                Ok((200, body)) => match Json::parse(&body) {
                    Ok(c) => c,
                    Err(_) => continue,
                },
                Ok((204, _)) => continue,
                _ => break,
            };
            let id = command.get("id").cloned().unwrap_or(Json::Null);
            let reply = match parse(&command) {
                Ok(command) => {
                    let (tx, rx) = channel();
                    if requests
                        .send(Request {
                            command,
                            answer: tx,
                        })
                        .is_err()
                    {
                        return; // The app dropped its client.
                    }
                    rx.recv()
                        .unwrap_or_else(|_| Err("the app dropped the request".into()))
                }
                Err(e) => Err(e),
            };
            let mut body = vec![("id", id)];
            match reply {
                Ok(Reply::Done) => {}
                Ok(Reply::Value(Value::Enum(s))) => body.push(("value", Json::Str(s))),
                Ok(Reply::Value(Value::Number(n))) => body.push(("value", Json::Num(n))),
                Ok(Reply::Png(bytes)) => body.push(("png", Json::Str(base64_encode(&bytes)))),
                Ok(Reply::Svg(svg)) => body.push(("svg", Json::Str(svg))),
                Err(e) => body.push(("error", Json::Str(e))),
            }
            let posted = http::request(
                addr,
                "POST",
                &format!("/reply?session={session}"),
                &Json::obj(body).to_string(),
                short,
            );
            if !matches!(posted, Ok((204, _))) {
                break;
            }
        }
    }
}

fn parse(c: &Json) -> Result<Command, String> {
    let text = |k: &str| {
        c.get(k)
            .and_then(Json::as_str)
            .map(String::from)
            .ok_or_else(|| format!("no {k}"))
    };
    let value = || match c.get("value") {
        Some(Json::Str(s)) => Ok(Value::Enum(s.clone())),
        Some(Json::Num(n)) => Ok(Value::Number(*n)),
        _ => Err("no value".to_string()),
    };
    Ok(match c.get("op").and_then(Json::as_str) {
        Some("reset") => Command::Reset,
        Some("arrange") => Command::Arrange {
            subject: text("subject")?,
            value: value()?,
        },
        Some("set") => Command::Set {
            subject: text("subject")?,
            value: value()?,
        },
        Some("perform") => Command::Perform {
            event: text("event")?,
        },
        Some("observe") => Command::Observe {
            subject: text("subject")?,
        },
        Some("screenshot") => Command::Screenshot,
        other => return Err(format!("unknown command {other:?}")),
    })
}

type Observer<W> = Box<dyn Fn(&W) -> Value + Send>;
type Setter<W> = Box<dyn Fn(&mut W, &Value) + Send>;
type Action<W> = Box<dyn Fn(&mut W) + Send>;
type Shooter<W> = Box<dyn Fn(&W) -> Result<Reply, String> + Send>;

/// The vocabulary bound to an app of type `W`, one closure per term, like
/// `expectation_harness::Bindings` but on the app's side of the wire.
pub struct Handlers<W> {
    observers: Vec<(String, Observer<W>)>,
    setters: Vec<(String, Setter<W>)>,
    arrangers: Vec<(String, Setter<W>)>,
    events: Vec<(String, Action<W>)>,
    reset: Option<Action<W>>,
    screenshot: Option<Shooter<W>>,
}

impl<W> Default for Handlers<W> {
    fn default() -> Self {
        Handlers {
            observers: vec![],
            setters: vec![],
            arrangers: vec![],
            events: vec![],
            reset: None,
            screenshot: None,
        }
    }
}

impl<W> Handlers<W> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe<V: Into<Value>>(
        mut self,
        subject: &str,
        f: impl Fn(&W) -> V + Send + 'static,
    ) -> Self {
        self.observers
            .push((subject.into(), Box::new(move |w| f(w).into())));
        self
    }

    pub fn set(mut self, subject: &str, f: impl Fn(&mut W, &Value) + Send + 'static) -> Self {
        self.setters.push((subject.into(), Box::new(f)));
        self
    }

    pub fn arrange(mut self, subject: &str, f: impl Fn(&mut W, &Value) + Send + 'static) -> Self {
        self.arrangers.push((subject.into(), Box::new(f)));
        self
    }

    pub fn on(mut self, event: &str, f: impl Fn(&mut W) + Send + 'static) -> Self {
        self.events.push((event.into(), Box::new(f)));
        self
    }

    pub fn on_reset(mut self, f: impl Fn(&mut W) + Send + 'static) -> Self {
        self.reset = Some(Box::new(f));
        self
    }

    pub fn on_screenshot(
        mut self,
        f: impl Fn(&W) -> Result<Reply, String> + Send + 'static,
    ) -> Self {
        self.screenshot = Some(Box::new(f));
        self
    }

    /// The hello that tells the runner what these handlers can do.
    pub fn hello(&self, platform: &str, app: &str) -> App {
        let names = |v: Vec<&String>| v.into_iter().cloned().collect();
        App {
            platform: platform.into(),
            name: app.into(),
            observe: names(self.observers.iter().map(|(k, _)| k).collect()),
            set: names(self.setters.iter().map(|(k, _)| k).collect()),
            arrange: names(self.arrangers.iter().map(|(k, _)| k).collect()),
            events: names(self.events.iter().map(|(k, _)| k).collect()),
            screenshot: self.screenshot.is_some(),
            reset: self.reset.is_some(),
        }
    }

    /// Runs the command on `world` and answers it.
    pub fn handle(&self, world: &mut W, request: Request) {
        let result = self.run(world, &request.command);
        request.reply(result);
    }

    fn run(&self, world: &mut W, command: &Command) -> Result<Reply, String> {
        fn find<'a, T>(list: &'a [(String, T)], name: &str) -> Result<&'a T, String> {
            list.iter()
                .find(|(k, _)| k == name)
                .map(|(_, f)| f)
                .ok_or_else(|| format!("no handler for {name}"))
        }
        match command {
            Command::Reset => {
                if let Some(f) = &self.reset {
                    f(world);
                }
            }
            Command::Arrange { subject, value } => match find(&self.arrangers, subject) {
                Ok(f) => f(world, value),
                Err(_) => find(&self.setters, subject)?(world, value),
            },
            Command::Set { subject, value } => find(&self.setters, subject)?(world, value),
            Command::Perform { event } => find(&self.events, event)?(world),
            Command::Observe { subject } => {
                return Ok(Reply::Value(find(&self.observers, subject)?(world)));
            }
            Command::Screenshot => {
                return match &self.screenshot {
                    Some(f) => f(world),
                    None => Err("this app takes no screenshots".into()),
                };
            }
        }
        Ok(Reply::Done)
    }
}
