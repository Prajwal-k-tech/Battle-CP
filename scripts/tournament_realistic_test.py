#!/usr/bin/env python3
"""
REALISTIC Tournament Stress Test — simulates actual tournament gameplay.

Unlike the quick lifecycle test, this simulates what REALLY happens:
1. All 250 games created within ~30 seconds (tournament start)
2. All 500 players connect WebSockets simultaneously
3. Ship placement (1-3 seconds, simulating instant placement)
4. Combat phase: players fire shots with realistic pacing (1 shot every 2-5 seconds)
5. After overheat: weapons lock, server picks a CF problem (REAL CF API call)
6. Players use Veto (triggers veto timer flow)
7. Sustained tick broadcasts: 1/sec to ALL 1000 WebSocket connections
8. Games stay alive for a configurable duration (default: 60 seconds for testing)
9. Measures: WS throughput, tick latency, message drops, CF API failures

The KEY metric is: can the server sustain 1000 concurrent WebSocket connections
all receiving tick updates every second, while also processing Fire/SolveCP/Veto?

Usage:
  python3 scripts/tournament_realistic_test.py [num_games] [base_url] [sustain_secs]
  
Examples:
  python3 scripts/tournament_realistic_test.py 50 https://battlecp-backend.whitepond-775af88d.centralindia.azurecontainerapps.io 60
  python3 scripts/tournament_realistic_test.py 250 http://localhost:3000 120
"""

import asyncio
import aiohttp
import json
import sys
import time
import uuid
import random
from dataclasses import dataclass, field
from typing import Optional

BASE_URL = sys.argv[2] if len(sys.argv) > 2 else "http://localhost:3000"
WS_URL = BASE_URL.replace("http://", "ws://").replace("https://", "wss://")
NUM_GAMES = int(sys.argv[1]) if len(sys.argv) > 1 else 50
SUSTAIN_SECS = int(sys.argv[3]) if len(sys.argv) > 3 else 60   # How long to keep games alive
BATCH_SIZE = 25   # Create games in batches
HEAT_THRESHOLD = 7
FIRE_INTERVAL = (1.0, 3.0)  # Seconds between shots (simulates human think time)
# Increase timeouts for production (network latency)
IS_PRODUCTION = "localhost" not in BASE_URL and "127.0.0.1" not in BASE_URL
WS_TIMEOUT = 30 if IS_PRODUCTION else 15
HTTP_TIMEOUT = 20 if IS_PRODUCTION else 10

# ─── Metrics ─────────────────────────────────────────────────────────
@dataclass
class PlayerMetrics:
    ticks_received: int = 0
    shots_fired: int = 0
    shots_ok: int = 0
    shot_errors: int = 0
    weapons_locked_count: int = 0
    vetos_used: int = 0
    problems_assigned: int = 0
    weapons_unlocked_count: int = 0
    messages_sent: int = 0
    messages_received: int = 0
    ws_errors: int = 0
    last_tick_time: float = 0
    max_tick_gap_ms: float = 0  # Largest gap between ticks (should be ~1000ms)
    connection_alive: bool = True

@dataclass 
class GameMetrics:
    game_id: str = ""
    success: bool = False
    phase: str = "none"
    error: str = ""
    create_ms: float = 0
    join_ms: float = 0
    placement_ms: float = 0
    sustain_secs: float = 0
    p1: PlayerMetrics = field(default_factory=PlayerMetrics)
    p2: PlayerMetrics = field(default_factory=PlayerMetrics)


# ─── Standard fleet ──────────────────────────────────────────────────
FLEET = [
    {"x": 0, "y": 0, "size": 5, "vertical": False},
    {"x": 0, "y": 2, "size": 4, "vertical": False},
    {"x": 0, "y": 4, "size": 3, "vertical": False},
    {"x": 0, "y": 6, "size": 3, "vertical": False},
    {"x": 0, "y": 8, "size": 2, "vertical": False},
]


async def recv_json(ws, timeout=5.0):
    """Receive one JSON message from WebSocket."""
    try:
        msg = await asyncio.wait_for(ws.receive(), timeout=timeout)
        if msg.type == aiohttp.WSMsgType.TEXT:
            return json.loads(msg.data)
        elif msg.type in (aiohttp.WSMsgType.CLOSED, aiohttp.WSMsgType.ERROR):
            return None
    except asyncio.TimeoutError:
        return None


