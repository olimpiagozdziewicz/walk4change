# walk4change — Backend Design Spec

**Date:** 2026-06-27
**Owner:** Kamil (backend only — everything DB-touching)
**Context:** Hackathon. Frontend built by teammates. Supabase project exists, used **only as a Postgres host**.

---

## 1. Goal

Backend for an app that nudges people to walk — in nature and with others — via live, gamified point scoring. A walk is streamed live; the server scores it (distance × nature × together multipliers), pushes live updates to a map and leaderboard, and lets users spend earned points on rewards.

Success metric the system serves: *people physically out walking, together* — not app opens. Scoring rewards being in nature and walking with friends.

## 2. Scope

### In scope (MVP)
- Auth (email + password, JWT) — owned by the Rust service.
- User profiles (incl. `interests` field stored for future matching).
- Friends list (request → accept).
- Walk sessions — solo or shared; shared join gated to friends of the host.
- Live GPS ping streaming → server-side scoring → live totals.
- Multipliers: nature 3× (PostGIS zones), together 1.5× (1 friend) / 2× (group ≥3), stacked, tunable.
- Live leaderboard.
- Rewards: catalog + atomic point redemption (discount / eco / sponsor).
- Demo replay path (stream recorded tracks on stage).

### Out of scope (future — schema leaves room)
Corporate teams, eco-action events, place comments/observations, photo uploads, push notifications, interest-based matching, OAuth/refresh tokens, RLS defense-in-depth.

## 3. Architecture — "Full C" (least vendor lock-in)

A **standalone Rust service** (Axum + Tokio + sqlx) is the entire backend. The frontend talks **only** to it: REST for commands, one WebSocket for live streaming/push. It connects to **plain Postgres + PostGIS**. Migrating off Supabase = change `DATABASE_URL`.

```
                 ┌──────────────────────────────────────────┐
   frontend ───► │  Rust service (Axum)                      │
   (REST + WS)   │   handlers ─► services ─► repositories     │ ──► Postgres + PostGIS
                 │   auth (argon2 + JWT)                      │     (Supabase = just a DB)
                 │   scoring engine                          │
                 │   ws hub (tokio::broadcast)               │
                 └──────────────────────────────────────────┘
   replay bin ──► (logs in demo users, streams recorded tracks over WS)
```

**Consequences of full ownership:**
- Auth in Rust: email + password (argon2id) → JWT (HS256). JWT middleware extracts the user.
- **No Supabase Auth, no Supabase Realtime, no RLS.** All authorization lives in Rust handlers — the primary risk of this approach, mitigated by per-handler ownership checks + tests.
- Migrations via `sqlx migrate` (in-repo, portable). PostGIS enabled as an extension.

## 4. Tech stack

Rust, `axum`, `tokio`, `sqlx` (postgres, compile-time-checked queries), `serde`, `jsonwebtoken`, `argon2`, `uuid`, `chrono`, `tracing` + `tracing-subscriber`, `tower-http` (CORS, trace, rate limit), `validator` (or manual boundary validation). Spatial via raw PostGIS SQL (`ST_Distance`, `ST_Contains`, `geography`). Replay client uses `tokio-tungstenite`. Scoring tunables in a Rust config (env/TOML), not a DB table.

## 5. Data model

