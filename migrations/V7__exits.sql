ALTER TABLE bp_exits
    ADD COLUMN id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE bp_exits
    DROP CONSTRAINT bp_exits_pkey;
ALTER TABLE bp_exits
    ADD PRIMARY KEY (id);

CREATE TABLE zone_exits
(
    zone_id UUID    NOT NULL,
    room_id UUID    NOT NULL,
    exit_id UUID    NOT NULL,
    locked  BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (zone_id, room_id, exit_id),
    FOREIGN KEY (zone_id) REFERENCES zones (id) ON DELETE CASCADE,
    FOREIGN KEY (room_id) REFERENCES bp_rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (exit_id) REFERENCES bp_exits (id) ON DELETE CASCADE
);

CREATE TABLE user_exits
(
    zone_id UUID    NOT NULL,
    room_id UUID    NOT NULL,
    exit_id UUID    NOT NULL,
    account_id UUID    NOT NULL,
    locked  BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (zone_id, room_id, exit_id, account_id),
    FOREIGN KEY (zone_id) REFERENCES zones (id) ON DELETE CASCADE,
    FOREIGN KEY (room_id) REFERENCES bp_rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (exit_id) REFERENCES bp_exits (id) ON DELETE CASCADE,
    FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
);

DROP TABLE IF EXISTS account_zone_items;
DROP TABLE IF EXISTS account_zone_state;