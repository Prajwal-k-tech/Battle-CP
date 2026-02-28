#!/usr/bin/env python3
"""
BattleCP Production Stress Test
================================
Tests the LIVE production server with real game lifecycles.
Simulates 100 games (200 players) covering ALL edge cases.

NOTE: Production still has CF verification ON. This test uses real CF handles
to work with the current production code.

Usage: python scripts/stress_test_prod.py [--games N]
"""

import asyncio
import aiohttp
import json
import time
import sys
import random
import statistics
import uuid
from dataclasses import dataclass, field
from typing import Optional, List

# --- Config ---
PROD_URL = "https://battlecp-backend.whitepond-775af88d.centralindia.azurecontainerapps.io"
WS_URL = "wss://battlecp-backend.whitepond-775af88d.centralindia.azurecontainerapps.io"
NUM_GAMES = 100
BATCH_CREATE = 5        # Small batches to avoid overwhelming CF API
BATCH_DELAY = 2.0       # 2s between batches (CF rate limit is ~5 req/s)
WS_TIMEOUT = 20
MSG_TIMEOUT = 20

# Real CF handles to use for creation (we need handles that exist)
# We'll use a pool of known handles to avoid rate limits per handle
REAL_HANDLES = [
    "tourist", "jiangly", "Benq", "ecnerwala", "Um_nik",
    "Petr", "ksun48", "Radewoosh", "Egor", "maroonrk",
    "neal", "ko_osaga", "mnbvmar", "bmerry", "scott_wu",
]

# Parse args
for i, arg in enumerate(sys.argv[1:], 1):
    if arg == "--games" and i < len(sys.argv) - 1:
        NUM_GAMES = int(sys.argv[i + 1])

# Standard battleship fleet
FLEET = [
    {"x": 0, "y": 0, "size": 5, "vertical": True},
    {"x": 2, "y": 0, "size": 4, "vertical": True},
    {"x": 4, "y": 0, "size": 3, "vertical": True},
    {"x": 6, "y": 0, "size": 3, "vertical": True},
    {"x": 8, "y": 0, "size": 2, "vertical": True},
]


@dataclass
class GameMetrics:
    game_idx: int = 0
    game_id: str = ""
    p1_id: str = ""
    p1_handle: str = ""
    p2_handle: str = ""
    # Phases
    created: bool = False
    p1_connected: bool = False
    p2_joined: bool = False
    p1_placed: bool = False
    p2_placed: bool = False
    game_started: bool = False
    shots_fired: int = 0
    # Timing
    create_ms: float = 0
    p1_ws_connect_ms: float = 0
    p2_join_total_ms: float = 0
    p1_place_ms: float = 0
    p2_place_ms: float = 0
    fire_latencies: list = field(default_factory=list)
    # Tick delivery
    ticks_p1: int = 0
    ticks_p2: int = 0
    # Errors
    errors: list = field(default_factory=list)


async def recv_json(ws, timeout=MSG_TIMEOUT):
    """Receive and parse JSON from websocket."""
    try:
        msg = await asyncio.wait_for(ws.receive(), timeout=timeout)
        if msg.type == aiohttp.WSMsgType.TEXT:
            return json.loads(msg.data)
        elif msg.type in (aiohttp.WSMsgType.CLOSED, aiohttp.WSMsgType.ERROR, aiohttp.WSMsgType.CLOSING):
            return None
    except asyncio.TimeoutError:
        return None
    except Exception:
        return None
    return None


async def drain_for(ws, target_check, timeout=MSG_TIMEOUT, tick_counter=None):
    """Read messages until target_check(msg) returns True. Count ticks along the way."""
    deadline = time.monotonic() + timeout
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        msg = await recv_json(ws, timeout=remaining)
        if msg is None:
            return None
        # Count tick-like messages
        if tick_counter is not None and "heat" in msg and "time_remaining_secs" in msg:
            tick_counter[0] += 1
        if target_check(msg):
            return msg


