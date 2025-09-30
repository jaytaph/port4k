-- Per-blueprint room & player state (used during playtests)
CREATE TABLE IF NOT EXISTS bp_room_kv
(
    bp_key   TEXT  NOT NULL,
    room_key TEXT  NOT NULL,
    key      TEXT  NOT NULL,
    value    JSONB NOT NULL,
    PRIMARY KEY (bp_key, room_key, key)
);

CREATE TABLE IF NOT EXISTS bp_player_kv
(
    bp_key       TEXT  NOT NULL,
    account_name TEXT  NOT NULL,
    room_key     TEXT  NOT NULL,
    key          TEXT  NOT NULL,
    value        JSONB NOT NULL,
    PRIMARY KEY (bp_key, account_name, room_key, key)
);
