CREATE TABLE IF NOT EXISTS characters
(
    id           BIGSERIAL PRIMARY KEY,
    account_name TEXT NOT NULL REFERENCES accounts (username) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    location_id  BIGINT REFERENCES rooms (id),
    UNIQUE (account_name, name)
);
