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
    - If you reach **7 Heat**, your weapons **OVERHEAT**.
    - You CANNOT fire while overheated.
- **Cooling Down:**
    - **Active:** Solve a Codeforces problem to instantly flush all heat.
    - **Veto:** Use a Veto to accept a penalty time-out, after which your weapon resets.

## 4. The Veto Mechanic
- You have **3 Vetoes**.
- **Purpose:** If you cannot solve the assigned problem, use a Veto to skip it.
- **Effect:** Your weapons remain **LOCKED** for a penalty duration (7, 10, or 15 minutes).
- **Game State:** The game continues! Your opponent is free to fire at you while you wait out your penalty.
- **After Penalty:** Your weapons unlock and heat resets to 0.

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
- If you are heat-locked when Sudden Death starts, you must solve your CP problem (or wait out your veto) to unlock before you can fire.
