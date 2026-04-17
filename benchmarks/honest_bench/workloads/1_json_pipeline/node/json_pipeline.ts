// JSON pipeline: read input, filter active, add 2 derived fields, serialize,
// write output. Node.js / Bun compatible.

import * as fs from 'fs';

const inPath = process.argv[2];
const outPath = process.argv[3];
if (!inPath || !outPath) {
  console.error('usage: json_pipeline <input.json> <output.json>');
  process.exit(1);
}

const text = fs.readFileSync(inPath, 'utf8');
const inputBytes = text.length;

interface Rec {
  id: number; name: string; email: string; age: number; country: string;
  tags: string[]; score: number; active: boolean;
  addr: { street: string; city: string; zip: number };
  display_name?: string; age_group?: string;
}

const records: Rec[] = JSON.parse(text);
const recordsIn = records.length;

const out: Rec[] = [];
for (let i = 0; i < recordsIn; i++) {
  const r = records[i];
  if (r.active !== true) continue;
  out.push({
    id: r.id, name: r.name, email: r.email, age: r.age,
    country: r.country, tags: r.tags, score: r.score,
    active: r.active, addr: r.addr,
    display_name: r.name.toUpperCase(),
    age_group: r.age < 30 ? 'young' : r.age < 50 ? 'mid' : 'senior',
  });
}
const recordsOut = out.length;

const serialized = JSON.stringify(out);
const outputBytes = serialized.length;

fs.writeFileSync(outPath, serialized);

let h = 0x811c9dc5 | 0;
for (let i = 0; i < serialized.length; i++) {
  h = (h ^ serialized.charCodeAt(i)) | 0;
  h = Math.imul(h, 0x01000193);
}
const hash = (h >>> 0).toString(16).padStart(8, '0');
console.log(`input_bytes=${inputBytes} records_in=${recordsIn} records_out=${recordsOut} output_bytes=${outputBytes} hash=${hash}`);
