// Reading an OpenAPI 3.0 or 3.1 document (JSON or YAML) and what it says about responses:
// operations and how a request is matched to one, the responses each documents, and the body
// variants (enum values, oneOf/anyOf branches) a response schema distinguishes.
//
// Only local references (`#/...`) are followed. Schema keywords that this extension doesn't
// assess are reported where they appear, and anything under them is unknown rather than guessed.

import { createHash } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';
import { parse as parseYaml } from 'yaml';
import { isJson } from './har.js';

export const MAX_SPEC_BYTES = 16 * 1024 * 1024;
const METHODS = ['get', 'put', 'post', 'delete', 'options', 'head', 'patch', 'trace'];
/** How deep a schema or a body is followed. */
const MAX_DEPTH = 32;
/** The most values of one enum listed as items. */
const MAX_ENUM = 100;
/** Keywords that decide what a value is, which this extension can't assess. */
const UNSUPPORTED = [
  'not',
  'if',
  'then',
  'else',
  'patternProperties',
  'dependentSchemas',
  'dependentRequired',
  'unevaluatedProperties',
  'unevaluatedItems',
  'prefixItems',
  'contains',
  '$dynamicRef',
  '$recursiveRef',
  'propertyNames',
];
const FORBIDDEN = new Set(['__proto__', 'constructor', 'prototype']);