```sql
users (
  id uuid PK default gen_random_uuid(),
  email citext UNIQUE NOT NULL,
  password_hash text NOT NULL,
  display_name text NOT NULL,
  avatar_url text,
  bio text,
  interests text[] NOT NULL default '{}',
  created_at timestamptz NOT NULL default now()
)

friendships (
  id uuid PK,
  requester_id uuid NOT NULL REFERENCES users(id),
  addressee_id uuid NOT NULL REFERENCES users(id),
  status text NOT NULL CHECK (status IN ('pending','accepted')),
  created_at timestamptz NOT NULL default now(),
  UNIQUE (requester_id, addressee_id),
  CHECK (requester_id <> addressee_id)
)
-- NOTE: UNIQUE does not stop reciprocal rows (A→B and B→A). The friends-only join
-- check and friend listing MUST be direction-agnostic: match (requester,addressee)
-- in EITHER order with status='accepted'. Enforce one-pending-pair in app logic.

nature_zones (
  id uuid PK,
  name text NOT NULL,
  geom geography(Polygon,4326) NOT NULL,
  multiplier numeric NOT NULL default 3.0,
  active boolean NOT NULL default true
)
-- GIST index on geom

walk_sessions (
  id uuid PK,
  host_id uuid NOT NULL REFERENCES users(id),
  status text NOT NULL CHECK (status IN ('active','finished')),
  join_code text UNIQUE,
  started_at timestamptz NOT NULL default now(),
  ended_at timestamptz
)

walk_participants (
  id uuid PK,
  session_id uuid NOT NULL REFERENCES walk_sessions(id),
  user_id uuid NOT NULL REFERENCES users(id),
  joined_at timestamptz NOT NULL default now(),
  left_at timestamptz,
  total_meters numeric NOT NULL default 0,
  total_points numeric NOT NULL default 0,
  UNIQUE (session_id, user_id)
)

location_pings (
  id uuid PK,
  session_id uuid NOT NULL REFERENCES walk_sessions(id),
  user_id uuid NOT NULL REFERENCES users(id),
  geom geography(Point,4326) NOT NULL,
  recorded_at timestamptz NOT NULL,
  seq integer NOT NULL,
  -- filled by scoring:
  segment_meters numeric NOT NULL default 0,
  companions integer NOT NULL default 0,
  nature_mult numeric NOT NULL default 1.0,
  together_mult numeric NOT NULL default 1.0,
  points numeric NOT NULL default 0,
  UNIQUE (session_id, user_id, seq)
)
-- index (session_id, user_id, seq)        -- previous-ping lookup, ordering, idempotent insert
-- index (session_id, recorded_at)          -- companion-count query (across all users in session)

user_totals (
  user_id uuid PK REFERENCES users(id),
  total_points numeric NOT NULL default 0,
  spent_points numeric NOT NULL default 0,
  total_meters numeric NOT NULL default 0,
  total_walks integer NOT NULL default 0,
  updated_at timestamptz NOT NULL default now()
)
-- balance = total_points - spent_points

rewards_catalog (
  id uuid PK,
  title text NOT NULL,
  description text,
  cost_points numeric NOT NULL CHECK (cost_points >= 0),
  partner_name text,
  type text NOT NULL CHECK (type IN ('discount','eco','sponsor')),
  stock integer,                       -- NULL = unlimited
  image_url text,
  active boolean NOT NULL default true,
  created_at timestamptz NOT NULL default now()
)

reward_redemptions (
  id uuid PK,
  user_id uuid NOT NULL REFERENCES users(id),
  reward_id uuid NOT NULL REFERENCES rewards_catalog(id),
  points_spent numeric NOT NULL,
  code text NOT NULL,
  status text NOT NULL CHECK (status IN ('reserved','redeemed','expired')),
  created_at timestamptz NOT NULL default now(),
  redeemed_at timestamptz
)
```

A **solo walk** = a session with one participant. The host's `walk_sessions` row plus a `walk_participants` row.

## 6. Scoring engine

Pings arrive over WS. For each ping (user U, session S), one DB transaction:

1. **Validate (Rust, boundary):** lat ∈ [-90,90], lng ∈ [-180,180]; `recorded_at` monotonic per (S,U) and within `±recorded_at_tolerance_s` of server time (used for ordering only, never as the scoring clock); `seq` is client-supplied and monotonic.
2. **Spatial read (PostGIS):** `segment_meters = ST_Distance(point, previous ping of (S,U) ordered by seq)`; `nature_mult` = multiplier of the active `nature_zones` polygon covering the point via **`ST_Covers(zone.geom, point)`** (geography-compatible; `ST_Contains`/`ST_Within` do **not** accept `geography`), else `1.0`; `companions` = count of OTHER participants of S whose last server-received ping is within `ping_window_s` of now.
3. **Apply config (Rust) — clock = server receive time, NOT client `recorded_at`:**
   - `Δt` = server-clock gap since this user's previous ping (guard `Δt <= 0`). Speed cap: if `segment_meters / Δt > max_speed_mps`, drop the segment (distance → 0) — anti-teleport. *Rationale: a client-supplied Δt lets an attacker fake a huge interval in one real second → unbounded points → unlimited real-value reward codes. Server clock can't be forged.*
   - `together_mult`: 0 companions → 1.0; 1 → 1.5; ≥2 → 2.0.
   - `points = (segment_meters / meters_per_point) × nature_mult × together_mult` (multiplicative; `stack` toggle).
   - **Per-second ceiling:** cap points awarded per real wall-clock second per user (`max_points_per_second`) as a second, independent anti-fraud layer.
