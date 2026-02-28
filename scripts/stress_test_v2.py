#!/usr/bin/env python3
"""
BattleCP Stress Test v2 — Separates CF API bottleneck from internal concurrency.

Strategy:
  Phase A: Slow game creation (batch=3, 4s delay) to warm CF API cache and avoid rate limits
  Phase B: Slam all games at once with WebSocket gameplay (the REAL concurrency test)
  Phase C: Sustained tick measurement — hold 100 games open and measure tick delivery

This tells us:
  - Can the backend's internal game engine handle 100 concurrent games?
  - Does the global write lock in the ticker cause latency spikes?
  - Do WebSocket broadcasts degrade under load?
"""

import argparse
import asyncio
import json
import random
import statistics
import time
import uuid
from dataclasses import dataclass, field
from typing import Optional

import aiohttp
import websockets

# ──────────────────────────────────────────────────
# CF handles — large list, all verified-real accounts
# ──────────────────────────────────────────────────
CF_HANDLES = [
    "tourist", "Benq", "jiangly", "ksun48", "ecnerwala",
    "Um_nik", "Petr", "Radewoosh", "heno239",
    "endagorion", "errorgorn", "ko_osaga", "dario2994", "maspy",
    "sunset", "mnbvmar", "noimi", "yosupo", "neal",
    "pashka", "Egor", "vepifanov", "SpyCheese", "adamant",
    "antontrygubO_o", "gamegame", "ScarletS", "wxhtzdy", "SSRS_",
    "Geothermal", "pajenegod", "BurnedChicken", "MiFaFaOvO",
    "Huah", "gyh20", "noshi91",
    "Pyqe", "zscoder", "FizzyDavid", "tabr", "Ormlis",
    "physics0523", "square1001", "nealwu", "bqi343",
    "Stonefeang", "orzdevinwang", "244mhq", "maroonrk",
    "Errichto", "Ashishgup",
    "Monogon", "tfg", "300iq", "Swistakk",
    "SecondThread", "Golovanov399",
    "Dominater069", "dorijanlendvaj",
    "Heltion", "EbTech",
    "Kubic", "cerberus97", "Rewinding",
    "dreamoon", "Rubikun",
    "GlebsHP", "Zlobober", "isaf27", "PavelKunyavskiy",
    "rng_58", "LHiC", "ainta",
]

STANDARD_FLEET = [
    {"x": 0, "y": 0, "size": 5, "vertical": False},
    {"x": 0, "y": 2, "size": 4, "vertical": False},
    {"x": 0, "y": 4, "size": 3, "vertical": False},
    {"x": 0, "y": 6, "size": 3, "vertical": False},
    {"x": 0, "y": 8, "size": 2, "vertical": False},
]


@dataclass
class GameResult:
    game_index: int
    game_id: Optional[str] = None
    p1_id: Optional[str] = None
    create_time: float = 0.0
    p1_ws_connect_time: float = 0.0
    p2_ws_connect_time: float = 0.0
    p1_join_time: float = 0.0
    p2_join_time: float = 0.0
    p1_place_time: float = 0.0
    p2_place_time: float = 0.0
    fire_times: list = field(default_factory=list)
    tick_times: list = field(default_factory=list)  # interval between ticks
    errors: list = field(default_factory=list)
    phase_reached: str = "init"


def get_handle_pair(game_index: int):
    idx1 = (game_index * 2) % len(CF_HANDLES)
    idx2 = (game_index * 2 + 1) % len(CF_HANDLES)
    if idx1 == idx2:
        idx2 = (idx2 + 1) % len(CF_HANDLES)
    return CF_HANDLES[idx1], CF_HANDLES[idx2]


# ──────────────────────────────────────────────────
# Helpers
# ──────────────────────────────────────────────────

async def recv_msg(ws, timeout: float = 15.0):
    try:
        raw = await asyncio.wait_for(ws.recv(), timeout=timeout)
        return json.loads(raw)
    except (asyncio.TimeoutError, Exception):
        return None