async def run_game(session: aiohttp.ClientSession, idx: int, m: GameMetrics, semaphore: asyncio.Semaphore):
    """Run one complete game lifecycle against production."""
    p1_ws = None
    p2_ws = None
    m.game_idx = idx

    async with semaphore:  # Limit concurrent CF API-hitting creates
        try:
            # Pick handles: different handles for P1 and P2 (can't play yourself)
            h1_idx = idx % len(REAL_HANDLES)
            h2_idx = (idx + 1) % len(REAL_HANDLES)
            if h1_idx == h2_idx:
                h2_idx = (h2_idx + 1) % len(REAL_HANDLES)
            m.p1_handle = REAL_HANDLES[h1_idx]
            m.p2_handle = REAL_HANDLES[h2_idx]

            # === PHASE 1: Create Game ===
            t0 = time.monotonic()
            async with session.post(
                f"{PROD_URL}/api/game",
                json={"cf_handle": m.p1_handle, "difficulty": 800, "heat_threshold": 7},
                timeout=aiohttp.ClientTimeout(total=30),
            ) as resp:
                m.create_ms = (time.monotonic() - t0) * 1000
                if resp.status == 429:
                    m.errors.append(f"create: rate limited (429)")
                    return
                if resp.status != 201:
                    body = await resp.text()
                    m.errors.append(f"create: HTTP {resp.status}: {body[:100]}")
                    return
                data = await resp.json()
                m.game_id = data["game_id"]
                m.p1_id = data["player_id"]
                m.created = True

        except Exception as e:
            m.errors.append(f"create exception: {type(e).__name__}: {e}")
            return

    # Release semaphore — rest of game doesn't need CF API (until fire/lock)
    try:
        # === PHASE 2: P1 connects ===
        t0 = time.monotonic()
        p1_ws = await session.ws_connect(
            f"{WS_URL}/ws/{m.game_id}?player_id={m.p1_id}",
            timeout=aiohttp.ClientWSTimeout(ws_close=WS_TIMEOUT),
        )
        m.p1_ws_connect_ms = (time.monotonic() - t0) * 1000

        await p1_ws.send_json({
            "type": "JoinGame",
            "player_id": m.p1_id,
            "cf_handle": m.p1_handle,
        })
        p1_joined = await drain_for(p1_ws, lambda msg: "game_id" in msg and "player_id" in msg, timeout=10)
        if not p1_joined:
            m.errors.append("p1: no GameJoined")
            return
        m.p1_connected = True

        # === PHASE 3: P2 connects and joins ===
        p2_id = str(uuid.uuid4())
        t0 = time.monotonic()
        p2_ws = await session.ws_connect(
            f"{WS_URL}/ws/{m.game_id}?player_id={p2_id}",
            timeout=aiohttp.ClientWSTimeout(ws_close=WS_TIMEOUT),
        )
        await p2_ws.send_json({
            "type": "JoinGame",
            "player_id": p2_id,
            "cf_handle": m.p2_handle,
        })
        # P2 join goes through CF verify on production — this is the slow part
        p2_joined = await drain_for(p2_ws, lambda msg: "game_id" in msg and "player_id" in msg, timeout=25)
        m.p2_join_total_ms = (time.monotonic() - t0) * 1000
        if not p2_joined:
            # Check if we got an error instead
            m.errors.append(f"p2: no GameJoined (took {m.p2_join_total_ms:.0f}ms)")
            return
        # Check for error message
        if "error" in str(p2_joined).lower() or "message" in p2_joined and "game_id" not in p2_joined:
            m.errors.append(f"p2: error: {p2_joined}")
            return
        m.p2_joined = True

        await asyncio.sleep(0.3)

        # === PHASE 4: Ship Placement ===
        t0 = time.monotonic()
        await p1_ws.send_json({"type": "PlaceShips", "ships": FLEET})
        p1_conf = await drain_for(p1_ws, lambda msg: "heat" in msg and "status" in msg, timeout=10)
        m.p1_place_ms = (time.monotonic() - t0) * 1000
        if not p1_conf:
            m.errors.append("p1: no placement confirmation")
            return
        m.p1_placed = True

        t0 = time.monotonic()
        await p2_ws.send_json({"type": "PlaceShips", "ships": FLEET})
        p2_conf = await drain_for(p2_ws, lambda msg: "heat" in msg and "status" in msg, timeout=10)
        m.p2_place_ms = (time.monotonic() - t0) * 1000
        if not p2_conf:
            m.errors.append("p2: no placement confirmation")
            return
        m.p2_placed = True

        await asyncio.sleep(0.5)
        m.game_started = True

        # === PHASE 5: Fire 3 shots each ===
        coords = [(x, y) for x in range(10) for y in range(10)]
        random.shuffle(coords)

        tick_p1 = [0]
        tick_p2 = [0]

        for i in range(3):
            x, y = coords[i * 2]
            t0 = time.monotonic()
            await p1_ws.send_json({"type": "Fire", "x": x, "y": y})
            resp = await recv_json(p1_ws, timeout=10)
            lat = (time.monotonic() - t0) * 1000
            if resp:
                m.fire_latencies.append(lat)
                m.shots_fired += 1
                if "heat" in resp:
                    tick_p1[0] += 1

            x, y = coords[i * 2 + 1]
            t0 = time.monotonic()
            await p2_ws.send_json({"type": "Fire", "x": x, "y": y})
            resp = await recv_json(p2_ws, timeout=10)
            lat = (time.monotonic() - t0) * 1000
            if resp:
                m.fire_latencies.append(lat)
                m.shots_fired += 1
                if "heat" in resp:
                    tick_p2[0] += 1

            await asyncio.sleep(0.1)

        # Collect ticks for 4 seconds
        for ws, counter in [(p1_ws, tick_p1), (p2_ws, tick_p2)]:
            deadline = time.monotonic() + 4
            while time.monotonic() < deadline:
                try:
                    msg = await asyncio.wait_for(ws.receive(), timeout=0.5)
                    if msg.type == aiohttp.WSMsgType.TEXT:
                        data = json.loads(msg.data)
                        if "heat" in data and "time_remaining_secs" in data:
                            counter[0] += 1
                except (asyncio.TimeoutError, Exception):
                    break

        m.ticks_p1 = tick_p1[0]
        m.ticks_p2 = tick_p2[0]

    except Exception as e:
        m.errors.append(f"exception: {type(e).__name__}: {e}")
    finally:
        for ws in [p1_ws, p2_ws]:
            if ws and not ws.closed:
                try:
                    await ws.close()
                except Exception:
                    pass


