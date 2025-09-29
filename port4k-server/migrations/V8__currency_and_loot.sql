ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS balance BIGINT NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS loot_spawns
(
    id            BIGSERIAL PRIMARY KEY,
    room_id       BIGINT      NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    item          TEXT        NOT NULL, -- e.g. 'coin'
    qty_min       INT         NOT NULL DEFAULT 1,
    qty_max       INT         NOT NULL DEFAULT 1,
    interval_ms   INT         NOT NULL DEFAULT 10000,
    max_instances INT         NOT NULL DEFAULT 5,
    next_spawn_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS room_loot
(
    id         BIGSERIAL PRIMARY KEY,
    room_id    BIGINT      NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    item       TEXT        NOT NULL, -- 'coin'
    qty        INT         NOT NULL CHECK (qty > 0),
    spawned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    picked_by  TEXT REFERENCES accounts (username),
    picked_at  TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_room_loot_available
    ON room_loot (room_id)
    WHERE picked_by IS NULL;