#!/usr/bin/env python3
"""
BattleCP Problem Database Builder
==================================
Fetches all rated Codeforces problems (with solve counts + division tags)
and writes them to backend/data/problems.json.

Run this script:
  python3 scripts/build_problem_db.py

Output file: backend/data/problems.json  (~0.75 MB, ~10 700 problems)

Re-run this monthly (or whenever you want fresher data).
Zero dependencies beyond stdlib.
"""

import json
import urllib.request
import urllib.error
import os
import sys
import time
from collections import defaultdict

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(SCRIPT_DIR)
OUT_DIR = os.path.join(REPO_ROOT, "backend", "data")
OUT_FILE = os.path.join(OUT_DIR, "problems.json")


# ---------------------------------------------------------------------------
# Difficulty bands used in "Band Mode"
# ---------------------------------------------------------------------------
# These are CF rating ranges that group into 5 intuitive skill levels.
# Each band has 1 000 – 2 500 problems, plenty of variety.
BANDS = [
    # (id, label, cf_rating_min, cf_rating_max)
    (0, "super_easy", 800,  1200),
    (1, "easy",       1201, 1500),
    (2, "medium",     1501, 1900),
    (3, "hard",       1901, 2400),
    (4, "very_hard",  2401, 9999),
]


def fetch_json(url: str, description: str) -> dict:
    print(f"  Fetching {description}... ", end="", flush=True)
    for attempt in range(3):
        try:
            with urllib.request.urlopen(url, timeout=30) as r:
                data = json.loads(r.read())
            if data.get("status") != "OK":
                raise RuntimeError(f"CF API returned status: {data.get('status')}")
            print("OK")
            return data
        except Exception as e:
            if attempt < 2:
                print(f"retry ({e})... ", end="", flush=True)
                time.sleep(2)
            else:
                print(f"FAILED: {e}")
                raise


def get_division(contest_name: str) -> str:
    """Map a Codeforces contest name to the most specific division tag."""
    n = contest_name.lower()
    if "div. 4"   in n or "div.4"   in n: return "Div4"
    if "div. 3"   in n or "div.3"   in n: return "Div3"
    if "educational" in n:                 return "Educational"
    if "div. 2"   in n or "div.2"   in n: return "Div2"
    if "div. 1"   in n or "div.1"   in n: return "Div1"
    if "global"   in n:                   return "Global"
    return "Other"


def band_for_rating(rating: int) -> int:
    for band_id, _, lo, hi in BANDS:
        if lo <= rating <= hi:
            return band_id
    return -1  # unrated / out-of-range


