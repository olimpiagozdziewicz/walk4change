-- 0011: weryfikacja e-maila (spec 2026-07-13-email-weryfikacja-rodo-android)
--
-- users.email_verified_at: NULL = niezweryfikowany. Backfill istniejących kont
-- na now() — to zespół/demo/testerzy zaproszeni świadomie; konta demo muszą
-- przechodzić smoke E2E bez dostępu do skrzynki, a nikomu nie zrywamy
-- open-walks wstecz. Nowe konta rodzą się z NULL.
--
-- email_verification_tokens: wzór magic_links (0003), ale TTL 24 h i bez
-- logowania przy konsumpcji (token z maila potwierdza skrzynkę, nie sesję).

ALTER TABLE users ADD COLUMN email_verified_at timestamptz;
UPDATE users SET email_verified_at = now();

CREATE TABLE email_verification_tokens (
  token       text PRIMARY KEY,
  user_id     uuid NOT NULL REFERENCES users(id),
  expires_at  timestamptz NOT NULL,
  used        boolean NOT NULL DEFAULT false,
  created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX email_verification_tokens_expires_idx
  ON email_verification_tokens (expires_at);

-- RLS + revoke od urodzenia (zasada po incydentach 07/2026): backend łączy się
-- jako postgres i omija RLS; brak polityk = deny-all dla kluczy PostgREST.
ALTER TABLE email_verification_tokens ENABLE ROW LEVEL SECURITY;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    EXECUTE 'REVOKE ALL ON email_verification_tokens FROM anon';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    EXECUTE 'REVOKE ALL ON email_verification_tokens FROM authenticated';
  END IF;
END $$;
