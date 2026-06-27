-- 0002_hardening.sql: money-path DB invariants
-- Unique constraint on redemption codes (prevents duplicate delivery).
ALTER TABLE reward_redemptions ADD CONSTRAINT reward_redemptions_code_key UNIQUE (code);

-- Non-negative stock check (NULL = unlimited).
ALTER TABLE rewards_catalog ADD CONSTRAINT rewards_catalog_stock_nonneg CHECK (stock IS NULL OR stock >= 0);

-- Index on user_id for fast per-user redemption lookups.
CREATE INDEX reward_redemptions_user_idx ON reward_redemptions (user_id);
