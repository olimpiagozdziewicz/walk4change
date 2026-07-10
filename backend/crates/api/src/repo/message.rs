use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, FieldError},
    models::{ConversationSummary, Message},
    repo::friend,
};

/// Hard cap on messages returned per conversation fetch.
const MAX_MESSAGES_LIMIT: i64 = 200;

/// Send a direct message from `sender` to `recipient`.
///
/// Errors:
/// - empty or >2000-char body → 422 Validation
/// - `sender == recipient` → 422 Validation
/// - the two users are not accepted friends → 403 Forbidden
pub async fn send(
    pool: &PgPool,
    id: Uuid,
    sender: Uuid,
    recipient: Uuid,
    body: &str,
) -> Result<Message, AppError> {
    let body = body.trim();
    if body.is_empty() || body.chars().count() > 2000 {
        return Err(AppError::Validation(vec![FieldError {
            field: "body".into(),
            message: "message must be 1..=2000 characters".into(),
            code: "INVALID_LENGTH".into(),
        }]));
    }
    if sender == recipient {
        return Err(AppError::Validation(vec![FieldError {
            field: "recipient_id".into(),
            message: "cannot message yourself".into(),
            code: "SELF_MESSAGE".into(),
        }]));
    }

    if !friend::are_friends(pool, sender, recipient).await? {
        return Err(AppError::Forbidden);
    }

    let message: Message = sqlx::query_as(
        "INSERT INTO messages (id, sender_id, recipient_id, body) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, sender_id, recipient_id, body, created_at, read_at",
    )
    .bind(id)
    .bind(sender)
    .bind(recipient)
    .bind(body)
    .fetch_one(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(message)
}

/// Fetch the conversation between `me` and `other`, oldest first.
///
/// Friends-only, also on read (audit 2026-07-10 B1.2/B1.3): after an unfriend
/// the history stops being readable, so severing the relation really cuts the
/// channel. When `after` is given, only messages strictly newer are returned
/// (polling). Side effect: marks the RETURNED incoming messages as read
/// (only what the client actually received — audit B1.5).
pub async fn conversation(
    pool: &PgPool,
    me: Uuid,
    other: Uuid,
    after: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<Message>, AppError> {
    if !friend::are_friends(pool, me, other).await? {
        return Err(AppError::Forbidden);
    }

    let limit = limit.clamp(1, MAX_MESSAGES_LIMIT);

    // WHERE po LEAST/GREATEST pary — dokładnie pod indeks messages_pair_idx
    // (forma z OR-em nie mapowała się na indeks wyrażeniowy → seq scan).
    let messages: Vec<Message> = sqlx::query_as(
        "SELECT id, sender_id, recipient_id, body, created_at, read_at \
         FROM messages \
         WHERE LEAST(sender_id, recipient_id) = LEAST($1, $2) \
           AND GREATEST(sender_id, recipient_id) = GREATEST($1, $2) \
           AND ($3::timestamptz IS NULL OR created_at > $3) \
         ORDER BY created_at DESC \
         LIMIT $4",
    )
    .bind(me)
    .bind(other)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    // Mark-read wyłącznie dla zwróconych wiadomości przychodzących —
    // bez zapisu, gdy nie ma nic nowego (koniec write'a co 5 s per okno czatu).
    let unread_ids: Vec<Uuid> = messages
        .iter()
        .filter(|m| m.recipient_id == me && m.read_at.is_none())
        .map(|m| m.id)
        .collect();
    if !unread_ids.is_empty() {
        sqlx::query(
            "UPDATE messages SET read_at = now() \
             WHERE id = ANY($1) AND recipient_id = $2 AND read_at IS NULL",
        )
        .bind(&unread_ids)
        .bind(me)
        .execute(pool)
        .await
        .map_err(AppError::internal)?;
    }

    // Query returns newest-first (so LIMIT keeps the tail); flip to oldest-first for the UI.
    let mut messages = messages;
    messages.reverse();
    Ok(messages)
}

/// List all conversations of `me`: one row per partner with the latest
/// message and an unread counter, newest conversations first.
pub async fn conversations(pool: &PgPool, me: Uuid) -> Result<Vec<ConversationSummary>, AppError> {
    let rows: Vec<ConversationSummary> = sqlx::query_as(
        "SELECT p.partner_id AS user_id, \
                u.display_name, \
                u.avatar_url, \
                m.body AS last_body, \
                m.created_at AS last_at, \
                (m.sender_id = $1) AS last_from_me, \
                (SELECT count(*) FROM messages um \
                  WHERE um.sender_id = p.partner_id AND um.recipient_id = $1 \
                    AND um.read_at IS NULL) AS unread \
         FROM ( \
             SELECT DISTINCT ON (partner_id) partner_id, id AS message_id \
             FROM ( \
                 SELECT id, created_at, \
                        CASE WHEN sender_id = $1 THEN recipient_id ELSE sender_id END AS partner_id \
                 FROM messages \
                 WHERE sender_id = $1 OR recipient_id = $1 \
             ) x \
             ORDER BY partner_id, created_at DESC \
         ) p \
         JOIN messages m ON m.id = p.message_id \
         JOIN users u ON u.id = p.partner_id \
         ORDER BY m.created_at DESC",
    )
    .bind(me)
    .fetch_all(pool)
    .await
    .map_err(AppError::internal)?;

    Ok(rows)
}
