# Battle CP - Rules of Engagement

## 1. The Objective
Destroy the enemy fleet before time runs out. If time expires, the player with the most ships remaining wins.

## 2. Phase 1: Deployment
- Place your 5 ships (Carrier, Battleship, Cruiser, Submarine, Destroyer) on the 10x10 grid.
- Ships cannot overlap. Touching other ships (adjacent placement) is allowed.
- Both players must confirm placement to begin combat.

## 3. Phase 2: Combat & Heat
- **Firing:** Click a cell on the enemy grid to fire.
- **Heat System:** Every shot generates **Heat**.
    - If you reach the **Heat Threshold** (configurable when creating a game, default: **9**), your weapons **OVERHEAT**.
    - You CANNOT fire while overheated.
- **Cooling Down:**
    - **Active:** Solve a Codeforces problem to instantly flush all heat.
    - **Veto:** Use a Veto to skip the current problem at the cost of a timed penalty (see below).

## 4. The Veto Mechanic
- You have **3 Vetoes** (configurable).
- **Purpose:** If you cannot solve the assigned problem, use a Veto to skip it.
- **Effect:** A countdown timer begins. Your weapons remain **LOCKED** for the full penalty duration.
- **When the timer expires:** A **new problem is automatically assigned**. You **MUST solve it** to unlock your weapons — vetoing just starts another penalty.
- **Penalty Durations** (escalating per veto used, configurable at game creation):
    - Low: **1 min → 2 min → 3 min**
    - Medium: **3 min → 5 min → 7 min** (default)
    - High: **5 min → 7 min → 10 min**
- **Game State:** The game continues! Your opponent is free to fire at you while you wait.

## 5. Tie-Breakers
If the game timer ends:
1. **Primary:** Most ships remaining wins.
2. **Secondary:** Most cells hit on enemy grid wins.
3. **Final:** Sudden Death — if both metrics are exactly equal.

## 6. Sudden Death
- Entered only when **ships remaining AND cells hit** are exactly equal at time-up.
- **No state is reset** when Sudden Death begins. Heat locks remain. Veto timers keep running. Unlock requirements are unchanged.
- **To win:** Be the first player to land a **Hit** on the enemy grid.
- A Miss does NOT win. Only a confirmed Hit ends Sudden Death.
- If you are heat-locked when Sudden Death starts, you must solve your CP problem (or wait out your veto penalty and then solve the new problem) to unlock before you can fire.

## 7. Scoring
- **Winner score:** `(time_limit_seconds - time_taken_seconds) + 1` — faster wins earn more.
- **Loser score:** `1` — always a consolation point.
- Used for tie-breaking in tournaments and match logs.

## 8. Default Lobby Settings
When creating a game, the recommended defaults are:
- **Mode:** Band — Super Easy
- **Time Limit:** 25 minutes
- **Overheat After:** 9 shots
- **Max Vetoes:** 3
- **Veto Penalty Tier:** Low (1/2/3 min)
