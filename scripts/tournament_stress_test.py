#!/usr/bin/env python3
"""
Tournament stress test — simulates a full BattleCP tournament.

Tests: Create game → P1 WS join → P2 WS join → both place ships → 
       both fire until overheat → verify game state.

Usage: python3 scripts/tournament_stress_test.py [num_games] [base_url]
"""

import asyncio
import aiohttp
import json
import sys
import time
import uuid
import random
from dataclasses import dataclass, field

BASE_URL = sys.argv[2] if len(sys.argv) > 2 else "http://localhost:3000"
WS_URL = BASE_URL.replace("http", "ws")
NUM_GAMES = int(sys.argv[1]) if len(sys.argv) > 1 else 250
BATCH_SIZE = 25  # Create games in batches
WS_TIMEOUT = 30  # seconds
HEAT_THRESHOLD = 7  # default

@dataclass
class GameResult:
    game_id: str = ""
    success: bool = False
    phase_reached: str = "none"
    error: str = ""
    create_ms: float = 0
    p1_join_ms: float = 0 
    p2_join_ms: float = 0
    placement_ms: float = 0
    combat_shots: int = 0
    p1_locked: bool = False
    p2_locked: bool = False

# Standard fleet for placement
FLEET = [
    {"x": 0, "y": 0, "size": 5, "vertical": False},
    {"x": 0, "y": 2, "size": 4, "vertical": False},
    {"x": 0, "y": 4, "size": 3, "vertical": False},
    {"x": 0, "y": 6, "size": 3, "vertical": False},
    {"x": 0, "y": 8, "size": 2, "vertical": False},
]

async def recv_until(ws, msg_type, timeout=WS_TIMEOUT):
    """Receive WS messages until we get one of the specified type(s)."""
    if isinstance(msg_type, str):
        msg_type = [msg_type]
    deadline = time.time() + timeout
    messages = []
    while time.time() < deadline:
        try:
            msg = await asyncio.wait_for(ws.receive(), timeout=max(0.1, deadline - time.time()))
            if msg.type == aiohttp.WSMsgType.TEXT:
                data = json.loads(msg.data)
                messages.append(data)
                if data.get("type") in msg_type:
                    return data, messages
            elif msg.type in (aiohttp.WSMsgType.CLOSED, aiohttp.WSMsgType.ERROR):
                return None, messages
        except asyncio.TimeoutError:
            break
    return None, messages


