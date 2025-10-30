CREATE TABLE user_zone_data
(
    account_id      UUID    NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    zone_id         UUID    REFERENCES zones (id) ON DELETE SET NULL,
    current_room_id UUID    REFERENCES bp_rooms (id) ON DELETE SET NULL,
    xp              INTEGER NOT NULL DEFAULT 0,
    health          INTEGER NOT NULL DEFAULT 0,
    coins           INTEGER NOT NULL DEFAULT 0,
    inventory       JSONB   NOT NULL DEFAULT '[]'::jsonb,
    PRIMARY KEY (account_id, zone_id)
);