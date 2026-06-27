# walk4change Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A standalone Rust backend (Axum) that owns auth, live GPS walk scoring, friends, rewards, and a WebSocket live feed, backed by plain Postgres+PostGIS — the full MVP from the spec at `docs/superpowers/specs/2026-06-27-walk4change-backend-design.md`.

**Architecture:** Single Axum service. Layering `handlers (HTTP/WS) → services → repositories (sqlx)`. Postgres+PostGIS via runtime-checked sqlx (no compile-time DB). Live scoring per ping inside a DB transaction; live push via `tokio::broadcast` hub. Supabase used only as a Postgres host.

**Tech Stack:** Rust (stable, edition 2021), axum 0.7, tokio 1, sqlx 0.8 (postgres, runtime-tokio, tls-rustls, uuid, chrono, rust_decimal), jsonwebtoken 9, argon2 0.5, rand 0.8, rust_decimal 1, uuid 1, chrono 0.4, serde/serde_json 1, tower-http 0.6, tower 0.5, tracing + tracing-subscriber, thiserror 1, dotenvy 0.15, tokio-tungstenite 0.24 (replay bin only).

## Global Constraints

- Rust edition **2021**, stable toolchain. Workspace at `walk4change/backend/`.
- **Use runtime-checked sqlx only** (`sqlx::query`, `sqlx::query_as`, `query_scalar`). **Never** `query!`/`query_as!` macros — there is no DB at compile time.
- All money/points/meters columns are Postgres `NUMERIC`, mapped to `rust_decimal::Decimal`. Never `f64` for points or balances.
- Geometry columns are `geography(...,4326)`. Insert points via `ST_SetSRID(ST_MakePoint($lng,$lat),4326)::geography`. Read coords via `ST_Y(geom::geometry)` / `ST_X(geom::geometry)`. Containment via `ST_Covers` (never `ST_Contains`/`ST_Within` on geography). `ST_Distance` on geography returns meters.
- Scoring clock = **server receive time** (`location_pings.received_at`, default `now()`), never client `recorded_at`. Δt and companion window use `received_at`.
- All HTTP routes under `/api/v1`. Success envelope `{ "data": ... , "meta"?, "links"? }`; error envelope `{ "error": { "code", "message", "details"? } }`.
- UUIDs generated in Rust (`Uuid::new_v4()`), passed explicitly to inserts.
- Timestamps are `chrono::DateTime<Utc>` ↔ `timestamptz`.
- Every authenticated handler takes the `AuthUser` extractor. Every owned-resource repo method takes `actor_id: Uuid` and enforces it in the SQL `WHERE`. No read-then-compare authz.
- Tests: a real Postgres+PostGIS via Docker (`backend/docker-compose.yml`, service `db`, image `postgis/postgis:16-3.4`). Integration tests connect via `TEST_DATABASE_URL`. Unit tests (pure logic) need no DB.
- TDD per task: write failing test → run (RED) → implement → run (GREEN) → commit. Commit messages use conventional commits, no attribution footer.

## File Structure

```
backend/
  Cargo.toml                      # workspace
  docker-compose.yml              # postgis db for dev/test
  .env.example
  rust-toolchain.toml             # pin stable
  crates/
    api/
      Cargo.toml
      src/
        main.rs                   # bootstrap: config, pool, migrate, router, serve
        lib.rs                    # re-exports for integration tests (app builder)
        config.rs                 # AppConfig from env + startup validation
        error.rs                  # AppError + IntoResponse + envelope
        response.rs               # ApiResponse/ApiError envelope helpers
        state.rs                  # AppState { pool, config, hub }
        db.rs                     # pool builder + run_migrations
        auth/
          mod.rs
          password.rs             # argon2 hash/verify (+ dummy hash)
          jwt.rs                  # encode/decode claims
          extractor.rs            # AuthUser FromRequestParts
          handlers.rs             # register/login/logout
        routes/
          mod.rs                  # /api/v1 router assembly
          profile.rs              # GET/PATCH /me
          friends.rs              # request/respond/list
          walks.rs                # start/join/leave/stop/get/track
          rewards.rs              # list/redeem/redemptions
          leaderboard.rs          # GET /leaderboard (paginated)
        ws/
          mod.rs
          protocol.rs             # client/server frame enums (serde)
          hub.rs                  # broadcast channel registry
          handler.rs              # upgrade + per-conn loop (auth-first-frame)
        scoring/
          mod.rs
          config.rs               # ScoringConfig
          engine.rs               # pure scoring math (unit-tested)
          repo.rs                 # score_ping transaction (PostGIS)
        repo/
          mod.rs
          user.rs
          friend.rs
          walk.rs
          reward.rs
        models/
          mod.rs                  # domain + DTO structs
        util/
          pagination.rs           # offset pagination params + meta
          ratelimit.rs            # simple per-key limiter layer
      tests/
        common/mod.rs             # test db setup, app spawn, helpers
        auth_test.rs
        friends_test.rs
        walks_test.rs
        scoring_test.rs           # integration (PostGIS)
        rewards_test.rs
        ws_test.rs
        leaderboard_test.rs
    replay/
      Cargo.toml
      src/main.rs                 # replay recorded tracks over WS
  migrations/
    0001_init.sql                 # extensions + all tables + indexes
  fixtures/
    nature_zones.geojson
    track_a.json                  # [{lat,lng,offset_ms}]
    track_b.json
  seeds/
    seed.rs (bin in api)          # demo users/friends/zones/rewards
```