/** JSON Pointer escaping for one key: `/` is `~1`, `~` is `~0`. */
export function pointerKey(k) {
  return String(k).replace(/~/g, '~0').replace(/\//g, '~1');
}

/** The document in `text` (JSON or YAML), and a revision that names this exact content. */
export function parseSpec(text, name) {
  let doc;
  try {
    // YAML is a superset of JSON. Aliases are capped against "billion laughs".
    doc = parseYaml(text, { maxAliasCount: 100, uniqueKeys: false });
  } catch (err) {
    throw new Error(`${name}: not JSON or YAML (${err.message})`);
  }
  const version = typeof doc?.openapi === 'string' ? doc.openapi : null;
  if (!version || !/^3\.[01]\./.test(version)) {
    throw new Error(`${name}: not an OpenAPI 3.0 or 3.1 document (openapi: ${doc?.openapi ?? doc?.swagger ?? 'missing'})`);
  }
  const hash = createHash('sha256').update(text).digest('hex').slice(0, 16);
  const tag = typeof doc.info?.version === 'string' ? ` (version ${doc.info.version})` : '';
  return {
    doc,
    version,
    title: typeof doc.info?.title === 'string' ? doc.info.title : name,
    revision: `sha256:${hash}${tag}`,
  };
}

export function readSpec(path) {
  if (statSync(path).size > MAX_SPEC_BYTES) {
    throw new Error(`${path}: over ${MAX_SPEC_BYTES >> 20} MiB, so not read`);
  }
  return parseSpec(readFileSync(path, 'utf8'), path);
}

/** The node at a local pointer (`#/components/schemas/Quote`), or null. */
function at(doc, ref) {
  if (typeof ref !== 'string' || !ref.startsWith('#')) return null;
  let node = doc;
  for (const raw of ref.slice(1).split('/').slice(1)) {
    const key = decodeURIComponent(raw).replace(/~1/g, '/').replace(/~0/g, '~');
    if (FORBIDDEN.has(key) || node === null || typeof node !== 'object' || !Object.hasOwn(node, key)) {
      return null;
    }
    node = node[key];
  }
  return node;
}

/**
 * Follow `$ref`s from `node` (found at `loc`): the node they lead to and where it is, or why
 * they can't be followed.
 * @returns {{ node: any, loc: string } | { unsupported: string, loc: string }}
 */
export function deref(doc, node, loc) {
  const seen = new Set();
  while (node && typeof node === 'object' && typeof node.$ref === 'string') {
    const ref = node.$ref;
    if (!ref.startsWith('#')) return { unsupported: `an external $ref (${ref})`, loc };
    if (seen.has(ref)) return { unsupported: `a $ref that leads back to itself (${ref})`, loc };
    seen.add(ref);
    const next = at(doc, ref);
    if (next === null || next === undefined) return { unsupported: `a $ref to nothing (${ref})`, loc };
    node = next;
    loc = ref;
  }
  return { node, loc };
}

/** OpenAPI 3.1 lets a $ref have siblings; they apply too, as if in an allOf. */
function withSiblings(spec, node, loc) {
  if (!spec.version.startsWith('3.1') || !node || typeof node !== 'object' || typeof node.$ref !== 'string') {
    return null;
  }
  const rest = Object.keys(node).filter((k) => !['$ref', 'description', 'summary', 'title', 'example', 'examples'].includes(k));
  if (rest.length === 0) return null;
  const sib = Object.fromEntries(rest.map((k) => [k, node[k]]));
  return { allOf: [{ $ref: node.$ref }, sib], _loc: loc };
}

// ---------------------------------------------------------------------------------------------
// Operations and matching requests to them.

/** A server URL's variables filled with their defaults (and enum values), as bases to match. */
function serverBases(servers) {
  const out = [];
  for (const s of Array.isArray(servers) ? servers : []) {
    if (typeof s?.url !== 'string') continue;
    let urls = [s.url];
    for (const [name, v] of Object.entries(s.variables ?? {})) {
      const values = [v?.default, ...(Array.isArray(v?.enum) ? v.enum : [])].filter((x) => typeof x === 'string');
      urls = urls.flatMap((u) => [...new Set(values)].slice(0, 20).map((val) => u.replaceAll(`{${name}}`, val)));
    }
    for (const u of urls) out.push(base(u));
  }
  return out.length ? out : [base('/')];
}

/** A server URL as an origin (when it has one) and a path to strip. */
export function base(u) {
  try {
    const url = new URL(u);
    return { origin: url.host.toLowerCase(), protocol: url.protocol, path: url.pathname.replace(/\/+$/, '') };
  } catch {
    // Relative: `/v1`, or a path without a leading slash.
    const path = ('/' + u.replace(/^\.?\/*/, '')).replace(/\/+$/, '');
    return { origin: null, protocol: null, path };
  }
}

/** A path template as a matcher: its segments, and how many are templated. */
function template(path) {
  const segments = path.split('/').filter((s, i) => i > 0 || s !== '');
  let templated = 0;
  const parts = segments.map((seg) => {
    if (!seg.includes('{')) return { literal: decodeURIComponent(seg) };
    templated += 1;
    const re = seg
      .split(/(\{[^}]*\})/)
      .map((p) => (p.startsWith('{') ? '[^/]+' : p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')))
      .join('');
    return { regex: new RegExp(`^${re}$`) };
  });
  return { parts, templated };
}

/**
 * Every operation of the document, with what's needed to match a request to it.
 * @returns {Array<{ key: string, method: string, path: string, label: string, loc: string,
 *   op: any, bases: any[], match: ReturnType<typeof template> }>}
 */
export function operations(spec, extraServers = []) {
  const { doc } = spec;
  const out = [];
  for (const [path, rawItem] of Object.entries(doc.paths ?? {})) {
    if (FORBIDDEN.has(path)) continue;
    const r = deref(doc, rawItem, `#/paths/${pointerKey(path)}`);
    if (!('node' in r) || typeof r.node !== 'object' || r.node === null) continue;
    const item = r.node;
    for (const method of METHODS) {
      const op = item[method];
      if (!op || typeof op !== 'object') continue;
      const servers = op.servers ?? item.servers ?? doc.servers;
      out.push({
        key: `${method.toUpperCase()} ${path}`,
        method: method.toUpperCase(),
        path,
        label: `${method.toUpperCase()} ${path}`,
        loc: `#/paths/${pointerKey(path)}/${method}`,
        op,
        bases: [...serverBases(servers), ...extraServers.map(base)],
        match: template(path),
      });
    }
  }
  return out;
}

/** The origins the document's servers name, lower case; empty when they are all relative. */
export function origins(ops) {
  return [...new Set(ops.flatMap((o) => o.bases.map((b) => b.origin).filter(Boolean)))];
}

function matchesPath(match, segments) {
  if (match.parts.length !== segments.length) return false;
  return match.parts.every((p, i) => (p.literal !== undefined ? p.literal === segments[i] : p.regex.test(segments[i])));
}

/**
 * The operation a request is for: `{ op }`, or `{ none: why }`, or `{ ambiguous: [labels] }`.
 * Concrete paths win over templated ones, as OpenAPI says; two equally specific matches are
 * ambiguous, and reported as such rather than guessed.
 */
export function matchRequest(ops, method, url) {
  const host = url.host.toLowerCase();
  const candidates = [];
  const otherMethods = new Set();
  for (const o of ops) {
    for (const b of o.bases) {
      if (b.origin && (b.origin !== host || (b.protocol && b.protocol !== url.protocol))) continue;
      const path = decodeURIComponent(url.pathname);
      if (b.path && !(path === b.path || path.startsWith(b.path + '/'))) continue;
      const rest = path.slice(b.path.length);
      const segments = rest.split('/').filter((s, i) => i > 0 || s !== '');
      if (!matchesPath(o.match, segments)) continue;
      if (o.method === method) candidates.push(o);
      else otherMethods.add(o.method);
      break;
    }
  }
  if (candidates.length === 0) {
    return otherMethods.size
      ? { none: `the contract has no ${method} there (only ${[...otherMethods].join(', ')})` }
      : { none: 'no operation in the contract matches its path' };
  }
  const best = Math.min(...candidates.map((c) => c.match.templated));
  const winners = [...new Set(candidates.filter((c) => c.match.templated === best))];
  if (winners.length > 1) return { ambiguous: winners.map((w) => w.label) };
  return { op: winners[0] };
}

/**
 * A request path for a label, with anything that might be an id or a secret replaced: a segment
 * is kept only if the contract's paths use it as a literal, or it is a version (`v1`) or a
 * short lowercase word. Digits become `{n}`, UUIDs `{uuid}`, anything else `{…}`.
 */
export function safePath(url, ops = []) {
  const known = new Set(
    ops.flatMap((o) => [...o.match.parts.filter((p) => p.literal !== undefined).map((p) => p.literal), ...o.bases.flatMap((b) => b.path.split('/'))]),
  );
  return (
    url.pathname
      .split('/')
      .map((seg) => {
        if (seg === '') return seg;
        let s;
        try {
          s = decodeURIComponent(seg);
        } catch {
          return '{…}';
        }
        if (known.has(s) || /^v\d{1,2}$/.test(s) || /^[a-z][a-z_-]{0,23}$/.test(s)) return s;
        if (/^\d+$/.test(s)) return '{n}';
        if (/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(s)) return '{uuid}';
        return '{…}';
      })
      .join('/') || '/'
  );
}

// ---------------------------------------------------------------------------------------------
// Responses.

/** The documented response an observed status falls under: exact, then a range (`4XX`), then `default`. */
export function responseFor(op, status) {
  const responses = op.op.responses ?? {};
  const keys = Object.keys(responses).filter((k) => !FORBIDDEN.has(k));
  const exact = keys.find((k) => k === String(status));
  if (exact) return { key: exact, how: 'exact' };
  const range = keys.find((k) => /^[1-5]XX$/i.test(k) && k[0] === String(status)[0]);
  if (range) return { key: range, how: 'range' };
  if (keys.includes('default')) return { key: 'default', how: 'default' };
  return null;
}

/** A response object, references followed, and where it is. */
export function response(spec, op, key) {
  const loc = `${op.loc}/responses/${pointerKey(key)}`;
  return deref(spec.doc, op.op.responses?.[key], loc);
}

/** The documented media type a response's content type falls under: exact, `type/*`, then `*\/*`. */
export function mediaFor(content, mime) {
  if (!content || typeof content !== 'object') return null;
  const keys = Object.keys(content).filter((k) => !FORBIDDEN.has(k));
  const norm = (k) => k.split(';')[0].trim().toLowerCase();
  const exact = keys.find((k) => norm(k) === mime);
  if (exact) return exact;
  const family = keys.find((k) => norm(k) === `${mime.split('/')[0]}/*`);
  if (family) return family;
  return keys.find((k) => norm(k) === '*/*') ?? null;
}

// ---------------------------------------------------------------------------------------------
// Body variants: what a schema distinguishes, and which of it a body shows.

/** A variant's id within its response: where it is, and which value or branch. */
const variantKey = (loc, which) => `${loc} ${which}`;

/** Short, safe text for an enum value: JSON, cut. */
function valueText(v) {
  const s = JSON.stringify(v);
  return s.length > 40 ? s.slice(0, 37) + '…' : s;
}

function branchName(b, i) {
  if (b && typeof b === 'object') {
    if (typeof b.$ref === 'string') return b.$ref.split('/').pop();
    if (typeof b.title === 'string') return b.title;
  }
  return `#${i + 1}`;
}

/**
 * Every variant a schema distinguishes, found by walking it once: enum values, oneOf/anyOf
 * branches, and places it uses something this extension can't assess.
 * @returns {Array<{ key: string, kind: 'enum'|'branch'|'unsupported', loc: string, where: string, label: string }>}
 */
export function variants(spec, schema, loc) {
  const out = [];
  const visited = new Set();
  const walk = (node, loc, where, depth) => {
    if (depth > MAX_DEPTH) return;
    const sib = withSiblings(spec, node, loc);
    if (sib) return walk(sib, loc, where, depth + 1);
    const r = deref(spec.doc, node, loc);
    if (!('node' in r)) {
      out.push({ key: variantKey(r.loc, 'unsupported'), kind: 'unsupported', loc: r.loc, where, label: `${where}: uses ${r.unsupported}` });
      return;
    }
    ({ node, loc } = r);
    if (!node || typeof node !== 'object') return;
    if (visited.has(loc)) return;
    visited.add(loc);
    const bad = UNSUPPORTED.filter((k) => Object.hasOwn(node, k));
    if (bad.length) {
      out.push({ key: variantKey(loc, 'unsupported'), kind: 'unsupported', loc, where, label: `${where}: uses ${bad.join(', ')}` });
      return;
    }
    if (Array.isArray(node.enum)) {
      for (const v of node.enum.slice(0, MAX_ENUM)) {
        out.push({ key: variantKey(loc, `= ${JSON.stringify(v)}`), kind: 'enum', loc, where, label: `${where} = ${valueText(v)}` });
      }
    }
    for (const k of ['oneOf', 'anyOf']) {
      if (!Array.isArray(node[k])) continue;
      node[k].forEach((b, i) => {
        out.push({
          key: variantKey(`${loc}/${k}`, `#${i}`),
          kind: 'branch',
          loc: `${loc}/${k}/${i}`,
          where,
          label: `${where} is ${branchName(b, i)} (${k})`,
        });
        walk(b, `${loc}/${k}/${i}`, where, depth + 1);
      });
    }
    if (Array.isArray(node.allOf)) node.allOf.forEach((b, i) => walk(b, `${loc}/allOf/${i}`, where, depth + 1));
    if (node.properties && typeof node.properties === 'object') {
      for (const [name, p] of Object.entries(node.properties)) {
        if (FORBIDDEN.has(name)) continue;
        walk(p, `${loc}/properties/${pointerKey(name)}`, where === 'body' ? name : `${where}.${name}`, depth + 1);
      }
    }
    if (node.additionalProperties && typeof node.additionalProperties === 'object') {
      walk(node.additionalProperties, `${loc}/additionalProperties`, `${where}.*`, depth + 1);
    }
    if (node.items && typeof node.items === 'object' && !Array.isArray(node.items)) {
      walk(node.items, `${loc}/items`, `${where}[]`, depth + 1);
    } else if (Array.isArray(node.items)) {
      out.push({ key: variantKey(loc, 'unsupported'), kind: 'unsupported', loc, where, label: `${where}: uses items as a list` });
    }
  };
  walk(schema, loc, 'body', 0);
  return out;
}

/** JSON Schema's type of a value. */
function typeOf(v) {
  if (v === null) return 'null';
  if (Array.isArray(v)) return 'array';
  if (typeof v === 'number') return Number.isInteger(v) ? 'integer' : 'number';
  return typeof v;
}

function same(a, b) {
  return JSON.stringify(a) === JSON.stringify(b);
}

/**
 * Does `value` fit `node`? `true`, `false`, or `null` when it can't be told (an unsupported
 * keyword, a reference that can't be followed, too deep).
 */
export function fits(spec, node, loc, value, depth = 0) {
  if (depth > MAX_DEPTH) return null;
  const sib = withSiblings(spec, node, loc);
  if (sib) return fits(spec, sib, loc, value, depth + 1);
  const r = deref(spec.doc, node, loc);
  if (!('node' in r)) return null;
  ({ node, loc } = r);
  if (node === true || node === undefined || (node && typeof node === 'object' && Object.keys(node).length === 0)) return true;
  if (node === false) return false;
  if (typeof node !== 'object' || node === null) return null;
  if (UNSUPPORTED.some((k) => Object.hasOwn(node, k))) return null;
  let unknown = false;
  const all = (results) => {
    if (results.includes(false)) return false;
    if (results.includes(null)) unknown = true;
    return true;
  };
  const t = typeOf(value);
  const types = node.type === undefined ? null : [].concat(node.type);
  if (types) {
    const ok = types.some((x) => x === t || (x === 'number' && t === 'integer'));
    const nullable = value === null && (node.nullable === true || types.includes('null'));
    if (!ok && !nullable) return false;
  }
  if (Array.isArray(node.enum) && !node.enum.some((e) => same(e, value))) {
    if (!(value === null && node.nullable === true)) return false;
  }
  if (Object.hasOwn(node, 'const') && !same(node.const, value)) return false;
  if (t === 'string') {
    if (typeof node.minLength === 'number' && [...value].length < node.minLength) return false;
    if (typeof node.maxLength === 'number' && [...value].length > node.maxLength) return false;
    if (typeof node.pattern === 'string') {
      try {
        if (!new RegExp(node.pattern, 'u').test(value)) return false;
      } catch {
        unknown = true;
      }
    }
  }
  if (t === 'number' || t === 'integer') {
    if (typeof node.minimum === 'number' && value < node.minimum) return false;
    if (typeof node.maximum === 'number' && value > node.maximum) return false;
    if (typeof node.exclusiveMinimum === 'number' && value <= node.exclusiveMinimum) return false;
    if (typeof node.exclusiveMaximum === 'number' && value >= node.exclusiveMaximum) return false;
  }
  if (t === 'object') {
    for (const req of Array.isArray(node.required) ? node.required : []) {
      if (!Object.hasOwn(value, req)) return false;
    }
    const props = node.properties && typeof node.properties === 'object' ? node.properties : {};
    for (const [k, v] of Object.entries(value)) {
      if (Object.hasOwn(props, k)) {
        if (!all([fits(spec, props[k], `${loc}/properties/${pointerKey(k)}`, v, depth + 1)])) return false;
      } else if (node.additionalProperties === false) {
        return false;
      } else if (node.additionalProperties && typeof node.additionalProperties === 'object') {
        if (!all([fits(spec, node.additionalProperties, `${loc}/additionalProperties`, v, depth + 1)])) return false;
      }
    }
  }
  if (t === 'array') {
    if (typeof node.minItems === 'number' && value.length < node.minItems) return false;
    if (typeof node.maxItems === 'number' && value.length > node.maxItems) return false;
    if (node.items && typeof node.items === 'object' && !Array.isArray(node.items)) {
      if (!all(value.slice(0, 1000).map((v) => fits(spec, node.items, `${loc}/items`, v, depth + 1)))) return false;
    } else if (Array.isArray(node.items)) {
      unknown = true;
    }
  }
  if (Array.isArray(node.allOf)) {
    if (!all(node.allOf.map((b, i) => fits(spec, b, `${loc}/allOf/${i}`, value, depth + 1)))) return false;
  }
  for (const k of ['oneOf', 'anyOf']) {
    if (!Array.isArray(node[k])) continue;
    const results = node[k].map((b, i) => fits(spec, b, `${loc}/${k}/${i}`, value, depth + 1));
    const yes = results.filter((x) => x === true).length;
    const maybe = results.filter((x) => x === null).length;
    if (k === 'oneOf' && yes > 1) return false;
    if (yes === 0 && maybe === 0) return false;
    if (maybe > 0 && (k === 'anyOf' ? yes === 0 : true)) unknown = true;
  }
  return unknown ? null : true;
}

/** The branch a discriminator picks, if it has one and the value names it. */
function discriminated(spec, node, k, value) {
  const d = node.discriminator;
  if (!d || typeof d.propertyName !== 'string' || typeOf(value) !== 'object') return null;
  const name = value[d.propertyName];
  if (typeof name !== 'string') return null;
  const target = d.mapping && typeof d.mapping === 'object' && Object.hasOwn(d.mapping, name) ? d.mapping[name] : null;
  const i = node[k].findIndex((b) => {
    if (typeof b?.$ref !== 'string') return false;
    if (target) return b.$ref === target || b.$ref.endsWith(`/${target}`);
    return b.$ref.split('/').pop() === name;
  });
  return i >= 0 ? i : null;
}

/**
 * What a body shows of a schema's variants. `seen` are variant keys; `undocumented` are values
 * the schema doesn't allow at an enum (described by type and length, never the value), or
 * bodies that match no branch; `unclear` are places this body can't be placed (several branches
 * fit, or something can't be assessed), by variant location.
 */
export function observe(spec, schema, loc, body) {
  const seen = new Set();
  const undocumented = [];
  const unclear = new Set();
  const walk = (node, loc, where, value, depth) => {
    if (depth > MAX_DEPTH) return;
    const sib = withSiblings(spec, node, loc);
    if (sib) return walk(sib, loc, where, value, depth + 1);
    const r = deref(spec.doc, node, loc);
    if (!('node' in r)) {
      unclear.add(variantKey(r.loc, 'unsupported'));
      return;
    }
    ({ node, loc } = r);
    if (!node || typeof node !== 'object') return;
    if (UNSUPPORTED.some((k) => Object.hasOwn(node, k))) {
      unclear.add(variantKey(loc, 'unsupported'));
      return;
    }
    if (Array.isArray(node.enum)) {
      const hit = node.enum.find((e) => same(e, value));
      if (hit !== undefined) seen.add(variantKey(loc, `= ${JSON.stringify(hit)}`));
      else if (!(value === null && node.nullable === true)) {
        const t = typeOf(value);
        const size = t === 'string' ? ` (${[...value].length} characters)` : '';
        undocumented.push(`${where}: a ${t}${size} that isn't one of its values`);
      }
    }
    for (const k of ['oneOf', 'anyOf']) {
      if (!Array.isArray(node[k])) continue;
      const picked = discriminated(spec, node, k, value);
      let chosen;
      if (picked !== null) {
        chosen = [picked];
      } else {
        const results = node[k].map((b, i) => fits(spec, b, `${loc}/${k}/${i}`, value, depth + 1));
        const yes = results.flatMap((x, i) => (x === true ? [i] : []));
        const maybe = results.some((x) => x === null);
        if (k === 'oneOf') {
          if (yes.length === 1 && !maybe) chosen = yes;
          else if (yes.length === 0 && !maybe) {
            undocumented.push(`${where}: matches no branch of its oneOf`);
            chosen = [];
          } else {
            node[k].forEach((_, i) => unclear.add(variantKey(`${loc}/${k}`, `#${i}`)));
            chosen = [];
          }
        } else {
          if (maybe) node[k].forEach((_, i) => unclear.add(variantKey(`${loc}/${k}`, `#${i}`)));
          if (yes.length === 0 && !maybe) undocumented.push(`${where}: matches no branch of its anyOf`);
          chosen = yes;
        }
      }
      for (const i of chosen) {
        seen.add(variantKey(`${loc}/${k}`, `#${i}`));
        walk(node[k][i], `${loc}/${k}/${i}`, where, value, depth + 1);
      }
    }
    if (Array.isArray(node.allOf)) node.allOf.forEach((b, i) => walk(b, `${loc}/allOf/${i}`, where, value, depth + 1));
    const t = typeOf(value);
    if (t === 'object') {
      const props = node.properties && typeof node.properties === 'object' ? node.properties : {};
      for (const [k, v] of Object.entries(value)) {
        if (FORBIDDEN.has(k)) continue;
        if (Object.hasOwn(props, k)) {
          walk(props[k], `${loc}/properties/${pointerKey(k)}`, where === 'body' ? k : `${where}.${k}`, v, depth + 1);
        } else if (node.additionalProperties && typeof node.additionalProperties === 'object') {
          walk(node.additionalProperties, `${loc}/additionalProperties`, `${where}.*`, v, depth + 1);
        }
      }
    }
    if (t === 'array' && node.items && typeof node.items === 'object' && !Array.isArray(node.items)) {
      for (const v of value.slice(0, 1000)) walk(node.items, `${loc}/items`, `${where}[]`, v, depth + 1);
    }
  };
  walk(schema, loc, 'body', body, 0);
  return { seen, undocumented, unclear };
}

/** Is a media type one whose body this extension reads? */
export function readable(mediaKey) {
  return isJson(mediaKey.split(';')[0].trim().toLowerCase());
}