async def recv_until(ws, msg_type: str, timeout: float = 15.0):
    deadline = time.monotonic() + timeout
    all_msgs = []
    while time.monotonic() < deadline:
        remaining = max(0.1, deadline - time.monotonic())
        msg = await recv_msg(ws, timeout=remaining)
        if msg is None:
            break
        all_msgs.append(msg)
        if msg.get("type") == msg_type:
            return msg, all_msgs
    return None, all_msgs


async def drain(ws, duration=0.5):
    msgs = []
    deadline = time.monotonic() + duration
    while time.monotonic() < deadline:
        msg = await recv_msg(ws, timeout=max(0.1, deadline - time.monotonic()))
        if msg is None:
            break
        msgs.append(msg)
    return msgs


# ──────────────────────────────────────────────────
# Phase A: Slow game creation
# ──────────────────────────────────────────────────

async def create_game(session, backend_url, game_index, result):
    h1, _ = get_handle_pair(game_index)
    payload = {
        "cf_handle": h1,
        "difficulty": 800,
        "heat_threshold": 3,
        "game_duration_mins": 5,
        "veto_strictness": "low",
    }
    t0 = time.monotonic()
    try:
        async with session.post(
            f"{backend_url}/api/game", json=payload,
            timeout=aiohttp.ClientTimeout(total=30),
        ) as resp:
            result.create_time = time.monotonic() - t0
            body = await resp.json()
            if resp.status == 201:
                result.game_id = body["game_id"]
                result.p1_id = body["player_id"]
                result.phase_reached = "created"
                return True
            else:
                result.errors.append(f"create: {body.get('error', resp.status)}")
                return False
    except Exception as e:
        result.create_time = time.monotonic() - t0
        result.errors.append(f"create: {e}")
        return False


# ──────────────────────────────────────────────────
# Phase B: Full game simulation
# ──────────────────────────────────────────────────

