#!/usr/bin/env python3
"""
BattleCP Stress Test v3 — No CF Verification
==============================================
Tests 100 concurrent games against the local server (CF verify removed).
Measures: creation latency, P2 join latency, placement success, tick delivery,
fire latency, and overall game lifecycle.

Usage: python scripts/stress_test_v3.py [--games N] [--url URL]
"""

import asyncio
import aiohttp
import json
import time
import sys
import random
import statistics
from dataclasses import dataclass, field
from typing import Optional

# --- Config ---
BASE_URL = "http://localhost:3000"
WS_BASE = "ws://localhost:3000"
NUM_GAMES = 100
BATCH_CREATE = 10       # Create 10 games per batch
BATCH_DELAY = 0.3       # 300ms between creation batches
WS_CONNECT_TIMEOUT = 10
MSG_TIMEOUT = 15

# Parse args
for i, arg in enumerate(sys.argv[1:], 1):
    if arg == "--games" and i < len(sys.argv) - 1:
        NUM_GAMES = int(sys.argv[i + 1])
    elif arg == "--url" and i < len(sys.argv) - 1:
        BASE_URL = sys.argv[i + 1]
        WS_BASE = BASE_URL.replace("http", "ws")


@dataclass
class GameMetrics:
    game_id: str = ""
    # Timing
    create_ms: float = 0
    p2_join_ms: float = 0
    p1_place_ms: float = 0
    p2_place_ms: float = 0
    fire_latencies_ms: list = field(default_factory=list)
    # Success flags
    created: bool = False
    p2_joined: bool = False
    p1_placed: bool = False
    p2_placed: bool = False
    game_started: bool = False
    shots_fired: int = 0
    ticks_received_p1: int = 0
    ticks_received_p2: int = 0
    # Errors
    errors: list = field(default_factory=list)


# Standard battleship fleet
FLEET = [
    {"x": 0, "y": 0, "size": 5, "vertical": True},
    {"x": 2, "y": 0, "size": 4, "vertical": True},
    {"x": 4, "y": 0, "size": 3, "vertical": True},
    {"x": 6, "y": 0, "size": 3, "vertical": True},
    {"x": 8, "y": 0, "size": 2, "vertical": True},
]


async def recv_msg(ws, timeout=MSG_TIMEOUT):
    """Receive and parse a JSON WS message with timeout."""
    try:
        msg = await asyncio.wait_for(ws.receive(), timeout=timeout)
        if msg.type == aiohttp.WSMsgType.TEXT:
            return json.loads(msg.data)
        elif msg.type in (aiohttp.WSMsgType.CLOSED, aiohttp.WSMsgType.ERROR):
            return None
    except asyncio.TimeoutError:
        return None
    return None


