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
