-- 0016: zgłoszenia błędów aplikacji („Zgłoś błąd”) — 2026-10-07
-- Spec: seasteps/spec/2026-10-07-zapis-spaceru-do-zgloszenia-bledu.md
--
-- Użytkownik zgłasza błąd aplikacji (POST /api/v1/bug-reports): kategoria,
-- opis, metadane urządzenia i opcjonalny zapis spaceru (`trace`, obiekt JSON
-- ≤ 1 MB, limit pilnuje backend). Admin dostaje mail BEZ opisu i trace (tylko
-- id i kategoria) i rozpatruje wpis tutaj.
-- Usunięcie konta kasuje zgłoszenia usera (repo/gdpr.rs); eksport RODO
-- zawiera je razem z trace.

CREATE TABLE bug_reports (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id      uuid NOT NULL REFERENCES users(id),
  category     text NOT NULL CHECK (category IN ('walk', 'steps', 'map', 'login', 'other')),
  description  text NOT NULL CHECK (char_length(description) BETWEEN 1 AND 2000),
  app_version  text CHECK (char_length(app_version) <= 40),
  device       text CHECK (char_length(device) <= 80),
  platform     text CHECK (char_length(platform) <= 20),
  screen       text CHECK (char_length(screen) <= 60),
  trace        jsonb,
  status       text NOT NULL DEFAULT 'open'
                 CHECK (status IN ('open', 'fixed', 'wontfix', 'duplicate')),
  created_at   timestamptz NOT NULL DEFAULT now(),
  resolved_at  timestamptz
);

CREATE INDEX bug_reports_open_idx ON bug_reports (created_at) WHERE status = 'open';
CREATE INDEX bug_reports_user_idx ON bug_reports (user_id);

-- RLS + revoke od urodzenia (zasada po incydentach 07/2026): backend łączy się
-- jako postgres i omija RLS; brak polityk = deny-all dla kluczy PostgREST.
ALTER TABLE bug_reports ENABLE ROW LEVEL SECURITY;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    EXECUTE 'REVOKE ALL ON bug_reports FROM anon';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    EXECUTE 'REVOKE ALL ON bug_reports FROM authenticated';
  END IF;
END $$;