async def drain_until(ws, msg_type: str, timeout=MSG_TIMEOUT, tick_counter=None):
    """Read messages until we find one with the given type field. Count ticks."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        msg = await recv_msg(ws, timeout=remaining)
        if msg is None:
            return None
        # Count ticks
        if tick_counter is not None and "status" in msg and "heat" in msg and "time_remaining_secs" in msg:
            tick_counter[0] += 1
        # Check for target message type
        if msg_type == "GameJoined" and "game_id" in msg and "player_id" in msg:
            return msg
        if msg_type == "PlayerJoined" and "player_id" in msg and "game_id" not in msg:
            return msg
        if msg_type == "ShipsConfirmed" and msg.get("type") == "ShipsConfirmed":
            return msg
        if msg_type == "GameStart" and msg.get("type") == "GameStart":
            return msg
        if msg_type == "GameUpdate" and "status" in msg and "heat" in msg:
            return msg
        if msg_type == "error" and "message" in msg and "game_id" not in msg and "heat" not in msg:
            return msg
    return None


async def run_game(session: aiohttp.ClientSession, game_idx: int, metrics: GameMetrics):
    """Run a complete game lifecycle: create → join → place → fire."""
    p1_ws = None
    p2_ws = None

    try:
        # === PHASE 1: Create Game ===
        t0 = time.monotonic()
        async with session.post(
            f"{BASE_URL}/api/game",
            json={"cf_handle": f"stress_p1_{game_idx}", "difficulty": 800, "heat_threshold": 3},
        ) as resp:
            metrics.create_ms = (time.monotonic() - t0) * 1000
            if resp.status != 201:
                metrics.errors.append(f"create: HTTP {resp.status}")
                return
            data = await resp.json()
            game_id = data["game_id"]
            p1_id = data["player_id"]
            metrics.game_id = game_id
            metrics.created = True

        # === PHASE 2: P1 connects via WebSocket ===
        p1_ws = await session.ws_connect(
            f"{WS_BASE}/ws/{game_id}?player_id={p1_id}",
            timeout=WS_CONNECT_TIMEOUT,
        )
        # P1 sends JoinGame
        await p1_ws.send_json({
            "type": "JoinGame",
            "player_id": p1_id,
            "cf_handle": f"stress_p1_{game_idx}",
        })
        p1_joined = await drain_until(p1_ws, "GameJoined", timeout=5)
        if not p1_joined:
            metrics.errors.append("p1: no GameJoined")
            return

        # === PHASE 3: P2 connects and joins ===
        import uuid
        p2_id = str(uuid.uuid4())
        t0 = time.monotonic()
        p2_ws = await session.ws_connect(
            f"{WS_BASE}/ws/{game_id}?player_id={p2_id}",
            timeout=WS_CONNECT_TIMEOUT,
        )
        await p2_ws.send_json({
            "type": "JoinGame",
            "player_id": p2_id,
            "cf_handle": f"stress_p2_{game_idx}",
        })
        p2_joined_msg = await drain_until(p2_ws, "GameJoined", timeout=5)
        metrics.p2_join_ms = (time.monotonic() - t0) * 1000
        if not p2_joined_msg:
            metrics.errors.append("p2: no GameJoined")
            return
        metrics.p2_joined = True

        # Small delay for broadcast propagation
        await asyncio.sleep(0.1)

        # === PHASE 4: Place Ships ===
        t0 = time.monotonic()
        await p1_ws.send_json({"type": "PlaceShips", "ships": FLEET})
        p1_conf = await drain_until(p1_ws, "GameUpdate", timeout=5)
        metrics.p1_place_ms = (time.monotonic() - t0) * 1000
        if p1_conf:
            metrics.p1_placed = True
        else:
            metrics.errors.append("p1: no placement confirmation")
            return

        t0 = time.monotonic()
        await p2_ws.send_json({"type": "PlaceShips", "ships": FLEET})
        p2_conf = await drain_until(p2_ws, "GameUpdate", timeout=5)
        metrics.p2_place_ms = (time.monotonic() - t0) * 1000
        if p2_conf:
            metrics.p2_placed = True
        else:
            metrics.errors.append("p2: no placement confirmation")
            return

        # Drain for GameStart on both sides
        await asyncio.sleep(0.2)
        metrics.game_started = True

        # === PHASE 5: Fire shots (3 rounds each) ===
        # Generate random non-repeating coordinates
        all_coords = [(x, y) for x in range(10) for y in range(10)]
        random.shuffle(all_coords)
        p1_targets = all_coords[:6]
        p2_targets = all_coords[6:12]

        tick_counter_p1 = [0]
        tick_counter_p2 = [0]

        for i in range(3):
            # P1 fires
            x, y = p1_targets[i]
            t0 = time.monotonic()
            await p1_ws.send_json({"type": "Fire", "x": x, "y": y})
            # Drain until we get something (ShotResult broadcast or GameUpdate tick)
            resp = await recv_msg(p1_ws, timeout=5)
            latency = (time.monotonic() - t0) * 1000
            if resp:
                metrics.fire_latencies_ms.append(latency)
                metrics.shots_fired += 1
                if "heat" in resp:
                    tick_counter_p1[0] += 1

            # P2 fires
            x, y = p2_targets[i]
            t0 = time.monotonic()
            await p2_ws.send_json({"type": "Fire", "x": x, "y": y})
            resp = await recv_msg(p2_ws, timeout=5)
            latency = (time.monotonic() - t0) * 1000
            if resp:
                metrics.fire_latencies_ms.append(latency)
                metrics.shots_fired += 1
                if "heat" in resp:
                    tick_counter_p2[0] += 1

            await asyncio.sleep(0.05)

        # Collect ticks for 3 seconds
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            try:
                msg = await asyncio.wait_for(p1_ws.receive(), timeout=0.5)
                if msg.type == aiohttp.WSMsgType.TEXT:
                    data = json.loads(msg.data)
                    if "heat" in data and "time_remaining_secs" in data:
                        tick_counter_p1[0] += 1
            except asyncio.TimeoutError:
                break

        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            try:
                msg = await asyncio.wait_for(p2_ws.receive(), timeout=0.5)
                if msg.type == aiohttp.WSMsgType.TEXT:
                    data = json.loads(msg.data)
                    if "heat" in data and "time_remaining_secs" in data:
                        tick_counter_p2[0] += 1
            except asyncio.TimeoutError:
                break

        metrics.ticks_received_p1 = tick_counter_p1[0]
        metrics.ticks_received_p2 = tick_counter_p2[0]

    except Exception as e:
        metrics.errors.append(f"exception: {type(e).__name__}: {e}")
    finally:
        if p1_ws and not p1_ws.closed:
            await p1_ws.close()
        if p2_ws and not p2_ws.closed:
            await p2_ws.close()


async def main():
    print(f"╔══════════════════════════════════════════════════╗")
    print(f"║  BattleCP Stress Test v3 — No CF Verification   ║")
    print(f"║  Games: {NUM_GAMES:<5}  URL: {BASE_URL:<23} ║")
    print(f"╚══════════════════════════════════════════════════╝")
    print()

    connector = aiohttp.TCPConnector(limit=0, limit_per_host=0)
    session = aiohttp.ClientSession(connector=connector)

    all_metrics = [GameMetrics() for _ in range(NUM_GAMES)]

    # Launch games in batches
    print(f"Launching {NUM_GAMES} games in batches of {BATCH_CREATE}...")
    t_start = time.monotonic()

    tasks = []
    for batch_start in range(0, NUM_GAMES, BATCH_CREATE):
        batch_end = min(batch_start + BATCH_CREATE, NUM_GAMES)
        batch_tasks = []
        for i in range(batch_start, batch_end):
            task = asyncio.create_task(run_game(session, i, all_metrics[i]))
            batch_tasks.append(task)
            tasks.append(task)
        await asyncio.sleep(BATCH_DELAY)

    # Wait for all games to complete
    await asyncio.gather(*tasks, return_exceptions=True)
    t_total = time.monotonic() - t_start

    await session.close()

    # === ANALYSIS ===
    print(f"\n{'='*60}")
    print(f"  RESULTS — {NUM_GAMES} games in {t_total:.1f}s")
    print(f"{'='*60}\n")

    created = [m for m in all_metrics if m.created]
    joined = [m for m in all_metrics if m.p2_joined]
    p1_placed = [m for m in all_metrics if m.p1_placed]
    p2_placed = [m for m in all_metrics if m.p2_placed]
    started = [m for m in all_metrics if m.game_started]
    fired = [m for m in all_metrics if m.shots_fired > 0]
    errored = [m for m in all_metrics if m.errors]

    print(f"  Phase Success Rates:")
    print(f"  {'Create:':<20} {len(created):>4}/{NUM_GAMES}  ({100*len(created)/NUM_GAMES:.0f}%)")
    print(f"  {'P2 Join:':<20} {len(joined):>4}/{len(created)}  ({100*len(joined)/max(1,len(created)):.0f}%)")
    print(f"  {'P1 Placement:':<20} {len(p1_placed):>4}/{len(joined)}  ({100*len(p1_placed)/max(1,len(joined)):.0f}%)")
    print(f"  {'P2 Placement:':<20} {len(p2_placed):>4}/{len(p1_placed)}  ({100*len(p2_placed)/max(1,len(p1_placed)):.0f}%)")
    print(f"  {'Game Started:':<20} {len(started):>4}/{len(p2_placed)}  ({100*len(started)/max(1,len(p2_placed)):.0f}%)")
    print(f"  {'Fired Shots:':<20} {len(fired):>4}/{len(started)}  ({100*len(fired)/max(1,len(started)):.0f}%)")

    # Latency stats
    if created:
        create_times = [m.create_ms for m in created]
        print(f"\n  Creation Latency (ms):")
        print(f"    avg={statistics.mean(create_times):.0f}  median={statistics.median(create_times):.0f}  "
              f"p95={sorted(create_times)[int(0.95*len(create_times))]:.0f}  "
              f"max={max(create_times):.0f}")

    if joined:
        join_times = [m.p2_join_ms for m in joined]
        print(f"\n  P2 Join Latency (ms):")
        print(f"    avg={statistics.mean(join_times):.0f}  median={statistics.median(join_times):.0f}  "
              f"p95={sorted(join_times)[int(0.95*len(join_times))]:.0f}  "
              f"max={max(join_times):.0f}")

    all_fire = []
    for m in all_metrics:
        all_fire.extend(m.fire_latencies_ms)
    if all_fire:
        print(f"\n  Fire Latency (ms) [{len(all_fire)} shots]:")
        print(f"    avg={statistics.mean(all_fire):.0f}  median={statistics.median(all_fire):.0f}  "
              f"p95={sorted(all_fire)[int(0.95*len(all_fire))]:.0f}  "
              f"max={max(all_fire):.0f}")

    # Tick delivery
    tick_counts_p1 = [m.ticks_received_p1 for m in started]
    tick_counts_p2 = [m.ticks_received_p2 for m in started]
    if tick_counts_p1:
        zero_ticks = sum(1 for t in tick_counts_p1 if t == 0)
        print(f"\n  Tick Delivery (P1):")
        print(f"    avg={statistics.mean(tick_counts_p1):.1f}  min={min(tick_counts_p1)}  "
              f"max={max(tick_counts_p1)}  zero_tick_games={zero_ticks}/{len(tick_counts_p1)}")

    if tick_counts_p2:
        zero_ticks = sum(1 for t in tick_counts_p2 if t == 0)
        print(f"  Tick Delivery (P2):")
        print(f"    avg={statistics.mean(tick_counts_p2):.1f}  min={min(tick_counts_p2)}  "
              f"max={max(tick_counts_p2)}  zero_ticks_games={zero_ticks}/{len(tick_counts_p2)}")

    # Errors
    if errored:
        print(f"\n  Errors ({len(errored)} games had errors):")
        error_counts = {}
        for m in errored:
            for e in m.errors:
                key = e.split(":")[0]
                error_counts[key] = error_counts.get(key, 0) + 1
        for err, count in sorted(error_counts.items(), key=lambda x: -x[1])[:10]:
            print(f"    {count:>4}x  {err}")

    # Final verdict
    print(f"\n{'='*60}")
    full_success = len(fired)
    rate = 100 * full_success / NUM_GAMES
    if rate >= 95:
        verdict = "PASS"
    elif rate >= 70:
        verdict = "DEGRADED"
    else:
        verdict = "FAIL"
    print(f"  VERDICT: {verdict}  ({full_success}/{NUM_GAMES} = {rate:.0f}% full lifecycle)")
    print(f"{'='*60}")


if __name__ == "__main__":
    asyncio.run(main())
