//! Live WebSocket handler: auth-first-frame, ping → score → broadcast, subscribe.
//!
//! Connection lifecycle:
//! 1. `GET /api/v1/ws` upgrades to a WebSocket (no token in the query string).
//! 2. The **first** client frame MUST be [`ClientFrame::Auth`] within
//!    [`WS_AUTH_TIMEOUT`]; otherwise an error frame is sent and the socket closes.
//! 3. After auth, the socket can send `Ping`, `Subscribe`, and
//!    `SubscribeLeaderboard` frames. Inbound broadcast frames (from the
//!    [`crate::ws::hub::Hub`])
//!    are forwarded back to the socket.
//!
//! Concurrency: a single per-connection task drives a `tokio::select!` over the
//! WS receiver and an internal mpsc `out_rx`. Each subscription spawns a small
//! forwarder task that drains a `broadcast::Receiver` into `out_tx`, so the set
//! of subscriptions can grow dynamically without rebuilding the `select!`.

use std::collections::HashSet;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::{
    repo::walk,
    routes::leaderboard::top_n,
    scoring::repo::{score_ping, PingInput},
    state::AppState,
    ws::protocol::{ClientFrame, ServerFrame},
};

/// Maximum time to wait for the first (auth) frame before closing.
const WS_AUTH_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum accepted size of a single inbound text frame (16 KiB).
const MAX_WS_FRAME_BYTES: usize = 16 * 1024;

/// Minimum interval accepted between two processed `Ping` frames on a single
/// connection. The HTTP rate-limit middleware only covers the upgrade
/// request — once upgraded, inbound `Ping` frames would otherwise be
/// unthrottled, and each one runs several sequential DB queries against a
/// small pool. The client legitimately pings roughly once every 4s, so a
/// floor of 1/s is very safe and never affects real users (security
/// hardening 2026-07-09).
const WS_PING_MIN_INTERVAL: Duration = Duration::from_millis(1000);

/// Capacity of the per-connection outbound queue. Previously unbounded: a
/// slow client could grow it without limit (audit 2026-07-10). When full, the
/// session forwarder blocks and the broadcast receiver lags — old frames are
/// then dropped via `Lagged`, which live GPS tolerates by design.
const WS_OUT_QUEUE_CAPACITY: usize = 256;

/// How often the connection re-checks the JWT expiry. The token was only
/// validated once at auth, so a socket outlived its TTL indefinitely
/// (audit 2026-07-10 N2). Worst-case overrun past `exp` = one interval.
const WS_TOKEN_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Max concurrent session subscriptions per connection. Each subscription
/// spawns a forwarder with DB re-checks, so an unbounded count let one socket
/// multiply load (audit 2026-07-10 N2). A real client tracks 1 session
/// (+ leaderboard, which is exempt).
const MAX_WS_SUBSCRIPTIONS: usize = 8;

/// How long a session forwarder trusts its cached membership verdict before
/// re-querying the DB. Per-frame checks made a full N-person group cost ~N²
/// queries per ping interval (audit 2026-07-10 part A). Revocation via
/// leave/kick/stop takes effect within this window at worst.
const FORWARDER_MEMBERSHIP_TTL: Duration = Duration::from_secs(10);

