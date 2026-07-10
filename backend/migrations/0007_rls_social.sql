-- 0007: RLS + revoke dla warstwy społecznej (2026-07-10)
--
-- Root-cause fix. Migracje 0005/0006 utworzyły messages/eco_likes/eco_comments
-- BEZ RLS, przez co publiczny klucz anon czytał je przez PostgREST — potwierdzone
-- na żywo 2026-07-10 (anon-GET = HTTP 200 z danymi) i naprawione tego samego dnia
-- out-of-band przez Management API. Ta migracja czyni fix TRWAŁYM/in-band, żeby
-- każde świeże środowisko/tabela rodziło się z RLS (defekt procesu z 08.07: RLS
-- był łatany z boku, nie w migracjach).
--
-- Backend łączy się jako postgres (superuser) => omija RLS, więc brak polityk =
-- deny-all dla anon/authenticated bez wpływu na backend.

ALTER TABLE messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE eco_likes ENABLE ROW LEVEL SECURITY;
ALTER TABLE eco_comments ENABLE ROW LEVEL SECURITY;

-- REVOKE tylko jeśli role Supabase istnieją (w czystym Postgresie CI ich nie ma).
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    EXECUTE 'REVOKE ALL ON messages FROM anon';
    EXECUTE 'REVOKE ALL ON eco_likes FROM anon';
    EXECUTE 'REVOKE ALL ON eco_comments FROM anon';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    EXECUTE 'REVOKE ALL ON messages FROM authenticated';
    EXECUTE 'REVOKE ALL ON eco_likes FROM authenticated';
    EXECUTE 'REVOKE ALL ON eco_comments FROM authenticated';
  END IF;
END $$;
