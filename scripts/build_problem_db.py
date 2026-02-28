#!/usr/bin/env python3
"""
BattleCP Problem Database Builder (v2 — with clist.by ratings)
===============================================================
Merges Codeforces problem data (CF rating, solve count, divisions) with
clist.by difficulty ratings to produce backend/data/problems.json.

Clist provides its own difficulty rating for each CF problem, which
starts at 0 and gives finer granularity than CF's 800-step system.
Band mode uses clist ratings; CF mode uses native CF ratings.

Run:
  python3 scripts/build_problem_db.py

Output: backend/data/problems.json  (~1 MB, ~10 000+ problems)

Requires: clist.by API key (hardcoded below — read-only).
Rate limit: 10 req/min on clist → script sleeps between pages.
"""

import json
import os
import sys
import time
import urllib.request
import urllib.error
from collections import defaultdict

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(SCRIPT_DIR)
OUT_DIR = os.path.join(REPO_ROOT, "backend", "data")
OUT_FILE = os.path.join(OUT_DIR, "problems.json")

# Clist API credentials (read-only key)
CLIST_API_KEY = "ApiKey oGhostyyy:2f053292c6c0aaf140c9c4deb499c4c9c8c6ef4a"
CLIST_BASE = "https://clist.by/api/v4/json/problem/"
CLIST_RESOURCE_ID = 1  # codeforces.com

# ---------------------------------------------------------------------------
# Band definitions — based on CLIST ratings (not CF ratings)
# ---------------------------------------------------------------------------
BANDS = [
    # (id, label, clist_rating_min, clist_rating_max)
    (0, "super_easy", 0,    300),
    (1, "easy",       301,  600),
    (2, "medium",     601,  1000),
    (3, "hard",       1001, 1500),
    (4, "very_hard",  1501, 9999),
]


def band_for_clist_rating(rating: int) -> int:
    """Assign a band id based on clist rating."""
    for band_id, _, lo, hi in BANDS:
        if lo <= rating <= hi:
            return band_id
    return -1


def fetch_json_cf(url: str, description: str) -> dict:
    """Fetch JSON from Codeforces API with retries."""
    print(f"  Fetching {description}... ", end="", flush=True)
    for attempt in range(3):
        try:
            req = urllib.request.Request(url)
            with urllib.request.urlopen(req, timeout=30) as r:
                data = json.loads(r.read())
            if data.get("status") != "OK":
                raise RuntimeError(f"CF API returned status: {data.get('status')}")
            print("OK")
            return data
        except Exception as e:
            if attempt < 2:
                print(f"retry ({e})... ", end="", flush=True)
                time.sleep(3)
            else:
                print(f"FAILED: {e}")
                raise


def fetch_clist_page(offset: int, limit: int = 1000) -> list[dict]:
    """Fetch one page of clist CF problems (with retry on rate-limit)."""
    url = f"{CLIST_BASE}?resource_id={CLIST_RESOURCE_ID}&limit={limit}&offset={offset}&order_by=id"
    for attempt in range(5):
        try:
            req = urllib.request.Request(url, headers={
                "Authorization": CLIST_API_KEY,
                "User-Agent": "BattleCP/1.0",
            })
            with urllib.request.urlopen(req, timeout=30) as r:
                data = json.loads(r.read())
            return data.get("objects", [])
        except urllib.error.HTTPError as e:
            if e.code in (403, 429) and attempt < 4:
                wait = 15 * (attempt + 1)  # 15, 30, 45, 60s
                print(f"rate-limited ({e.code}), waiting {wait}s... ", end="", flush=True)
                time.sleep(wait)
            else:
                raise


def fetch_all_clist_problems() -> dict[str, dict]:
    """
    Fetch ALL codeforces problems from clist.by, paginating through the API.
    Returns a dict keyed by "contestId-index" -> {clist_rating, n_accepted}.
    """
    print("\n[2/4] Fetching clist.by ratings for Codeforces problems")
    print("  (Rate limit: 10 req/min — this will take ~90 seconds)")

    all_problems: dict[str, dict] = {}
    offset = 0
    page = 0

    while True:
        page += 1
        print(f"  Page {page} (offset={offset})... ", end="", flush=True)
        try:
            items = fetch_clist_page(offset)
        except Exception as e:
            print(f"FAILED: {e}")
            if all_problems:
                print(f"  WARNING: Stopped at {len(all_problems)} problems due to error")
                break
            raise

        if not items:
            print("(end of data)")
            break

        for item in items:
            url = item.get("url", "")
            clist_rating = item.get("rating")
            short = item.get("short", "")  # problem index like "A", "B1"

            if clist_rating is None:
                continue

            # Parse contest ID from URL: .../contest/1234/problem/A
            contest_id = None
            if "/contest/" in url:
                try:
                    parts = url.split("/contest/")[1].split("/")
                    contest_id = int(parts[0])
                except (IndexError, ValueError):
                    pass
            elif "/problemset/problem/" in url:
                try:
                    parts = url.split("/problemset/problem/")[1].split("/")
                    contest_id = int(parts[0])
                except (IndexError, ValueError):
                    pass

            if contest_id and short:
                key = f"{contest_id}-{short}"
                all_problems[key] = {
                    "clist_rating": int(clist_rating),
                    "n_accepted": item.get("n_accepted", 0),
                }

        count = len(items)
        print(f"{count} items -> {len(all_problems)} total with ratings")
        offset += count

        # Rate limit: 7s between requests (10 req/min = 6s/req + 1s buffer)
        if count >= 1000:
            time.sleep(7)

    print(f"  Total clist problems with ratings: {len(all_problems)}")
    return all_problems


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


