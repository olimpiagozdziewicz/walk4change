//! Demo data seeder: idempotent, safe to run repeatedly.
//!
//! `run` creates two demo users (Ana & Bek), makes them accepted friends,
//! inserts a Baltic-coast nature zone (Gdańsk Brzeźno beach area), and
//! a few rewards_catalog rows.  All operations are idempotent via
//! existence checks and ON CONFLICT DO NOTHING.

use sqlx::PgPool;
use uuid::Uuid;

use crate::{auth::password, config::AppConfig, error::AppError};

/// Result returned by [`run`] — useful for logging and testing.
#[derive(Debug)]
pub struct SeedResult {
    /// UUIDs of the two demo users (Ana, Bek).
    pub user_ids: Vec<Uuid>,
    /// The plaintext password used for demo accounts.
    /// Either taken from the `SEED_PASSWORD` env var, or randomly generated.
    pub password: String,
    /// Total number of active nature zones in the database.
    pub zone_count: u32,
    /// Total number of rewards in the rewards_catalog.
    pub reward_count: u32,
}

// ── Fixed UUIDs for idempotent inserts ──────────────────────────────────────

fn ana_id() -> Uuid { Uuid::parse_str("a1a1a1a1-0000-0000-0000-000000000001").unwrap() }
fn bek_id() -> Uuid { Uuid::parse_str("b2b2b2b2-0000-0000-0000-000000000002").unwrap() }
fn friendship_id() -> Uuid { Uuid::parse_str("f0f0f0f0-0000-0000-0000-000000000001").unwrap() }
fn zone_brzezno_id() -> Uuid { Uuid::parse_str("c3c3c3c3-0000-0000-0000-000000000001").unwrap() }
fn reward_cafe_id() -> Uuid { Uuid::parse_str("d4d4d4d4-0000-0000-0000-000000000001").unwrap() }
fn reward_tree_id() -> Uuid { Uuid::parse_str("d4d4d4d4-0000-0000-0000-000000000002").unwrap() }
fn reward_cinema_id() -> Uuid { Uuid::parse_str("d4d4d4d4-0000-0000-0000-000000000003").unwrap() }

// ── Nature zone polygon (Gdańsk Brzeźno beach, Baltic coast) ────────────────
// Small ~500m × 300m rectangle, lon-first, closed ring.
const BRZEZNO_POLYGON: &str = "SRID=4326;POLYGON((\
    18.610 54.393,\
    18.635 54.393,\
    18.635 54.403,\
    18.610 54.403,\
    18.610 54.393\
))";