async def simulate_game(backend_url, result, fire_rounds=6, hold_secs=5):
    """Full lifecycle: connect → join → place → fire → hold for ticks."""
    ws_url = backend_url.replace("https://", "wss://").replace("http://", "ws://")
    game_id = result.game_id
    p1_id = result.p1_id
    p2_id = str(uuid.uuid4())
    _, h2 = get_handle_pair(result.game_index)

    ws1 = ws2 = None
    try:
        # P1 connect + join
        t0 = time.monotonic()
        ws1 = await asyncio.wait_for(
            websockets.connect(f"{ws_url}/ws/{game_id}?player_id={p1_id}",
                               additional_headers={"Origin": "https://battle-cp.vercel.app"},
                               ping_interval=20, ping_timeout=10, close_timeout=5),
            timeout=15)
        result.p1_ws_connect_time = time.monotonic() - t0

        t0 = time.monotonic()
        await ws1.send(json.dumps({"type": "JoinGame", "player_id": p1_id,
                                   "cf_handle": get_handle_pair(result.game_index)[0]}))
        msg, _ = await recv_until(ws1, "GameJoined", timeout=10)
        result.p1_join_time = time.monotonic() - t0
        if not msg:
            result.errors.append("p1: no GameJoined")
            return
        result.phase_reached = "p1_joined"

        # P2 connect + join (this triggers CF API verify_user_exists)
        t0 = time.monotonic()
        ws2 = await asyncio.wait_for(
            websockets.connect(f"{ws_url}/ws/{game_id}?player_id={p2_id}",
                               additional_headers={"Origin": "https://battle-cp.vercel.app"},
                               ping_interval=20, ping_timeout=10, close_timeout=5),
            timeout=15)
        result.p2_ws_connect_time = time.monotonic() - t0

        t0 = time.monotonic()
        await ws2.send(json.dumps({"type": "JoinGame", "player_id": p2_id, "cf_handle": h2}))
        msg, all_msgs = await recv_until(ws2, "GameJoined", timeout=20)
        result.p2_join_time = time.monotonic() - t0
        if not msg:
            errs = [m for m in all_msgs if m.get("type") == "Error"]
            err_detail = errs[0].get("message", "unknown") if errs else "no GameJoined"
            result.errors.append(f"p2_join: {err_detail}")
            return
        result.phase_reached = "p2_joined"

        # Wait for P1 to receive PlayerJoined
        await drain(ws1, 2.0)

        # Place ships
        t0 = time.monotonic()
        await ws1.send(json.dumps({"type": "PlaceShips", "ships": STANDARD_FLEET}))
        await recv_until(ws1, "GameUpdate", timeout=10)
        result.p1_place_time = time.monotonic() - t0

        t0 = time.monotonic()
        await ws2.send(json.dumps({"type": "PlaceShips", "ships": STANDARD_FLEET}))
        await recv_until(ws2, "GameUpdate", timeout=10)
        result.p2_place_time = time.monotonic() - t0

        if result.p1_place_time == 0 or result.p2_place_time == 0:
            result.errors.append("placement failed")
            return
        result.phase_reached = "ships_placed"

        # Drain GameStart
        await asyncio.gather(drain(ws1, 1.0), drain(ws2, 1.0))

        # Fire shots
        coords = [(x, y) for x in range(10) for y in range(10)]
        random.shuffle(coords)

        shots_done = 0
        for i in range(min(fire_rounds, len(coords))):
            x, y = coords[i]
            ws = ws1 if i % 2 == 0 else ws2

            t0 = time.monotonic()
            await ws.send(json.dumps({"type": "Fire", "x": x, "y": y}))

            # Wait for broadcast ShotResult or Error
            deadline = time.monotonic() + 5.0
            while time.monotonic() < deadline:
                msg = await recv_msg(ws, timeout=max(0.1, deadline - time.monotonic()))
                if msg is None:
                    break
                if msg.get("type") in ("ShotResult", "Error", "GameOver"):
                    result.fire_times.append(time.monotonic() - t0)
                    shots_done += 1
                    break

        result.phase_reached = f"fired_{shots_done}"

        # ── Phase C: Hold connection and measure tick delivery ──
        tick_times = []
        last_tick = time.monotonic()
        deadline = time.monotonic() + hold_secs
        while time.monotonic() < deadline:
            msg = await recv_msg(ws1, timeout=max(0.1, deadline - time.monotonic()))
            if msg and msg.get("type") == "GameUpdate":
                now = time.monotonic()
                tick_times.append(now - last_tick)
                last_tick = now
        result.tick_times = tick_times
        if not tick_times:
            result.errors.append("no ticks during hold phase")

    except asyncio.TimeoutError:
        result.errors.append(f"timeout at {result.phase_reached}")
    except websockets.exceptions.ConnectionClosed as e:
        result.errors.append(f"ws_closed: {e.code} at {result.phase_reached}")
    except Exception as e:
        result.errors.append(f"{type(e).__name__}: {e}")
    finally:
        for ws in [ws1, ws2]:
            if ws:
                try:
                    await ws.close()
                except Exception:
                    pass


# ──────────────────────────────────────────────────
# Orchestrator
# ──────────────────────────────────────────────────