/// Axum entry point: upgrade the connection and hand off to [`handle_socket`].
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Per-connection driver. Runs the auth handshake, then the main event loop.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // ── auth-first-frame ──────────────────────────────────────────────────────
    let (actor, token_exp) = match authenticate(&mut sender, &mut receiver, &state).await {
        Some(auth) => auth,
        None => return, // error already reported (or socket closed); we're done.
    };

    // ── main event loop ───────────────────────────────────────────────────────
    let (out_tx, mut out_rx) = mpsc::channel::<ServerFrame>(WS_OUT_QUEUE_CAPACITY);
    let mut subscriptions: HashSet<Uuid> = HashSet::new();
    let mut leaderboard_subscribed = false;
    let mut forwarders: Vec<JoinHandle<()>> = Vec::new();
    // Per-connection throttle state for inbound `Ping` frames (local to this
    // task — no shared state needed, see `WS_PING_MIN_INTERVAL`).
    let mut last_ping_at: Option<tokio::time::Instant> = None;
    // Periodic JWT-expiry check (audit N2): the first tick fires immediately,
    // which is harmless — the token was just validated.
    let mut token_check = tokio::time::interval(WS_TOKEN_CHECK_INTERVAL);
    token_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            // Outbound broadcast frame → write to the socket.
            Some(frame) = out_rx.recv() => {
                if sender.send(Message::Text(frame.to_json())).await.is_err() {
                    break;
                }
            }

            // JWT TTL enforcement: close the socket once the token expires
            // instead of letting the connection outlive it (audit N2).
            // Same tick also kills sockets of deleted accounts (RODO tail,
            // spec 2026-07-13) — worst case the connection survives delete
            // by one interval (30 s).
            _ = token_check.tick() => {
                if chrono::Utc::now().timestamp() >= token_exp {
                    send_close_with_error(&mut sender, "token expired").await;
                    break;
                }
                if crate::repo::user::is_deleted(&state.pool, actor).await.unwrap_or(false) {
                    send_close_with_error(&mut sender, "account deleted").await;
                    break;
                }
            }

            // Inbound client frame.
            incoming = receiver.next() => {
                let msg = match incoming {
                    Some(Ok(msg)) => msg,
                    Some(Err(_)) | None => break, // transport error or stream end
                };

                let text = match msg {
                    Message::Text(t) => t,
                    Message::Close(_) => break,
                    // Ignore binary/ping/pong: control frames are handled by axum.
                    _ => continue,
                };

                if text.len() > MAX_WS_FRAME_BYTES {
                    let _ = sender
                        .send(Message::Text(
                            ServerFrame::error("frame too large").to_json(),
                        ))
                        .await;
                    break;
                }

                let frame: ClientFrame = match serde_json::from_str(&text) {
                    Ok(f) => f,
                    Err(_) => {
                        let _ = sender
                            .send(Message::Text(
                                ServerFrame::error("invalid frame").to_json(),
                            ))
                            .await;
                        continue;
                    }
                };

                match frame {
                    // A second Auth frame is a no-op (already authenticated).
                    ClientFrame::Auth { .. } => {}

                    ClientFrame::Ping {
                        session_id,
                        seq,
                        lat,
                        lng,
                        recorded_at,
                        accuracy,
                    } => {
                        // Per-connection throttle: silently drop pings that arrive
                        // faster than WS_PING_MIN_INTERVAL, before they reach the
                        // DB-backed authz/scoring path. Real clients ping ~1/4s,
                        // so this never affects legitimate traffic.
                        let now = tokio::time::Instant::now();
                        let too_fast = last_ping_at
                            .map(|prev| now.duration_since(prev) < WS_PING_MIN_INTERVAL)
                            .unwrap_or(false);
                        if too_fast {
                            continue;
                        }
                        last_ping_at = Some(now);

                        handle_ping(
                            &mut sender,
                            &state,
                            actor,
                            session_id,
                            seq,
                            lat,
                            lng,
                            recorded_at,
                            accuracy,
                            &out_tx,
                            &mut subscriptions,
                            &mut forwarders,
                        )
                        .await;
                    }

                    ClientFrame::Subscribe { session_id } => {
                        // Cap subscriptions per connection (audit N2): each one
                        // spawns a forwarder task with DB re-checks.
                        if !subscriptions.contains(&session_id)
                            && subscriptions.len() >= MAX_WS_SUBSCRIPTIONS
                        {
                            let _ = sender
                                .send(Message::Text(
                                    ServerFrame::error("too many subscriptions").to_json(),
                                ))
                                .await;
                            continue;
                        }
                        // Spec §271: live subscription requires ACTIVE membership
                        // (session active AND left_at IS NULL). Historical access
                        // for members who have left is fine for HTTP GET, but not
                        // for the live WS stream.
                        match walk::is_active_participant(&state.pool, session_id, actor).await {
                            Ok(true) => {
                                if subscriptions.insert(session_id) {
                                    let rx = state.hub.session_sender(session_id).subscribe();
                                    forwarders.push(spawn_session_forwarder(
                                        rx,
                                        out_tx.clone(),
                                        state.pool.clone(),
                                        session_id,
                                        actor,
                                    ));
                                }
                            }
                            Ok(false) => {
                                let _ = sender
                                    .send(Message::Text(
                                        ServerFrame::error(
                                            "not an active participant of this session",
                                        )
                                        .to_json(),
                                    ))
                                    .await;
                            }
                            Err(_) => {
                                let _ = sender
                                    .send(Message::Text(
                                        ServerFrame::error("subscribe failed").to_json(),
                                    ))
                                    .await;
                            }
                        }
                    }

                    ClientFrame::SubscribeLeaderboard => {
                        if !leaderboard_subscribed {
                            leaderboard_subscribed = true;
                            let rx = state.hub.leaderboard_sender().subscribe();
                            forwarders.push(spawn_forwarder(rx, out_tx.clone()));
                        }
                    }
                }
            }
        }
    }

    // Tear down forwarder tasks so none linger after the socket closes.
    for f in forwarders {
        f.abort();
    }
}

