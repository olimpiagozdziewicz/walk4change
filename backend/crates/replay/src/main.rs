//! walk4change-replay — Real-time track replay client.
//!
//! Streams GPS pings from a JSON track file over the walk4change WebSocket API,
//! honouring the `offset_ms` field so each segment reaches the server at the
//! right wall-clock time (≈ 1 s apart), which keeps segment speeds inside the
//! scoring engine's speed cap.
//!
//! # Usage
//!
//! Single-user replay:
//! ```text
//! walk4change-replay --base http://localhost:8080 \
//!   --email ana@demo.walk4change --password secret \
//!   --track fixtures/track_a.json
//! ```
//!
//! Two-friend mode (demonstrates the together multiplier):
//! ```text
//! walk4change-replay --base http://localhost:8080 \
//!   --email ana@demo.walk4change --password secret \
//!   --track fixtures/track_a.json \
//!   --friend-email bek@demo.walk4change --friend-password secret \
//!   --friend-track fixtures/track_b.json
//! ```

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

// ── Types ─────────────────────────────────────────────────────────────────────

/// A single point in a track file.
#[derive(Debug, Deserialize)]
struct TrackPoint {
    lat: f64,
    lng: f64,
    offset_ms: u64,
}

/// CLI configuration (parsed from argv).
struct Config {
    base: String,
    email: String,
    password: String,
    track: String,
    session: Option<Uuid>,
    friend: Option<FriendConfig>,
}

struct FriendConfig {
    email: String,
    password: String,
    track: String,
}

/// REST login response envelope.
#[derive(Deserialize)]
struct LoginResponse {
    token: String,
}

/// REST walk-start response data.
#[derive(Deserialize)]
struct WalkData {
    id: Uuid,
    join_code: Option<String>,
}

#[derive(Deserialize)]
struct WalkResponse {
    data: WalkData,
}

// ── CLI argument parsing ──────────────────────────────────────────────────────

fn parse_args() -> Result<Config> {
    let mut args = std::env::args().skip(1).peekable();

    let mut base = None::<String>;
    let mut email = None::<String>;
    let mut password = None::<String>;
    let mut track = None::<String>;
    let mut session = None::<Uuid>;
    let mut friend_email = None::<String>;
    let mut friend_password = None::<String>;
    let mut friend_track = None::<String>;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--base" => base = Some(args.next().context("--base needs a value")?),
            "--email" => email = Some(args.next().context("--email needs a value")?),
            "--password" => password = Some(args.next().context("--password needs a value")?),
            "--track" => track = Some(args.next().context("--track needs a value")?),
            "--session" => {
                let raw = args.next().context("--session needs a value")?;
                session = Some(raw.parse::<Uuid>().context("--session must be a UUID")?);
            }
            "--friend-email" => {
                friend_email = Some(args.next().context("--friend-email needs a value")?)
            }
            "--friend-password" => {
                friend_password = Some(args.next().context("--friend-password needs a value")?)
            }
            "--friend-track" => {
                friend_track = Some(args.next().context("--friend-track needs a value")?)
            }
            other => return Err(anyhow!("unknown argument: {other}")),
        }
    }

    let base = base.context("--base is required")?;
    let email = email.context("--email is required")?;
    let password = password.context("--password is required")?;
    let track = track.context("--track is required")?;

    let friend = match (friend_email, friend_password, friend_track) {
        (Some(e), Some(p), Some(t)) => Some(FriendConfig { email: e, password: p, track: t }),
        (None, None, None) => None,
        _ => return Err(anyhow!("--friend-email, --friend-password, and --friend-track must all be provided together")),
    };

    Ok(Config { base, email, password, track, session, friend })
}

// ── REST helpers ──────────────────────────────────────────────────────────────

async fn login(client: &reqwest::Client, base: &str, email: &str, password: &str) -> Result<String> {
    let resp = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .context("login request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("login failed ({status}): {body}"));
    }

    let parsed: LoginResponse = resp.json().await.context("login response parse failed")?;
    Ok(parsed.token)
}

