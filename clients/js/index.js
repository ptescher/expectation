// The Expectation driver for JavaScript apps (React Native, web, Node): bind the vocabulary to
// the app, and the Expectation runner can drive it. See https://expectation.sim001.net/guide/remote.html
//
//   const driver = new ExpectationDriver({ platform: 'React Native', app: 'Kettle' })
//     .observe('the kettle', () => kettle.phase)
//     .set('the dial', v => kettle.setDial(v))
//     .on('the user presses lock', () => kettle.pressLock());
//   driver.start('http://127.0.0.1:4477');
//
// Handlers may be async. The driver says hello, long-polls the runner for commands and
// reconnects when it goes away, so it can stay in a development build.
'use strict';

const DEFAULT_URL = 'http://127.0.0.1:4477';

const sleep = ms => new Promise(r => setTimeout(r, ms));

async function request(url, options, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(url, { ...options, signal: controller.signal });
    const text = await res.text();
    return { status: res.status, body: text ? JSON.parse(text) : null };
  } finally {
    clearTimeout(timer);
  }
}

class ExpectationDriver {
  constructor({ platform, app }) {
    this.platform = platform;
    this.app = app;
    this.observers = new Map();
    this.setters = new Map();
    this.arrangers = new Map();
    this.events = new Map();
    this.resetHandler = null;
    this.screenshotHandler = null;
    this.running = false;
  }

  observe(subject, f) { this.observers.set(subject, f); return this; }
  set(subject, f) { this.setters.set(subject, f); return this; }
  arrange(subject, f) { this.arrangers.set(subject, f); return this; }
  on(event, f) { this.events.set(event, f); return this; }
  onReset(f) { this.resetHandler = f; return this; }
  /** `f` returns a base64 PNG (or a promise of one). Without it, the runner can take its own. */
  onScreenshot(f) { this.screenshotHandler = f; return this; }

  hello() {
    return {
      platform: this.platform,
      app: this.app,
      observe: [...this.observers.keys()],
      set: [...this.setters.keys()],
      arrange: [...this.arrangers.keys()],
      events: [...this.events.keys()],
      screenshot: !!this.screenshotHandler,
      reset: !!this.resetHandler,
    };
  }

  /** Connects to the runner at `url` (http://127.0.0.1:4477 by default) and serves it. */
  start(url = DEFAULT_URL) {
    if (this.running) return;
    this.running = true;
    this.loop(url.replace(/\/$/, ''));
  }

  stop() { this.running = false; }

  async loop(base) {
    const json = body => ({ method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
    while (this.running) {
      let session;
      try {
        const hello = await request(`${base}/hello`, json(this.hello()), 5000);
        session = hello.status === 200 && hello.body && hello.body.session;
      } catch (_) {}
      if (!session) { await sleep(1000); continue; }
      while (this.running) {
        let next;
        try {
          next = await request(`${base}/next?session=${session}`, { method: 'GET' }, 30000);
        } catch (_) { break; }
        if (next.status === 204) continue;
        if (next.status !== 200 || !next.body) break;
        const reply = { id: next.body.id, ...(await this.handle(next.body)) };
        try {
          const posted = await request(`${base}/reply?session=${session}`, json(reply), 5000);
          if (posted.status !== 204) break;
        } catch (_) { break; }
      }
    }
  }

  async handle(c) {
    try {
      switch (c.op) {
        case 'reset':
          if (this.resetHandler) await this.resetHandler();
          return {};
        case 'arrange': {
          const f = this.arrangers.get(c.subject) || this.setters.get(c.subject);
          if (!f) return { error: `can't arrange ${c.subject}` };
          await f(c.value);
          return {};
        }
        case 'set': {
          const f = this.setters.get(c.subject);
          if (!f) return { error: `can't set ${c.subject}` };
          await f(c.value);
          return {};
        }
        case 'perform': {
          const f = this.events.get(c.event);
          if (!f) return { error: `can't make ${c.event} happen` };
          await f();
          return {};
        }
        case 'observe': {
          const f = this.observers.get(c.subject);
          if (!f) return { error: `can't observe ${c.subject}` };
          return { value: await f() };
        }
        case 'screenshot': {
          if (!this.screenshotHandler) return { error: 'no screenshot' };
          return { png: await this.screenshotHandler() };
        }
        default:
          return { error: `unknown command ${c.op}` };
      }
    } catch (e) {
      return { error: String(e && e.message || e) };
    }
  }
}

module.exports = { ExpectationDriver, DEFAULT_URL };