/// Wait for and validate the first frame as [`ClientFrame::Auth`].
///
/// Returns `Some((actor, token_exp))` on success — the expiry is enforced
/// periodically by the main loop (audit N2). On any failure (timeout, wrong
/// frame type, invalid token, oversized frame, transport error) an error frame
/// is sent and `None` is returned so the caller drops the socket.
async fn authenticate(
    sender: &mut SplitSink<WebSocket, Message>,
    receiver: &mut SplitStream<WebSocket>,
    state: &AppState,
) -> Option<(Uuid, i64)> {
    let first = match tokio::time::timeout(WS_AUTH_TIMEOUT, receiver.next()).await {
        Ok(Some(Ok(msg))) => msg,
        Ok(Some(Err(_))) | Ok(None) => return None, // transport error / closed
        Err(_) => {
            // Timed out waiting for the first frame.
            let _ = sender
                .send(Message::Text(
                    ServerFrame::error("authentication timeout").to_json(),
                ))
                .await;
            let _ = sender.send(Message::Close(None)).await;
            return None;
        }
    };

    let text = match first {
        Message::Text(t) if t.len() <= MAX_WS_FRAME_BYTES => t,
        _ => {
            let _ = sender
                .send(Message::Text(
                    ServerFrame::error("first frame must be auth").to_json(),
                ))
                .await;
            let _ = sender.send(Message::Close(None)).await;
            return None;
        }
    };

    match serde_json::from_str::<ClientFrame>(&text) {
        Ok(ClientFrame::Auth { token }) => match crate::auth::jwt::decode(&state.config, &token) {
            Ok(claims) => Some((claims.sub, claims.exp)),
            Err(_) => {
                send_close_with_error(sender, "invalid token").await;
                None
            }
        },
        _ => {
            send_close_with_error(sender, "first frame must be auth").await;
            None
        }
    }
}

/// Send an error frame followed by a close frame (best-effort).
async fn send_close_with_error(sender: &mut SplitSink<WebSocket, Message>, msg: &str) {
    let _ = sender
        .send(Message::Text(ServerFrame::error(msg).to_json()))
        .await;
    let _ = sender.send(Message::Close(None)).await;
}

