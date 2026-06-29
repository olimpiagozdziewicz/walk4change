-- Eco reports: problem reports ("zgłoś problem") and clean-up brags
-- ("pochwal się"). Photos are uploaded by the client straight to Supabase
-- Storage; we persist only their public URLs so images never transit the API
-- (which has a 64 KiB body cap).
CREATE TABLE eco_reports (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES users(id),
  kind text NOT NULL CHECK (kind IN ('report', 'cleanup')),
  category text NOT NULL DEFAULT '',
  description text NOT NULL DEFAULT '',
  location text NOT NULL DEFAULT '',
  status text NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'reported', 'cleaned')),
  photo_url text,
  photo_before_url text,
  photo_after_url text,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX eco_reports_user_idx ON eco_reports (user_id, created_at DESC);
CREATE INDEX eco_reports_recent_idx ON eco_reports (created_at DESC);
