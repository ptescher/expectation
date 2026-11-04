// Reading HTTP Archive (HAR) captures: each entry's request, its response status and media type,
// and whether its body can be read. Headers, cookies, query values and request bodies are never
// kept; a response body is parsed only to be analysed, and never written anywhere.

import { readFileSync, statSync } from 'node:fs';

/** The largest HAR file read. */
export const MAX_HAR_BYTES = 256 * 1024 * 1024;
/** The largest response body parsed. */
export const MAX_BODY_BYTES = 5 * 1024 * 1024;

/**
 * Why a body can or can't be assessed.
 * - `ok`: JSON, complete, parsed.
 * - `empty`: there is no body (204, 304, or a size of 0).
 * - `missing`: the capture left it out.
 * - `truncated`: the capture kept only part of it.
 * - `redacted`: the capture (or a sanitizer) removed it.
 * - `not-json`: a body that isn't JSON, which this extension doesn't assess.
 * - `too-large`: over MAX_BODY_BYTES.
 * - `invalid`: said to be JSON, but isn't.
 * @typedef {'ok'|'empty'|'missing'|'truncated'|'redacted'|'not-json'|'too-large'|'invalid'} BodyState
 *
 * @typedef {{ run?: string, test?: string, step?: number, source?: string, redacted?: boolean }} Declared
 * @typedef {{ index: number, method: string, url: URL, start: number, end: number,
 *             status: number, mime: string, body: { state: BodyState, value?: unknown },
 *             stream: boolean, declared: Declared | null }} Entry
 */

const UNUSABLE = 'unusable';

/** Media type without parameters, lower case: `application/json`. */
export function mediaType(s) {
  return String(s ?? '')
    .split(';')[0]
    .trim()
    .toLowerCase();
}

/** Is this media type JSON (`application/json`, `application/problem+json`)? */
export function isJson(mime) {
  return mime === 'application/json' || /^[a-z]+\/[a-z0-9.+-]*\+json$/.test(mime);
}

function header(headers, name) {
  if (!Array.isArray(headers)) return undefined;
  const h = headers.find((x) => typeof x?.name === 'string' && x.name.toLowerCase() === name);
  return typeof h?.value === 'string' ? h.value : undefined;
}

/** What the response body is, and its value when it can be read. */
function body(entry, mime, status) {
  const content = entry.response?.content ?? {};
  const marks = [content.comment, entry.comment].filter((c) => typeof c === 'string');
  if (entry._expectation?.redacted === true || content._redacted === true || marks.some((c) => /redact/i.test(c))) {
    return { state: 'redacted' };
  }
  const text = typeof content.text === 'string' ? content.text : null;
  if (status === 204 || status === 304 || (content.size === 0 && !text)) return { state: 'empty' };
  if (text === null) return { state: 'missing' };
  if (content._truncated === true || marks.some((c) => /truncat/i.test(c))) return { state: 'truncated' };
  const bytes = content.encoding === 'base64' ? Buffer.from(text, 'base64') : Buffer.from(text, 'utf8');
  // HAR's `size` is the length of the whole body; less text than that is part of it.
  if (typeof content.size === 'number' && content.size > 0 && bytes.length < content.size) {
    return { state: 'truncated' };
  }
  if (bytes.length > MAX_BODY_BYTES) return { state: 'too-large' };
  if (!isJson(mime)) return { state: 'not-json' };
  if (bytes.length === 0) return { state: 'empty' };
  try {
    return { state: 'ok', value: JSON.parse(bytes.toString('utf8')) };
  } catch {
    return { state: 'invalid' };
  }
}

/** One HAR entry, as this extension needs it, or a reason it can't be read. */
export function entry(e, index) {
  if (typeof e !== 'object' || e === null) return { [UNUSABLE]: 'not an object' };
  const method = typeof e.request?.method === 'string' ? e.request.method.toUpperCase() : null;
  let url;
  try {
    url = new URL(e.request?.url);
  } catch {
    return { [UNUSABLE]: 'no URL' };
  }
  const start = Date.parse(e.startedDateTime);
  if (!method || Number.isNaN(start)) return { [UNUSABLE]: 'no method or start time' };
  const time = typeof e.time === 'number' && e.time >= 0 ? e.time : 0;
  const status = Number(e.response?.status ?? 0);
  const mime = mediaType(e.response?.content?.mimeType || header(e.response?.headers, 'content-type'));
  const stream =
    status === 101 ||
    mime === 'text/event-stream' ||
    Array.isArray(e._webSocketMessages) ||
    e._resourceType === 'websocket';
  const d = e._expectation;
  const declared =
    d && typeof d === 'object'
      ? {
          run: typeof d.run === 'string' ? d.run : undefined,
          test: typeof d.test === 'string' ? d.test : undefined,
          step: Number.isInteger(d.step) && d.step >= 0 ? d.step : undefined,
          source: typeof d.source === 'string' ? d.source : undefined,
          redacted: d.redacted === true,
        }
      : null;
  return {
    index,
    method,
    url,
    start,
    end: start + time,
    status,
    mime,
    body: body(e, mime, status),
    stream,
    declared,
  };
}

/** The entries of a HAR document, and how many couldn't be read. */
export function parseHar(text, name) {
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (err) {
    throw new Error(`${name}: not JSON (${err.message})`);
  }
  const raw = doc?.log?.entries;
  if (!Array.isArray(raw)) throw new Error(`${name}: not a HAR file (no log.entries)`);
  const entries = [];
  let unusable = 0;
  raw.forEach((e, i) => {
    const x = entry(e, i);
    if (UNUSABLE in x) unusable += 1;
    else entries.push(x);
  });
  return { entries, unusable };
}

export function readHar(path) {
  const size = statSync(path).size;
  if (size > MAX_HAR_BYTES) {
    throw new Error(`${path}: over ${MAX_HAR_BYTES >> 20} MiB, so not read`);
  }
  return parseHar(readFileSync(path, 'utf8'), path);
}
