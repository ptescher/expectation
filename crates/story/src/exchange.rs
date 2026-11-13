//! HTTP exchanges: the requests a step made and the responses it got, kept as evidence a manual
//! can show. Where a screenshot shows a person what to expect on screen, an exchange shows a
//! developer the request to send and what comes back: the docs draw it as a `curl` command and a
//! response.
//!
//! Unlike an [`crate::Attachment`], an exchange is made to be published, so it is redacted before
//! it is kept (see [`Redaction`]): only a short list of headers survives, credentials become
//! `$API_KEY`, and the values of secret-looking query parameters and JSON fields are replaced.

use std::fmt::Write;

/// What an exchange did in its step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Putting the test's start in place ("Given …").
    Setup,
    /// The step's action: what the reader is told to do.
    Action,
    /// Reading the system back to check the step's Thens.
    Check,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Setup => "setup",
            Role::Action => "action",
            Role::Check => "check",
        }
    }

    fn parse(s: &str) -> Option<Role> {
        [Role::Setup, Role::Action, Role::Check]
            .into_iter()
            .find(|r| r.name() == s)
    }
}

/// One request and its response.
///
/// ```
/// use expectation_story::Exchange;
/// let x = Exchange::new("POST", "http://127.0.0.1:8080/v1/invoices")
///     .header("Content-Type", "application/json")
///     .body(r#"{"customer":"cus_1"}"#)
///     .response(201)
///     .response_header("Content-Type", "application/json")
///     .response_body(r#"{"id":"in_1","status":"draft"}"#);
/// assert_eq!(x.status, 201);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exchange {
    /// Set by the runner from when the driver handed it over.
    pub role: Role,
    pub method: String,
    /// The whole URL, with its query.
    pub url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<String>,
    /// 0 if no response came (the connection failed).
    pub status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: Option<String>,
}

impl Exchange {
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Exchange {
            role: Role::Action,
            method: method.into().to_ascii_uppercase(),
            url: url.into(),
            request_headers: vec![],
            request_body: None,
            status: 0,
            response_headers: vec![],
            response_body: None,
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.request_headers.push((name.into(), value.into()));
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.request_body = Some(body.into());
        self
    }

    pub fn response(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    pub fn response_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.response_headers.push((name.into(), value.into()));
        self
    }

    pub fn response_body(mut self, body: impl Into<String>) -> Self {
        self.response_body = Some(body.into());
        self
    }

    /// The path and query, without the origin: `/v1/invoices?limit=2`.
    pub fn path(&self) -> &str {
        let (_, rest) = split_origin(&self.url);
        if rest.is_empty() { "/" } else { rest }
    }

    /// `POST /v1/invoices → 201`, for a caption or a list.
    pub fn label(&self) -> String {
        let path = self.path();
        let path = path.split_once('?').map_or(path, |(p, _)| p);
        match self.status {
            0 => format!("{} {path} (no response)", self.method),
            s => format!("{} {path} → {s}", self.method),
        }
    }

    /// A request header's value, by name ignoring case.
    pub fn request_header(&self, name: &str) -> Option<&str> {
        find(&self.request_headers, name)
    }

    /// A response header's value, by name ignoring case.
    pub fn response_header_value(&self, name: &str) -> Option<&str> {
        find(&self.response_headers, name)
    }
}

fn find<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// `("http://127.0.0.1:8080", "/v1/x?y=1")`, and `("$BASE_URL", "/v1/x")` once redacted. A URL
/// without a scheme is all path.
fn split_origin(url: &str) -> (&str, &str) {
    if let Some(n) = variable(url) {
        return url.split_at(n);
    }
    let Some(i) = url.find("://") else {
        return ("", url);
    };
    match url[i + 3..].find(['/', '?']) {
        Some(j) => url.split_at(i + 3 + j),
        None => (url, ""),
    }
}

