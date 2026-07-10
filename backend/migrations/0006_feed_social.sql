-- 0006: polubienia i komentarze wpisów eko (feed społeczności), 2026-07-10

CREATE TABLE eco_likes (
  report_id uuid NOT NULL REFERENCES eco_reports(id),
  user_id uuid NOT NULL REFERENCES users(id),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (report_id, user_id)
);

CREATE TABLE eco_comments (
  id uuid PRIMARY KEY,
  report_id uuid NOT NULL REFERENCES eco_reports(id),
  user_id uuid NOT NULL REFERENCES users(id),
  body text NOT NULL CHECK (char_length(body) BETWEEN 1 AND 500),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX eco_comments_report_idx ON eco_comments (report_id, created_at);