async def run(backend_url, num_games, create_batch, sim_batch, fire_rounds, hold_secs):
    print(f"\n{'='*64}")
    print(f"  BattleCP Stress Test v2")
    print(f"  Backend:    {backend_url}")
    print(f"  Games:      {num_games}")
    print(f"  Create:     batch={create_batch} (slow, CF-API-safe)")
    print(f"  Simulate:   batch={sim_batch} (concurrent gameplay)")
    print(f"  Fire:       {fire_rounds} shots/game | Hold: {hold_secs}s for ticks")
    print(f"{'='*64}\n")

    game_results = []
    wall_start = time.monotonic()

    # ═══════════════════════════════════════════════
    # Phase A: Create games slowly
    # ═══════════════════════════════════════════════
    print(f"[Phase A] Creating {num_games} games (slow, {create_batch}/batch)...")

    async with aiohttp.ClientSession() as session:
        for batch_start in range(0, num_games, create_batch):
            batch_end = min(batch_start + create_batch, num_games)
            batch = []
            for i in range(batch_start, batch_end):
                gr = GameResult(game_index=i)
                game_results.append(gr)
                batch.append(create_game(session, backend_url, i, gr))

            results = await asyncio.gather(*batch)
            ok = sum(1 for r in results if r is True)
            print(f"  [{batch_start:3d}-{batch_end:3d}] {ok}/{batch_end-batch_start} created", end="")

            # Check for rate limiting and back off
            fails = [gr for gr in game_results[batch_start:batch_end] if not gr.game_id]
            rate_limited = any("rate" in str(e).lower() or "Too many" in str(e) for gr in fails for e in gr.errors)
            cf_failed = any("not found" in str(e) or "verify" in str(e).lower() for gr in fails for e in gr.errors)

            if rate_limited:
                print(" [rate limited — backing off 10s]")
                await asyncio.sleep(10)
            elif cf_failed:
                print(" [CF API failure — backing off 5s]")
                await asyncio.sleep(5)
            else:
                print()
                await asyncio.sleep(2)  # Normal delay to keep CF API happy

    created = [gr for gr in game_results if gr.game_id]
    print(f"\n  Created: {len(created)}/{num_games}")

    create_times = [gr.create_time for gr in game_results if gr.create_time > 0]
    if create_times:
        latency_line("  Create latency", create_times)

    if not created:
        print("  FATAL: No games created.")
        return

    # ═══════════════════════════════════════════════
    # Phase B+C: Simulate ALL games concurrently
    # ═══════════════════════════════════════════════
    print(f"\n[Phase B+C] Simulating {len(created)} games concurrently...")

    for batch_start in range(0, len(created), sim_batch):
        batch_end = min(batch_start + sim_batch, len(created))
        batch = created[batch_start:batch_end]

        t0 = time.monotonic()
        await asyncio.gather(*[simulate_game(backend_url, gr, fire_rounds, hold_secs) for gr in batch])
        elapsed = time.monotonic() - t0

        phases = {}
        for gr in batch:
            phases[gr.phase_reached] = phases.get(gr.phase_reached, 0) + 1
        print(f"  [{batch_start:3d}-{batch_end:3d}] {elapsed:.1f}s | {phases}")

        if batch_end < len(created):
            await asyncio.sleep(0.5)

    wall_time = time.monotonic() - wall_start

    # ═══════════════════════════════════════════════
    # Report
    # ═══════════════════════════════════════════════
    print(f"\n{'='*64}")
    print(f"  STRESS TEST RESULTS  ({num_games} games requested)")
    print(f"{'='*64}")
    print(f"  Total wall time:      {wall_time:.1f}s")
    print(f"  Games created:        {len(created)}/{num_games}")

    # Count phases
    phases = {}
    for gr in game_results:
        phases[gr.phase_reached] = phases.get(gr.phase_reached, 0) + 1

    joined = sum(1 for gr in created if "joined" in gr.phase_reached or "placed" in gr.phase_reached or "fired" in gr.phase_reached)
    placed = sum(1 for gr in created if "placed" in gr.phase_reached or "fired" in gr.phase_reached)
    fired = sum(1 for gr in created if "fired" in gr.phase_reached)

    print(f"  P2 joined:            {joined}/{len(created)}")
    print(f"  Ships placed:         {placed}/{len(created)}")
    print(f"  Combat reached:       {fired}/{len(created)}")

    total_shots = sum(len(gr.fire_times) for gr in created)
    print(f"  Total shots fired:    {total_shots}")

    total_errors = sum(len(gr.errors) for gr in game_results)
    print(f"  Total errors:         {total_errors}")

    # Latencies
    print(f"\n  LATENCIES:")
    latency_line("  HTTP Create", [gr.create_time for gr in game_results if gr.create_time > 0])
    latency_line("  WS Connect", [t for gr in created for t in [gr.p1_ws_connect_time, gr.p2_ws_connect_time] if t > 0])
    latency_line("  P2 Join (inc. CF verify)", [gr.p2_join_time for gr in created if gr.p2_join_time > 0])
    latency_line("  Place Ships", [t for gr in created for t in [gr.p1_place_time, gr.p2_place_time] if t > 0])
    latency_line("  Fire Shot", [t for gr in created for t in gr.fire_times])

    # Tick analysis
    all_tick_intervals = [t for gr in created for t in gr.tick_times if t > 0]
    if all_tick_intervals:
        print(f"\n  TICK DELIVERY:")
        latency_line("  Tick interval", all_tick_intervals)
        # How many games got NO ticks?
        no_ticks = sum(1 for gr in created if not gr.tick_times)
        print(f"  Games with zero ticks: {no_ticks}/{len(created)}")
        # Jitter: how far from 1.0s are ticks?
        jitters = [abs(t - 1.0) for t in all_tick_intervals]
        if jitters:
            print(f"  Tick jitter (|interval - 1s|): avg={statistics.mean(jitters)*1000:.0f}ms max={max(jitters)*1000:.0f}ms")
    else:
        no_ticks = sum(1 for gr in created if not gr.tick_times)
        print(f"\n  TICK DELIVERY: NO DATA ({no_ticks} games with zero ticks)")

    # Error breakdown
    errs = {}
    for gr in game_results:
        for e in gr.errors:
            t = e.split(":")[0]
            errs[t] = errs.get(t, 0) + 1
    if errs:
        print(f"\n  ERRORS:")
        for t, c in sorted(errs.items(), key=lambda x: -x[1]):
            print(f"    {t}: {c}")

    # Phase breakdown
    print(f"\n  PHASES:")
    for p, c in sorted(phases.items(), key=lambda x: -x[1]):
        pct = c / num_games * 100
        print(f"    {p}: {c} ({pct:.0f}%)")

    # Sample errors
    failed = [gr for gr in game_results if gr.errors]
    if failed:
        print(f"\n  SAMPLE ERRORS (first 15):")
        for gr in failed[:15]:
            print(f"    Game {gr.game_index} [{gr.phase_reached}]: {'; '.join(gr.errors[:2])}")

    # Verdict
    success_pct = placed / num_games * 100 if num_games else 0
    fire_pct = fired / num_games * 100 if num_games else 0
    print(f"\n  {'='*60}")
    if success_pct >= 90:
        print(f"  VERDICT: PASS — {success_pct:.0f}% placed, {fire_pct:.0f}% in combat")
    elif success_pct >= 60:
        print(f"  VERDICT: DEGRADED — {success_pct:.0f}% placed, {fire_pct:.0f}% in combat")
    else:
        print(f"  VERDICT: FAIL — {success_pct:.0f}% placed, {fire_pct:.0f}% in combat")
    print(f"  {'='*60}\n")


def latency_line(label, values):
    if not values:
        print(f"{label}: no data")
        return
    ms = [v * 1000 for v in values]
    s = sorted(ms)
    n = len(s)
    print(f"{label}: n={n} min={s[0]:.0f}ms avg={statistics.mean(ms):.0f}ms "
          f"p50={s[n//2]:.0f}ms p95={s[int(n*0.95)]:.0f}ms p99={s[int(n*0.99)]:.0f}ms "
          f"max={s[-1]:.0f}ms")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--games", type=int, default=100)
    p.add_argument("--backend", default="https://battlecp-backend.whitepond-775af88d.centralindia.azurecontainerapps.io")
    p.add_argument("--create-batch", type=int, default=3, help="Games per creation batch (slow)")
    p.add_argument("--sim-batch", type=int, default=25, help="Games per simulation batch (concurrent)")
    p.add_argument("--fire-rounds", type=int, default=6)
    p.add_argument("--hold-secs", type=int, default=5, help="Seconds to hold WS open for tick measurement")
    args = p.parse_args()

    asyncio.run(run(args.backend, args.games, args.create_batch, args.sim_batch, args.fire_rounds, args.hold_secs))