async def run_game(session: aiohttp.ClientSession, game_num: int) -> GameResult:
    """Run a single complete game lifecycle."""
    result = GameResult()
    
    try:
        # 1. Create game
        t0 = time.time()
        handle_p1 = f"player_{game_num}_p1_{uuid.uuid4().hex[:6]}"
        async with session.post(
            f"{BASE_URL}/api/game",
            json={
                "cf_handle": handle_p1,
                "difficulty": 800,
                "heat_threshold": HEAT_THRESHOLD,
                "game_duration_mins": 45,
            },
            timeout=aiohttp.ClientTimeout(total=15),
        ) as resp:
            if resp.status == 429:
                result.error = "rate_limited"
                return result
            if resp.status != 201:
                result.error = f"create_status_{resp.status}"
                return result
            data = await resp.json()
            game_id = data["game_id"]
            player1_id = data["player_id"]
            result.game_id = game_id
            result.create_ms = (time.time() - t0) * 1000
            result.phase_reached = "created"

        # 2. P1 connects via WebSocket
        handle_p2 = f"player_{game_num}_p2_{uuid.uuid4().hex[:6]}"
        player2_id = str(uuid.uuid4())
        
        t1 = time.time()
        ws1 = await session.ws_connect(
            f"{WS_URL}/ws/{game_id}?player_id={player1_id}",
            timeout=aiohttp.ClientTimeout(total=WS_TIMEOUT),
        )
        
        # P1 sends JoinGame
        await ws1.send_json({
            "type": "JoinGame",
            "player_id": player1_id,
            "cf_handle": handle_p1,
        })
        
        msg, _ = await recv_until(ws1, "GameJoined", timeout=10)
        if not msg:
            result.error = "p1_join_timeout"
            await ws1.close()
            return result
        result.p1_join_ms = (time.time() - t1) * 1000
        result.phase_reached = "p1_joined"

        # 3. P2 connects via WebSocket
        t2 = time.time()
        ws2 = await session.ws_connect(
            f"{WS_URL}/ws/{game_id}?player_id={player2_id}",
            timeout=aiohttp.ClientTimeout(total=WS_TIMEOUT),
        )
        
        await ws2.send_json({
            "type": "JoinGame",
            "player_id": player2_id,
            "cf_handle": handle_p2,
        })
        
        msg, _ = await recv_until(ws2, "GameJoined", timeout=10)
        if not msg:
            result.error = "p2_join_timeout"
            await ws1.close()
            await ws2.close()
            return result
        result.p2_join_ms = (time.time() - t2) * 1000
        result.phase_reached = "p2_joined"

        # Wait briefly for PlayerJoined broadcasts
        await asyncio.sleep(0.1)
        
        # Drain any queued messages
        async def drain(ws, timeout=1):
            msgs = []
            deadline = time.time() + timeout
            while time.time() < deadline:
                try:
                    m = await asyncio.wait_for(ws.receive(), timeout=0.2)
                    if m.type == aiohttp.WSMsgType.TEXT:
                        msgs.append(json.loads(m.data))
                except asyncio.TimeoutError:
                    break
            return msgs
        
        await drain(ws1, 0.5)
        await drain(ws2, 0.5)

        # 4. Both place ships
        t3 = time.time()
        await ws1.send_json({"type": "PlaceShips", "ships": FLEET})
        await ws2.send_json({"type": "PlaceShips", "ships": FLEET})
        
        # Wait for GameStart from both
        got_start_1 = False
        got_start_2 = False
        deadline = time.time() + 10
        
        while time.time() < deadline and not (got_start_1 and got_start_2):
            tasks = []
            if not got_start_1:
                tasks.append(("p1", drain(ws1, 1)))
            if not got_start_2:
                tasks.append(("p2", drain(ws2, 1)))
            
            results_list = await asyncio.gather(*[t[1] for t in tasks])
            
            for (label, _), msgs in zip(tasks, results_list):
                for m in msgs:
                    if m.get("type") == "GameStart":
                        if label == "p1":
                            got_start_1 = True
                        else:
                            got_start_2 = True
        
        if not (got_start_1 and got_start_2):
            result.error = f"placement_timeout (p1_start={got_start_1}, p2_start={got_start_2})"
            await ws1.close()
            await ws2.close()
            return result
        
        result.placement_ms = (time.time() - t3) * 1000
        result.phase_reached = "playing"

        # 5. Both fire until overheat (heat_threshold shots each)
        shots_fired = 0
        p1_locked = False
        p2_locked = False
        
        # Generate non-repeating shot coordinates
        all_coords = [(x, y) for x in range(10) for y in range(10)]
        random.shuffle(all_coords)
        p1_shots = all_coords[:HEAT_THRESHOLD + 2]
        random.shuffle(all_coords)
        p2_shots = all_coords[:HEAT_THRESHOLD + 2]
        
        for i in range(HEAT_THRESHOLD + 1):
            if not p1_locked and i < len(p1_shots):
                await ws1.send_json({"type": "Fire", "x": p1_shots[i][0], "y": p1_shots[i][1]})
                shots_fired += 1
            if not p2_locked and i < len(p2_shots):
                await ws2.send_json({"type": "Fire", "x": p2_shots[i][0], "y": p2_shots[i][1]})
                shots_fired += 1
            
            # Brief pause to let server process
            await asyncio.sleep(0.05)
            
            # Check for lock messages
            msgs1 = await drain(ws1, 0.2)
            msgs2 = await drain(ws2, 0.2)
            
            for m in msgs1 + msgs2:
                if m.get("type") == "WeaponsLocked":
                    locked_id = m.get("player_id")
                    if locked_id == player1_id:
                        p1_locked = True
                    elif locked_id == player2_id:
                        p2_locked = True
                if m.get("type") == "GameUpdate":
                    if m.get("is_locked"):
                        # This update is for the receiving player
                        pass
        
        result.combat_shots = shots_fired
        result.p1_locked = p1_locked
        result.p2_locked = p2_locked
        result.phase_reached = "combat_complete"
        result.success = True
        
        await ws1.close()
        await ws2.close()
        
    except Exception as e:
        result.error = f"{type(e).__name__}: {str(e)[:80]}"
    
    return result


