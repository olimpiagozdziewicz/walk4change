-- 0015: spacer z psem (spec 2026-10-06) — osobna kategoria spaceru.
--
-- Tylko etykieta/filtr: punktacja BEZ zmian (żadnego mnożnika za psa).
-- Ustawiana przy starcie (`POST /walks` z `with_dog`), zwracana w listach
-- otwartych spacerów i historii (`/walks/open`, `/me/walks`).
--
-- Idempotentna: ADD COLUMN IF NOT EXISTS — bezpieczna przy ponownym uruchomieniu.
-- Tabela ma już RLS (0014), nowa kolumna nie wymaga dodatkowych uprawnień.

ALTER TABLE walk_sessions
    ADD COLUMN IF NOT EXISTS with_dog boolean NOT NULL DEFAULT false;