/// What may be published of an exchange. [`Redaction::apply`] is the only way the runner keeps
/// one, so a driver that hands over everything it sent still publishes nothing secret:
///
/// - Request headers other than [`Redaction::request_headers`] and response headers other than
///   [`Redaction::response_headers`] are dropped, and with them cookies and anything a gateway
///   added.
/// - `Authorization` keeps its scheme and its credential becomes `$API_KEY`; so does the value of
///   each of [`Redaction::key_headers`] (`X-Api-Key`). A key listed in [`Redaction::keys`]
///   becomes its own name instead (`$MEMBER_API_KEY`), so the docs can say whose it was.
/// - Query parameters, form fields and JSON fields named like a secret (a `password`, a `token`,
///   a `client_secret`, an `api_key`), and those in [`Redaction::fields`], read `REDACTED`.
/// - A loopback origin (`http://127.0.0.1:53211`) becomes `$BASE_URL`, or
///   [`Redaction::base_url`] replaces every origin.
/// - JSON bodies are pretty-printed. A JSON body that doesn't parse can't be checked for secrets,
///   so it is replaced with a note; so is anything past [`Redaction::max_body`].
#[derive(Clone, Debug)]
pub struct Redaction {
    /// The origin the docs show instead of the one the tests used: `https://api.example.com`.
    pub base_url: Option<String>,
    /// Request headers to keep, besides `Authorization`. Matched ignoring case.
    pub request_headers: Vec<String>,
    /// Response headers to keep. Matched ignoring case.
    pub response_headers: Vec<String>,
    /// Headers that carry a key, kept with the value `$API_KEY`.
    pub key_headers: Vec<String>,
    /// Keys the tests use, and what the docs call each: `("sk_test_4f…", "$MEMBER_API_KEY")`. A
    /// name is `$` and capitals, digits and `_`; any other reads `$API_KEY`. The key itself is
    /// never kept.
    pub keys: Vec<(String, String)>,
    /// More fields (JSON keys, query and form parameters) whose values aren't shown, besides those
    /// named like a secret: `email`, `phone`. Matched ignoring case.
    pub fields: Vec<String>,
    /// The longest body kept, in bytes, after pretty-printing.
    pub max_body: usize,
}

impl Default for Redaction {
    fn default() -> Self {
        let list = |l: &[&str]| l.iter().map(|s| s.to_string()).collect();
        Redaction {
            base_url: None,
            request_headers: list(&[
                "Accept",
                "Content-Type",
                "Idempotency-Key",
                "If-Match",
                "If-None-Match",
                "Prefer",
            ]),
            response_headers: list(&["Content-Type", "ETag", "Location", "Retry-After"]),
            key_headers: list(&["X-Api-Key", "Api-Key"]),
            keys: vec![],
            fields: vec![],
            max_body: 16 * 1024,
        }
    }
}

/// What a secret value reads as.
pub const REDACTED: &str = "REDACTED";

impl Redaction {
    pub fn apply(&self, x: &Exchange) -> Exchange {
        let has = |l: &[String], name: &str| l.iter().any(|k| k.eq_ignore_ascii_case(name));
        let mut request_headers = vec![];
        for (k, v) in &x.request_headers {
            if k.eq_ignore_ascii_case("authorization") {
                let v = match v.trim().split_once(' ') {
                    Some((scheme, key)) => format!("{scheme} {}", self.key_name(key)),
                    None => self.key_name(v).to_string(),
                };
                request_headers.push((k.clone(), v));
            } else if has(&self.key_headers, k) {
                request_headers.push((k.clone(), self.key_name(v).to_string()));
            } else if has(&self.request_headers, k) {
                request_headers.push((k.clone(), clean(v, 500)));
            }
        }
        let response_headers = x
            .response_headers
            .iter()
            .filter(|(k, _)| has(&self.response_headers, k))
            .map(|(k, v)| {
                let v = if k.eq_ignore_ascii_case("location") {
                    self.url(v)
                } else {
                    clean(v, 500)
                };
                (k.clone(), v)
            })
            .collect();
        let request_type = find(&x.request_headers, "content-type");
        let response_type = find(&x.response_headers, "content-type");
        Exchange {
            role: x.role,
            method: clean(&x.method, 20),
            url: self.url(&x.url),
            request_headers,
            request_body: x
                .request_body
                .as_deref()
                .map(|b| self.body(b, request_type)),
            status: x.status,
            response_headers,
            response_body: x
                .response_body
                .as_deref()
                .map(|b| self.body(b, response_type)),
        }
    }

    /// What the docs call a key: its name in [`Redaction::keys`], or `$API_KEY`.
    fn key_name(&self, key: &str) -> &str {
        self.keys
            .iter()
            .find(|(k, name)| k == key.trim() && variable(name) == Some(name.len()))
            .map_or("$API_KEY", |(_, name)| name.as_str())
    }