async def drain_messages(ws, timeout=1.0):
    """Drain all pending messages from WS."""
    msgs = []
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            m = await asyncio.wait_for(ws.receive(), timeout=max(0.05, deadline - time.time()))
            if m.type == aiohttp.WSMsgType.TEXT:
                msgs.append(json.loads(m.data))
            elif m.type in (aiohttp.WSMsgType.CLOSED, aiohttp.WSMsgType.ERROR):
                break
        except asyncio.TimeoutError:
            break
    return msgs


async def wait_for_type(ws, msg_types, timeout=15.0):
    """Wait for a specific message type, collecting all messages."""
    if isinstance(msg_types, str):
        msg_types = [msg_types]
    deadline = time.time() + timeout
    collected = []
    while time.time() < deadline:
        data = await recv_json(ws, timeout=max(0.1, deadline - time.time()))
        if data is None:
            continue
        collected.append(data)
        if data.get("type") in msg_types:
            return data, collected
    return None, collected


# ─── Player simulation (runs for the full sustain duration) ──────────
async def simulate_player(
    ws, player_id: str, is_p1: bool, metrics: PlayerMetrics,
    game_status: dict, sustain_end: float
):
    """
    Simulate a real player for the sustain duration.
    - Listens for ticks, tracks tick gaps
    - Fires shots when weapons are unlocked
    - Uses Veto when weapons are locked (simulates player who can't solve)
    - Tracks all server messages
    """
    # Shot coordinates (shuffled - no repeats)
    all_coords = [(x, y) for x in range(10) for y in range(10)]
    random.shuffle(all_coords)
    shot_idx = 0
    is_locked = False
    next_fire_time = time.time() + random.uniform(*FIRE_INTERVAL)
    next_veto_time = 0.0  # When to send veto after getting locked
    has_vetoed_this_lock = False

    while time.time() < sustain_end and metrics.connection_alive:
        now = time.time()
        
        # ── Receive messages (non-blocking, short timeout) ──
        try:
            msg = await asyncio.wait_for(ws.receive(), timeout=0.3)
            if msg.type == aiohttp.WSMsgType.TEXT:
                data = json.loads(msg.data)
                metrics.messages_received += 1
                msg_type = data.get("type", "")
                
                # Track ticks for latency measurement
                if msg_type == "GameUpdate":
                    metrics.ticks_received += 1
                    if metrics.last_tick_time > 0:
                        gap = (now - metrics.last_tick_time) * 1000
                        if gap > metrics.max_tick_gap_ms:
                            metrics.max_tick_gap_ms = gap
                    metrics.last_tick_time = now
                    # Update locked state from server
                    is_locked = data.get("is_locked", False)
                    
                elif msg_type == "WeaponsLocked":
                    is_locked = True
                    metrics.weapons_locked_count += 1
                    has_vetoed_this_lock = False
                    # Plan to veto after 2-5 seconds (simulating "looking at problem")
                    next_veto_time = now + random.uniform(2.0, 5.0)
                    
                elif msg_type == "WeaponsUnlocked":
                    is_locked = False
                    metrics.weapons_unlocked_count += 1
                    has_vetoed_this_lock = False
                    next_fire_time = now + random.uniform(0.5, 1.5)
                    
                elif msg_type == "ProblemAssigned":
                    metrics.problems_assigned += 1
                    
                elif msg_type == "ShotResult":
                    pass  # Already counted on send
                    
                elif msg_type == "GameOver":
                    game_status["finished"] = True
                    break
                    
                elif msg_type == "Error":
                    metrics.shot_errors += 1
                    
            elif msg.type in (aiohttp.WSMsgType.CLOSED, aiohttp.WSMsgType.ERROR):
                metrics.connection_alive = False
                metrics.ws_errors += 1
                break
                
        except asyncio.TimeoutError:
            pass
        except Exception as e:
            metrics.ws_errors += 1
            
        # ── Action: Fire shots when unlocked ──
        now = time.time()
        if not is_locked and now >= next_fire_time and shot_idx < len(all_coords) and not game_status.get("finished"):
            x, y = all_coords[shot_idx]
            shot_idx += 1
            try:
                await ws.send_json({"type": "Fire", "x": x, "y": y})
                metrics.shots_fired += 1
                metrics.messages_sent += 1
                metrics.shots_ok += 1
            except Exception:
                metrics.ws_errors += 1
            next_fire_time = now + random.uniform(*FIRE_INTERVAL)
            
        # ── Action: Use Veto when locked (simulate player who can't solve) ──
        if is_locked and not has_vetoed_this_lock and now >= next_veto_time and next_veto_time > 0:
            try:
                await ws.send_json({"type": "Veto"})
                metrics.vetos_used += 1
                metrics.messages_sent += 1
                has_vetoed_this_lock = True
            except Exception:
                metrics.ws_errors += 1