async fn start_walk(client: &reqwest::Client, base: &str, token: &str) -> Result<(Uuid, Option<String>)> {
    let resp = client
        .post(format!("{base}/api/v1/walks"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .context("start walk request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("start walk failed ({status}): {body}"));
    }

    let parsed: WalkResponse = resp.json().await.context("start walk response parse failed")?;
    Ok((parsed.data.id, parsed.data.join_code))
}

async fn join_walk(client: &reqwest::Client, base: &str, token: &str, session_id: Uuid) -> Result<()> {
    let resp = client
        .post(format!("{base}/api/v1/walks/{session_id}/join"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .context("join walk request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("join walk failed ({status}): {body}"));
    }

    Ok(())
}

// ── Track loading ─────────────────────────────────────────────────────────────

fn load_track(path: &str) -> Result<Vec<TrackPoint>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read track file: {path}"))?;
    let points: Vec<TrackPoint> = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse track file: {path}"))?;
    if points.is_empty() {
        return Err(anyhow!("track file {path} contains no points"));
    }
    Ok(points)
}

// ── WebSocket streaming ───────────────────────────────────────────────────────

/// Maximum sleep between pings (caps the demo replay speed).
const MAX_STEP_MS: u64 = 2_000;

/// Open a WS, authenticate, and stream all track points in real time.
///
/// Prints each received `ping_scored` frame to stdout.
async fn stream_track(
    base: &str,
    token: &str,
    session_id: Uuid,
    track: Vec<TrackPoint>,
    label: &str,
) -> Result<()> {
    let ws_url = base
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    let ws_url = format!("{ws_url}/api/v1/ws");

    let (mut ws, _) = connect_async(&ws_url)
        .await
        .with_context(|| format!("WS connect failed: {ws_url}"))?;

    // Auth handshake.
    ws.send(Message::Text(json!({"type": "auth", "token": token}).to_string()))
        .await
        .context("WS auth send failed")?;

    let start = Instant::now();
    let mut seq = 0i32;

    for point in &track {
        // Sleep until the target wall-clock offset (honouring offset_ms, but capped).
        let target_ms = point.offset_ms;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        if target_ms > elapsed_ms {
            let sleep_ms = (target_ms - elapsed_ms).min(MAX_STEP_MS);
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }

        seq += 1;
        let ping = json!({
            "type": "ping",
            "session_id": session_id,
            "seq": seq,
            "lat": point.lat,
            "lng": point.lng,
            "recorded_at": Utc::now().to_rfc3339(),
        });
        ws.send(Message::Text(ping.to_string()))
            .await
            .context("WS ping send failed")?;

        // Read the ping_scored ack (skip non-text control frames).
        loop {
            match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => {
                    let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                    if v["type"] == "ping_scored" {
                        let d = &v["data"];
                        println!(
                            "[{label}] seq={seq} points={} together_mult={} nature_mult={}",
                            d["points"], d["together_mult"], d["nature_mult"]
                        );
                    } else if v["type"] == "error" {
                        eprintln!("[{label}] server error: {}", v["error"]["message"]);
                    }
                    // Break on any data frame (ping_scored for us or another user — keep moving).
                    break;
                }
                Ok(Some(Ok(_))) => continue, // ping/pong/binary control — skip
                Ok(Some(Err(e))) => return Err(anyhow!("[{label}] WS error: {e}")),
                Ok(None) => return Err(anyhow!("[{label}] WS closed unexpectedly")),
                Err(_) => return Err(anyhow!("[{label}] timeout waiting for ping_scored")),
            }
        }
    }

    ws.send(Message::Close(None)).await.ok();
    println!("[{label}] stream complete ({} pings)", track.len());
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = parse_args()?;
    let client = reqwest::Client::new();

    // ── Login ─────────────────────────────────────────────────────────────────
    println!("Logging in as {} ...", cfg.email);
    let token_a = login(&client, &cfg.base, &cfg.email, &cfg.password).await?;

    let token_b = if let Some(ref f) = cfg.friend {
        println!("Logging in as {} ...", f.email);
        Some(login(&client, &cfg.base, &f.email, &f.password).await?)
    } else {
        None
    };

    // ── Load tracks ───────────────────────────────────────────────────────────
    let track_a = load_track(&cfg.track)?;

    let track_b = if let Some(ref f) = cfg.friend {
        Some(load_track(&f.track)?)
    } else {
        None
    };

    // ── Session setup ─────────────────────────────────────────────────────────
    let session_id = if let Some(id) = cfg.session {
        println!("Using existing session {id}");
        id
    } else {
        let (id, join_code) = start_walk(&client, &cfg.base, &token_a).await?;
        println!("Started walk — session_id={id}  join_code={}", join_code.as_deref().unwrap_or("-"));

        if let (Some(ref tb), Some(ref f)) = (&token_b, &cfg.friend) {
            println!("Friend {} joining ...", f.email);
            join_walk(&client, &cfg.base, tb, id).await?;
        }

        id
    };

    // ── Stream ────────────────────────────────────────────────────────────────
    let base_a = cfg.base.clone();
    let email_a = cfg.email.clone();

    match (token_b, track_b, cfg.friend) {
        (Some(tb), Some(trk_b), Some(f)) => {
            // Two-friend mode: stream both tracks concurrently.
            let base_b = base_a.clone();
            let (res_a, res_b) = tokio::join!(
                stream_track(&base_a, &token_a, session_id, track_a, &email_a),
                stream_track(&base_b, &tb, session_id, trk_b, &f.email),
            );
            res_a?;
            res_b?;
        }
        _ => {
            // Single-user mode.
            stream_track(&base_a, &token_a, session_id, track_a, &email_a).await?;
        }
    }

    Ok(())
}
