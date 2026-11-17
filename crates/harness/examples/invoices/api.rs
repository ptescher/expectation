//! A stand-in invoices API, served over real HTTP on a loopback port, and a client for it that
//! keeps every request it sends as an [`Exchange`]. Only the standard library: a test or an
//! example can start it without anything installed.
//!
//! The service keeps its invoices in memory. Two API keys are known: the admin's and a member's.
//! Members can create, finalize and read invoices; only admins can void one.

use expectation_harness::Exchange;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

pub const ADMIN_KEY: &str = "sk_test_admin_9f8e7d6c5b4a";
pub const MEMBER_KEY: &str = "sk_test_member_1a2b3c4d5e6f";

#[derive(Clone, Debug)]
struct Invoice {
    id: String,
    customer: String,
    amount_due: u64,
    status: &'static str,
    created_by: &'static str,
}

impl Invoice {
    fn json(&self) -> String {
        format!(
            r#"{{"id":"{}","object":"invoice","customer":"{}","amount_due":{},"currency":"usd","status":"{}","created_by":{{"email":"{}"}}}}"#,
            self.id, self.customer, self.amount_due, self.status, self.created_by
        )
    }
}

#[derive(Default)]
struct State {
    invoices: Vec<Invoice>,
    next: u32,
    /// A defect, for the example's `--bug`: members can void invoices too.
    members_can_void: bool,
}

/// The service, running until the process ends.
pub struct Service {
    pub url: String,
    state: Arc<Mutex<State>>,
}

impl Service {
    pub fn start() -> Service {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state: Arc<Mutex<State>> = Default::default();
        let shared = state.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = shared.clone();
                std::thread::spawn(move || serve(stream, &state));
            }
        });
        Service { url, state }
    }

    /// Forget every invoice: a fresh account for the next test.
    pub fn reset(&self) {
        let mut s = self.state.lock().unwrap();
        s.invoices.clear();
    }

    /// A defect to see a failure: members can void invoices too.
    pub fn let_members_void(&self) {
        self.state.lock().unwrap().members_can_void = true;
    }
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

fn serve(stream: TcpStream, state: &Mutex<State>) {
    let mut reader = BufReader::new(&stream);
    let Some(req) = read_request(&mut reader) else {
        return;
    };
    let (status, body) = route(&req, &mut state.lock().unwrap());
    let mut out = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Request-Id: req_{:x}\r\nConnection: close\r\n",
        expectation_story::exchange::reason(status),
        body.len(),
        expectation_story::now_ms()
    );
    if status == 201
        && let Some(id) = field(&body, "id")
    {
        out.push_str(&format!("Location: {}/{id}\r\n", req.path));
    }
    out.push_str("\r\n");
    out.push_str(&body);
    let _ = (&stream).write_all(out.as_bytes());
}

fn read_request(reader: &mut impl BufRead) -> Option<Request> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_string(), parts.next()?.to_string());
    let mut headers = vec![];
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let (k, v) = h.split_once(':')?;
        headers.push((k.trim().to_string(), v.trim().to_string()));
    }
    let len = header(&headers, "content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; len];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        headers,
        body: String::from_utf8(body).ok()?,
    })
}

