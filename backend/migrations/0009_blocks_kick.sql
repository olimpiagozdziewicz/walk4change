-- 0009: pełna blokada osoby + kick uczestnika przez hosta (audyt 2026-07-10, paczka WS-core)
--
-- user_blocks: unfriend (0008-era) tnie kanał czatu, ale nie zabrania ponownego
-- zaproszenia — block domyka wektor nękania (B1.3 + rozszerzenie na eco feed).
-- walk_participants.kicked_at: host może wyrzucić uczestnika (B3.1); kicked_at
-- blokuje powrót do TEJ sesji (rejoin-po-leave z N1 pozostaje możliwy tylko dla
-- nie-wyrzuconych).

CREATE TABLE user_blocks (
    blocker_id  uuid NOT NULL REFERENCES users(id),
    blocked_id  uuid NOT NULL REFERENCES users(id),
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (blocker_id, blocked_id),
    CHECK (blocker_id <> blocked_id)
);

-- Odwrotny kierunek zapytania is_blocked_either (PK pokrywa blocker→blocked).
CREATE INDEX user_blocks_blocked_idx ON user_blocks (blocked_id);

-- RLS od urodzenia (zasada po incydencie 2026-07-10: każda nowa tabela = RLS w migracji).
ALTER TABLE user_blocks ENABLE ROW LEVEL SECURITY;

-- REVOKE tylko jeśli role Supabase istnieją (w czystym Postgresie CI ich nie ma).
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    EXECUTE 'REVOKE ALL ON user_blocks FROM anon';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    EXECUTE 'REVOKE ALL ON user_blocks FROM authenticated';
  END IF;
END $$;

ALTER TABLE walk_participants ADD COLUMN kicked_at timestamptz;