/// Seed demo data into the database.  Safe to run multiple times.
///
/// On each call:
/// - Looks up Ana & Bek by email; inserts (with totals) if absent.
/// - Inserts an accepted friendship if none exists between them.
/// - Inserts the Brzeźno nature zone if no zone with that name exists.
/// - Inserts each reward if no reward with that title exists.
///
/// Returns [`SeedResult`] with the UUIDs, password used, and total counts.
pub async fn run(pool: &PgPool, cfg: &AppConfig) -> Result<SeedResult, AppError> {
    // ── Password ─────────────────────────────────────────────────────────────
    let plain_password = std::env::var("SEED_PASSWORD").unwrap_or_else(|_| {
        use rand::distributions::Alphanumeric;
        use rand::Rng;
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(20)
            .map(char::from)
            .collect()
    });

    // ── Users ─────────────────────────────────────────────────────────────────
    let ana_id = upsert_user(pool, cfg, ana_id(), "ana@demo.walk4change", "Ana", &plain_password).await?;
    let bek_id = upsert_user(pool, cfg, bek_id(), "bek@demo.walk4change", "Bek", &plain_password).await?;

    // ── Friendship ────────────────────────────────────────────────────────────
    upsert_friendship(pool, friendship_id(), ana_id, bek_id).await?;

    // ── Nature zone ───────────────────────────────────────────────────────────
    upsert_nature_zone(pool, zone_brzezno_id(), "Gdańsk Brzeźno Beach").await?;

    // ── Rewards ───────────────────────────────────────────────────────────────
    upsert_reward(pool, reward_cafe_id(), "Local Cafe Discount", "10% off at Café Bałtyk", "discount", "Café Bałtyk", 150).await?;
    upsert_reward(pool, reward_tree_id(), "Plant a Tree", "We plant a tree in your name via EcoWave", "eco", "EcoWave", 500).await?;
    upsert_reward(pool, reward_cinema_id(), "Cinema Ticket", "Free ticket at Multikino Gdańsk", "sponsor", "Multikino", 1000).await?;

    // ── Counts ────────────────────────────────────────────────────────────────
    let zone_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM nature_zones WHERE active = true")
            .fetch_one(pool)
            .await
            .map_err(AppError::internal)?;

    let reward_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rewards_catalog")
        .fetch_one(pool)
        .await
        .map_err(AppError::internal)?;

    Ok(SeedResult {
        user_ids: vec![ana_id, bek_id],
        password: plain_password,
        zone_count: zone_count as u32,
        reward_count: reward_count as u32,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Insert user + user_totals if not present; return the resolved UUID.
///
/// Uses fixed `id` on insert.  If the email already exists the INSERT is
/// skipped (ON CONFLICT DO NOTHING) and we fall through to the SELECT.
async fn upsert_user(
    pool: &PgPool,
    cfg: &AppConfig,
    id: Uuid,
    email: &str,
    display_name: &str,
    plain_password: &str,
) -> Result<Uuid, AppError> {
    // Try find first to avoid hashing if already present.
    if let Some(row) = crate::repo::user::find_by_email(pool, email).await? {
        return Ok(row.id);
    }

    // Hash and insert.
    let hash = password::hash(cfg, plain_password)?;
    // Seed/demo accounts count as consenting (they are ours, not real users).
    crate::repo::user::create(pool, id, email, &hash, display_name, true).await.map_err(|e| {
        // Another concurrent insert (race) may have won; ignore conflicts here.
        match e {
            AppError::Conflict(_) => AppError::internal("concurrent seed conflict"),
            other => other,
        }
    })?;

    // Return the ID we used (or that was stored by the winner in a race).
    Ok(
        crate::repo::user::find_by_email(pool, email)
            .await?
            .ok_or_else(|| AppError::internal("user vanished after insert"))?
            .id,
    )
}

/// Insert an accepted friendship between two users if none exists.
async fn upsert_friendship(
    pool: &PgPool,
    id: Uuid,
    requester_id: Uuid,
    addressee_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
            SELECT 1 FROM friendships \
            WHERE (requester_id = $1 AND addressee_id = $2) \
               OR (requester_id = $2 AND addressee_id = $1) \
        )",
    )
    .bind(requester_id)
    .bind(addressee_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    if exists {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO friendships (id, requester_id, addressee_id, status) \
         VALUES ($1, $2, $3, 'accepted')",
    )
    .bind(id)
    .bind(requester_id)
    .bind(addressee_id)
    .execute(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(())
}

/// Insert a nature zone by name if no zone with that name already exists.
///
/// `nature_zones.name` has no UNIQUE constraint, so we use an existence
/// check rather than ON CONFLICT.
async fn upsert_nature_zone(pool: &PgPool, id: Uuid, name: &str) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM nature_zones WHERE name = $1)")
            .bind(name)
            .fetch_one(pool)
            .await
            .map_err(AppError::internal)?;

    if exists {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO nature_zones (id, name, geom, multiplier, active) \
         VALUES ($1, $2, ST_GeogFromText($3), 3.0, true)",
    )
    .bind(id)
    .bind(name)
    .bind(BRZEZNO_POLYGON)
    .execute(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(())
}

/// Insert a reward by title if no reward with that title already exists.
///
/// `rewards_catalog.title` has no UNIQUE constraint, so we use an existence
/// check rather than ON CONFLICT.
async fn upsert_reward(
    pool: &PgPool,
    id: Uuid,
    title: &str,
    description: &str,
    reward_type: &str,
    partner_name: &str,
    cost_points: i64,
) -> Result<(), AppError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM rewards_catalog WHERE title = $1)")
            .bind(title)
            .fetch_one(pool)
            .await
            .map_err(AppError::internal)?;

    if exists {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO rewards_catalog (id, title, description, type, partner_name, cost_points, stock, active) \
         VALUES ($1, $2, $3, $4, $5, $6, 100, true)",
    )
    .bind(id)
    .bind(title)
    .bind(description)
    .bind(reward_type)
    .bind(partner_name)
    .bind(cost_points)
    .execute(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(())
}
