//! Discord webhook integration for post-game match logging.
//! Set the `DISCORD_WEBHOOK_URL` environment variable to enable.

use crate::state::{DifficultyMode, Game};
use uuid::Uuid;

/// Snapshot of all data needed for the Discord match report.
/// Cloneable so it can be moved into a `tokio::spawn` task.
#[derive(Clone)]
pub struct MatchReport {
    pub game_id: String,
    pub reason: String,
    pub time_taken_secs: u64,
    pub winner_id: Option<Uuid>,
    pub p1_id: Uuid,
    pub p1_handle: String,
    pub p1_score: f64,
    pub p1_ships_sunk: u32,
    pub p1_ships_lost: u32,
    pub p1_problems_solved: u32,
    pub p1_cells_hit: u32,
    pub p1_cells_missed: u32,
    pub p1_vetoes_used: u32,
    pub p2_handle: String,
    pub p2_score: f64,
    pub p2_ships_sunk: u32,
    pub p2_ships_lost: u32,
    pub p2_problems_solved: u32,
    pub p2_cells_hit: u32,
    pub p2_cells_missed: u32,
    pub p2_vetoes_used: u32,
    pub difficulty: u32,
    pub difficulty_mode: DifficultyMode,
    pub heat_threshold: u32,
    pub veto_penalties: [u64; 3],
    pub max_vetoes: u32,
    pub game_duration_secs: u64,
}

impl MatchReport {
    /// Extract all match data from a finished `Game` instance.
    pub fn from_game(game: &Game, winner_id: Option<Uuid>, reason: String) -> Self {
        let game_duration = game.config.game_duration_secs;
        let time_taken_secs = game
            .game_started_at
            .map(|s| s.elapsed().as_secs().min(game_duration))
            .unwrap_or(game_duration);

        let (winner_score, loser_score) = if winner_id.is_some() {
            let w = (game_duration - time_taken_secs) as f64 + 1.0;
            (w, 1.0_f64)
        } else {
            (1.0_f64, 1.0_f64)
        };

        let (p1_score, p2_score) = match winner_id {
            Some(id) if id == game.player1.id => (winner_score, loser_score),
            Some(_) => (loser_score, winner_score),
            None => (winner_score, loser_score),
        };

        let p2 = game.player2.as_ref();

        Self {
            game_id: game.id.to_string()[..8].to_string(),
            reason,
            time_taken_secs,
            winner_id,
            p1_id: game.player1.id,
            p1_handle: game.player1.cf_handle.clone(),
            p1_score,
            p1_ships_sunk: game.player1.stats.ships_sunk,
            p1_ships_lost: game.player1.ships.iter().filter(|s| s.sunk).count() as u32,
            p1_problems_solved: game.player1.stats.problems_solved,
            p1_cells_hit: game.player1.stats.cells_hit,
            p1_cells_missed: game.player1.stats.cells_missed,
            p1_vetoes_used: game.player1.vetoes_used,
            p2_handle: p2
                .map(|p| p.cf_handle.clone())
                .unwrap_or_else(|| "—".to_string()),
            p2_score,
            p2_ships_sunk: p2.map(|p| p.stats.ships_sunk).unwrap_or(0),
            p2_ships_lost: p2
                .map(|p| p.ships.iter().filter(|s| s.sunk).count() as u32)
                .unwrap_or(0),
            p2_problems_solved: p2.map(|p| p.stats.problems_solved).unwrap_or(0),
            p2_cells_hit: p2.map(|p| p.stats.cells_hit).unwrap_or(0),
            p2_cells_missed: p2.map(|p| p.stats.cells_missed).unwrap_or(0),
            p2_vetoes_used: p2.map(|p| p.vetoes_used).unwrap_or(0),
            difficulty: game.config.difficulty,
            difficulty_mode: game.config.difficulty_mode.clone(),
            heat_threshold: game.config.heat_threshold,
            veto_penalties: game.config.veto_penalties,
            max_vetoes: game.config.max_vetoes,
            game_duration_secs: game.config.game_duration_secs,
        }
    }
}

// ── Helper formatters ──────────────────────────────────────────────────────────

