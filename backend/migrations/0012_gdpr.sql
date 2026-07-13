-- 0012: RODO — usuwanie konta i zgody (spec 2026-07-13-email-weryfikacja-rodo-android)
--
-- deleted_at: konto usunięte = wiersz-tombstone (anonimizacja zamiast DELETE,
-- bo żaden FK do users nie ma ON DELETE, a wspólne sesje spacerów innych
-- userów muszą przeżyć). Dane wrażliwe (pingi GPS, wiadomości, tokeny,
-- blokady, znajomości, oceny, eco-zgłoszenia) są twardo kasowane w transakcji
-- DELETE /api/v1/me — patrz repo/user.rs::delete_account.
--
-- accepted_terms_at + terms_version: zgoda na regulamin/politykę przy
-- rejestracji (art. 6.1.b + obowiązek informacyjny). Istniejących kont NIE
-- backfillujemy — nie fabrykujemy zgód wstecz.

ALTER TABLE users ADD COLUMN deleted_at timestamptz;
ALTER TABLE users ADD COLUMN accepted_terms_at timestamptz;
ALTER TABLE users ADD COLUMN terms_version text;

-- Częste zapytanie extractora (SELECT deleted_at WHERE id = $1) idzie po PK —
-- osobny indeks zbędny.