/// Authorize, score, and broadcast a single ping.
#[allow(clippy::too_many_arguments)]
async fn handle_ping(
    sender: &mut SplitSink<WebSocket, Message>,
    state: &AppState,
    actor: Uuid,
    session_id: Uuid,
    seq: i32,
    lat: f64,
    lng: f64,
    recorded_at: chrono::DateTime<chrono::Utc>,
    accuracy: Option<f64>,
    out_tx: &mpsc::Sender<ServerFrame>,
    subscriptions: &mut HashSet<Uuid>,
    forwarders: &mut Vec<JoinHandle<()>>,
) {
    // Authz: only an active participant may submit pings for this session.
    match walk::is_active_participant(&state.pool, session_id, actor).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = sender
                .send(Message::Text(
                    ServerFrame::error("not an active participant").to_json(),
                ))
                .await;
            return;
        }
        Err(_) => {
            let _ = sender
                .send(Message::Text(
                    ServerFrame::error("authorization failed").to_json(),
                ))
                .await;
            return;
        }
    }

    let input = PingInput {
        session_id,
        user_id: actor,
        seq,
        lat,
        lng,
        recorded_at,
        accuracy,
    };

    let score = match score_ping(&state.pool, &state.config.scoring, input).await {
        Ok(Some(score)) => score,
        Ok(None) => return, // duplicate seq — no broadcast
        Err(_) => {
            let _ = sender
                .send(Message::Text(
                    ServerFrame::error("scoring failed").to_json(),
                ))
                .await;
            return;
        }
    };

    // Ensure this socket is subscribed to its own session so the pinger receives
    // its own scored ping through the single broadcast path (no echo, no dupes).
    if subscriptions.insert(session_id) {
        let rx = state.hub.session_sender(session_id).subscribe();
        forwarders.push(spawn_session_forwarder(
            rx,
            out_tx.clone(),
            state.pool.clone(),
            session_id,
            actor,
        ));
    }

    let ping_frame = ServerFrame::PingScored {
        data: serde_json::json!({
            "session_id": session_id,
            "user_id": actor,
            "seq": score.seq,
            "point": { "lat": score.lat, "lng": score.lng },
            "segment_meters": score.segment_meters,
            "nature_mult": score.nature_mult,
            "together_mult": score.together_mult,
            "points": score.points,
            "participant_total": score.participant_total,
        }),
    };
    state.hub.publish_session(session_id, ping_frame);

    // Global leaderboard throttle: at most once per 500 ms across all connections.
    // The throttle check is sync; the DB query only runs when the slot is reserved.
    if state.hub.should_publish_leaderboard() {
        match top_n(&state.pool, 10).await {
            Ok(entries) => {
                let data =
                    serde_json::to_value(&entries).unwrap_or_else(|_| serde_json::json!([]));
                state
                    .hub
                    .publish_leaderboard(ServerFrame::LeaderboardUpdate { data });
            }
            Err(_) => {
                // Non-fatal: scoring already succeeded; skip the leaderboard push.
            }
        }
    }
}

/// Spawn a task that forwards broadcast frames into the connection's `out_tx`.
///
/// `Lagged(n)` is treated as "skip and keep going" (never closes the socket).
/// The task exits when the broadcast channel closes or `out_tx` has no receiver.
/// `out_tx` is bounded: when a slow client fills it, `send().await` applies
/// backpressure here and the broadcast receiver lags (drops old frames)
/// instead of growing memory without limit (audit 2026-07-10).
fn spawn_forwarder(
    mut rx: broadcast::Receiver<ServerFrame>,
    out_tx: mpsc::Sender<ServerFrame>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(frame) => {
                    if out_tx.send(frame).await.is_err() {
                        break; // connection task gone
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

/// Spawn a task that forwards session broadcast frames into the connection's `out_tx`,
/// revoking the subscription when the actor is no longer an active participant.
///
/// Spec §271: once `left_at` is set (via /leave, kick or stop), the forwarder
/// stops relaying live GPS frames to the departed subscriber.
///
/// The membership verdict is CACHED for [`FORWARDER_MEMBERSHIP_TTL`] instead
/// of being re-queried per frame: with N subscribers each verifying every
/// frame, a full group session cost ~N² queries per ping interval against a
/// 10-connection pool (audit 2026-07-10 part A). Worst-case revocation delay
/// is one TTL — acceptable for live GPS, and kick also bars re-join.
///
/// Fails closed: any DB error is treated as "no longer active" to prevent
/// inadvertent data leakage.
fn spawn_session_forwarder(
    mut rx: broadcast::Receiver<ServerFrame>,
    out_tx: mpsc::Sender<ServerFrame>,
    pool: sqlx::PgPool,
    session_id: Uuid,
    actor: Uuid,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // (when the verdict was fetched, verdict) — None until the first frame.
        let mut membership: Option<(tokio::time::Instant, bool)> = None;
        loop {
            match rx.recv().await {
                Ok(frame) => {
                    let stale = match membership {
                        Some((checked_at, _)) => {
                            checked_at.elapsed() >= FORWARDER_MEMBERSHIP_TTL
                        }
                        None => true,
                    };
                    if stale {
                        let active =
                            walk::is_active_participant(&pool, session_id, actor)
                                .await
                                .unwrap_or(false); // Err → fail-closed
                        membership = Some((tokio::time::Instant::now(), active));
                    }
                    match membership {
                        Some((_, true)) => {
                            if out_tx.send(frame).await.is_err() {
                                break; // connection task gone
                            }
                        }
                        // left / kicked / session stopped / DB error → stop relaying.
                        _ => break,
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
