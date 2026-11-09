// Writing tab-separated records that can't break their own format: no tabs, newlines or other
// control characters in a field, and an empty field written as `-`.

/** A field as it can be written: control characters dropped, cut to `max` characters. */
export function field(value, max = 300) {
  const s = String(value ?? '').replace(/[\u0000-\u001f\u007f-\u009f]/g, ' ').trim();
  const cut = [...s].slice(0, max).join('');
  return cut === '' ? '-' : cut;
}

/** One record: a kind and its fields, as a line. */
export function line(kind, ...fields) {
  return [kind, ...fields.map((f) => field(f))].join('\t') + '\n';
}
