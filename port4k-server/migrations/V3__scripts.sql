-- Script storage & room hooks
CREATE TABLE IF NOT EXISTS scripts (
    id         BIGSERIAL PRIMARY KEY,
    lang       TEXT NOT NULL,         -- 'lua'
    source     TEXT NOT NULL,
    sha256     TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS room_scripts (
    room_id   BIGINT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    event     TEXT   NOT NULL,        -- 'on_enter','on_command', ...
    script_id BIGINT NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    PRIMARY KEY (room_id, event)
);