    fn secret(&self, name: &str) -> bool {
        secret_name(name) || self.fields.iter().any(|f| f.eq_ignore_ascii_case(name))
    }

    /// The URL as the docs show it: the origin swapped, secret query values replaced.
    fn url(&self, url: &str) -> String {
        let url = clean(url, 2000);
        let (origin, rest) = split_origin(&url);
        let origin = match &self.base_url {
            Some(base) if !origin.is_empty() => base.trim_end_matches('/').to_string(),
            _ if loopback(origin) => "$BASE_URL".to_string(),
            _ => origin.to_string(),
        };
        let (path, query) = match rest.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (rest, None),
        };
        let mut out = format!("{origin}{path}");
        if let Some(q) = query {
            out.push('?');
            out.push_str(&self.form(q));
        }
        out
    }

    /// `a=1&password=x` → `a=1&password=REDACTED`.
    fn form(&self, q: &str) -> String {
        q.split('&')
            .map(|pair| match pair.split_once('=') {
                Some((k, _)) if self.secret(&percent_decode(k)) => format!("{k}={REDACTED}"),
                _ => pair.to_string(),
            })
            .collect::<Vec<_>>()
            .join("&")
    }

    fn body(&self, body: &str, media_type: Option<&str>) -> String {
        let media = media_type
            .and_then(|m| m.split(';').next())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let looks_json = media.ends_with("json")
            || media.ends_with("+json")
            || (media.is_empty() && matches!(body.trim_start().chars().next(), Some('{' | '[')));
        let out = if looks_json {
            match Json::parse(body) {
                Ok(mut j) => {
                    j.redact(&|k| self.secret(k));
                    j.pretty()
                }
                Err(_) => {
                    return format!(
                        "(a {}-byte body that isn't valid JSON, not shown)",
                        body.len()
                    );
                }
            }
        } else if media == "application/x-www-form-urlencoded" {
            self.form(body.trim())
        } else {
            body.replace("\r\n", "\n")
        };
        if out.len() > self.max_body {
            let mut cut = self.max_body;
            while !out.is_char_boundary(cut) {
                cut -= 1;
            }
            format!("{}\n… ({} more bytes)", &out[..cut], out.len() - cut)
        } else {
            out
        }
    }
}

fn loopback(origin: &str) -> bool {
    let host = origin.split_once("://").map_or("", |(_, h)| h);
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host.split(':').next().unwrap_or(""),
    };
    host == "localhost"
        || host.ends_with(".localhost")
        || host.starts_with("127.")
        || host == "::1"
        || host == "0.0.0.0"
}

