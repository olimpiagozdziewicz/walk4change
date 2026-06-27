-- Passwordless magic-link login: one-time, short-lived tokens.
CREATE TABLE magic_links (
  token       text PRIMARY KEY,
  user_id     uuid NOT NULL REFERENCES users(id),
  expires_at  timestamptz NOT NULL,
  used        boolean NOT NULL DEFAULT false,
  created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX magic_links_expires_idx ON magic_links (expires_at);
