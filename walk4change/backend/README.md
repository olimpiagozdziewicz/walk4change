# walk4change Backend

A standalone Rust service (Axum + Tokio + sqlx) that powers the walk4change platform. Supabase is used exclusively as a Postgres+PostGIS host; the service owns all application logic including authentication (email+password → JWT), live GPS walk scoring, friend management, rewards, and a WebSocket live feed.

For the full design specification, see `docs/superpowers/specs/2026-06-27-walk4change-backend-design.md`.

## Prerequisites

- **Rust**: Latest stable toolchain
- **Docker**: For running PostgreSQL+PostGIS (or run `postgis/postgis:16-3.4` directly)

## Database Setup (Local Development & Testing)

### Option 1: Docker Compose (Recommended)

The repository includes a preconfigured `docker-compose.yml`:

```bash
cd backend
docker compose up -d
```

### Option 2: Direct Docker

If the `docker compose` plugin is unavailable, run PostgreSQL directly:

```bash
docker run -d \
  --name walk4change-db \
  -e POSTGRES_USER=walk \
  -e POSTGRES_PASSWORD=walk \
  -e POSTGRES_DB=walk4change \
  -p 5433:5432 \
  postgis/postgis:16-3.4
```

### Connection Details

- **URL**: `postgres://walk:walk@localhost:5433/walk4change`
- **Host**: `localhost`
- **Port**: `5433`
- **Database**: `walk4change`
- **User**: `walk`
- **Password**: `walk`

Migrations are applied automatically on startup (sourced from `backend/migrations/`).

## Environment Setup

Copy `backend/.env.example` to `.env`:

```bash
cp backend/.env.example backend/.env
```

Configure the following required variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | `postgres://walk:walk@localhost:5433/walk4change` |
| `JWT_SECRET` | JWT signing secret (**MUST be ≥32 characters**) | Validated at startup |
| `BIND_ADDR` | Server bind address and port | `0.0.0.0:8080` |
| `CORS_ALLOWED_ORIGINS` | Comma-separated allowed origins | `http://localhost:3000,https://app.example.com` |

Optional configuration variables (scoring, rate limiting, password hashing):
- `ARGON2_*` parameters (see `config.rs` and design spec §6)
- Rate-limit tunables
- `SEED_PASSWORD` — for stable demo login seeds (used by the seeder)

## Running the Server

Export environment variables and start the server:

```bash
cd backend
export DATABASE_URL=postgres://walk:walk@localhost:5433/walk4change
export JWT_SECRET=replace-with-a-32+-char-random-secret
export BIND_ADDR=0.0.0.0:8080

cargo run -p walk4change-api
```

The server starts on the address specified by `BIND_ADDR` (default: `:8080`).

## Seeding Demo Data

Populate the database with demo users, friendships, zones, and rewards:

```bash
cargo run -p walk4change-api --bin seed
```

This creates:
- Demo users: **Ana** and **Bek**
- A friend connection between them
- A Baltic nature zone
- Sample rewards

The seeder prints user IDs and JWT tokens for immediate testing.

## Demo Replay (Live Map & Scoring Without Walking)

The `walk4change-replay` binary streams recorded GPS tracks over WebSocket in real time, simulating live walks. This demonstrates the "together" multiplier (1.5×) and nature-zone multiplier (3×) stacking without actual movement.

```bash
cargo run -p walk4change-replay -- \
  --base http://localhost:8080 \
  --email ana@demo.walk4change \
  --password <pw> \
  --track fixtures/track_a.json \
  --friend-email bek@demo.walk4change \
  --friend-password <pw> \
  --friend-track fixtures/track_b.json
```

**Note**: The `--friend-email`, `--friend-password`, and `--friend-track` flags specify the second concurrent track. Both tracks replay simultaneously to demonstrate multiplier interactions.

## Testing

Run the full test suite (requires a running database):

```bash
cd backend
export TEST_DATABASE_URL=postgres://walk:walk@localhost:5433/walk4change
export DATABASE_URL=$TEST_DATABASE_URL

cargo test --workspace
```