/// A field named like a secret: `password`, `client_secret`, `accessToken`, `api-key`.
pub fn secret_name(name: &str) -> bool {
    // Words, split at `_`, `-`, `.`, spaces and camelCase: `clientSecret` is `client secret`.
    let mut words: Vec<String> = vec![];
    let mut word = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if !c.is_alphanumeric() {
            words.extend((!word.is_empty()).then(|| std::mem::take(&mut word)));
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower {
            words.push(std::mem::take(&mut word));
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        word.extend(c.to_lowercase());
    }
    words.extend((!word.is_empty()).then_some(word));
    let one = |w: &str| {
        matches!(
            w,
            "password"
                | "passwd"
                | "passphrase"
                | "secret"
                | "token"
                | "apikey"
                | "credential"
                | "credentials"
                | "authorization"
                | "cookie"
                | "otp"
                | "pin"
                | "cvc"
                | "cvv"
                | "ssn"
        )
    };
    let two = |a: &str, b: &str| {
        b == "key" && matches!(a, "api" | "private" | "access" | "secret" | "signing")
    };
    words.iter().any(|w| one(w)) || words.windows(2).any(|w| two(&w[0], &w[1]))
}

/// Control characters out, and at most `max` bytes.
fn clean(s: &str, max: usize) -> String {
    let mut out: String = s.chars().filter(|c| !c.is_control()).collect();
    if out.len() > max {
        let mut cut = max;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out.push('…');
    }
    out
}

fn percent_decode(s: &str) -> String {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match (
            b[i],
            b.get(i + 1).copied().and_then(hex),
            b.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
            (b'+', ..) => out.push(b' '),
            (c, ..) => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// JSON as it was written: numbers keep their text, so an id past 2^53 reads as sent.
#[derive(Clone, Debug, PartialEq)]
enum Json {
    Lit(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn parse(s: &str) -> Result<Json, String> {
        let mut p = Parser {
            s: s.as_bytes(),
            i: 0,
            depth: 0,
        };
        let v = p.value()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(format!("unexpected text at {}", p.i));
        }
        Ok(v)
    }

    /// Replace the values of the fields `secret` names, wherever they are.
    fn redact(&mut self, secret: &dyn Fn(&str) -> bool) {
        match self {
            Json::Arr(a) => a.iter_mut().for_each(|v| v.redact(secret)),
            Json::Obj(o) => {
                for (k, v) in o {
                    if secret(k) && *v != Json::Lit("null".into()) {
                        *v = Json::Str(REDACTED.into());
                    } else {
                        v.redact(secret);
                    }
                }
            }
            _ => {}
        }
    }

    fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String, indent: usize) {
        let pad = |out: &mut String, n: usize| out.extend(std::iter::repeat_n("  ", n));
        match self {
            Json::Lit(s) => out.push_str(s),
            Json::Str(s) => quote(out, s),
            Json::Arr(a) if a.is_empty() => out.push_str("[]"),
            Json::Obj(o) if o.is_empty() => out.push_str("{}"),
            Json::Arr(a) => {
                out.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    pad(out, indent + 1);
                    v.write(out, indent + 1);
                    out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
                }
                pad(out, indent);
                out.push(']');
            }
            Json::Obj(o) => {
                out.push_str("{\n");
                for (i, (k, v)) in o.iter().enumerate() {
                    pad(out, indent + 1);
                    quote(out, k);
                    out.push_str(": ");
                    v.write(out, indent + 1);
                    out.push_str(if i + 1 < o.len() { ",\n" } else { "\n" });
                }
                pad(out, indent);
                out.push('}');
            }
        }
    }
}

fn quote(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at {}", c as char, self.i))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        self.depth += 1;
        if self.depth > 128 {
            return Err("nested too deeply".into());
        }
        let v = match self.peek() {
            Some(b'{') => {
                self.i += 1;
                let mut fields = vec![];
                self.ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                } else {
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.eat(b':')?;
                        fields.push((k, self.value()?));
                        self.ws();
                        match self.peek() {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err(format!("expected `,` or `}}` at {}", self.i)),
                        }
                    }
                }
                Json::Obj(fields)
            }
            Some(b'[') => {
                self.i += 1;
                let mut items = vec![];
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                } else {
                    loop {
                        items.push(self.value()?);
                        self.ws();
                        match self.peek() {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err(format!("expected `,` or `]` at {}", self.i)),
                        }
                    }
                }
                Json::Arr(items)
            }
            Some(b'"') => Json::Str(self.string()?),
            Some(_) => {
                let start = self.i;
                while self.i < self.s.len()
                    && matches!(self.s[self.i], b'-' | b'+' | b'.' | b'0'..=b'9' | b'a'..=b'z' | b'E')
                {
                    self.i += 1;
                }
                let lit = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
                let number = lit.parse::<f64>().is_ok()
                    && lit
                        .bytes()
                        .all(|c| matches!(c, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
                    && lit
                        .bytes()
                        .next()
                        .is_some_and(|c| c == b'-' || c.is_ascii_digit());
                if !(number || matches!(lit, "true" | "false" | "null")) {
                    return Err(format!("unexpected `{lit}` at {start}"));
                }
                Json::Lit(lit.to_string())
            }
            None => return Err("unexpected end".into()),
        };
        self.depth -= 1;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("expected a string at {}", self.i));
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self.i < self.s.len() && !matches!(self.s[self.i], b'"' | b'\\') {
                if self.s[self.i] < 0x20 {
                    return Err(format!("a control character in a string at {}", self.i));
                }
                self.i += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "not UTF-8".to_string())?,
            );
            match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    let c = *self.s.get(self.i + 1).ok_or("unexpected end")?;
                    self.i += 2;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xD800..0xDC00).contains(&code)
                                && self.s.get(self.i) == Some(&b'\\')
                                && self.s.get(self.i + 1) == Some(&b'u')
                            {
                                self.i += 2;
                                let low = self.hex4()?;
                                code = 0x10000
                                    + ((code - 0xD800) << 10)
                                    + (low.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(format!("a bad escape at {}", self.i)),
                    }
                }
                _ => return Err("an unterminated string".into()),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.s.get(self.i..self.i + 4).ok_or("unexpected end")?;
        let h = std::str::from_utf8(h).map_err(|_| "a bad escape".to_string())?;
        self.i += 4;
        u32::from_str_radix(h, 16).map_err(|_| "a bad escape".into())
    }
}

