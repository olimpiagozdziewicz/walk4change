-- 0008: indeksy pod listę rozmów (audyt 2026-07-10 B1.4).
-- Zapytanie conversations() filtruje po sender_id/recipient_id z ORDER BY
-- created_at DESC — dotąd bez żadnego pasującego indeksu (seq scan przy każdym
-- wejściu na listę). Historia pary używa od teraz formy LEAST/GREATEST, którą
-- pokrywa istniejący messages_pair_idx z 0005.

CREATE INDEX messages_sender_time_idx ON messages (sender_id, created_at DESC);
CREATE INDEX messages_recipient_time_idx ON messages (recipient_id, created_at DESC);