Approximately 114 tests cover unit, integration, and end-to-end scenarios.

## API Overview

All endpoints are prefixed with `/api/v1` and use consistent envelope formats (see below).

### Authentication

- `POST /auth/register` — Create a new account
- `POST /auth/login` — Login (returns JWT token)
- `POST /auth/logout` — Logout (invalidates session)

### User Profile

- `GET /me` — Fetch current user's profile
- `PATCH /me` — Update current user's profile

### Friends

- `POST /friends/request` — Send a friend request
- `POST /friends/respond` — Accept or reject a friend request
- `GET /friends` — List all friends and pending requests

### Walks

- `POST /walks` — Create a new walk
- `GET /walks/:id` — Fetch walk details
- `GET /walks/:id/track` — Fetch GPS track for a walk
- `POST /walks/:id/join` — Join an existing walk
- `POST /walks/:id/leave` — Leave a walk
- `POST /walks/:id/stop` — End a walk

### Scoring & Leaderboard

- `GET /leaderboard?page=<n>&per_page=<limit>` — Fetch paginated leaderboard (`per_page` capped at 100)

### Rewards

- `GET /rewards` — List all available rewards
- `POST /rewards/:id/redeem` — Redeem a reward
- `GET /me/redemptions` — Fetch current user's redemption history

### WebSocket Live Feed

- `GET /ws` — WebSocket endpoint for live updates

**Connection protocol**:
1. First frame (client → server): `{"type":"auth","token":"<jwt>"}`
2. Subsequent frames (client → server): `{"type":"ping","timestamp":"<iso8601>"}`
3. Server sends: `ping_scored`, `leaderboard_update`, `session_event`, or `error`

### Response Envelopes

**Success** (HTTP 200–299):
```json
{
  "data": <payload>,
  "meta": { "page": 1, "per_page": 10, "total": 42 }
}
```

**Error** (HTTP 4xx–5xx):
```json
{
  "error": {
    "code": "INVALID_INPUT",
    "message": "Human-readable error message",
    "details": { "field": "description" }
  }
}
```

### Debug Route

- `GET /api/v1/_whoami` — Returns the authenticated caller's user ID (remove before production)

## Deployment Notes & Caveats

### Numeric Serialization

Numeric fields (points, meters, multipliers, reward costs) are serialized as **JSON strings** using Rust's `rust_decimal` type. Frontend code must call `parseFloat()` when working with these values.

### Authorization Model

**No Row-Level Security (RLS)**: All authorization is enforced at the service layer. Every query for owned resources (walks, friends, leaderboard entries) filters by the authenticated user's ID. **Do not expose the Postgres database directly to clients.**

### Authentication Transport

- Uses **JWT Bearer tokens** (native-first approach)
- Web cookies and CSRF flows are not implemented
- All authenticated requests must include the `Authorization: Bearer <token>` header

### Rate Limiting

- Keys on peer IP address
- **Behind a reverse proxy or TLS terminator**: You must forward and parse the real client IP via the `X-Forwarded-For` header, or all clients will share a single rate-limit bucket
- HSTS header assumes TLS termination in front of the service

### GPS & Scoring

- GPS is streamed and server-scored using the server's clock
- A **speed cap** and **per-second points ceiling** prevent simple replay/teleport attacks
- A **determined client can still emit plausible fake tracks** — the scoring is not cryptographically tamper-proof
- Use additional verification (e.g., anomaly detection, backend geofencing) for high-value rewards in production

### Recommendations for Production

1. **Rotate the JWT secret** regularly; store it securely (e.g., AWS Secrets Manager, HashiCorp Vault)
2. **Forward real client IPs** through `X-Forwarded-For` or equivalent header from your TLS terminator
3. **Remove or guard the `_whoami` endpoint** before deploying publicly
4. **Implement anomaly detection** for suspicious walk patterns (teleporting, unrealistic speeds)
5. **Set `CORS_ALLOWED_ORIGINS`** to exact production domains only
6. **Monitor JWT expiration and refresh** flows to prevent token sprawl
7. **Enable database backups** and test restore procedures regularly
