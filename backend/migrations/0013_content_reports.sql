-- 0013: zgłoszenia treści (UGC, polityka Google Play + DSA art. 16) — 2026-10-01
--
-- Użytkownik zgłasza cudze eko-zgłoszenie, komentarz albo profil
-- (POST /api/v1/reports). Admin dostaje mail BEZ treści zgłoszenia (tylko id
-- i typ celu) i rozpatruje wpis tutaj. target_id bez FK (cel polimorficzny;
-- usunięcie treści nie może kasować historii moderacji). Jedno zgłoszenie na
-- (zgłaszający, cel) — ponowne kliknięcie jest idempotentne.
-- Usunięcie konta kasuje zgłoszenia ZŁOŻONE przez usera (repo/gdpr.rs).

CREATE TABLE content_reports (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  reporter_id  uuid NOT NULL REFERENCES users(id),
  target_type  text NOT NULL CHECK (target_type IN ('eco_post', 'eco_comment', 'user')),
  target_id    uuid NOT NULL,
  reason       text NOT NULL CHECK (reason IN (
                 'spam', 'harassment', 'hate', 'sexual', 'violence',
                 'privacy', 'illegal', 'other')),
  note         text CHECK (char_length(note) <= 500),
  status       text NOT NULL DEFAULT 'open'
                 CHECK (status IN ('open', 'actioned', 'dismissed')),
  created_at   timestamptz NOT NULL DEFAULT now(),
  resolved_at  timestamptz,
  UNIQUE (reporter_id, target_type, target_id)
);

CREATE INDEX content_reports_open_idx ON content_reports (created_at) WHERE status = 'open';
CREATE INDEX content_reports_target_idx ON content_reports (target_type, target_id);

-- RLS + revoke od urodzenia (zasada po incydentach 07/2026): backend łączy się
-- jako postgres i omija RLS; brak polityk = deny-all dla kluczy PostgREST.
ALTER TABLE content_reports ENABLE ROW LEVEL SECURITY;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    EXECUTE 'REVOKE ALL ON content_reports FROM anon';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    EXECUTE 'REVOKE ALL ON content_reports FROM authenticated';
  END IF;
END $$;
