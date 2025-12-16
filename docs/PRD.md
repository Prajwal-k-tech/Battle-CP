# Battle CP - Product Requirements Document

## Core Game Rules

### Victory
- **45 minute game**
- **First to sink all opponent ships wins immediately**

### Heat System
- Every shot (hit or miss) = +1 heat
- At **7 heat** → weapons LOCK
- No ammo/burst system - just heat

### When Locked (Two Options)

**Option 1: SOLVE**
- Complete the assigned 800-rated problem
- Weapons unlock, heat resets to 0

**Option 2: VETO**
- Skip the problem, start penalty timer
- **CAN'T shoot during penalty** - must wait
- Timer durations: 7 min → 10 min → 15 min (progressive)
- After timer ends → weapons unlock, heat resets to 0
- **CRITICAL: If you veto, solving a problem does NOT unlock you. You MUST wait the timer.**
- Max 3 vetoes per game

### Default Lobby Settings
| Setting | Default |
|---------|---------|
| Difficulty | 800 |
| Game Duration | 45 min |
| Heat Threshold | 7 |
| Max Vetoes | 3 |
| Veto Penalties | 7, 10, 15 min |

## Technical Stack
- **Frontend**: Next.js, React, TailwindCSS, Framer Motion
- **Backend**: Rust, Axum, Tokio
- **Communication**: WebSockets
- **External API**: Codeforces