fn fmt_duration(secs: u64) -> String {
    if secs == 0 {
        return "0s".to_string();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {}m {:02}s", h, m, s)
    } else if m > 0 {
        format!("{}m {:02}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn accuracy(hit: u32, miss: u32) -> String {
    let total = hit + miss;
    if total == 0 {
        return "—".to_string();
    }
    format!("{:.1}%", hit as f64 / total as f64 * 100.0)
}

fn penalty_time(vetoes_used: u32, penalties: &[u64; 3]) -> u64 {
    (0..vetoes_used.min(3) as usize).map(|i| penalties[i]).sum()
}

fn band_name(band: u32) -> &'static str {
    match band {
        0 => "Super Easy",
        1 => "Easy",
        2 => "Medium",
        3 => "Hard",
        4 => "Very Hard",
        _ => "Custom",
    }
}

fn stats_block(
    ships_sunk: u32,
    ships_lost: u32,
    problems: u32,
    hit: u32,
    miss: u32,
    vetoes: u32,
    max_vetoes: u32,
    pen_secs: u64,
) -> String {
    format!(
        concat!(
            "```\n",
            "Ships Sunk  : {:>2} / 5\n",
            "Ships Lost  : {:>2} / 5\n",
            "CP Solved   : {:>2}\n",
            "Shots Fired : {:>2}  ({} hit  {} miss)\n",
            "Accuracy    : {}\n",
            "Vetoes      : {:>2} / {}\n",
            "Penalty     : {}\n",
            "```",
        ),
        ships_sunk,
        ships_lost,
        problems,
        hit + miss,
        hit,
        miss,
        accuracy(hit, miss),
        vetoes,
        max_vetoes,
        fmt_duration(pen_secs),
    )
}

// ── Embed builder ──────────────────────────────────────────────────────────────

fn build_embed(rep: &MatchReport) -> serde_json::Value {
    let is_draw = rep.winner_id.is_none();
    let p1_wins = rep.winner_id.map(|id| id == rep.p1_id).unwrap_or(false);

    // Left side = winner (or P1 for draw), Right side = loser (or P2 for draw)
    let (lft_handle, rgt_handle) = if !is_draw && !p1_wins {
        (rep.p2_handle.as_str(), rep.p1_handle.as_str())
    } else {
        (rep.p1_handle.as_str(), rep.p2_handle.as_str())
    };

    let (lft_score, rgt_score) = if !is_draw && !p1_wins {
        (rep.p2_score, rep.p1_score)
    } else {
        (rep.p1_score, rep.p2_score)
    };

    let (
        lft_sunk, lft_lost, lft_prb, lft_hit, lft_miss, lft_vet,
        rgt_sunk, rgt_lost, rgt_prb, rgt_hit, rgt_miss, rgt_vet,
    ) = if !is_draw && !p1_wins {
        (
            rep.p2_ships_sunk, rep.p2_ships_lost, rep.p2_problems_solved,
            rep.p2_cells_hit,  rep.p2_cells_missed, rep.p2_vetoes_used,
            rep.p1_ships_sunk, rep.p1_ships_lost, rep.p1_problems_solved,
            rep.p1_cells_hit,  rep.p1_cells_missed, rep.p1_vetoes_used,
        )
    } else {
        (
            rep.p1_ships_sunk, rep.p1_ships_lost, rep.p1_problems_solved,
            rep.p1_cells_hit,  rep.p1_cells_missed, rep.p1_vetoes_used,
            rep.p2_ships_sunk, rep.p2_ships_lost, rep.p2_problems_solved,
            rep.p2_cells_hit,  rep.p2_cells_missed, rep.p2_vetoes_used,
        )
    };

    let (lft_label, rgt_label) = if is_draw {
        ("⚔️  PLAYER 1", "⚔️  PLAYER 2")
    } else {
        ("🏆  WINNER", "💀  DEFEATED")
    };

    let outcome_desc = if is_draw {
        format!("🤝  **DRAW** · `{}`", rep.reason)
    } else if p1_wins {
        format!(
            "🏆  **{}** defeated **{}**",
            rep.p1_handle, rep.p2_handle
        )
    } else {
        format!(
            "🏆  **{}** defeated **{}**",
            rep.p2_handle, rep.p1_handle
        )
    };

    let difficulty_str = match rep.difficulty_mode {
        DifficultyMode::Band => format!(
            "Band — {} (tier {})",
            band_name(rep.difficulty),
            rep.difficulty
        ),
        DifficultyMode::Cf => format!("CF Rating — {}", rep.difficulty),
    };

    let config_str = format!(
        "**Mode:** {}   **|**   **Time Limit:** {}   **|**   **Overheat:** {} shots   **|**   **Max Vetoes:** {}   **|**   **Penalties:** {}/{}/{} min",
        difficulty_str,
        fmt_duration(rep.game_duration_secs),
        rep.heat_threshold,
        rep.max_vetoes,
        rep.veto_penalties[0] / 60,
        rep.veto_penalties[1] / 60,
        rep.veto_penalties[2] / 60,
    );

    // Green if decisive winner, gray if draw/timeout-no-winner
    let color: u32 = if rep.winner_id.is_some() { 5_763_719 } else { 10_066_613 };

    let lft_pen = penalty_time(lft_vet, &rep.veto_penalties);
    let rgt_pen = penalty_time(rgt_vet, &rep.veto_penalties);

    serde_json::json!({
        "title": "⚔️  BATTLE CP — MATCH REPORT",
        "description": format!(
            "> **Game:** `{}`   **|**   `{}`   **|**   ⏱ **{}**\n> {}",
            rep.game_id,
            rep.reason,
            fmt_duration(rep.time_taken_secs),
            outcome_desc
        ),
        "color": color,
        "fields": [
            {
                "name": format!("{} — {}", lft_label, lft_handle),
                "value": format!("**Time Penalty:** `+{:.2} pts`", lft_score),
                "inline": true
            },
            {
                "name": format!("{} — {}", rgt_label, rgt_handle),
                "value": format!("**Time Penalty:** `+{:.2} pts`", rgt_score),
                "inline": true
            },
            { "name": "\u{200b}", "value": "\u{200b}", "inline": false },
            {
                "name": format!("📊  {} Stats", lft_handle),
                "value": stats_block(
                    lft_sunk, lft_lost, lft_prb,
                    lft_hit, lft_miss, lft_vet,
                    rep.max_vetoes, lft_pen
                ),
                "inline": true
            },
            {
                "name": format!("📊  {} Stats", rgt_handle),
                "value": stats_block(
                    rgt_sunk, rgt_lost, rgt_prb,
                    rgt_hit, rgt_miss, rgt_vet,
                    rep.max_vetoes, rgt_pen
                ),
                "inline": true
            },
            { "name": "\u{200b}", "value": "\u{200b}", "inline": false },
            {
                "name": "⚙️  Game Configuration",
                "value": config_str,
                "inline": false
            }
        ],
        "footer": {
            "text": "Battle CP  ·  Match Log  ·  oGhostyyy"
        }
    })
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Shared HTTP client — avoids creating a new client per request.
static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    })
}

