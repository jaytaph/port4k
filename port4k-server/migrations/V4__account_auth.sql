-- Add password hash for accounts (nullable for back-compat; tighten later)
ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS password_hash TEXT;