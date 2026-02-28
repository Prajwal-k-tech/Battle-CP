#!/usr/bin/env python3
"""
BattleCP Stress Test — spawns N concurrent games end-to-end.

Phases:
  1. Create N games via POST /api/game
  2. Connect 2 WebSocket clients per game (P1 host + P2 guest)
  3. Both players place ships
  4. Both players fire shots (triggering heat, locks, ticks)
  5. Measure latency at every step + detect errors

Usage:
  python stress_test.py [--games N] [--backend URL]
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
# Configuration
# ──────────────────────────────────────────────────

# Real CF handles (we need valid ones for the backend's verify_user_exists check)
# These are well-known competitive programmers whose handles definitely exist
# Expanded list to avoid hitting the 3-per-handle-per-5-min rate limit
CF_HANDLES = [
    "tourist", "Benq", "jiangly", "ksun48", "ecnerwala",
    "Um_nik", "Petr", "maroonrk", "Radewoosh", "heno239",
    "endagorion", "errorgorn", "ko_osaga", "dario2994", "maspy",
    "sunset", "mnbvmar", "noimi", "yosupo", "neal",
    "pashka", "Egor", "vepifanov", "SpyCheese", "adamant",
    "antontrygubO_o", "gamegame", "ScarletS", "wxhtzdy", "SSRS_",
    "Geothermal", "pajenegod", "BurnedChicken", "MiFaFaOvO", "rainboy",
    "Huah", "gyh20", "noshi91", "maspypy", "nnnn",
    "Pyqe", "zscoder", "FizzyDavid", "tabr", "Ormlis",
    "physics0523", "square1001", "mechanicalpulse", "nealwu", "bqi343",
    "272000",  "tempura0224", "Stonefeang", "orzdevinwang", "244mhq",
    "semiexp", "zhangzj", "Kaban5", "Errichto", "Ashishgup",
    "Monogon", "tfg", "300iq", "Swistakk", "1-gon",
    "Savior-of-Cross", "SecondThread", "Golovanov399", "Trick0lumber",
    "Dominater069", "yash_daga", "dorijanlendvaj", "kczno1", "I_love_Hoang_Yen",
    "tute7627", "Heltion", "EbTech", "mnaeraxr", "beet",
    "Kubic", "Al.Cash", "cerberus97", "Rewinding", "jqdai0815",
    "dreamoon", "apiad", "nong", "hbi1234", "chemthan",
    "rainboy", "eddy1021", "Emily2023", "demoralizer", "Rubikun",
    "GlebsHP", "Zlobober", "Merkurev", "isaf27", "PavelKunyavskiy",
    "rng_58", "LHiC", "yjq_naiive", "cmk", "ainta",
    # 100+ CF handles to support 100+ games without rate limit collisions
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
    # Timings (seconds)
    create_time: float = 0.0
    p1_ws_connect_time: float = 0.0
    p2_ws_connect_time: float = 0.0
    p1_join_time: float = 0.0
    p2_join_time: float = 0.0
    p1_place_time: float = 0.0
    p2_place_time: float = 0.0
    fire_times: list = field(default_factory=list)
    tick_latencies: list = field(default_factory=list)
    # Errors
    errors: list = field(default_factory=list)
    phase_reached: str = "init"


@dataclass
class StressResults:
    total_games: int = 0
    successful_creates: int = 0
    successful_ws_connects: int = 0
    successful_joins: int = 0
    successful_placements: int = 0
    successful_fires: int = 0
    total_errors: int = 0
    game_results: list = field(default_factory=list)
    create_times: list = field(default_factory=list)
    ws_connect_times: list = field(default_factory=list)
    join_times: list = field(default_factory=list)
    place_times: list = field(default_factory=list)
    fire_times: list = field(default_factory=list)
    tick_latencies: list = field(default_factory=list)
    errors_by_type: dict = field(default_factory=dict)
    wall_clock_start: float = 0.0
    wall_clock_end: float = 0.0


def get_handle_pair(game_index: int):
    """Get a unique pair of CF handles for a game."""
    h1 = CF_HANDLES[game_index % len(CF_HANDLES)]
    h2 = CF_HANDLES[(game_index + 1) % len(CF_HANDLES)]
    if h1 == h2:
        h2 = CF_HANDLES[(game_index + 2) % len(CF_HANDLES)]
    return h1, h2


# ──────────────────────────────────────────────────
# Phase 1: Create games via HTTP
# ──────────────────────────────────────────────────

async def create_game(session: aiohttp.ClientSession, backend_url: str,
                      game_index: int, result: GameResult):
    """Create a single game via POST /api/game."""
    h1, _ = get_handle_pair(game_index)
    payload = {
        "cf_handle": h1,
        "difficulty": 800,
        "heat_threshold": 3,  # Low threshold = more locks = more stress
        "game_duration_mins": 5,
        "veto_strictness": "low",
    }

    t0 = time.monotonic()
    try:
        async with session.post(
            f"{backend_url}/api/game",
            json=payload,
            timeout=aiohttp.ClientTimeout(total=30),
        ) as resp:
            elapsed = time.monotonic() - t0
            result.create_time = elapsed
            body = await resp.json()

            if resp.status == 201:
                result.game_id = body["game_id"]
                result.p1_id = body["player_id"]
                result.phase_reached = "created"
                return True
            else:
                err = body.get("error", f"HTTP {resp.status}")
                result.errors.append(f"create: {err}")
                return False
    except Exception as e:
        result.create_time = time.monotonic() - t0
        result.errors.append(f"create: {type(e).__name__}: {e}")
        return False


# ──────────────────────────────────────────────────
# Phase 2-5: WebSocket game simulation
# ──────────────────────────────────────────────────

async def recv_until(ws, msg_type: str, timeout: float = 15.0):
    """Receive messages until we get one with the given type, or timeout."""
    deadline = time.monotonic() + timeout
    messages = []
    while time.monotonic() < deadline:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=remaining)
            msg = json.loads(raw)
            messages.append(msg)
            if msg.get("type") == msg_type:
                return msg, messages
        except asyncio.TimeoutError:
            break
        except Exception:
            break
    return None, messages


async def drain_messages(ws, duration: float = 0.5):
    """Drain any buffered messages for a short duration."""
    messages = []
    deadline = time.monotonic() + duration
    while time.monotonic() < deadline:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=remaining)
            messages.append(json.loads(raw))
        except (asyncio.TimeoutError, Exception):
            break
    return messages


async def simulate_game(backend_url: str, result: GameResult, fire_rounds: int = 6):
    """
    Full game lifecycle for one game:
      1. P1 connects WS + JoinGame
      2. P2 connects WS + JoinGame
      3. Both place ships
      4. Both fire N rounds of shots
      5. Collect tick latencies throughout
    """
    ws_url = backend_url.replace("https://", "wss://").replace("http://", "ws://")
    game_id = result.game_id
    p1_id = result.p1_id
    p2_id = str(uuid.uuid4())
    _, h2 = get_handle_pair(result.game_index)

    ws1 = None
    ws2 = None

    try:
        # ── P1 Connect ──
        t0 = time.monotonic()
        ws1 = await asyncio.wait_for(
            websockets.connect(
                f"{ws_url}/ws/{game_id}?player_id={p1_id}",
                additional_headers={"Origin": "https://battle-cp.vercel.app"},
                ping_interval=20,
                ping_timeout=10,
                close_timeout=5,
            ),
            timeout=15,
        )
        result.p1_ws_connect_time = time.monotonic() - t0

        # P1 JoinGame
        t0 = time.monotonic()
        await ws1.send(json.dumps({
            "type": "JoinGame",
            "player_id": p1_id,
            "cf_handle": get_handle_pair(result.game_index)[0],
        }))
        msg, _ = await recv_until(ws1, "GameJoined", timeout=15)
        result.p1_join_time = time.monotonic() - t0
        if not msg:
            result.errors.append("p1_join: no GameJoined received")
            return
        result.phase_reached = "p1_joined"

        # ── P2 Connect ──
        t0 = time.monotonic()
        ws2 = await asyncio.wait_for(
            websockets.connect(
                f"{ws_url}/ws/{game_id}?player_id={p2_id}",
                additional_headers={"Origin": "https://battle-cp.vercel.app"},
                ping_interval=20,
                ping_timeout=10,
                close_timeout=5,
            ),
            timeout=15,
        )
        result.p2_ws_connect_time = time.monotonic() - t0

        # P2 JoinGame
        t0 = time.monotonic()
        await ws2.send(json.dumps({
            "type": "JoinGame",
            "player_id": p2_id,
            "cf_handle": h2,
        }))
        msg, all_msgs = await recv_until(ws2, "GameJoined", timeout=15)
        result.p2_join_time = time.monotonic() - t0
        if not msg:
            errs = [m for m in all_msgs if m.get("type") == "Error"]
            err_detail = errs[0].get("message", "unknown") if errs else "no GameJoined"
            result.errors.append(f"p2_join: {err_detail}")
            return
        result.phase_reached = "p2_joined"

        # Wait for P1 to get PlayerJoined notification
        await drain_messages(ws1, 2.0)

        # ── Place Ships (both players) ──
        t0 = time.monotonic()
        await ws1.send(json.dumps({"type": "PlaceShips", "ships": STANDARD_FLEET}))
        msg1, _ = await recv_until(ws1, "GameUpdate", timeout=10)
        result.p1_place_time = time.monotonic() - t0
        if not msg1:
            result.errors.append("p1_place: no GameUpdate after placement")
            return

        t0 = time.monotonic()
        await ws2.send(json.dumps({"type": "PlaceShips", "ships": STANDARD_FLEET}))
        msg2, _ = await recv_until(ws2, "GameUpdate", timeout=10)
        result.p2_place_time = time.monotonic() - t0
        if not msg2:
            result.errors.append("p2_place: no GameUpdate after placement")
            return
        result.phase_reached = "ships_placed"

        # Drain GameStart broadcasts
        await asyncio.gather(
            drain_messages(ws1, 1.0),
            drain_messages(ws2, 1.0),
        )

        # ── Fire Shots ──
        # Generate random non-overlapping coordinates
        coords = [(x, y) for x in range(10) for y in range(10)]
        random.shuffle(coords)

        shots_fired = 0
        for i in range(min(fire_rounds, len(coords))):
            x, y = coords[i]
            # Alternate: P1 fires odd rounds, P2 fires even
            shooter_ws = ws1 if i % 2 == 0 else ws2

            t0 = time.monotonic()
            await shooter_ws.send(json.dumps({"type": "Fire", "x": x, "y": y}))

            # Wait for either a ShotResult broadcast or a GameUpdate or Error
            deadline = time.monotonic() + 5.0
            got_response = False
            while time.monotonic() < deadline:
                try:
                    raw = await asyncio.wait_for(
                        shooter_ws.recv(),
                        timeout=deadline - time.monotonic(),
                    )
                    msg = json.loads(raw)
                    if msg.get("type") in ("ShotResult", "Error", "GameOver"):
                        fire_elapsed = time.monotonic() - t0
                        result.fire_times.append(fire_elapsed)
                        got_response = True
                        shots_fired += 1
                        if msg.get("type") == "Error":
                            # Weapons locked — expected when heat_threshold=3
                            break
                        if msg.get("type") == "GameOver":
                            result.phase_reached = "game_over"
                            return
                        break
                    elif msg.get("type") == "GameUpdate":
                        # Tick — record latency
                        result.tick_latencies.append(time.monotonic() - t0)
                except (asyncio.TimeoutError, Exception):
                    break

            if not got_response:
                result.errors.append(f"fire_{i}: no response within 5s")

        result.phase_reached = f"fired_{shots_fired}_shots"

        # ── Collect ticks for a few seconds to measure tick delivery ──
        tick_start = time.monotonic()
        tick_count = 0
        while time.monotonic() - tick_start < 3.0:
            try:
                raw = await asyncio.wait_for(ws1.recv(), timeout=1.5)
                msg = json.loads(raw)
                if msg.get("type") == "GameUpdate":
                    tick_count += 1
                    result.tick_latencies.append(0)  # placeholder — we just count delivery
            except (asyncio.TimeoutError, Exception):
                break

        if tick_count == 0:
            result.errors.append("ticks: no ticks received in 3 seconds")

    except asyncio.TimeoutError:
        result.errors.append(f"timeout at phase {result.phase_reached}")
    except websockets.exceptions.ConnectionClosed as e:
        result.errors.append(f"ws_closed: {e.code} {e.reason} at {result.phase_reached}")
    except Exception as e:
        result.errors.append(f"{type(e).__name__}: {e} at {result.phase_reached}")
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

async def run_stress_test(backend_url: str, num_games: int, batch_size: int = 10,
                          fire_rounds: int = 6):
    results = StressResults()
    results.total_games = num_games
    results.wall_clock_start = time.monotonic()
    game_results = []

    print(f"\n{'='*60}")
    print(f"  BattleCP Stress Test")
    print(f"  Backend: {backend_url}")
    print(f"  Games: {num_games} | Batch size: {batch_size}")
    print(f"  Fire rounds per game: {fire_rounds}")
    print(f"{'='*60}\n")

    # ── Phase 1: Create all games (batched to avoid rate limits) ──
    print(f"[Phase 1] Creating {num_games} games...")
    created_results = []

    async with aiohttp.ClientSession() as session:
        for batch_start in range(0, num_games, batch_size):
            batch_end = min(batch_start + batch_size, num_games)
            batch = []
            for i in range(batch_start, batch_end):
                gr = GameResult(game_index=i)
                game_results.append(gr)
                batch.append(create_game(session, backend_url, i, gr))

            batch_results = await asyncio.gather(*batch, return_exceptions=True)
            successes = sum(1 for r in batch_results if r is True)
            results.successful_creates += successes
            print(f"  Batch {batch_start}-{batch_end}: {successes}/{len(batch)} created")

            # Small delay between batches to respect rate limits
            if batch_end < num_games:
                await asyncio.sleep(0.5)

    created_games = [gr for gr in game_results if gr.game_id]
    results.create_times = [gr.create_time for gr in game_results if gr.create_time > 0]

    print(f"\n  Total created: {len(created_games)}/{num_games}")
    if results.create_times:
        print(f"  Create latency: min={min(results.create_times)*1000:.0f}ms "
              f"avg={statistics.mean(results.create_times)*1000:.0f}ms "
              f"max={max(results.create_times)*1000:.0f}ms "
              f"p95={sorted(results.create_times)[int(len(results.create_times)*0.95)]*1000:.0f}ms")

    if not created_games:
        print("\n  FATAL: No games created. Aborting.")
        return results

    # ── Phase 2-5: Simulate all games (batched) ──
    print(f"\n[Phase 2-5] Simulating {len(created_games)} games (WS connect → join → place → fire)...")

    for batch_start in range(0, len(created_games), batch_size):
        batch_end = min(batch_start + batch_size, len(created_games))
        batch_games = created_games[batch_start:batch_end]
        tasks = [simulate_game(backend_url, gr, fire_rounds) for gr in batch_games]

        print(f"  Batch {batch_start}-{batch_end}: simulating {len(tasks)} games...")
        await asyncio.gather(*tasks, return_exceptions=True)

        # Aggregate batch stats
        for gr in batch_games:
            if gr.p1_ws_connect_time > 0:
                results.ws_connect_times.append(gr.p1_ws_connect_time)
            if gr.p2_ws_connect_time > 0:
                results.ws_connect_times.append(gr.p2_ws_connect_time)
                results.successful_ws_connects += 1
            if gr.p1_join_time > 0:
                results.join_times.append(gr.p1_join_time)
            if gr.p2_join_time > 0:
                results.join_times.append(gr.p2_join_time)
                results.successful_joins += 1
            if gr.p1_place_time > 0:
                results.place_times.append(gr.p1_place_time)
            if gr.p2_place_time > 0:
                results.place_times.append(gr.p2_place_time)
                results.successful_placements += 1
            results.fire_times.extend(gr.fire_times)
            results.successful_fires += len(gr.fire_times)
            results.tick_latencies.extend(gr.tick_latencies)

            for err in gr.errors:
                results.total_errors += 1
                err_type = err.split(":")[0]
                results.errors_by_type[err_type] = results.errors_by_type.get(err_type, 0) + 1

        phases = {}
        for gr in batch_games:
            phases[gr.phase_reached] = phases.get(gr.phase_reached, 0) + 1
        print(f"    Phases reached: {phases}")

        if batch_end < len(created_games):
            await asyncio.sleep(1.0)

    results.wall_clock_end = time.monotonic()
    results.game_results = game_results

    # ── Print Report ──
    print_report(results)
    return results


def latency_stats(values, label):
    if not values:
        print(f"  {label}: no data")
        return
    values_ms = [v * 1000 for v in values]
    p50 = sorted(values_ms)[len(values_ms) // 2]
    p95 = sorted(values_ms)[int(len(values_ms) * 0.95)]
    p99 = sorted(values_ms)[int(len(values_ms) * 0.99)]
    print(f"  {label}: n={len(values_ms)} min={min(values_ms):.0f}ms "
          f"avg={statistics.mean(values_ms):.0f}ms p50={p50:.0f}ms "
          f"p95={p95:.0f}ms p99={p99:.0f}ms max={max(values_ms):.0f}ms")


def print_report(r: StressResults):
    wall_time = r.wall_clock_end - r.wall_clock_start
    print(f"\n{'='*60}")
    print(f"  STRESS TEST RESULTS")
    print(f"{'='*60}")
    print(f"\n  Wall clock time: {wall_time:.1f}s")
    print(f"  Games attempted: {r.total_games}")
    print(f"  Games created: {r.successful_creates}")
    print(f"  WS connections (P2): {r.successful_ws_connects}")
    print(f"  Successful joins: {r.successful_joins}")
    print(f"  Successful placements: {r.successful_placements}")
    print(f"  Shots fired: {r.successful_fires}")
    print(f"  Total errors: {r.total_errors}")

    print(f"\n  LATENCY BREAKDOWN:")
    latency_stats(r.create_times, "Game Create (HTTP)")
    latency_stats(r.ws_connect_times, "WS Connect")
    latency_stats(r.join_times, "Join Game (WS)")
    latency_stats(r.place_times, "Place Ships")
    latency_stats(r.fire_times, "Fire Shot")

    if r.errors_by_type:
        print(f"\n  ERRORS BY TYPE:")
        for err_type, count in sorted(r.errors_by_type.items(), key=lambda x: -x[1]):
            print(f"    {err_type}: {count}")

    # Phase analysis
    phases = {}
    for gr in r.game_results:
        phases[gr.phase_reached] = phases.get(gr.phase_reached, 0) + 1
    print(f"\n  GAME PHASES REACHED:")
    for phase, count in sorted(phases.items(), key=lambda x: -x[1]):
        pct = count / r.total_games * 100
        print(f"    {phase}: {count} ({pct:.0f}%)")

    # Print failed game details (first 10)
    failed = [gr for gr in r.game_results if gr.errors]
    if failed:
        print(f"\n  SAMPLE ERRORS (first 10 of {len(failed)}):")
        for gr in failed[:10]:
            print(f"    Game {gr.game_index} [{gr.phase_reached}]: {'; '.join(gr.errors)}")

    # Verdict
    print(f"\n  {'='*56}")
    success_rate = r.successful_placements / r.total_games * 100 if r.total_games else 0
    fire_rate = r.successful_fires / (r.total_games * 6) * 100 if r.total_games else 0
    if success_rate >= 95 and fire_rate >= 80:
        print(f"  VERDICT: PASS — {success_rate:.0f}% placement, {fire_rate:.0f}% fire rate")
    elif success_rate >= 70:
        print(f"  VERDICT: DEGRADED — {success_rate:.0f}% placement, {fire_rate:.0f}% fire rate")
    else:
        print(f"  VERDICT: FAIL — {success_rate:.0f}% placement, {fire_rate:.0f}% fire rate")
    print(f"  {'='*56}\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="BattleCP Stress Test")
    parser.add_argument("--games", type=int, default=100, help="Number of concurrent games")
    parser.add_argument("--backend", type=str,
                        default="https://battlecp-backend.whitepond-775af88d.centralindia.azurecontainerapps.io",
                        help="Backend URL")
    parser.add_argument("--batch", type=int, default=10, help="Batch size for parallel operations")
    parser.add_argument("--fire-rounds", type=int, default=6, help="Shots per game")
    args = parser.parse_args()

    asyncio.run(run_stress_test(args.backend, args.games, args.batch, args.fire_rounds))
