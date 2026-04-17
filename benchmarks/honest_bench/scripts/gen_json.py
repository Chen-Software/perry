#!/usr/bin/env python3
"""Generate a deterministic ~100MB JSON array of ~500k records.

Each record has: id, name (short string), email, age, country code,
tags (list of 3 strings), score (float), active (bool), addr (nested object
with street/city/zip). Size target: ~100MB on disk.

Deterministic: xorshift32 seeded with 0x9E3779B9 drives all random choices,
so the file is byte-identical across runs.
"""
import sys
import json
from pathlib import Path

SEED = 0x9E3779B9
N = 500_000

FIRST_NAMES = ["Alice","Bob","Carol","Dave","Eve","Frank","Grace","Henry","Ivy","Jack","Kate","Leo","Mia","Noah","Olivia","Peter","Quinn","Ruth","Sam","Tara","Uma","Victor","Wendy","Xander","Yara","Zane"]
LAST_NAMES = ["Anderson","Baker","Carter","Davis","Edwards","Foster","Garcia","Hill","Irving","Jones","King","Lewis","Martin","Nelson","Owens","Parker","Quinn","Reed","Smith","Turner","Underwood","Vance","Wilson","Xu","Young","Zhang"]
COUNTRIES = ["US","CA","GB","DE","FR","JP","AU","BR","IN","MX","ES","IT","NL","SE","NO","DK","FI","PL","KR","SG"]
STREETS = ["Main","Oak","Maple","Pine","Cedar","Elm","Park","Lake","Hill","River"]
CITIES = ["Springfield","Rivertown","Hillcrest","Fairview","Lakeside","Oakwood","Ridgefield","Brookline","Pinehurst","Westport"]
TAGS = ["admin","user","vip","beta","legacy","trial","pro","basic","guest","archived","active","pending","suspended","verified","new"]

def xorshift_stream(seed):
    s = seed & 0xFFFFFFFF
    while True:
        s ^= (s << 13) & 0xFFFFFFFF
        s ^= (s >> 17) & 0xFFFFFFFF
        s ^= (s << 5) & 0xFFFFFFFF
        s &= 0xFFFFFFFF
        yield s

def main():
    out = Path(__file__).resolve().parent.parent / "assets" / "input.json"
    out.parent.mkdir(parents=True, exist_ok=True)

    rng = xorshift_stream(SEED)
    with open(out, "w") as f:
        f.write("[\n")
        for i in range(N):
            first = FIRST_NAMES[next(rng) % len(FIRST_NAMES)]
            last = LAST_NAMES[next(rng) % len(LAST_NAMES)]
            name = f"{first} {last}"
            email = f"{first.lower()}.{last.lower()}{i}@example.com"
            age = 18 + (next(rng) % 70)
            country = COUNTRIES[next(rng) % len(COUNTRIES)]
            tag_idx = [next(rng) % len(TAGS) for _ in range(3)]
            tags = [TAGS[j] for j in tag_idx]
            # score stored as hundredths-of-a-point integer (0..99999) so all
            # three languages emit byte-identical JSON — their float-formatters
            # disagree on whether whole-number floats print as "19" or "19.0".
            score = next(rng) % 100000
            active = (next(rng) & 1) == 1
            street_num = next(rng) % 9999
            street = STREETS[next(rng) % len(STREETS)]
            city = CITIES[next(rng) % len(CITIES)]
            zip_code = 10000 + (next(rng) % 89999)
            rec = {
                "id": i,
                "name": name,
                "email": email,
                "age": age,
                "country": country,
                "tags": tags,
                "score": score,
                "active": active,
                "addr": {
                    "street": f"{street_num} {street} St",
                    "city": city,
                    "zip": zip_code,
                },
            }
            f.write(json.dumps(rec, separators=(",", ":")))
            if i < N - 1:
                f.write(",\n")
            else:
                f.write("\n")
        f.write("]\n")
    size = out.stat().st_size
    print(f"wrote {out} ({size} bytes, {size / (1024*1024):.1f} MB, {N} records)")

if __name__ == "__main__":
    main()
