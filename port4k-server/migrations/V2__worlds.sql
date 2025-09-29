CREATE TABLE IF NOT EXISTS zones
(
    id    BIGSERIAL PRIMARY KEY,
    key   TEXT UNIQUE NOT NULL,
    title TEXT        NOT NULL
);

CREATE TABLE IF NOT EXISTS rooms
(
    id      BIGSERIAL PRIMARY KEY,
    zone_id BIGINT NOT NULL REFERENCES zones (id) ON DELETE CASCADE,
    key     TEXT   NOT NULL,
    title   TEXT   NOT NULL,
    body    TEXT   NOT NULL,
    flags   JSONB  NOT NULL DEFAULT '[]'::jsonb,
    UNIQUE (zone_id, key)
);

CREATE TABLE IF NOT EXISTS exits
(
    id        BIGSERIAL PRIMARY KEY,
    from_room BIGINT NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    dir       TEXT   NOT NULL,
    to_room   BIGINT NOT NULL REFERENCES rooms (id) ON DELETE RESTRICT,
    flags     JSONB  NOT NULL DEFAULT '{}'::jsonb,
    UNIQUE (from_room, dir)
);