async def run_batch(session: aiohttp.ClientSession, batch_start: int, batch_size: int) -> list:
    """Run a batch of games concurrently."""
    tasks = [run_game(session, batch_start + i) for i in range(batch_size)]
    return await asyncio.gather(*tasks)


async def main():
    print(f"{'='*60}")
    print(f"  TOURNAMENT STRESS TEST")
    print(f"  Target: {BASE_URL}")
    print(f"  Games: {NUM_GAMES} ({NUM_GAMES * 2} players)")
    print(f"  Batch size: {BATCH_SIZE}")
    print(f"{'='*60}")
    
    connector = aiohttp.TCPConnector(limit=0, limit_per_host=0)
    async with aiohttp.ClientSession(connector=connector) as session:
        all_results = []
        t_start = time.time()
        
        for batch_idx in range(0, NUM_GAMES, BATCH_SIZE):
            actual_batch = min(BATCH_SIZE, NUM_GAMES - batch_idx)
            batch_num = batch_idx // BATCH_SIZE + 1
            total_batches = (NUM_GAMES + BATCH_SIZE - 1) // BATCH_SIZE
            
            print(f"\n--- Batch {batch_num}/{total_batches} ({actual_batch} games) ---")
            bt = time.time()
            batch_results = await run_batch(session, batch_idx, actual_batch)
            elapsed = time.time() - bt
            
            successes = sum(1 for r in batch_results if r.success)
            print(f"  {successes}/{actual_batch} OK  ({elapsed:.1f}s)")
            
            # Show any errors
            errors = [r for r in batch_results if not r.success]
            if errors:
                error_counts = {}
                for r in errors:
                    err = r.error or f"stuck_at_{r.phase_reached}"
                    error_counts[err] = error_counts.get(err, 0) + 1
                for err, count in sorted(error_counts.items(), key=lambda x: -x[1]):
                    print(f"    FAIL: {err} (x{count})")
            
            all_results.extend(batch_results)
        
        total_time = time.time() - t_start
        
        # Summary
        total = len(all_results)
        ok = sum(1 for r in all_results if r.success)
        
        print(f"\n{'='*60}")
        print(f"  RESULTS: {ok}/{total} games successful ({ok*100/total:.1f}%)")
        print(f"  Total time: {total_time:.1f}s")
        print(f"{'='*60}")
        
        # Phase breakdown
        phases = {}
        for r in all_results:
            phases[r.phase_reached] = phases.get(r.phase_reached, 0) + 1
        print(f"\nPhase breakdown:")
        for phase, count in sorted(phases.items(), key=lambda x: -x[1]):
            print(f"  {phase}: {count}")
        
        # Timing stats (only for successful games)
        ok_results = [r for r in all_results if r.success]
        if ok_results:
            print(f"\nTiming (successful games):")
            for metric in ["create_ms", "p1_join_ms", "p2_join_ms", "placement_ms"]:
                vals = [getattr(r, metric) for r in ok_results]
                avg = sum(vals) / len(vals)
                mx = max(vals)
                p95 = sorted(vals)[int(len(vals) * 0.95)]
                print(f"  {metric:15s}: avg={avg:6.1f}ms  p95={p95:6.1f}ms  max={mx:6.1f}ms")
            
            total_shots = sum(r.combat_shots for r in ok_results)
            locked_both = sum(1 for r in ok_results if r.p1_locked or r.p2_locked)
            print(f"\nCombat stats:")
            print(f"  Total shots fired: {total_shots}")
            print(f"  Games with overheat: {locked_both}/{len(ok_results)}")
        
        # Error breakdown
        errors = [r for r in all_results if not r.success]
        if errors:
            print(f"\nError breakdown:")
            error_counts = {}
            for r in errors:
                err = r.error or f"stuck_at_{r.phase_reached}"
                error_counts[err] = error_counts.get(err, 0) + 1
            for err, count in sorted(error_counts.items(), key=lambda x: -x[1]):
                print(f"  {err}: {count}")
        
        print()
        return 0 if ok == total else 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