---

## Phase 0 — Scaffold & infrastructure

### Task 1: Workspace scaffold + health endpoint

**Files:**
- Create: `backend/Cargo.toml`, `backend/rust-toolchain.toml`, `backend/.env.example`, `backend/docker-compose.yml`
- Create: `backend/crates/api/Cargo.toml`, `backend/crates/api/src/main.rs`, `backend/crates/api/src/lib.rs`
- Test: `backend/crates/api/tests/health_test.rs`

**Interfaces:**
- Produces: `walk4change_api::build_app(state: AppState) -> axum::Router` (added in later tasks; for now `build_app()` with no state returning a router with `GET /api/v1/health`). Define `pub fn router_health() -> Router` now; full `build_app` arrives in Task 9.

- [ ] **Step 1: Write failing test** — `tests/health_test.rs`:
```rust
use axum::http::StatusCode;
use tower::ServiceExt; // oneshot

#[tokio::test]
async fn health_returns_ok() {
    let app = walk4change_api::router_health();
    let resp = app
        .oneshot(axum::http::Request::builder().uri("/api/v1/health").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run (RED)** — `cd backend && cargo test -p walk4change-api health` → fails to compile (no `router_health`).

- [ ] **Step 3: Implement.** `backend/Cargo.toml` workspace:
```toml
[workspace]
resolver = "2"
members = ["crates/api", "crates/replay"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
axum = { version = "0.7", features = ["ws", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls", "postgres", "uuid", "chrono", "rust_decimal", "migrate"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = { version = "1", features = ["serde"] }
jsonwebtoken = "9"
argon2 = "0.5"
rand = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "limit"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1"
dotenvy = "0.15"
```
`rust-toolchain.toml`: `[toolchain]\nchannel = "stable"`.
`crates/api/Cargo.toml`:
```toml
[package]
name = "walk4change-api"
version = "0.1.0"
edition = "2021"

[lib]
name = "walk4change_api"
path = "src/lib.rs"

[[bin]]
name = "walk4change-api"
path = "src/main.rs"

[dependencies]
tokio.workspace = true
axum.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
uuid.workspace = true
chrono.workspace = true
rust_decimal.workspace = true
jsonwebtoken.workspace = true
argon2.workspace = true
rand.workspace = true
tower.workspace = true
tower-http.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
thiserror.workspace = true
dotenvy.workspace = true

[dev-dependencies]
tower.workspace = true
reqwest = { version = "0.12", features = ["json"] }
tokio-tungstenite = "0.24"
futures-util = "0.3"
```
`crates/api/src/lib.rs`:
```rust
use axum::{routing::get, Router};

pub fn router_health() -> Router {
    Router::new().route("/api/v1/health", get(|| async { "ok" }))
}
```
`crates/api/src/main.rs`:
```rust
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    ).init();
    let app = walk4change_api::router_health();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("listening on 8080");
    axum::serve(listener, app).await.unwrap();
}
```
`docker-compose.yml`:
```yaml
services:
  db:
    image: postgis/postgis:16-3.4
    environment:
      POSTGRES_USER: walk
      POSTGRES_PASSWORD: walk
      POSTGRES_DB: walk4change
    ports: ["5433:5432"]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U walk -d walk4change"]
      interval: 2s
      timeout: 3s
      retries: 20
```
`.env.example`:
```
DATABASE_URL=postgres://walk:walk@localhost:5433/walk4change
TEST_DATABASE_URL=postgres://walk:walk@localhost:5433/walk4change
JWT_SECRET=change-me-to-a-32-byte-min-random-secret-string
BIND_ADDR=0.0.0.0:8080
CORS_ALLOWED_ORIGINS=http://localhost:3000
```

- [ ] **Step 4: Run (GREEN)** — `cargo test -p walk4change-api health` → passes. Also `cargo build` succeeds.

- [ ] **Step 5: Commit** — `git add backend && git commit -m "feat(backend): workspace scaffold + health endpoint"`

---

### Task 2: Config + AppError + response envelope

**Files:**
- Create: `crates/api/src/config.rs`, `src/error.rs`, `src/response.rs`
- Modify: `src/lib.rs` (add `pub mod`s)
- Test: unit tests inside `config.rs` and `error.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `AppConfig { database_url: String, jwt_secret: String, bind_addr: String, cors_origins: Vec<String>, argon2_m_cost: u32, argon2_t_cost: u32, argon2_p_cost: u32, jwt_ttl_secs: i64, scoring: ScoringConfig }`
  - `AppConfig::from_env() -> Result<AppConfig, ConfigError>` — validates `JWT_SECRET` length ≥ 32, `argon2_m_cost ≥ 19456`.
  - `enum AppError { Unauthorized, Forbidden, NotFound, Conflict(String), Validation(Vec<FieldError>), RateLimited, Internal }` implementing `axum::response::IntoResponse` mapping to status + error envelope; `FieldError { field, message, code }`.
  - `ApiResponse<T>` / helpers in `response.rs`: `fn data<T: Serialize>(t: T) -> Json<Value>`, `fn data_paginated<T>(items, meta) -> Json<Value>`.

- [ ] **Step 1: Failing test** (`config.rs` tests):
```rust
#[test]
fn rejects_short_jwt_secret() {
    std::env::set_var("JWT_SECRET", "short");
    std::env::set_var("DATABASE_URL", "postgres://x");
    let err = AppConfig::from_env().unwrap_err();
    assert!(matches!(err, ConfigError::JwtSecretTooShort));
}
#[test]
fn error_maps_to_status() {
    use axum::response::IntoResponse;
    assert_eq!(AppError::Unauthorized.into_response().status(), axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(AppError::Forbidden.into_response().status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(AppError::NotFound.into_response().status(), axum::http::StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run (RED)** — `cargo test -p walk4change-api config` → fails to compile.

- [ ] **Step 3: Implement** `config.rs`, `error.rs`, `response.rs` per the Interfaces block. `AppError::Validation` → 422, `Conflict` → 409, `RateLimited` → 429, `Internal` → 500 (generic message, log detail). Error body: `{"error":{"code","message","details"?}}`. `ScoringConfig` lives in `scoring/config.rs` (Task 11) — for now define it there as a stub `#[derive(Clone)] pub struct ScoringConfig {...}` with `Default`, and have `AppConfig` embed it via `ScoringConfig::from_env_or_default()`. Add `pub mod config; pub mod error; pub mod response;` to `lib.rs`.

- [ ] **Step 4: Run (GREEN)** — `cargo test -p walk4change-api config` passes.

- [ ] **Step 5: Commit** — `git commit -am "feat(backend): config validation, AppError, response envelope"`

---

### Task 3: DB pool + migrations + test harness

**Files:**
- Create: `src/db.rs`, `src/state.rs`, `migrations/0001_init.sql`, `crates/api/tests/common/mod.rs`
- Modify: `src/lib.rs`, `src/main.rs` (build pool + migrate on boot)
- Test: `crates/api/tests/db_test.rs`

**Interfaces:**
- Produces:
  - `db::make_pool(url: &str) -> Result<sqlx::PgPool, sqlx::Error>` (max 10 conns)
  - `db::run_migrations(pool: &PgPool) -> Result<(), sqlx::Error>` via `sqlx::migrate!("../../migrations")`
  - `AppState { pool: PgPool, config: Arc<AppConfig>, hub: Hub }` (Hub stubbed until Task 14; use `hub: ()` placeholder now, widen in Task 14)
  - test helper `common::TestApp { pool, base_url, client }` with `common::spawn().await -> TestApp` that: connects to `TEST_DATABASE_URL`, runs migrations, truncates all tables, binds the app to an ephemeral port, returns base URL + reqwest client.

**`migrations/0001_init.sql`** (full DDL — copy verbatim, follows spec §5 + `received_at` refinement):
```sql
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS citext;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
  id uuid PRIMARY KEY,
  email citext UNIQUE NOT NULL,
  password_hash text NOT NULL,
  display_name text NOT NULL,
  avatar_url text,
  bio text,
  interests text[] NOT NULL DEFAULT '{}',
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE friendships (
  id uuid PRIMARY KEY,
  requester_id uuid NOT NULL REFERENCES users(id),
  addressee_id uuid NOT NULL REFERENCES users(id),
  status text NOT NULL CHECK (status IN ('pending','accepted')),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (requester_id, addressee_id),
  CHECK (requester_id <> addressee_id)
);

CREATE TABLE nature_zones (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  geom geography(Polygon,4326) NOT NULL,
  multiplier numeric NOT NULL DEFAULT 3.0,
  active boolean NOT NULL DEFAULT true
);
CREATE INDEX nature_zones_geom_idx ON nature_zones USING gist (geom);

CREATE TABLE walk_sessions (
  id uuid PRIMARY KEY,
  host_id uuid NOT NULL REFERENCES users(id),
  status text NOT NULL CHECK (status IN ('active','finished')),
  join_code text UNIQUE,
  started_at timestamptz NOT NULL DEFAULT now(),
  ended_at timestamptz
);

CREATE TABLE walk_participants (
  id uuid PRIMARY KEY,
  session_id uuid NOT NULL REFERENCES walk_sessions(id),
  user_id uuid NOT NULL REFERENCES users(id),
  joined_at timestamptz NOT NULL DEFAULT now(),
  left_at timestamptz,
  total_meters numeric NOT NULL DEFAULT 0,
  total_points numeric NOT NULL DEFAULT 0,
  UNIQUE (session_id, user_id)
);

CREATE TABLE location_pings (
  id uuid PRIMARY KEY,
  session_id uuid NOT NULL REFERENCES walk_sessions(id),
  user_id uuid NOT NULL REFERENCES users(id),
  geom geography(Point,4326) NOT NULL,
  recorded_at timestamptz NOT NULL,
  received_at timestamptz NOT NULL DEFAULT now(),
  seq integer NOT NULL,
  segment_meters numeric NOT NULL DEFAULT 0,
  companions integer NOT NULL DEFAULT 0,
  nature_mult numeric NOT NULL DEFAULT 1.0,
  together_mult numeric NOT NULL DEFAULT 1.0,
  points numeric NOT NULL DEFAULT 0,
  UNIQUE (session_id, user_id, seq)
);
CREATE INDEX location_pings_seq_idx ON location_pings (session_id, user_id, seq);
CREATE INDEX location_pings_recv_idx ON location_pings (session_id, received_at);

CREATE TABLE user_totals (
  user_id uuid PRIMARY KEY REFERENCES users(id),
  total_points numeric NOT NULL DEFAULT 0,
  spent_points numeric NOT NULL DEFAULT 0,
  total_meters numeric NOT NULL DEFAULT 0,
  total_walks integer NOT NULL DEFAULT 0,
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE rewards_catalog (
  id uuid PRIMARY KEY,
  title text NOT NULL,
  description text,
  cost_points numeric NOT NULL CHECK (cost_points >= 0),
  partner_name text,
  type text NOT NULL CHECK (type IN ('discount','eco','sponsor')),
  stock integer,
  image_url text,
  active boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE reward_redemptions (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES users(id),
  reward_id uuid NOT NULL REFERENCES rewards_catalog(id),
  points_spent numeric NOT NULL,
  code text NOT NULL,
  status text NOT NULL CHECK (status IN ('reserved','redeemed','expired')),
  created_at timestamptz NOT NULL DEFAULT now(),
  redeemed_at timestamptz
);
```

- [ ] **Step 1: Failing test** (`db_test.rs`, ignored if no DB):
```rust
#[tokio::test]
async fn migrations_apply_and_tables_exist() {
    let app = walk4change_api::test_support::spawn().await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&app.pool).await.unwrap();
    assert_eq!(n, 0);
}
```
(Expose `pub mod test_support` behind `#[cfg(any(test, feature = "test-support"))]` or a small `tests/common`.)

- [ ] **Step 2: Run (RED)** — start DB: `docker compose -f backend/docker-compose.yml up -d db` then `TEST_DATABASE_URL=... cargo test -p walk4change-api migrations_apply` → fails (no spawn).

- [ ] **Step 3: Implement** `db.rs`, `state.rs`, `common/mod.rs::spawn`, wire `main.rs` to build pool + migrate + serve. `spawn()` truncates all tables (`TRUNCATE users, friendships, ... RESTART IDENTITY CASCADE`) for isolation.

- [ ] **Step 4: Run (GREEN)** — test passes against Docker DB.

- [ ] **Step 5: Commit** — `git commit -am "feat(backend): pg pool, migrations, integration test harness"`

---

## Phase 1 — Auth

### Task 4: Password hashing (argon2id)

**Files:** Create `src/auth/mod.rs`, `src/auth/password.rs`. Modify `lib.rs`.
**Interfaces — Produces:**
- `password::hash(cfg: &AppConfig, plain: &str) -> Result<String, AppError>`
- `password::verify(hash: &str, plain: &str) -> bool` (returns false on any error, constant-ish)
- `password::DUMMY_HASH: &str` — a precomputed argon2id hash used to verify against for unknown users (enumeration defense).
- Boundary: callers enforce `plain.len() <= 128` before calling `hash`.

- [ ] **Step 1: Failing test** (`password.rs` tests):
```rust
#[test]
fn hash_then_verify_roundtrip() {
    let cfg = AppConfig::test_default();
    let h = hash(&cfg, "correct horse").unwrap();
    assert!(verify(&h, "correct horse"));
    assert!(!verify(&h, "wrong"));
}
#[test]
fn dummy_hash_verifies_false_but_runs() {
    assert!(!verify(DUMMY_HASH, "anything"));
}
```
- [ ] **Step 2: RED** — `cargo test -p walk4change-api password`.
- [ ] **Step 3: Implement** argon2id with `Params::new(cfg.argon2_m_cost, cfg.argon2_t_cost, cfg.argon2_p_cost, None)`. `DUMMY_HASH` = a `const` real hash string generated once (compute via a `#[test]` printer or hardcode a valid PHC string).
- [ ] **Step 4: GREEN.**
- [ ] **Step 5: Commit** — `git commit -am "feat(auth): argon2id password hashing + dummy hash"`

---

### Task 5: JWT encode/decode

**Files:** Create `src/auth/jwt.rs`.
**Interfaces — Produces:**
- `struct Claims { sub: Uuid, iat: i64, exp: i64 }`
- `jwt::encode(cfg, user_id: Uuid) -> Result<String, AppError>` (exp = now + cfg.jwt_ttl_secs)
- `jwt::decode(cfg, token: &str) -> Result<Claims, AppError>` — `Validation::new(Algorithm::HS256)`, rejects expired/invalid → `AppError::Unauthorized`.

- [ ] **Step 1: Failing test:**
```rust
#[test]
fn encode_then_decode_roundtrip() {
    let cfg = AppConfig::test_default();
    let id = Uuid::new_v4();
    let t = encode(&cfg, id).unwrap();
    assert_eq!(decode(&cfg, &t).unwrap().sub, id);
}
#[test]
fn rejects_tampered_token() {
    let cfg = AppConfig::test_default();
    let t = encode(&cfg, Uuid::new_v4()).unwrap();
    let bad = format!("{}x", t);
    assert!(decode(&cfg, &bad).is_err());
}
```
- [ ] **Step 2: RED.** **Step 3: Implement.** **Step 4: GREEN.**
- [ ] **Step 5: Commit** — `git commit -am "feat(auth): JWT HS256 encode/decode"`

---

### Task 6: AuthUser extractor

**Files:** Create `src/auth/extractor.rs`.
**Interfaces — Produces:**
- `struct AuthUser { pub id: Uuid }` implementing `axum::extract::FromRequestParts<AppState>`: reads `Authorization: Bearer <jwt>` (and, if absent, the `wc_session` cookie), `jwt::decode`, returns `AuthUser` or `AppError::Unauthorized`.

- [ ] **Step 1: Failing integration test** (`auth_test.rs`): a protected probe route returns 401 without token, 200 with a valid token. (Add a temporary `GET /api/v1/_whoami` returning `data(auth.id)` for the test; keep it — it's harmless and handy.)
- [ ] **Step 2: RED.** **Step 3: Implement** extractor + `_whoami` route. **Step 4: GREEN.**
- [ ] **Step 5: Commit** — `git commit -am "feat(auth): AuthUser bearer/cookie extractor"`

---

### Task 7: User repo + register/login/logout

**Files:** Create `src/repo/mod.rs`, `src/repo/user.rs`, `src/auth/handlers.rs`, `src/models/mod.rs`. Modify `routes/mod.rs`.
**Interfaces — Produces (repo):**
- `user::create(pool, id, email, password_hash, display_name) -> Result<(), AppError>` (also inserts a `user_totals` row in the same tx; maps unique-violation → `AppError::Conflict("email_taken")`)
- `user::find_by_email(pool, email) -> Result<Option<UserAuthRow>, AppError>` where `UserAuthRow { id: Uuid, password_hash: String }`
- `user::get_profile(pool, id) -> Result<Profile, AppError>`; `user::update_profile(pool, id, patch) -> Result<Profile, AppError>`
- `Profile { id, email, display_name, avatar_url, bio, interests, created_at }` (Serialize)

**Handlers:**
- `POST /api/v1/auth/register` body `{ email, password, display_name }` → 201 + `Location: /api/v1/me`, returns `{token, data: Profile}`. Validation: email contains `@`, `8 <= password.len() <= 128`, display_name non-empty.
- `POST /api/v1/auth/login` body `{ email, password }` → 200 `{token}`. **Always** run `verify` (use `DUMMY_HASH` when email unknown); uniform `AppError::Unauthorized` with message "invalid credentials".
- `POST /api/v1/auth/logout` → 204 (clears `wc_session` cookie via `Set-Cookie` max-age=0).

- [ ] **Step 1: Failing tests** (`auth_test.rs`): register→200ish + token; duplicate email→409; login wrong password→401; login unknown email→401 (and not distinguishable); register short password→422.
- [ ] **Step 2: RED.** **Step 3: Implement.** **Step 4: GREEN.**
- [ ] **Step 5: Commit** — `git commit -am "feat(auth): register/login/logout + user repo"`

---

### Task 8: Profile routes

**Files:** Create `src/routes/profile.rs`.
**Interfaces:** `GET /api/v1/me` (AuthUser) → `data(Profile)`. `PATCH /api/v1/me` body `{ display_name?, avatar_url?, bio?, interests? }` → updated `data(Profile)`. `avatar_url` stored as-is, never fetched server-side.
- [ ] Steps: failing test (get me; patch updates display_name; 401 without token) → RED → implement → GREEN → commit `feat(profile): GET/PATCH /me`.

---

### Task 9: Assemble `build_app` + AppState wiring

**Files:** Modify `lib.rs` (add `pub fn build_app(state: AppState) -> Router` combining health, auth, profile routers with `tower_http` Trace + a `DefaultBodyLimit`), `main.rs` (use `build_app`). Update `common::spawn` to use `build_app`.
**Interfaces — Produces:** `build_app(state) -> Router`.
- [ ] Steps: adapt existing tests to `build_app` → ensure all green → commit `refactor(backend): unified build_app + app state`.

---

## Phase 2 — Friends

### Task 10: Friend repo + routes (direction-agnostic)

**Files:** Create `src/repo/friend.rs`, `src/routes/friends.rs`.
**Interfaces — Produces:**
- `friend::send_request(pool, id, requester, addressee) -> Result<(), AppError>` — reject self (422), reject if any row exists in EITHER direction (409), insert `pending`.
- `friend::respond(pool, request_id, actor, accept) -> Result<(), AppError>` — `UPDATE friendships SET status=... WHERE id=$1 AND addressee_id=$actor AND status='pending'`; 0 rows → `AppError::Forbidden` (or NotFound) — pick `Forbidden`.
- `friend::are_friends(pool, a, b) -> Result<bool, AppError>` — `status='accepted'` in either direction. (Consumed by Walks join.)
- `friend::list(pool, actor) -> Result<FriendsList, AppError>` → `{ accepted: Vec<Profile>, incoming_pending: Vec<...>, outgoing_pending: Vec<...> }`
- Routes: `POST /friends/request {addressee_id}` 201; `POST /friends/respond {request_id, accept}` 200; `GET /friends` 200.

- [ ] **Step 1: Failing tests** (`friends_test.rs`): A requests B → B sees incoming pending; B accepts → both `are_friends`; B→A duplicate request 409; respond by non-addressee → 403; self request → 422.
- [ ] Steps RED→implement→GREEN→commit `feat(friends): requests, accept, direction-agnostic friendship`.

---

## Phase 3 — Walks

### Task 11: ScoringConfig + scoring engine (pure, unit-tested)

**Files:** Create `src/scoring/mod.rs`, `src/scoring/config.rs`, `src/scoring/engine.rs`. (Replace the Task 2 stub `ScoringConfig`.)
**Interfaces — Produces:**
- `ScoringConfig { meters_per_point: Decimal, mult_solo: Decimal, mult_pair: Decimal, mult_group: Decimal, stack: bool, max_speed_mps: f64, ping_window_secs: i64, recorded_at_tolerance_secs: i64, max_points_per_second: Decimal }` + `from_env_or_default()`.
- `together_mult(cfg, companions: i32) -> Decimal` — 0→solo, 1→pair, ≥2→group.
- `struct SpatialInput { segment_meters: f64, dt_secs: f64, nature_mult: Decimal, companions: i32 }`
- `score_segment(cfg, &SpatialInput) -> ScoredSegment` where `ScoredSegment { effective_meters: Decimal, together_mult: Decimal, points: Decimal }`. Logic: if `dt_secs <= 0` OR `segment_meters/dt_secs > max_speed_mps` → effective_meters = 0. `points = (effective_meters / meters_per_point) * nature_mult * together_mult` (if `stack==false`, use `max(nature_mult, together_mult)` instead of product). Caller applies the per-second ceiling separately (Task 13).

- [ ] **Step 1: Failing unit tests** (`engine.rs`): solo flat ground (1 companion=0) baseline points; nature 3× triples; pair 1.5×; group (2 companions) 2×; stack product (nature×together); teleport (speed over cap) → 0 points; dt<=0 → 0; companions mapping boundaries.
- [ ] Steps RED→implement→GREEN→commit `feat(scoring): pure scoring engine + config`.

---

### Task 12: Walk repo + start/join/leave/stop/get/track routes

**Files:** Create `src/repo/walk.rs`, `src/routes/walks.rs`.
**Interfaces — Produces (repo):**
- `walk::start(pool, session_id, host_id) -> Result<WalkSession, AppError>` — insert session `active` + random `join_code` (8-char base32) + host participant row.
- `walk::join(pool, session_id, actor) -> Result<(), AppError>` — must be `active` (404 else); `friend::are_friends(host, actor)` false → 403; insert participant (409 if already, via unique).
- `walk::leave(pool, session_id, actor) -> Result<(), AppError>` — `UPDATE walk_participants SET left_at=now() WHERE session_id=$ AND user_id=$actor AND left_at IS NULL`.
- `walk::stop(pool, session_id, actor) -> Result<(), AppError>` — host only (`WHERE id=$ AND host_id=$actor`); set `finished`, `ended_at`; in same tx set `left_at` for open participants and bump `user_totals.total_walks` for each participant.
- `walk::get(pool, session_id, actor) -> Result<WalkDetail, AppError>` — member-only (WHERE EXISTS participant actor); returns session + participants.
- `walk::track(pool, session_id, actor, limit) -> Result<Vec<PingPoint>, AppError>` — member-only; pings ordered by seq; `PingPoint { user_id, seq, lat, lng, points, recorded_at }` read via `ST_Y/ST_X`.
- `walk::is_active_participant(pool, session_id, actor) -> Result<bool, AppError>` (consumed by WS ping authz).
- `walk::is_member(pool, session_id, actor) -> Result<bool, AppError>` (consumed by WS subscribe authz).

**Routes:** `POST /walks` 201 + Location; `POST /walks/:id/join` 200; `POST /walks/:id/leave` 204; `POST /walks/:id/stop` 204; `GET /walks/:id` 200; `GET /walks/:id/track` 200.

- [ ] **Step 1: Failing tests** (`walks_test.rs`): host starts → get returns 1 participant; non-friend join → 403; friend join → 200 & appears in participants; non-member get → 403; leave sets left_at; non-host stop → 403; host stop → finished + total_walks bumped.
- [ ] Steps RED→implement→GREEN→commit `feat(walks): sessions start/join/leave/stop/get/track`.

---

### Task 13: Scoring repo — `score_ping` transaction (PostGIS)

**Files:** Create `src/scoring/repo.rs`.
**Interfaces — Produces:**
- `struct PingInput { session_id: Uuid, user_id: Uuid, seq: i32, lat: f64, lng: f64, recorded_at: DateTime<Utc> }`
- `struct PingScore { seq: i32, lat: f64, lng: f64, segment_meters: Decimal, companions: i32, nature_mult: Decimal, together_mult: Decimal, points: Decimal, participant_total: Decimal }`
- `score_ping(pool, cfg: &ScoringConfig, input: PingInput) -> Result<Option<PingScore>, AppError>` — one transaction:
  1. Validate coords + `recorded_at` within `±tolerance` of `now()` (else `AppError::Validation`).
  2. `prev` = last ping for (session,user) by seq (geom + received_at).
  3. `segment_meters` = `ST_Distance(prev.geom, new_point)` (0 if no prev). `dt_secs` = now - prev.received_at (or large if no prev → not teleport).
  4. `nature_mult` = `SELECT multiplier FROM nature_zones WHERE active AND ST_Covers(geom, $point) ORDER BY multiplier DESC LIMIT 1` (default 1.0).
  5. `companions` = `SELECT count(DISTINCT user_id) FROM location_pings WHERE session_id=$ AND user_id<>$actor AND received_at > now() - ($window || ' seconds')::interval`.
  6. `engine::score_segment(...)`; apply per-second ceiling: clamp `points` so the user's points in the trailing 1s window don't exceed `max_points_per_second` (query sum of points where received_at > now()-1s; clamp).
  7. INSERT ping with `ON CONFLICT (session_id,user_id,seq) DO NOTHING` — if 0 rows, return `Ok(None)` (idempotent dup).
  8. `UPDATE walk_participants SET total_meters+=, total_points+= ... RETURNING total_points`.
  9. `INSERT INTO user_totals ... ON CONFLICT (user_id) DO UPDATE SET total_points=user_totals.total_points+$, total_meters=..., updated_at=now()`.
  10. Return `PingScore`.

- [ ] **Step 1: Failing integration tests** (`scoring_test.rs`, PostGIS): seed a nature zone polygon; first ping → 0 points (no prev); second ping ~100m away inside zone within ~1s → points = (100/100)*3*solo; teleport (100km in 1s) → 0; duplicate seq → no double count; two participants pinging within window → companions≥1 → together mult applies.
- [ ] Steps RED→implement→GREEN→commit `feat(scoring): score_ping PostGIS transaction`.

---

## Phase 4 — WebSocket

### Task 14: Hub + protocol

**Files:** Create `src/ws/mod.rs`, `src/ws/hub.rs`, `src/ws/protocol.rs`. Widen `AppState.hub` to `Hub`.
**Interfaces — Produces:**
- `protocol`: `enum ClientFrame { Auth{token}, Ping{session_id,seq,lat,lng,recorded_at}, Subscribe{session_id}, SubscribeLeaderboard }` (serde tag="type", rename_all snake_case); `enum ServerFrame { PingScored{data}, LeaderboardUpdate{data}, SessionEvent{data}, Error{error} }`.
- `Hub { ... }` (Clone): `session_sender(session_id) -> broadcast::Sender<ServerFrame>` (create-on-demand, reap when no receivers); `leaderboard_sender() -> broadcast::Sender<ServerFrame>`; `publish_session(session_id, frame)`; `publish_leaderboard(frame)`.

- [ ] **Step 1: Failing unit test** (`hub.rs`): subscribe to a session, publish, receive the frame; second subscriber also receives.
- [ ] Steps RED→implement→GREEN→commit `feat(ws): broadcast hub + frame protocol`.

---

### Task 15: WS handler (auth-first-frame, ping→score→broadcast, subscribe)

**Files:** Create `src/ws/handler.rs`. Add route `GET /api/v1/ws`.
**Interfaces:** Upgrade → spawn loop. First frame must be `Auth` within `ws_auth_timeout` (10s) else close. After auth: `Ping` → authz `walk::is_active_participant` → `scoring::score_ping` → on `Some`, `hub.publish_session(PingScored)` + throttled `hub.publish_leaderboard`. `Subscribe` → authz `walk::is_member` → forward that session's broadcast to this socket. `SubscribeLeaderboard` → forward leaderboard broadcast. Enforce max frame size; handle `broadcast::error::RecvError::Lagged` by skipping.

- [ ] **Step 1: Failing integration test** (`ws_test.rs`): connect, send non-auth first → closed; auth then ping into own active session → receive `ping_scored`; subscribe as member → receive another participant's `ping_scored`; subscribe to non-member session → `error` frame.
- [ ] Steps RED→implement→GREEN→commit `feat(ws): live ping ingestion + scoring + broadcast`.

---

## Phase 5 — Leaderboard & Rewards

### Task 16: Leaderboard (offset pagination)

**Files:** Create `src/routes/leaderboard.rs`, `src/util/pagination.rs`.
**Interfaces:** `GET /api/v1/leaderboard?page=&per_page=` (AuthUser) → `data` = `[{user_id, display_name, total_points}]` ordered by `total_points DESC`, joined to `users` for name; `meta {total,page,per_page,total_pages}`. `per_page` default 20, **cap 100** (422 above). `Pagination::from_query(page, per_page) -> Result<Pagination, AppError>`.
- [ ] Steps: failing test (seed 3 users with totals; page 1 per_page 2 returns 2 ordered desc + meta.total=3; per_page=1000 → 422) → RED → implement → GREEN → commit `feat(leaderboard): paginated standings`.

---

### Task 17: Reward repo + catalog + atomic redeem

**Files:** Create `src/repo/reward.rs`, `src/routes/rewards.rs`.
**Interfaces — Produces:**
- `reward::list(pool) -> Result<Vec<Reward>, AppError>` (active only).
- `reward::redeem(pool, reward_id, actor) -> Result<Redemption, AppError>` — transaction, lock order `user_totals` then `rewards_catalog`:
  1. `SELECT total_points, spent_points FROM user_totals WHERE user_id=$actor FOR UPDATE`.
  2. `UPDATE rewards_catalog SET stock = stock - 1 WHERE id=$reward AND active AND (stock IS NULL OR stock > 0) RETURNING cost_points, title, type` — 0 rows → `AppError::Conflict("unavailable")`.
  3. balance = total-spent; if `< cost_points` → `AppError::Conflict("insufficient_points")` (tx rolls back, restoring stock).
  4. `code` = 16 random bytes (`rand::rngs::OsRng`) base32-encoded.
  5. INSERT `reward_redemptions` (`reserved`); `UPDATE user_totals SET spent_points += cost_points`.
  6. Return `Redemption { id, reward_id, code, points_spent, status, created_at }`.
- `reward::list_redemptions(pool, actor) -> Result<Vec<Redemption>, AppError>` (WHERE user_id=$actor).
- Routes: `GET /rewards` 200; `POST /rewards/:id/redeem` 201 + Location; `GET /me/redemptions` 200.

- [ ] **Step 1: Failing tests** (`rewards_test.rs`): seed reward cost 50 stock 1; user with 100 points redeems → 201, code present, spent_points=50; second redeem of same stock-1 reward → 409 unavailable; user with 10 points → 409 insufficient & stock unchanged; redemptions list shows the redemption. Add a concurrency test: two tasks redeem the stock-1 reward simultaneously → exactly one 201, one 409, stock ends at 0 (never -1).
- [ ] Steps RED→implement→GREEN→commit `feat(rewards): catalog + atomic redeem (no oversell)`.

---

## Phase 6 — Hardening

### Task 18: Rate limiting + security headers + CORS + body limit

**Files:** Create `src/util/ratelimit.rs`. Modify `lib.rs` (layer stack).
**Interfaces — Produces:** a simple in-memory per-IP (and per-user where available) sliding-window limiter as a `tower` layer: strict bucket for `/auth/*` (e.g., 10/min), moderate global (e.g., 120/min). Security headers layer: HSTS, `X-Content-Type-Options: nosniff`, `Content-Security-Policy: frame-ancestors 'none'`, `Referrer-Policy`. CORS from `cfg.cors_origins` (exact list, credentials true). `DefaultBodyLimit` (e.g., 64 KiB).
- [ ] **Step 1: Failing test:** hammer `/auth/login` past the bucket → eventually 429 with `Retry-After`; response carries `X-Content-Type-Options: nosniff`.
- [ ] Steps RED→implement→GREEN→commit `feat(security): rate limiting, headers, CORS, body limit`.

---

## Phase 7 — Demo

### Task 19: Seed bin (demo users, friendships, zones, rewards)

**Files:** Create `crates/api/src/bin/seed.rs`, `fixtures/nature_zones.geojson`.
**Interfaces:** `cargo run -p walk4change-api --bin seed` — idempotent: creates demo users (passwords from `SEED_PASSWORD` env or random printed once), makes them friends, inserts a Baltic-coast nature-zone polygon, and a few `rewards_catalog` rows (discount/eco/sponsor). Prints demo user ids + tokens.
- [ ] **Step 1: Failing test:** an integration test calls the seed function (`seed::run(&pool, &cfg)`) then asserts ≥2 users, an accepted friendship, ≥1 active nature_zone, ≥1 reward. Keep seed logic in a `pub fn run` so it's testable; the bin is a thin wrapper.
- [ ] Steps RED→implement→GREEN→commit `feat(seed): demo data seeder`.

---

### Task 20: Replay bin (real-time track streaming over WS)

**Files:** Create `crates/replay/Cargo.toml`, `crates/replay/src/main.rs`, `fixtures/track_a.json`, `fixtures/track_b.json`.
**Interfaces:** `cargo run -p walk4change-replay -- --base ws://localhost:8080 --token <jwt> --session <id> --track fixtures/track_a.json`. Reads `[{lat,lng,offset_ms}]`, opens WS, sends `Auth`, starts a walk session (or uses provided), streams `Ping` frames **honoring `offset_ms` in real time** (~1/sec), incrementing `seq`. A `--with-friend` mode launches two tracks concurrently against two demo tokens to demonstrate the together multiplier.
- [ ] **Step 1: End-to-end smoke** (manual + a gated integration test): boot app + seed; run replay for two friends concurrently; assert via `GET /walks/:id/track` that points accrued and at least some pings have `together_mult > 1`. This is the mandatory pre-stage check (validates the §6 server-clock decision).
- [ ] Steps RED→implement→GREEN→commit `feat(replay): real-time track replay client + fixtures`.

---

## Final verification

- [ ] `cargo build --workspace` clean; `cargo clippy --workspace` no warnings (add `#![deny(warnings)]`? no — keep clippy advisory).
- [ ] `cargo test --workspace` green with Docker DB up (`TEST_DATABASE_URL` set).
- [ ] Two-track replay produces nature + together multipliers end-to-end.
- [ ] Update `backend/README.md` (how to run: docker compose up db, migrate, seed, run, replay).
- [ ] Final commit `docs(backend): README run instructions`.

## Notes for executors
- If `cargo`/Docker is unavailable, still implement to spec; mark DB-dependent tests `#[ignore]` with a reason and ensure `cargo build` + pure unit tests pass.
- Follow the spec for any detail not spelled out here; the spec is the source of truth.
- Keep files <400 lines; split if a module grows.