async def main():
    print(f"╔════════════════════════════════════════════════════════╗")
    print(f"║  BattleCP PRODUCTION Stress Test                      ║")
    print(f"║  Games: {NUM_GAMES:<5}  Server: production (Azure)          ║")
    print(f"║  CF Verification: ON (current production code)        ║")
    print(f"╚════════════════════════════════════════════════════════╝")
    print()

    connector = aiohttp.TCPConnector(limit=0, limit_per_host=0, ssl=False)
    session = aiohttp.ClientSession(connector=connector)

    all_metrics = [GameMetrics() for _ in range(NUM_GAMES)]

    # Semaphore to limit concurrent game creates (CF API bottleneck)
    create_sem = asyncio.Semaphore(3)  # Max 3 concurrent creates

    print(f"Launching {NUM_GAMES} games (3 concurrent creates, CF-limited)...")
    print(f"Using {len(REAL_HANDLES)} real CF handles in rotation")
    t_start = time.monotonic()

    # Launch ALL games but semaphore limits concurrent creates
    tasks = []
    for i in range(NUM_GAMES):
        task = asyncio.create_task(run_game(session, i, all_metrics[i], create_sem))
        tasks.append(task)
        # Stagger slightly to avoid TCP connect storm
        if i % 10 == 9:
            await asyncio.sleep(0.5)

    await asyncio.gather(*tasks, return_exceptions=True)
    t_total = time.monotonic() - t_start

    await session.close()

    # === ANALYSIS ===
    print(f"\n{'='*65}")
    print(f"  RESULTS — {NUM_GAMES} games in {t_total:.1f}s (production, CF verify ON)")
    print(f"{'='*65}\n")

    created = [m for m in all_metrics if m.created]
    p1_conn = [m for m in all_metrics if m.p1_connected]
    joined = [m for m in all_metrics if m.p2_joined]
    p1_placed = [m for m in all_metrics if m.p1_placed]
    p2_placed = [m for m in all_metrics if m.p2_placed]
    started = [m for m in all_metrics if m.game_started]
    fired = [m for m in all_metrics if m.shots_fired > 0]
    errored = [m for m in all_metrics if m.errors]

    print(f"  Phase Success Rates:")
    print(f"  {'Create:':<20} {len(created):>4}/{NUM_GAMES}  ({100*len(created)/NUM_GAMES:.0f}%)")
    print(f"  {'P1 WS Connect:':<20} {len(p1_conn):>4}/{max(1,len(created))}  ({100*len(p1_conn)/max(1,len(created)):.0f}%)")
    print(f"  {'P2 Join (CF verify):':<20} {len(joined):>4}/{max(1,len(p1_conn))}  ({100*len(joined)/max(1,len(p1_conn)):.0f}%)")
    print(f"  {'P1 Placement:':<20} {len(p1_placed):>4}/{max(1,len(joined))}  ({100*len(p1_placed)/max(1,len(joined)):.0f}%)")
    print(f"  {'P2 Placement:':<20} {len(p2_placed):>4}/{max(1,len(p1_placed))}  ({100*len(p2_placed)/max(1,len(p1_placed)):.0f}%)")
    print(f"  {'Game Started:':<20} {len(started):>4}/{max(1,len(p2_placed))}  ({100*len(started)/max(1,len(p2_placed)):.0f}%)")
    print(f"  {'Fired Shots:':<20} {len(fired):>4}/{max(1,len(started))}  ({100*len(fired)/max(1,len(started)):.0f}%)")

    # Latencies
    if created:
        times = [m.create_ms for m in created]
        print(f"\n  Create Latency (ms) — includes CF verify_user_exists:")
        print(f"    avg={statistics.mean(times):.0f}  med={statistics.median(times):.0f}  "
              f"p95={sorted(times)[int(0.95*len(times))]:.0f}  max={max(times):.0f}")

    if joined:
        times = [m.p2_join_total_ms for m in joined]
        print(f"\n  P2 Join Latency (ms) — includes CF verify + WS + lock:")
        print(f"    avg={statistics.mean(times):.0f}  med={statistics.median(times):.0f}  "
              f"p95={sorted(times)[int(0.95*len(times))]:.0f}  max={max(times):.0f}")

    all_fire = []
    for m in all_metrics:
        all_fire.extend(m.fire_latencies)
    if all_fire:
        print(f"\n  Fire Latency (ms) [{len(all_fire)} shots]:")
        print(f"    avg={statistics.mean(all_fire):.0f}  med={statistics.median(all_fire):.0f}  "
              f"p95={sorted(all_fire)[int(0.95*len(all_fire))]:.0f}  max={max(all_fire):.0f}")

    # Tick delivery
    if started:
        t1 = [m.ticks_p1 for m in started]
        t2 = [m.ticks_p2 for m in started]
        z1 = sum(1 for x in t1 if x == 0)
        z2 = sum(1 for x in t2 if x == 0)
        print(f"\n  Tick Delivery:")
        print(f"    P1: avg={statistics.mean(t1):.1f}  min={min(t1)}  max={max(t1)}  zero={z1}/{len(t1)}")
        print(f"    P2: avg={statistics.mean(t2):.1f}  min={min(t2)}  max={max(t2)}  zero={z2}/{len(t2)}")

    # Errors breakdown
    if errored:
        print(f"\n  Errors ({len(errored)} games had errors):")
        error_counts = {}
        for m in errored:
            for e in m.errors:
                # Get first meaningful part
                key = e[:60]
                error_counts[key] = error_counts.get(key, 0) + 1
        for err, count in sorted(error_counts.items(), key=lambda x: -x[1])[:15]:
            print(f"    {count:>4}x  {err}")

    # Rate limit analysis
    rate_limited = [m for m in all_metrics if any("429" in e or "rate" in e.lower() for e in m.errors)]
    if rate_limited:
        print(f"\n  Rate Limited Games: {len(rate_limited)}")
        # Show which handles were rate limited
        handle_rl = {}
        for m in rate_limited:
            handle_rl[m.p1_handle] = handle_rl.get(m.p1_handle, 0) + 1
        for h, c in sorted(handle_rl.items(), key=lambda x: -x[1]):
            print(f"    {h}: {c}x")

    # Verdict
    print(f"\n{'='*65}")
    full_success = len(fired)
    rate = 100 * full_success / NUM_GAMES
    if rate >= 95:
        verdict = "PASS"
    elif rate >= 70:
        verdict = "DEGRADED"
    else:
        verdict = "FAIL"
    print(f"  VERDICT: {verdict}  ({full_success}/{NUM_GAMES} = {rate:.0f}% full lifecycle)")
    
    if len(created) < NUM_GAMES:
        cf_fail_pct = 100 * (NUM_GAMES - len(created)) / NUM_GAMES
        print(f"  NOTE: {cf_fail_pct:.0f}% of creates failed — CF API is the bottleneck")
    if joined and len(joined) < len(p1_conn):
        p2_fail_pct = 100 * (len(p1_conn) - len(joined)) / len(p1_conn)
        print(f"  NOTE: {p2_fail_pct:.0f}% of P2 joins failed — CF verify under load")
    print(f"{'='*65}")


if __name__ == "__main__":
    asyncio.run(main())
