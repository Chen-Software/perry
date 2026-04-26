// JSON pipeline: read input JSON, filter active records, add 2 derived fields,
// serialize, write output. Perry stdlib JSON + fs (utf-8 text).
//
// PERRY GAP NOTES (honest-benchmark findings, all in v0.5.29):
//
// 1. process.argv.slice(2) returns an array whose elements come back as
//    garbage numbers. Indexing argv directly works.
//
// 2. Iterating over a large `JSON.parse` result and accessing .fields on each
//    record triggers a GC-scan issue at scale — records allocated by the JSON
//    parser get swept mid-iteration, yielding fewer .active===true matches
//    than exist, or corrupting fields in the serialized output. Observed
//    break-point is ~200 records; above that, output is non-deterministic.
//
// 3. Mutating a parsed-record object (`r.display_name = …`) then
//    `JSON.stringify(r)` also trips the same issue — stringify sometimes
//    panics inside `perry-runtime/src/json.rs:427` with "byte index … is not
//    a char boundary" reading corrupted strings. Constructing a fresh object
//    literal and stringifying *that* is reliable, so we do that here.
//
// 4. Reading a ~108 MB file into a string via fs.readFileSync works, but
//    JSON.parse on 500k objects tips the same GC issues even before we get
//    to iteration.
//
// The driver runs this binary on the 100-record fixture only. Rust and Zig
// also run on the full 108 MB / 500k-record fixture; the report calls out
// the scale gap explicitly.

import * as fs from 'fs';

function imul32(a: number, b: number): number {
  const aHi = (a >>> 16) & 0xffff;
  const aLo = a & 0xffff;
  const bHi = (b >>> 16) & 0xffff;
  const bLo = b & 0xffff;
  return ((aLo * bLo) + (((aHi * bLo + aLo * bHi) << 16) >>> 0)) | 0;
}
function fnv1a32(s: string): number {
  let h = 0x811c9dc5 | 0;
  for (let i = 0; i < s.length; i++) {
    h = (h ^ s.charCodeAt(i)) | 0;
    h = imul32(h, 0x01000193);
  }
  return h >>> 0;
}

if (process.argv.length < 4) {
  console.error('usage: json_pipeline <input.json> <output.json>');
  process.exit(1);
}
const inPath = process.argv[2];
const outPath = process.argv[3];

const text = fs.readFileSync(inPath, 'utf8');
const inputBytes = text.length;

const records = JSON.parse(text) as any[];
const recordsIn = records.length;

const out: any[] = [];
for (let i = 0; i < recordsIn; i++) {
  const r = records[i];
  if (r.active !== true) continue;
  const age = r.age;
  out.push({
    id: r.id,
    name: r.name,
    email: r.email,
    age: age,
    country: r.country,
    tags: r.tags,
    score: r.score,
    active: r.active,
    addr: r.addr,
    display_name: r.name.toUpperCase(),
    age_group: age < 30 ? 'young' : age < 50 ? 'mid' : 'senior',
  });
}
const recordsOut = out.length;

const serialized = JSON.stringify(out);
const outputBytes = serialized.length;

fs.writeFileSync(outPath, serialized);

const hash = fnv1a32(serialized);
const hex = hash.toString(16).padStart(8, '0');
console.log(`input_bytes=${inputBytes} records_in=${recordsIn} records_out=${recordsOut} output_bytes=${outputBytes} hash=${hex}`);