def build() -> None:
    print("=" * 60)
    print("BattleCP Problem Database Builder v2 (with clist ratings)")
    print("=" * 60)

    # ------------------------------------------------------------------
    # Step 1 — Fetch all CF problems + statistics
    # ------------------------------------------------------------------
    print("\n[1/4] Fetching problem set from Codeforces API")
    ps_data = fetch_json_cf(
        "https://codeforces.com/api/problemset.problems",
        "problemset.problems"
    )

    problems_raw  = ps_data["result"]["problems"]
    stats_raw     = ps_data["result"]["problemStatistics"]

    solve_map: dict[str, int] = {
        f"{s['contestId']}-{s['index']}": s["solvedCount"]
        for s in stats_raw
    }

    print(f"     Total problems: {len(problems_raw)}")
    rated = [p for p in problems_raw if p.get("rating") and p.get("contestId")]
    print(f"     Rated with contestId: {len(rated)}")

    # ------------------------------------------------------------------
    # Step 2 — Fetch clist.by ratings
    # ------------------------------------------------------------------
    clist_map = fetch_all_clist_problems()

    # ------------------------------------------------------------------
    # Step 3 — Fetch contest list for division tags
    # ------------------------------------------------------------------
    print("\n[3/4] Fetching contest list from Codeforces API")
    time.sleep(2)  # Be nice to CF API
    cl_data = fetch_json_cf(
        "https://codeforces.com/api/contest.list",
        "contest.list"
    )
    contest_div: dict[int, str] = {
        c["id"]: get_division(c["name"])
        for c in cl_data["result"]
    }
    print(f"     Contests mapped: {len(contest_div)}")

    # ------------------------------------------------------------------
    # Step 4 — Merge and build the output
    # ------------------------------------------------------------------
    print("\n[4/4] Building enriched problem list")

    problems: list[dict] = []
    band_counts: dict[int, int] = defaultdict(int)
    div_counts:  dict[str, int] = defaultdict(int)
    matched = 0
    unmatched = 0

    for p in rated:
        cid     = p["contestId"]
        idx     = p["index"]
        rating  = int(p["rating"])
        key     = f"{cid}-{idx}"
        div     = contest_div.get(cid, "Other")
        solved  = solve_map.get(key, 0)

        # Clist rating lookup
        clist_info = clist_map.get(key)
        if clist_info:
            clist_rating = clist_info["clist_rating"]
            matched += 1
        else:
            # No clist rating → still include for CF mode, but skip from band mode
            clist_rating = -1
            unmatched += 1

        band = band_for_clist_rating(clist_rating) if clist_rating >= 0 else -1

        entry = {
            "c": cid,            # contestId
            "i": idx,            # problem index
            "n": p["name"],      # problem name
            "r": rating,         # CF rating
            "s": solved,         # solve count
            "d": div,            # division tag
            "b": band,           # band id (clist-based, -1 if no clist rating)
            "l": clist_rating,   # clist rating (-1 if unavailable)
        }
        problems.append(entry)
        if band >= 0:
            band_counts[band] += 1
        div_counts[div] += 1

    # Sort: by clist rating asc (unrated last), then CF rating asc, then solve count desc
    problems.sort(key=lambda x: (x["l"] if x["l"] >= 0 else 99999, x["r"], -x["s"]))

    # ------------------------------------------------------------------
    # Print statistics
    # ------------------------------------------------------------------
    print(f"\n  Total problems: {len(problems)}")
    print(f"  With clist rating: {matched} ({matched*100/(matched+unmatched):.1f}%)")
    print(f"  Without clist rating: {unmatched}")

    print("\n  Problems per band (clist-rated):")
    for band_id, label, lo, hi in BANDS:
        count = band_counts.get(band_id, 0)
        print(f"    [{band_id}] {label:12s}  clist {lo:4d} - {hi:4d}  ->  {count:5d} problems")

    print(f"\n  Total in bands: {sum(band_counts.values())}")

    print("\n  Problems per division:")
    for div in ["Div1", "Div2", "Div3", "Div4", "Educational", "Global", "Other"]:
        if div in div_counts:
            print(f"    {div:12s}: {div_counts[div]:5d}")

    # Sanity: each band should have enough problems
    for band_id, label, _, _ in BANDS:
        count = band_counts.get(band_id, 0)
        if count < 50:
            print(f"  WARNING: band '{label}' only has {count} problems!")

    # ------------------------------------------------------------------
    # Write output
    # ------------------------------------------------------------------
    os.makedirs(OUT_DIR, exist_ok=True)
    with open(OUT_FILE, "w") as f:
        json.dump(problems, f, separators=(",", ":"), ensure_ascii=False)

    size_kb = os.path.getsize(OUT_FILE) / 1024
    print(f"\n  Written to: {OUT_FILE}")
    print(f"  File size:  {size_kb:.0f} KB ({len(problems)} entries)")
    print("\n  Done! Re-run monthly for fresh data.\n")


if __name__ == "__main__":
    build()