/// A field of the results file: `\`, tabs and line breaks escaped, so a body fits on one line.
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(c) => out.push(c),
            None => out.push('\\'),
        }
    }
    out
}

/// The lines [`crate::Report::save`] writes for an exchange of step `n`.
pub(crate) fn save(out: &mut String, n: usize, x: &Exchange) {
    let _ = writeln!(
        out,
        "exchange\t{n}\t{}\t{}\t{}\t{}",
        x.role.name(),
        escape(&x.method),
        escape(&x.url),
        x.status
    );
    for (k, v) in &x.request_headers {
        let _ = writeln!(out, "request-header\t{}\t{}", escape(k), escape(v));
    }
    if let Some(b) = &x.request_body {
        let _ = writeln!(out, "request-body\t{}", escape(b));
    }
    for (k, v) in &x.response_headers {
        let _ = writeln!(out, "response-header\t{}\t{}", escape(k), escape(v));
    }
    if let Some(b) = &x.response_body {
        let _ = writeln!(out, "response-body\t{}", escape(b));
    }
}

/// An `exchange` line read back: its step and the exchange, still without headers or bodies.
pub(crate) fn load(f: &[&str]) -> Option<(usize, Exchange)> {
    let [_, n, role, method, url, status] = f else {
        return None;
    };
    let mut x = Exchange::new(unescape(method), unescape(url));
    x.role = Role::parse(role)?;
    x.status = status.parse().ok()?;
    Some((n.parse().ok()?, x))
}

/// What a status code says: `201` → `Created`.
pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        409 => "Conflict",
        410 => "Gone",
        412 => "Precondition Failed",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Content",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "",
    }
}

/// The request as a `curl` command, one option per line. Variables (`$BASE_URL`, `$API_KEY`: a
/// `$` and capitals, digits and `_`) are left for the shell to fill in; any other `$` is
/// escaped. A JSON body is indented under the command.
pub fn curl(x: &Exchange) -> String {
    let mut lines = vec![];
    let url = dq(&x.url);
    lines.push(match (x.method.as_str(), &x.request_body) {
        ("GET", None) => format!("curl {url}"),
        (m, _) => format!("curl -X {m} {url}"),
    });
    for (k, v) in &x.request_headers {
        lines.push(format!("  -H {}", dq(&format!("{k}: {v}"))));
    }
    if let Some(b) = &x.request_body {
        let json = x
            .request_header("content-type")
            .is_some_and(|t| t.split(';').next().unwrap_or("").trim().ends_with("json"));
        // Whitespace between JSON tokens means nothing, so it can line up with the command.
        let b = if json {
            b.replace('\n', "\n  ")
        } else {
            b.clone()
        };
        lines.push(format!("  -d {}", sq(&b)));
    }
    lines.join(" \\\n")
}

/// The response as HTTP shows it: the status line, the headers kept, a blank line and the body.
pub fn response(x: &Exchange) -> String {
    if x.status == 0 {
        return "(no response)".into();
    }
    let mut out = format!("HTTP/1.1 {} {}", x.status, reason(x.status))
        .trim_end()
        .to_string();
    for (k, v) in &x.response_headers {
        let _ = write!(out, "\n{k}: {v}");
    }
    if let Some(b) = x.response_body.as_deref().filter(|b| !b.is_empty()) {
        let _ = write!(out, "\n\n{b}");
    }
    out
}

/// How long the variable `s` starts with is (`$BASE_URL`: a `$`, a capital, then capitals,
/// digits and `_`), if it starts with one.
fn variable(s: &str) -> Option<usize> {
    let name = s.strip_prefix('$')?;
    if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    let len = name
        .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
        .unwrap_or(name.len());
    Some(1 + len)
}