4. **Write (idempotent):** insert ping with scoring columns using `ON CONFLICT (session_id,user_id,seq) DO NOTHING` (dedups reconnect retransmits); increment `walk_participants.total_meters/total_points`; upsert `user_totals` (`total_points`, `total_meters`). `total_walks` bumps for **all** participants when the session finishes.
5. **Broadcast:** publish a `ping_scored` frame to the session channel; publish `leaderboard_update` to the leaderboard channel **throttled to ≤2/sec** (debounced, not per-ping). Handle `tokio::broadcast` `Lagged` explicitly (skip, don't treat as fatal).

**Config (env/TOML):** `meters_per_point` (default 100), `mult_solo=1.0`, `mult_pair=1.5`, `mult_group=2.0`, `nature_default=3.0`, `stack=true`, `max_speed_mps`, `ping_window_s`, `recorded_at_tolerance_s` (default 30–60), `max_points_per_second`.

**Honesty note:** streamed GPS is still spoofable (a client can emit plausible fake tracks at walking speed); the speed cap + per-second ceiling bound the damage but do not eliminate cheating. Not framed as tamper-proof. The "together" multiplier verifies session co-membership + temporal activity, **not** physical proximity — do not market it as "physically together."

## 7. Live push — WebSocket

One WS endpoint `GET /api/v1/ws`. **Auth via the first frame only** — the JWT is **never** passed in the query string (query params leak into access/proxy logs = session takeover). Any non-`auth` frame received before a valid `auth` frame → close. A connection that sends no valid `auth` frame within `ws_auth_timeout_s` (10–30s) is closed (pre-auth exhaustion defense). Internally a hub of `tokio::broadcast` channels: one per `session_id` + one global leaderboard; empty per-session channels are reaped.

**Client → server frames:**
- `{ "type": "auth", "token": "<jwt>" }`  (must be first)
- `{ "type": "ping", "session_id", "seq", "lat", "lng", "recorded_at" }`  (`seq` client-supplied, monotonic)
- `{ "type": "subscribe", "session_id" }`
- `{ "type": "subscribe_leaderboard" }`

**Server → client frames** (envelope-consistent):
- `{ "type": "ping_scored", "data": { session_id, user_id, seq, point: {lat,lng}, segment_meters, nature_mult, together_mult, points, participant_total } }`
- `{ "type": "leaderboard_update", "data": [ { user_id, display_name, total_points } ] }`
- `{ "type": "session_event", "data": { session_id, event } }`  (join/leave/stop)
- `{ "type": "error", "error": { code, message } }`

**Authorization on every frame (no RLS — enforced in SQL WHERE):** a `ping` is accepted only when a query confirms `walk_participants(session_id,user_id=actor,left_at IS NULL)` AND `walk_sessions.status='active'`; `subscribe {session_id}` only for a current session member. When a participant sets `left_at`, their session subscription is revoked (stop forwarding that session's frames). **Limits:** max frame size; per-IP cap on concurrent unauthenticated connections; per-user cap on concurrent connections. **Token lifetime vs. long walks:** WS is authenticated at the `auth` frame; the connection's lifetime is not re-bound to JWT `exp` (documented deviation) — client may send a fresh `auth` frame to re-auth without reconnecting.

## 8. REST API

All under `/api/v1/`. Success → `{ "data": ..., "meta"?, "links"? }`. Error → `{ "error": { "code", "message", "details"? } }`.

| Method | Path | Auth | Success | Notes |
|---|---|---|---|---|
| POST | `/auth/register` | public | 201 + `Location: /me` | email, password, display_name |
| POST | `/auth/login` | public | 200 `{token}` | rate-limited; uniform error; constant-time |
| POST | `/auth/logout` | yes | 204 | clears cookie (web); no-op for Bearer |
| GET | `/me` | yes | 200 | own profile |
| PATCH | `/me` | yes | 200 | display_name, avatar_url, bio, interests |
| POST | `/friends/request` | yes | 201 | body: addressee_id; 409 if exists |
| POST | `/friends/respond` | yes | 200 | body: request_id, accept; 403 if not addressee |
| GET | `/friends` | yes | 200 | accepted friends + pending |
| POST | `/walks` | yes | 201 + `Location` | starts session, returns join_code |
| POST | `/walks/:id/join` | yes | 200 | 403 unless friend of host; 409 if already in |
| POST | `/walks/:id/leave` | yes | 204 | own membership |
| POST | `/walks/:id/stop` | yes | 204 | host only |
| GET | `/walks/:id` | yes | 200 | session + participants; members only |
| GET | `/walks/:id/track` | yes | 200 | pings for the live map; members only |
| GET | `/leaderboard` | yes | 200 + `meta` | offset pagination `?page=&per_page=` |
| GET | `/rewards` | yes | 200 | active catalog |
| POST | `/rewards/:id/redeem` | yes | 201 + `Location` | atomic; 409 if insufficient balance/stock |
| GET | `/me/redemptions` | yes | 200 | own redemptions |
| GET | `/ws` | yes | 101 | WebSocket upgrade |

**Status codes:** 401 (missing/invalid token) vs 403 (not your resource) vs 404; 409 (duplicate/conflict/insufficient); 422 (validation); 429 (rate limit) with `Retry-After` + `X-RateLimit-*`.

## 9. Rewards flow

`POST /rewards/:id/redeem` runs one atomic transaction. **Consistent lock order: `user_totals` then `rewards_catalog`** (deadlock avoidance across all txns touching both).
1. `SELECT ... FOR UPDATE` the user's `user_totals` row; compute `balance = total_points - spent_points`.
2. Atomically claim stock + verify active: `UPDATE rewards_catalog SET stock = stock - 1 WHERE id=$id AND active AND (stock IS NULL OR stock > 0) RETURNING ...`. **Zero rows → 409** (sold out / inactive). This closes the stock-oversell race (two concurrent redeems cannot both decrement past 0).
3. Verify `balance >= cost_points` (else 409, rollback).
4. Insert `reward_redemptions` (status `reserved`, `code` = CSPRNG ≥128-bit, base32/58-encoded — never sequential/guessable); add `cost_points` to `spent_points`.
5. Return the redemption (with code). Any failure → 409 with reason, **no partial write**.

Types: `discount` (partner code, "collect on site" near beaches — ties to local-business model), `eco` (plant a tree / adopt a seal), `sponsor` (e.g., cinema ticket). Codes map to **real-value inventory**, so the atomicity + entropy requirements above are mandatory, not optional. A partner-side code-verification endpoint (rate-limited) is future scope.

## 10. Auth & security

**Secrets**
- Only in env: `DATABASE_URL`, `JWT_SECRET`. At startup, validate **presence AND `JWT_SECRET` ≥ 32 bytes entropy** (fail fast). `.env` gitignored; `.env.example` committed. Demo seed passwords randomized per-deploy or from env — never hardcoded.

**Passwords (argon2id)**
- Explicit params in config, validated ≥ OWASP minimums at startup (`m_cost ≥ 19456 KiB`, `t_cost ≥ 2`, `p=1`).
- **Max password length enforced at the boundary (128 chars)** before hashing — unbounded input is a CPU/memory DoS.

**JWT**
- HS256 with `sub`, `iat`, `exp`. **Explicit `exp`** (≤8h web / ≤24h native). Validation pins `Algorithm::HS256` — reject `alg:none` / alg-confusion.
- `POST /auth/logout` (clears the cookie on web; for native, expiry is the sole control — documented).

**Authorization (no RLS — the central risk)**
- A typed `AuthUser` Axum extractor on **every** authenticated handler. Repo methods on owned resources take an `actor_id` param.
- The ownership/membership check is a **SQL `WHERE` predicate in the query itself**, never a read-then-compare (TOCTOU + refactor-fragile). Applies to: `GET /walks/:id`, `/walks/:id/track` (member), `/walks/:id/leave` (own row), `/walks/:id/stop` (host), `/friends/respond` (addressee), `/me/redemptions` (own), WS `ping` (own active participant), WS `subscribe` (member).
- **Login enumeration:** always run argon2 verify (against a dummy hash for unknown emails); uniform "invalid credentials" message; same rate limit regardless of email existence.

**Input validation (boundary)**
- Parameterized queries only (sqlx). Coords lat ∈ [-90,90] / lng ∈ [-180,180]; `recorded_at` monotonic per (session,user) and ≤ `server_time + tolerance` (reject future); `per_page` ≤ 100 (422 above); email format; password policy.
- `avatar_url` is **display-only — server never fetches it** (SSRF guard).

**Transport / headers / limits**
- Generic client errors; detail only in server logs. **Never log** GPS coords, tokens, or password material.
- Rate limiting: strict on `/auth/*`; moderate on ping ingestion; **moderate blanket per-IP on all endpoints** (incl. `/friends/request`, `/walks/:id/join`, `/leaderboard`). `429` + `Retry-After`.
- Security headers via `tower-http`: HSTS, `X-Content-Type-Options: nosniff`, `frame-ancestors 'none'`, `Referrer-Policy: strict-origin-when-cross-origin`. CORS = exact origin allowlist (`allow_credentials` only with specific origins, never `*`). HTTPS at host.
- **Token transport:** native → `Authorization: Bearer` (CSRF-immune); web → httpOnly + `SameSite=Strict` cookie + double-submit CSRF token on state-changing requests.
- **WS:** auth via first frame only (never query string); pre-auth idle close; max frame size; per-IP unauth-conn cap; per-user conn cap.

**GPS data sharing (consent)**
- `ping_scored` and `/walks/:id/track` expose precise participant GPS to all session members — an intended feature; flag as an explicit consent decision for the frontend/UX. Subscription is revoked when a participant sets `left_at`.

## 11. Error handling & response shape

Central `AppError` enum (e.g., `Unauthorized`, `Forbidden`, `NotFound`, `Conflict`, `Validation(details)`, `RateLimited`, `Internal`) with one `IntoResponse` impl mapping to status + error envelope. Handlers return `Result<Json<...>, AppError>`. Unexpected errors → `500` generic message + logged with `request_id`.

## 12. Project layout (many small files, <400 lines each)

```
backend/                      cargo workspace
  crates/api/src/
    main.rs                   bootstrap, router, listener
    config.rs                 env/TOML config, startup validation
    error.rs                  AppError + IntoResponse
    db.rs                     pool, migrations runner
    auth/{jwt,password,middleware,handlers}.rs
    routes/{profile,friends,walks,rewards,leaderboard}.rs
    ws/{hub,protocol,handler}.rs
    scoring/{engine,config}.rs
    repo/{user,friend,walk,reward}.rs    repository traits + sqlx impls
    models/*.rs               domain + DTO types
  crates/replay/              bin: replay recorded tracks over WS
  migrations/                 sqlx migrations (schema + indexes)
  fixtures/                   recorded GPS tracks + Baltic nature-zone GeoJSON
  seeds/                      demo users, friendships, rewards, zones
  .env.example
```

Layering: `handlers (HTTP/WS) → services → repositories (sqlx)`. Repository traits per aggregate enable mocking in unit tests.

## 13. Demo replay

Migrations + a seed bin create demo users, friendships, Baltic `nature_zones` polygons, and `rewards_catalog` rows. The `replay` bin logs in demo users and streams recorded tracks **in real time, ~1 ping/sec, preserving original inter-ping cadence** (NOT time-compressed) — this is what makes the server-clock Δt land at walking speed so segments pass the speed cap and the per-second ceiling, while the companion window (also server-clock) matches across the two concurrent streams. Result: live map, scoring, together multiplier, and leaderboard all fire on stage with nobody walking. Two concurrent friend tracks demo the together multiplier. **The two-track replay is the mandatory pre-stage end-to-end check** (validates the §6 clock decision) and doubles as the WS integration smoke test.

## 14. Testing & TDD plan

Methodology: **RED → GREEN → refactor**, git checkpoint commit per stage, target 80%+ coverage. Coverage concentrated on the risky core:

- **Unit (pure):** scoring engine — each multiplier, stacking on/off, speed cap, companion→multiplier mapping, division-by-time edge cases.
- **Integration (throwaway Postgres via sqlx test / testcontainers):**
  - Scoring SQL: `ST_Distance` segment, nature-zone containment, companion count.
  - Redeem: atomicity, balance check, stock decrement, insufficient-balance → 409, no partial writes.
  - Friendship-gated join: non-friend → 403; friend → 200.
  - Auth: register/login, JWT verify, expired/invalid token → 401.
  - Authz: accessing another user's walk/redemption → 403; subscribe to non-member session rejected.
  - Rate limiting: `/auth/*` over limit → 429.
- **WS smoke:** replay bin streams a track end-to-end and asserts `ping_scored` / `leaderboard_update` frames.

## 15. Risks & open items

Resolved in this spec after architecture + security review (§6/§7/§9/§10):
scoring clock = server time + real-time replay; `ST_Covers` for geography; rewards stock-oversell via conditional UPDATE + consistent lock order; no-RLS IDOR via `AuthUser` extractor + WHERE-clause predicates; WS first-frame auth + caps; idempotent pings via client `seq`; argon2id/JWT hardening; login-enumeration constant-time.

Remaining:
- **No RLS** → authz entirely in app code. Residual risk accepted for least-lock-in; mitigated by the extractor pattern + dedicated authz tests.
- **GPS spoofing** — a client can still emit plausible walking-speed fake tracks; speed cap + per-second ceiling bound but don't eliminate it. Acceptable for hackathon.
- **Hackathon time** — Full C includes building auth. Contingency only (not chosen): "C-lite" keeps Supabase Auth. Build the vertical slice first (register/login → start walk → WS ping → score → broadcast) — that single path is the demo.
- **sqlx + PostGIS:** `query!` macros have no `geography` mapping and need a DB at compile time. Use **runtime-checked** `sqlx::query`/`query_as` with explicit `ST_*` I/O wrapping (`ST_MakePoint(lng,lat)::geography` in, `ST_X/ST_Y` out) so the project builds and unit-tests run without a live DB.
- **Tunables to confirm:** `meters_per_point`, `max_speed_mps`, `ping_window_s`, `max_points_per_second`, `recorded_at_tolerance_s`, group threshold for 2× (currently ≥3 people total).
