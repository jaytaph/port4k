-- Create accounts & characters
CREATE TABLE IF NOT EXISTS accounts (
    username    TEXT PRIMARY KEY,
    role        TEXT NOT NULL DEFAULT 'player',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS characters (
    id           BIGSERIAL PRIMARY KEY,
    account_name TEXT NOT NULL REFERENCES accounts(username) ON DELETE CASCADE,
    name         TEXT UNIQUE NOT NULL,
    location_id  BIGINT,
    stats_json   JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);