/// Bounded channel for serializing webhook POSTs (prevents burst flooding Discord).
static QUEUE_TX: std::sync::OnceLock<tokio::sync::mpsc::Sender<MatchReport>> =
    std::sync::OnceLock::new();

/// Initialize the webhook worker. Call once at startup.
pub fn init_worker() {
    let (tx, rx) = tokio::sync::mpsc::channel::<MatchReport>(64);
    QUEUE_TX.set(tx).ok();
    tokio::spawn(webhook_worker(rx));
}

/// Background worker: drains the queue and POSTs one report at a time.
async fn webhook_worker(mut rx: tokio::sync::mpsc::Receiver<MatchReport>) {
    while let Some(report) = rx.recv().await {
        post_report_with_retry(&report).await;
    }
}

/// Fire-and-forget: extract match data and enqueue for the webhook worker.
/// Safe to call while holding the games write-lock — enqueue is non-blocking.
pub fn log_game(game: &Game, winner_id: Option<Uuid>, reason: &str) {
    let report = MatchReport::from_game(game, winner_id, reason.to_string());
    if let Some(tx) = QUEUE_TX.get() {
        // try_send: don't block the game loop; drop report if queue full (unlikely at 64)
        if tx.try_send(report).is_err() {
            tracing::warn!("Discord webhook queue full — dropping match report");
        }
    }
}

/// POST with up to 3 retries, respecting Discord 429 Retry-After header.
async fn post_report_with_retry(report: &MatchReport) {
    let url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return,
    };

    let embed = build_embed(report);
    let body = serde_json::json!({
        "username": "Battle CP",
        "embeds": [embed]
    });

    let client = get_client();
    for attempt in 0..3u32 {
        match client.post(&url).json(&body).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    return; // Done
                }
                if resp.status().as_u16() == 429 {
                    // Rate limited — respect Retry-After header
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(2.0);
                    let wait_ms = (retry_after * 1000.0) as u64 + 100; // small padding
                    tracing::warn!(
                        "Discord 429 rate-limited (attempt {}), retrying in {}ms",
                        attempt + 1,
                        wait_ms
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                    continue;
                }
                // Other non-success status
                tracing::warn!(
                    "Discord webhook returned {} (attempt {})",
                    resp.status(),
                    attempt + 1
                );
            }
            Err(e) => {
                tracing::warn!("Discord webhook POST failed (attempt {}): {}", attempt + 1, e);
            }
        }
        // Exponential backoff for non-429 failures
        let backoff = std::time::Duration::from_millis(500 * 2u64.pow(attempt));
        tokio::time::sleep(backoff).await;
    }
    tracing::error!("Discord webhook failed after 3 retries — match report lost");
}
