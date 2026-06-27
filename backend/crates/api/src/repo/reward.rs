use rand::{rngs::OsRng, RngCore};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{Redemption, Reward},
};

/// RFC 4648 base-32 alphabet (no padding).
const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Generate a redemption code with at least 128 bits of entropy.
///
/// Draws 16 cryptographically-secure random bytes from [`OsRng`] and base-32
/// encodes them (RFC 4648, no padding) → a 26-character code.
fn generate_code() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    base32_encode(&bytes)
}

/// Minimal RFC 4648 base-32 encoder (no padding). Avoids pulling in a crate.
fn base32_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(5) * 8);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for &byte in input {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buffer >> bits) & 0x1f) as usize;
            out.push(BASE32_ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
        out.push(BASE32_ALPHABET[idx] as char);
    }
    out
}

/// List all active rewards in the catalog.
///
/// `type` is a Rust keyword, so it is aliased to `type_` for the model.
pub async fn list(pool: &PgPool) -> Result<Vec<Reward>, AppError> {
    let rewards: Vec<Reward> = sqlx::query_as(
        "SELECT id, title, description, cost_points, partner_name, \
                type AS type_, stock, image_url \
         FROM rewards_catalog \
         WHERE active \
         ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(rewards)
}

/// Atomically redeem a reward for `actor` — single transaction, no oversell.
///
/// Consistent lock order: `user_totals` (FOR UPDATE) **then** `rewards_catalog`
/// (via the conditional, guarded UPDATE). Steps:
/// 1. Lock the caller's `user_totals` row (missing row ⇒ balance 0).
/// 2. Claim one unit of stock atomically — the `stock > 0` guard ensures only
///    one concurrent caller wins; unlimited stock (NULL) is never decremented.
///    0 rows ⇒ `Conflict("unavailable")` (sold out / inactive / missing).
/// 3. Verify balance ≥ cost — failure returns `Err`, dropping the tx and
///    rolling back the stock claim.
/// 4. Reserve a unique code, insert the redemption, debit `spent_points`.
pub async fn redeem(
    pool: &PgPool,
    reward_id: Uuid,
    actor: Uuid,
) -> Result<Redemption, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::internal)?;

    // 1. Lock the user's totals row first (consistent lock order).
    let totals: Option<(Decimal, Decimal)> = sqlx::query_as(
        "SELECT total_points, spent_points FROM user_totals \
         WHERE user_id = $1 FOR UPDATE",
    )
    .bind(actor)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    // 2. Atomically claim stock and verify the reward is active.
    //    Unlimited stock (NULL) stays NULL; finite stock is decremented.
    let claimed: Option<(Decimal,)> = sqlx::query_as(
        "UPDATE rewards_catalog \
         SET stock = CASE WHEN stock IS NULL THEN NULL ELSE stock - 1 END \
         WHERE id = $1 AND active AND (stock IS NULL OR stock > 0) \
         RETURNING cost_points",
    )
    .bind(reward_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    let cost_points = match claimed {
        Some((cost,)) => cost,
        None => return Err(AppError::Conflict("unavailable".into())),
    };

    // 3. Balance check — returning Err here rolls back the stock claim.
    let balance = match totals {
        Some((total, spent)) => total - spent,
        None => Decimal::ZERO,
    };
    if balance < cost_points {
        return Err(AppError::Conflict("insufficient_points".into()));
    }

    // 4. Reserve the code and persist the redemption + debit.
    let redemption_id = Uuid::new_v4();
    let code = generate_code();

    let redemption: Redemption = sqlx::query_as(
        "INSERT INTO reward_redemptions \
            (id, user_id, reward_id, points_spent, code, status) \
         VALUES ($1, $2, $3, $4, $5, 'reserved') \
         RETURNING id, reward_id, code, points_spent, status, created_at",
    )
    .bind(redemption_id)
    .bind(actor)
    .bind(reward_id)
    .bind(cost_points)
    .bind(&code)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    sqlx::query(
        "UPDATE user_totals SET spent_points = spent_points + $1, updated_at = now() \
         WHERE user_id = $2",
    )
    .bind(cost_points)
    .bind(actor)
    .execute(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(redemption)
}

/// List all redemptions belonging to `actor`, newest first.
pub async fn list_redemptions(
    pool: &PgPool,
    actor: Uuid,
) -> Result<Vec<Redemption>, AppError> {
    let redemptions: Vec<Redemption> = sqlx::query_as(
        "SELECT id, reward_id, code, points_spent, status, created_at \
         FROM reward_redemptions \
         WHERE user_id = $1 \
         ORDER BY created_at DESC",
    )
    .bind(actor)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(redemptions)
}
