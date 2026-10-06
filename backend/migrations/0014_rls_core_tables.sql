-- 0014: RLS + revoke dla tabel z migracji 0001-0004 (audyt 2026-10-06, M2)
--
-- Na żywej bazie RLS na tych tabelach włączono ręcznie 2026-07-08 (poza
-- migracjami). Ta migracja robi to samo in-band, żeby świeże środowisko
-- (odtworzenie, nowy projekt Supabase) nie wystawiło users/magic_links/
-- location_pings itd. kluczowi anon przez PostgREST. Wzór jak 0007/0013.
--
-- Idempotentna: ENABLE ROW LEVEL SECURITY na tabeli, która już ma RLS, i
-- REVOKE uprawnień, których nie ma, to no-op — bezpieczna na żywej bazie.
-- Backend łączy się jako postgres (superuser/właściciel) => omija RLS
-- (brak FORCE), więc brak polityk = deny-all tylko dla anon/authenticated.

ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE friendships ENABLE ROW LEVEL SECURITY;
ALTER TABLE nature_zones ENABLE ROW LEVEL SECURITY;
ALTER TABLE walk_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE walk_participants ENABLE ROW LEVEL SECURITY;
ALTER TABLE location_pings ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_totals ENABLE ROW LEVEL SECURITY;
ALTER TABLE rewards_catalog ENABLE ROW LEVEL SECURITY;
ALTER TABLE reward_redemptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE magic_links ENABLE ROW LEVEL SECURITY;
ALTER TABLE eco_reports ENABLE ROW LEVEL SECURITY;

-- REVOKE tylko jeśli role Supabase istnieją (w czystym Postgresie CI ich nie ma).
DO $$
DECLARE
  t text;
  r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['anon', 'authenticated'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      FOREACH t IN ARRAY ARRAY[
        'users', 'friendships', 'nature_zones', 'walk_sessions',
        'walk_participants', 'location_pings', 'user_totals',
        'rewards_catalog', 'reward_redemptions', 'magic_links', 'eco_reports'
      ] LOOP
        EXECUTE format('REVOKE ALL ON %I FROM %I', t, r);
      END LOOP;
    END IF;
  END LOOP;
END $$;
