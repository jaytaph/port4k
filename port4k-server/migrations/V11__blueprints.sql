CREATE TABLE IF NOT EXISTS blueprints
(
    key            TEXT PRIMARY KEY,
    title          TEXT        NOT NULL,
    owner          TEXT        NOT NULL REFERENCES accounts (username),
    status         TEXT        NOT NULL DEFAULT 'draft', -- draft | submitted | published
    entry_room_key TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS bp_rooms
(
    bp_key TEXT NOT NULL REFERENCES blueprints (key) ON DELETE CASCADE,
    key    TEXT NOT NULL,
    title  TEXT NOT NULL,
    body   TEXT NOT NULL,
    PRIMARY KEY (bp_key, key)
);

CREATE TABLE IF NOT EXISTS bp_exits
(
    bp_key   TEXT NOT NULL REFERENCES blueprints (key) ON DELETE CASCADE,
    from_key TEXT NOT NULL,
    dir      TEXT NOT NULL,
    to_key   TEXT NOT NULL,
    PRIMARY KEY (bp_key, from_key, dir)
);