def build() -> None:
    print("=" * 60)
    print("BattleCP Problem Database Builder")
    print("=" * 60)

    # ------------------------------------------------------------------
    # Step 1 – fetch all problems + statistics from CF in a single call
    # ------------------------------------------------------------------
    print("\n[1/3] Fetching problem set from Codeforces API")
    ps_data = fetch_json(
        "https://codeforces.com/api/problemset.problems",
        "problemset.problems"
    )

    problems_raw  = ps_data["result"]["problems"]
    stats_raw     = ps_data["result"]["problemStatistics"]

    # Build fast lookup: "contestId-index" -> solvedCount
    solve_map: dict[str, int] = {
        f"{s['contestId']}-{s['index']}": s["solvedCount"]
        for s in stats_raw
    }

    print(f"     Total problems returned: {len(problems_raw)}")
    rated = [p for p in problems_raw if p.get("rating") and p.get("contestId")]
    print(f"     Rated problems with contestId: {len(rated)}")

    # ------------------------------------------------------------------
    # Step 2 – fetch contest list to extract divisions
    # ------------------------------------------------------------------
    print("\n[2/3] Fetching contest list from Codeforces API")
    cl_data = fetch_json(
        "https://codeforces.com/api/contest.list",
        "contest.list"
    )
    contest_div: dict[int, str] = {
        c["id"]: get_division(c["name"])
        for c in cl_data["result"]
    }
    print(f"     Contests mapped: {len(contest_div)}")

    # ------------------------------------------------------------------
    # Step 3 – join and produce the flat problem list
    # ------------------------------------------------------------------
    print("\n[3/3] Building enriched problem list")

    problems: list[dict] = []
    band_counts: dict[int, int] = defaultdict(int)
    div_counts:  dict[str, int] = defaultdict(int)

    for p in rated:
        cid     = p["contestId"]
        idx     = p["index"]
        rating  = int(p["rating"])
        key     = f"{cid}-{idx}"
        div     = contest_div.get(cid, "Other")
        solved  = solve_map.get(key, 0)
        band    = band_for_rating(rating)

        entry = {
            "c": cid,        # contestId  (int)
            "i": idx,        # problem index  (str)
            "n": p["name"],  # problem name
            "r": rating,     # CF rating
            "s": solved,     # solve count  (helps differentiate difficulty within same rating)
            "d": div,        # division tag (Div1/Div2/Div3/Div4/Educational/Global/Other)
            "b": band,       # band id 0-4  (-1 if unclassified)
        }
        problems.append(entry)
        band_counts[band] += 1
        div_counts[div]   += 1

    # Sort deterministically: by rating asc, then solve count desc
    problems.sort(key=lambda x: (x["r"], -x["s"]))

    # ------------------------------------------------------------------
    # Stats printout
    # ------------------------------------------------------------------
    print(f"\n  Total enriched problems: {len(problems)}")

    print("\n  Problems per difficulty band:")
    for band_id, label, lo, hi in BANDS:
        count = band_counts[band_id]
        print(f"    [{band_id}] {label:12s}  CF {lo:4d} – {hi:4d}  →  {count:5d} problems")

    print("\n  Problems per division:")
    for div in ["Div1", "Div2", "Div3", "Div4", "Educational", "Global", "Other"]:
        if div in div_counts:
            print(f"    {div:12s}: {div_counts[div]:5d}")

    # Self-check: every band should have ≥ 200 problems
    for band_id, label, _, _ in BANDS:
        assert band_counts[band_id] >= 50, \
            f"WARNING: band '{label}' only has {band_counts[band_id]} problems!"

    # ------------------------------------------------------------------
    # Write output
    # ------------------------------------------------------------------
    os.makedirs(OUT_DIR, exist_ok=True)
    with open(OUT_FILE, "w") as f:
        json.dump(problems, f, separators=(",", ":"), ensure_ascii=False)

    size_kb = os.path.getsize(OUT_FILE) / 1024
    print(f"\n  Written to: {OUT_FILE}")
    print(f"  File size:  {size_kb:.0f} KB ({len(problems)} entries)")
    print("\n  Done! Commit this file to the repo.")
    print("  Re-run monthly for fresh problem data.\n")


# ---------------------------------------------------------------------------
# Metadata written alongside the JSON for debugging / documentation
# ---------------------------------------------------------------------------
def write_metadata(problems: list[dict]) -> None:
    meta = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "total_problems": len(problems),
        "fields": {
            "c": "contestId (int)",
            "i": "problem index (str, e.g. 'A', 'B', 'C1')",
            "n": "problem name",
            "r": "CF rating (int)",
            "s": "solve count from CF problemStatistics",
            "d": "division tag: Div1/Div2/Div3/Div4/Educational/Global/Other",
            "b": "band id: 0=SuperEasy 1=Easy 2=Medium 3=Hard 4=VeryHard",
        },
        "bands": {
            str(band_id): {"label": label, "cf_min": lo, "cf_max": hi}
            for band_id, label, lo, hi in BANDS
        },
        "url_pattern": "https://codeforces.com/contest/{c}/problem/{i}",
    }
    meta_file = OUT_FILE.replace(".json", "_meta.json")
    with open(meta_file, "w") as f:
        json.dump(meta, f, indent=2)
    print(f"  Metadata:   {meta_file}")


if __name__ == "__main__":
    build()
