-- 0010: oceny po spacerze (spec 2026-07-13) — warstwa zaufania między obcymi.
--
-- Binarna ocena (recommend), nie gwiazdki — te degenerują się do 5.0.
-- flag = niepubliczny sygnał moderacyjny (zgłoszenie problemu po spacerze).
-- UNIQUE (session, rater, rated): jedna ocena na parę na sesję; POST nadpisuje
-- (zmiana zdania w oknie 48 h — egzekwowanym w kodzie, nie w schemacie).

CREATE TABLE walk_ratings (
    id          uuid PRIMARY KEY,
    session_id  uuid NOT NULL REFERENCES walk_sessions(id),
    rater_id    uuid NOT NULL REFERENCES users(id),
    rated_id    uuid NOT NULL REFERENCES users(id),
    recommend   boolean NOT NULL,
    flag        text CHECK (flag IN ('no_show', 'unsafe', 'spam', 'other')),
    comment     text CHECK (char_length(comment) <= 280),
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (session_id, rater_id, rated_id),
    CHECK (rater_id <> rated_id)
);

-- Agregat reputacji liczy się po rated_id (UNIQUE pokrywa ścieżkę per sesja).
CREATE INDEX walk_ratings_rated_idx ON walk_ratings (rated_id);

-- RLS od urodzenia (zasada po incydentach 07/2026: każda nowa tabela = RLS w migracji).
ALTER TABLE walk_ratings ENABLE ROW LEVEL SECURITY;

-- REVOKE tylko jeśli role Supabase istnieją (w czystym Postgresie CI ich nie ma).
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    EXECUTE 'REVOKE ALL ON walk_ratings FROM anon';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    EXECUTE 'REVOKE ALL ON walk_ratings FROM authenticated';
  END IF;
END $$;