/// In double quotes, for the shell, leaving variables (`$BASE_URL`) to expand.
fn dq(s: &str) -> String {
    let mut out = String::from("\"");
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if let Some(n) = variable(rest) {
            out.push_str(&rest[..n]);
            rest = &rest[n..];
            continue;
        }
        if matches!(c, '"' | '\\' | '`' | '$') {
            out.push('\\');
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out.push('"');
    out
}

/// In single quotes, for the shell: nothing in it expands.
fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoice() -> Exchange {
        Exchange::new("post", "http://127.0.0.1:53211/v1/invoices?expand=customer&access_token=abc")
            .header("Authorization", "Bearer sk_live_123")
            .header("Cookie", "session=xyz")
            .header("Content-Type", "application/json")
            .header("X-Forwarded-For", "10.0.0.1")
            .body(r#"{"customer":"cus_1","amount":12345678901234567890,"card":{"number":"4242","cvc":"123"},"client_secret":null}"#)
            .response(201)
            .response_header("Content-Type", "application/json; charset=utf-8")
            .response_header("Set-Cookie", "session=abc")
            .response_header("Location", "http://127.0.0.1:53211/v1/invoices/in_1")
            .response_body(r#"{"id":"in_1","status":"draft","lines":[],"meta":{},"owner":{"email":"a@b.c","apiKey":"k"}}"#)
    }

    #[test]
    fn redaction_keeps_what_can_be_published() {
        let r = Redaction {
            fields: vec!["email".into()],
            ..Default::default()
        };
        let x = r.apply(&invoice());
        assert_eq!(x.method, "POST");
        assert_eq!(
            x.url,
            "$BASE_URL/v1/invoices?expand=customer&access_token=REDACTED"
        );
        assert_eq!(x.label(), "POST /v1/invoices → 201");
        assert_eq!(
            x.request_headers,
            vec![
                ("Authorization".to_string(), "Bearer $API_KEY".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ]
        );
        assert_eq!(
            x.request_body.as_deref(),
            Some(
                r#"{
  "customer": "cus_1",
  "amount": 12345678901234567890,
  "card": {
    "number": "4242",
    "cvc": "REDACTED"
  },
  "client_secret": null
}"#
            )
        );
        assert_eq!(
            x.response_header_value("location"),
            Some("$BASE_URL/v1/invoices/in_1")
        );
        assert!(x.response_header_value("set-cookie").is_none());
        let body = x.response_body.unwrap();
        assert!(body.contains(r#""lines": []"#), "{body}");
        assert!(body.contains(r#""meta": {}"#), "{body}");
        assert!(body.contains(r#""email": "REDACTED""#), "{body}");
        assert!(body.contains(r#""apiKey": "REDACTED""#), "{body}");
    }

    #[test]
    fn named_keys_say_whose_they_were() {
        let r = Redaction {
            keys: vec![
                ("sk_member".into(), "$MEMBER_API_KEY".into()),
                ("sk_odd".into(), "$not a name".into()),
            ],
            ..Default::default()
        };
        let auth = |key: &str| {
            let x = Exchange::new("GET", "/").header("Authorization", format!("Bearer {key}"));
            r.apply(&x).request_headers[0].1.clone()
        };
        assert_eq!(auth("sk_member"), "Bearer $MEMBER_API_KEY");
        assert_eq!(auth("sk_admin"), "Bearer $API_KEY");
        assert_eq!(auth("sk_odd"), "Bearer $API_KEY");
        let x = Exchange::new("GET", "/").header("X-Api-Key", "sk_member");
        assert_eq!(r.apply(&x).request_headers[0].1, "$MEMBER_API_KEY");
        let c = curl(&r.apply(
            &Exchange::new("GET", "$BASE_URL/").header("Authorization", "Bearer sk_member"),
        ));
        assert_eq!(
            c,
            "curl \"$BASE_URL/\" \\\n  -H \"Authorization: Bearer $MEMBER_API_KEY\""
        );
    }

    #[test]
    fn a_base_url_replaces_every_origin() {
        let r = Redaction {
            base_url: Some("https://api.example.com/".into()),
            ..Default::default()
        };
        let x = r.apply(&Exchange::new("GET", "http://10.1.2.3:9000/v1/x"));
        assert_eq!(x.url, "https://api.example.com/v1/x");
        // Only loopback addresses are swapped without one.
        let x = Redaction::default().apply(&Exchange::new("GET", "https://api.example.com/v1/x"));
        assert_eq!(x.url, "https://api.example.com/v1/x");
        let x = Redaction::default().apply(&Exchange::new("GET", "http://[::1]:80"));
        assert_eq!(x.url, "$BASE_URL");
    }

    #[test]
    fn json_that_does_not_parse_is_not_shown() {
        let x = Exchange::new("GET", "/x")
            .response(200)
            .response_header("Content-Type", "application/json")
            .response_body(r#"{"password": "hunter2""#);
        let x = Redaction::default().apply(&x);
        let body = x.response_body.unwrap();
        assert!(!body.contains("hunter2"), "{body}");
        assert!(body.contains("isn't valid JSON"), "{body}");
    }

    #[test]
    fn form_bodies_and_long_bodies() {
        let x = Exchange::new("POST", "/oauth/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body("grant_type=client_credentials&client_secret=s3cr3t")
            .response(200)
            .response_header("Content-Type", "text/plain")
            .response_body("é".repeat(100));
        let x = Redaction {
            max_body: 60,
            ..Default::default()
        }
        .apply(&x);
        assert_eq!(
            x.request_body.as_deref(),
            Some("grant_type=client_credentials&client_secret=REDACTED")
        );
        let body = x.response_body.unwrap();
        assert!(body.ends_with("… (140 more bytes)"), "{body}");
    }

    #[test]
    fn secret_names() {
        for yes in [
            "password",
            "client_secret",
            "accessToken",
            "api-key",
            "API_KEY",
            "privateKey",
            "refresh_token",
            "card.cvc",
        ] {
            assert!(secret_name(yes), "{yes}");
        }
        for no in [
            "customer",
            "keyboard",
            "monkey",
            "status",
            "tokenizer",
            "key",
        ] {
            assert!(!secret_name(no), "{no}");
        }
    }

    #[test]
    fn curl_and_response() {
        let x = Redaction::default().apply(&invoice());
        let c = curl(&x);
        assert!(
            c.starts_with(
                "curl -X POST \"$BASE_URL/v1/invoices?expand=customer&access_token=REDACTED\" \\\n  -H \"Authorization: Bearer $API_KEY\" \\\n"
            ),
            "{c}"
        );
        assert!(c.contains("  -d '{\n    \"customer\""), "{c}");
        assert!(c.ends_with("\n  }'"), "{c}");
        let get = Exchange::new("GET", "$BASE_URL/a?q=$5 `x`");
        assert_eq!(curl(&get), r#"curl "$BASE_URL/a?q=\$5 \`x\`""#);
        let quoted = Exchange::new("POST", "/a").body("it's");
        assert_eq!(curl(&quoted), "curl -X POST \"/a\" \\\n  -d 'it'\\''s'");
        let r = response(&x);
        assert!(
            r.starts_with("HTTP/1.1 201 Created\nContent-Type: application/json; charset=utf-8\nLocation: $BASE_URL/v1/invoices/in_1\n\n{\n  \"id\": \"in_1\""),
            "{r}"
        );
        assert_eq!(response(&Exchange::new("GET", "/")), "(no response)");
        assert_eq!(
            response(&Exchange::new("GET", "/").response(299)),
            "HTTP/1.1 299"
        );
    }

    #[test]
    fn json_keeps_what_was_written() {
        let j =
            Json::parse(r#" {"a":[1,-2.5e3,true,null],"b":"\u00e9\ud83d\ude00\"\n"} "#).unwrap();
        assert_eq!(
            j.pretty(),
            "{\n  \"a\": [\n    1,\n    -2.5e3,\n    true,\n    null\n  ],\n  \"b\": \"é😀\\\"\\n\"\n}"
        );
        for bad in [
            "{",
            "[1,]",
            "{\"a\" 1}",
            "nope",
            "01x",
            "\"\u{1}\"",
            "[1] 2",
        ] {
            assert!(Json::parse(bad).is_err(), "{bad}");
        }
        let deep = "[".repeat(200) + &"]".repeat(200);
        assert!(Json::parse(&deep).is_err());
    }

    #[test]
    fn escaping_round_trips() {
        let s = "a\tb\nc\\d\re";
        assert_eq!(unescape(&escape(s)), s);
        assert!(!escape(s).contains(['\t', '\n']));
    }
}