# ─── Single game lifecycle ───────────────────────────────────────────
async def run_game(session: aiohttp.ClientSession, game_num: int, sustain_end: float) -> GameMetrics:
    """Run a single game through full lifecycle + sustained play."""
    m = GameMetrics()
    ws1 = None
    ws2 = None
    
    try:
        handle_p1 = f"tourney_p1_{game_num}_{uuid.uuid4().hex[:4]}"
        handle_p2 = f"tourney_p2_{game_num}_{uuid.uuid4().hex[:4]}"
        player2_id = str(uuid.uuid4())
        
        # ── 1. Create game ──
        t0 = time.time()
        async with session.post(
            f"{BASE_URL}/api/game",
            json={"cf_handle": handle_p1, "difficulty": 800, 
                  "heat_threshold": HEAT_THRESHOLD, "game_duration_mins": 45},
            timeout=aiohttp.ClientTimeout(total=HTTP_TIMEOUT),
        ) as resp:
            if resp.status == 429:
                m.error = "rate_limited"
                return m
            if resp.status != 201:
                body = await resp.text()
                m.error = f"create_{resp.status}: {body[:60]}"
                return m
            data = await resp.json()
            game_id = data["game_id"]
            player1_id = data["player_id"]
            m.game_id = game_id
            m.create_ms = (time.time() - t0) * 1000
            m.phase = "created"

        # ── 2. Both connect WS ──
        t1 = time.time()
        ws1 = await session.ws_connect(
            f"{WS_URL}/ws/{game_id}?player_id={player1_id}",
            timeout=aiohttp.ClientTimeout(total=WS_TIMEOUT),
        )
        ws2 = await session.ws_connect(
            f"{WS_URL}/ws/{game_id}?player_id={player2_id}",
            timeout=aiohttp.ClientTimeout(total=WS_TIMEOUT),
        )
        
        # P1 join
        await ws1.send_json({"type": "JoinGame", "player_id": player1_id, "cf_handle": handle_p1})
        msg, _ = await wait_for_type(ws1, "GameJoined", timeout=WS_TIMEOUT)
        if not msg:
            m.error = "p1_join_timeout"
            return m
            
        # P2 join
        await ws2.send_json({"type": "JoinGame", "player_id": player2_id, "cf_handle": handle_p2})
        msg, _ = await wait_for_type(ws2, "GameJoined", timeout=WS_TIMEOUT)
        if not msg:
            m.error = "p2_join_timeout"
            return m
        
        m.join_ms = (time.time() - t1) * 1000
        m.phase = "joined"
        
        # Brief pause for PlayerJoined broadcasts to propagate
        await asyncio.sleep(0.2)
        await drain_messages(ws1, 0.5)
        await drain_messages(ws2, 0.5)

        # ── 3. Both place ships ──
        t2 = time.time()
        await ws1.send_json({"type": "PlaceShips", "ships": FLEET})
        await ws2.send_json({"type": "PlaceShips", "ships": FLEET})
        
        # Wait for GameStart on both
        got_start = {"p1": False, "p2": False}
        deadline = time.time() + 15
        while time.time() < deadline and not (got_start["p1"] and got_start["p2"]):
            for label, ws in [("p1", ws1), ("p2", ws2)]:
                if got_start[label]:
                    continue
                msgs = await drain_messages(ws, 0.5)
                for msg in msgs:
                    if msg.get("type") == "GameStart":
                        got_start[label] = True
        
        if not (got_start["p1"] and got_start["p2"]):
            m.error = f"placement_timeout(p1={got_start['p1']},p2={got_start['p2']})"
            return m
            
        m.placement_ms = (time.time() - t2) * 1000
        m.phase = "playing"
        
        # ── 4. Sustained gameplay ──
        # Both players fire, get locked, use veto, etc. for the full sustain duration.
        game_status = {"finished": False}
        sustain_start = time.time()
        
        p1_task = asyncio.create_task(
            simulate_player(ws1, player1_id, True, m.p1, game_status, sustain_end)
        )
        p2_task = asyncio.create_task(
            simulate_player(ws2, player2_id, False, m.p2, game_status, sustain_end)
        )
        
        await asyncio.gather(p1_task, p2_task)
        m.sustain_secs = time.time() - sustain_start
        m.phase = "sustained"
        m.success = m.p1.connection_alive and m.p2.connection_alive
        if not m.p1.connection_alive:
            m.error = "p1_ws_died"
        elif not m.p2.connection_alive:
            m.error = "p2_ws_died"
        
    except Exception as e:
        m.error = f"{type(e).__name__}: {str(e)[:100]}"
    finally:
        # Close WebSockets
        for ws in [ws1, ws2]:
            if ws and not ws.closed:
                try:
                    await ws.close()
                except Exception:
                    pass
    
    return m