fn error(status: u16, kind: &str, message: &str) -> (u16, String) {
    (
        status,
        format!(r#"{{"error":{{"type":"{kind}","message":"{message}"}}}}"#),
    )
}

fn route(req: &Request, s: &mut State) -> (u16, String) {
    let admin = match header(&req.headers, "authorization") {
        Some(v) if v == format!("Bearer {ADMIN_KEY}") => true,
        Some(v) if v == format!("Bearer {MEMBER_KEY}") => false,
        _ => return error(401, "unauthorized", "Pass your API key as a bearer token."),
    };
    let parts: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    match (req.method.as_str(), parts.as_slice()) {
        ("POST", ["v1", "invoices"]) => {
            let Some(customer) = field(&req.body, "customer") else {
                return error(422, "invalid_request", "An invoice needs a customer.");
            };
            s.next += 1;
            let invoice = Invoice {
                id: format!("in_{:04}", s.next),
                customer,
                amount_due: number(&req.body, "amount_due").unwrap_or(0),
                status: "draft",
                created_by: if admin {
                    "ada@example.com"
                } else {
                    "grace@example.com"
                },
            };
            let body = invoice.json();
            s.invoices.push(invoice);
            (201, body)
        }
        (method, ["v1", "invoices", id, rest @ ..]) => {
            let Some(invoice) = s.invoices.iter_mut().find(|i| i.id == *id) else {
                return error(404, "not_found", "No such invoice.");
            };
            match (method, rest) {
                ("GET", []) => (200, invoice.json()),
                ("POST", ["finalize"]) if invoice.status == "draft" => {
                    invoice.status = "open";
                    (200, invoice.json())
                }
                ("POST", ["void"]) if !admin && !s.members_can_void => error(
                    403,
                    "forbidden",
                    "Only an admin of the account can void an invoice.",
                ),
                ("POST", ["void"]) if invoice.status == "open" => {
                    invoice.status = "void";
                    (200, invoice.json())
                }
                ("POST", ["finalize"]) => error(
                    409,
                    "invalid_state",
                    &format!(
                        "Only a draft invoice can be finalized; this one is {}.",
                        invoice.status
                    ),
                ),
                ("POST", ["void"]) => error(
                    409,
                    "invalid_state",
                    &format!(
                        "Only an open invoice can be voided; this one is {}.",
                        invoice.status
                    ),
                ),
                _ => error(404, "not_found", "No such route."),
            }
        }
        _ => error(404, "not_found", "No such route."),
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// A string field of a flat JSON object: `"id":"in_1"` → `in_1`. Enough for the bodies this
/// service writes and the example sends.
pub fn field(json: &str, name: &str) -> Option<String> {
    let key = format!("\"{name}\"");
    let at = json.find(&key)? + key.len();
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

fn number(json: &str, name: &str) -> Option<u64> {
    let key = format!("\"{name}\"");
    let at = json.find(&key)? + key.len();
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// A client for the service, as a caller would write one: it sends the API key it is given,
/// and keeps every request it sends, with what came back, until they are taken.
pub struct Client {
    pub base: String,
    pub log: Vec<Exchange>,
}

impl Client {
    pub fn new(base: &str) -> Client {
        Client {
            base: base.to_string(),
            log: vec![],
        }
    }

    /// Send a request and say what came back: the status and the body.
    pub fn send(
        &mut self,
        key: &str,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> (u16, String) {
        let host = self.base.trim_start_matches("http://");
        let mut x = Exchange::new(method, format!("{}{path}", self.base))
            .header("Authorization", format!("Bearer {key}"))
            .header("User-Agent", "invoices-example/1.0");
        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {key}\r\nUser-Agent: invoices-example/1.0\r\nConnection: close\r\n"
        );
        if let Some(b) = body {
            x = x.header("Content-Type", "application/json").body(b);
            req.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                b.len()
            ));
        }
        req.push_str("\r\n");
        req.push_str(body.unwrap_or(""));
        let answer = TcpStream::connect(host).and_then(|mut s| {
            s.write_all(req.as_bytes())?;
            let mut text = String::new();
            s.read_to_string(&mut text)?;
            Ok(text)
        });
        let Ok(answer) = answer else {
            self.log.push(x);
            return (0, String::new());
        };
        let (head, body) = answer.split_once("\r\n\r\n").unwrap_or((&answer, ""));
        let mut lines = head.lines();
        let status = lines
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        x = x.response(status);
        for l in lines {
            if let Some((k, v)) = l.split_once(':') {
                x = x.response_header(k.trim(), v.trim());
            }
        }
        self.log.push(x.response_body(body));
        (status, body.to_string())
    }
}

/// What the tests drive: the service, a client, the invoice the test is about and whose key
/// the client is sending.
pub struct World {
    pub service: Service,
    /// Behind a `RefCell` because observing reads the API, and reading it is a request too.
    client: std::cell::RefCell<Client>,
    invoice: Option<String>,
    member: bool,
}

impl World {
    pub fn new() -> World {
        let service = Service::start();
        let client = Client::new(&service.url).into();
        World {
            service,
            client,
            invoice: None,
            member: false,
        }
    }

    fn key(&self) -> &'static str {
        if self.member { MEMBER_KEY } else { ADMIN_KEY }
    }

    fn create(&mut self, key: &str) {
        let (status, body) = self.client.get_mut().send(
            key,
            "POST",
            "/v1/invoices",
            Some(r#"{"customer":"cus_0042","amount_due":12000,"currency":"usd"}"#),
        );
        if status == 201 {
            self.invoice = field(&body, "id");
        }
    }

    fn act(&mut self, key: &str, action: &str) {
        if let Some(id) = self.invoice.clone() {
            let path = format!("/v1/invoices/{id}/{action}");
            self.client.get_mut().send(key, "POST", &path, None);
        }
    }

    /// The invoice's `status`, read from the API.
    fn status(&self) -> String {
        let Some(id) = &self.invoice else {
            return "none".into();
        };
        let (_, body) =
            self.client
                .borrow_mut()
                .send(self.key(), "GET", &format!("/v1/invoices/{id}"), None);
        field(&body, "status").unwrap_or_else(|| "none".into())
    }

    /// Put an invoice in place as the admin, whoever the test then calls as.
    fn arrange(&mut self, status: &str) {
        self.invoice = None;
        let steps: &[&str] = match status {
            "draft" => &[],
            "open" => &["finalize"],
            "void" => &["finalize", "void"],
            _ => return,
        };
        self.create(ADMIN_KEY);
        for step in steps {
            self.act(ADMIN_KEY, step);
        }
    }
}

/// The suite's vocabulary bound to the API. With `bug`, members can void invoices.
pub fn driver(bug: bool) -> expectation_harness::Bindings<World> {
    let world = World::new();
    if bug {
        world.service.let_members_void();
    }
    expectation_harness::Bindings::new(world)
        .observe("the invoice", World::status)
        .observe(
            "the caller",
            |w: &World| {
                if w.member { "member" } else { "admin" }
            },
        )
        .arrange("the invoice", |w, v| w.arrange(v.as_str().unwrap_or("")))
        .set("the caller", |w, v| w.member = v.as_str() == Some("member"))
        .on("the caller creates an invoice", |w| w.create(w.key()))
        .on("the caller finalizes the invoice", |w| {
            w.act(w.key(), "finalize")
        })
        .on("the caller voids the invoice", |w| w.act(w.key(), "void"))
        .on_reset(|w| {
            w.service.reset();
            w.invoice = None;
            w.member = false;
        })
        .on_exchanges(|w| std::mem::take(&mut w.client.get_mut().log))
}

/// The run's options: the docs call each key by whose it is, and who created an invoice is a
/// person's email, which they don't show.
pub fn options() -> expectation_harness::RunOptions {
    expectation_harness::RunOptions {
        redaction: expectation_harness::Redaction {
            keys: vec![
                (ADMIN_KEY.into(), "$ADMIN_API_KEY".into()),
                (MEMBER_KEY.into(), "$MEMBER_API_KEY".into()),
            ],
            fields: vec!["email".into()],
            ..Default::default()
        },
        ..Default::default()
    }
}