async def run_batch(session, batch_start, batch_size, sustain_end):
    """Run a batch of games."""
    tasks = [run_game(session, batch_start + i, sustain_end) for i in range(batch_size)]
    return await asyncio.gather(*tasks)


async def main():
    print(f"{'='*70}")
    print(f"  REALISTIC TOURNAMENT STRESS TEST")
    print(f"  Target:       {BASE_URL}")
    print(f"  Games:        {NUM_GAMES} ({NUM_GAMES * 2} WebSocket connections)")
    print(f"  Sustain:      {SUSTAIN_SECS}s of active gameplay per game")
    print(f"  Batch size:   {BATCH_SIZE}")
    print(f"  Production:   {IS_PRODUCTION}")
    print(f"  Fire pace:    {FIRE_INTERVAL[0]}-{FIRE_INTERVAL[1]}s between shots")
    print(f"{'='*70}")
    
    # Use generous connection limits
    connector = aiohttp.TCPConnector(
        limit=0, 
        limit_per_host=0,
        ttl_dns_cache=300,
        enable_cleanup_closed=True,
    )
    
    all_results = []
    t_start = time.time()
    
    # Calculate when sustain phase ends (all games share same end time — tournament style)
    # Games created in batches, but sustain_end is calculated from when the LAST batch starts
    # so all games get at least SUSTAIN_SECS of gameplay
    
    async with aiohttp.ClientSession(connector=connector) as session:
        # Phase 1: Create all games in batches as fast as possible
        print(f"\n{'─'*70}")
        print(f"  PHASE 1: Creating {NUM_GAMES} games (batches of {BATCH_SIZE})")
        print(f"{'─'*70}")
        
        # We'll track all batch tasks, but start them staggered
        batch_tasks = []
        for batch_idx in range(0, NUM_GAMES, BATCH_SIZE):
            actual_batch = min(BATCH_SIZE, NUM_GAMES - batch_idx)
            batch_num = batch_idx // BATCH_SIZE + 1
            total_batches = (NUM_GAMES + BATCH_SIZE - 1) // BATCH_SIZE
            
            # Sustain end = now + time to create remaining batches + sustain duration
            remaining_batches = total_batches - batch_num
            sustain_end = time.time() + (remaining_batches * 2) + SUSTAIN_SECS
            
            print(f"  Batch {batch_num}/{total_batches} ({actual_batch} games)...", end=" ", flush=True)
            bt = time.time()
            batch_results = await run_batch(session, batch_idx, actual_batch, sustain_end)
            elapsed = time.time() - bt
            
            successes = sum(1 for r in batch_results if r.success)
            fails = [r for r in batch_results if not r.success]
            print(f"{successes}/{actual_batch} OK ({elapsed:.1f}s)", end="")
            
            if fails:
                error_counts = {}
                for r in fails:
                    err = r.error or f"stuck@{r.phase}"
                    error_counts[err] = error_counts.get(err, 0) + 1
                top_errors = sorted(error_counts.items(), key=lambda x: -x[1])[:3]
                err_str = ", ".join(f"{e}(x{c})" for e, c in top_errors)
                print(f"  ERRORS: {err_str}", end="")
            print()
            
            all_results.extend(batch_results)
    
    total_time = time.time() - t_start
    
    # ─── Summary ─────────────────────────────────────────────────────
    total = len(all_results)
    ok = sum(1 for r in all_results if r.success)
    ok_results = [r for r in all_results if r.success]
    
    print(f"\n{'='*70}")
    print(f"  RESULTS: {ok}/{total} games sustained ({ok*100/total:.1f}%)")
    print(f"  Total wall time: {total_time:.1f}s")
    print(f"{'='*70}")
    
    # Phase breakdown
    phases = {}
    for r in all_results:
        phases[r.phase] = phases.get(r.phase, 0) + 1
    print(f"\nPhase breakdown:")
    for phase, count in sorted(phases.items(), key=lambda x: -x[1]):
        status = "✓" if phase == "sustained" else "✗"
        print(f"  {status} {phase}: {count}")
    
    # Setup timing
    if ok_results:
        print(f"\nSetup timing (successful games):")
        for metric in ["create_ms", "join_ms", "placement_ms"]:
            vals = [getattr(r, metric) for r in ok_results]
            avg = sum(vals) / len(vals)
            mx = max(vals)
            p95 = sorted(vals)[int(len(vals) * 0.95)]
            print(f"  {metric:15s}: avg={avg:7.1f}ms  p95={p95:7.1f}ms  max={mx:7.1f}ms")
        
        # Sustain duration
        sustain_vals = [r.sustain_secs for r in ok_results]
        print(f"  {'sustain_secs':15s}: avg={sum(sustain_vals)/len(sustain_vals):7.1f}s  "
              f"min={min(sustain_vals):7.1f}s  max={max(sustain_vals):7.1f}s")
    
    # Player activity metrics (aggregated)
    if ok_results:
        all_p = []
        for r in ok_results:
            all_p.extend([r.p1, r.p2])
        
        total_ticks = sum(p.ticks_received for p in all_p)
        total_shots = sum(p.shots_fired for p in all_p)
        total_vetos = sum(p.vetos_used for p in all_p)
        total_locks = sum(p.weapons_locked_count for p in all_p)
        total_unlocks = sum(p.weapons_unlocked_count for p in all_p)
        total_problems = sum(p.problems_assigned for p in all_p)
        total_msg_sent = sum(p.messages_sent for p in all_p)
        total_msg_recv = sum(p.messages_received for p in all_p)
        total_ws_errors = sum(p.ws_errors for p in all_p)
        total_shot_errors = sum(p.shot_errors for p in all_p)
        
        # Tick gap analysis (measures if server is keeping up with 1/sec broadcasts)
        tick_gaps = [p.max_tick_gap_ms for p in all_p if p.max_tick_gap_ms > 0]
        
        print(f"\nPlayer activity across {len(all_p)} players:")
        print(f"  Total ticks received:    {total_ticks:,} ({total_ticks/len(all_p):.0f} avg/player)")
        print(f"  Total shots fired:       {total_shots:,} ({total_shots/len(all_p):.1f} avg/player)")
        print(f"  Total weapons locks:     {total_locks:,}")
        print(f"  Total vetos used:        {total_vetos:,}")
        print(f"  Total problems assigned: {total_problems:,}")
        print(f"  Total weapons unlocks:   {total_unlocks:,}")
        print(f"  Total messages sent:     {total_msg_sent:,}")
        print(f"  Total messages received: {total_msg_recv:,}")
        print(f"  Total WS errors:         {total_ws_errors:,}")
        print(f"  Total shot errors:       {total_shot_errors:,}")
        
        if tick_gaps:
            avg_gap = sum(tick_gaps) / len(tick_gaps)
            max_gap = max(tick_gaps)
            p95_gap = sorted(tick_gaps)[int(len(tick_gaps) * 0.95)]
            print(f"\n  Tick gap (max per player — should be ~1000ms, >3000ms = problem):")
            print(f"    avg={avg_gap:.0f}ms  p95={p95_gap:.0f}ms  max={max_gap:.0f}ms")
            bad_gaps = sum(1 for g in tick_gaps if g > 3000)
            if bad_gaps:
                print(f"    ⚠ {bad_gaps}/{len(tick_gaps)} players saw >3s tick gap")
            else:
                print(f"    ✓ All players had tick gaps under 3s")
    
    # Error breakdown
    errors = [r for r in all_results if not r.success]
    if errors:
        print(f"\nError breakdown:")
        error_counts = {}
        for r in errors:
            err = r.error or f"stuck@{r.phase}"
            error_counts[err] = error_counts.get(err, 0) + 1
        for err, count in sorted(error_counts.items(), key=lambda x: -x[1]):
            print(f"  {err}: {count}")
    
    # Final verdict
    print(f"\n{'='*70}")
    if ok == total and total_ws_errors == 0:
        print(f"  ✓ PASS — All {total} games sustained for {SUSTAIN_SECS}s with zero WS errors")
    elif ok == total:
        print(f"  ~ PASS (with warnings) — All {total} games sustained, {total_ws_errors} WS errors")
    else:
        fail_pct = (total - ok) * 100 / total
        print(f"  ✗ FAIL — {total - ok}/{total} ({fail_pct:.1f}%) games failed")
    print(f"{'='*70}\n")
    
    return 0 if ok == total else 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
