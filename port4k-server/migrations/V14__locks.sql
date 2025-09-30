ALTER TABLE bp_rooms
    ADD COLUMN IF NOT EXISTS lockdown BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE bp_exits
    ADD COLUMN IF NOT EXISTS locked BOOLEAN NOT NULL DEFAULT FALSE;

CREATE UNIQUE INDEX IF NOT EXISTS bp_exits_from_dir_uidx
    ON bp_exits (bp_key, from_key, dir);

ALTER TABLE bp_exits
    ADD CONSTRAINT bp_exits_from_fk
        FOREIGN KEY (bp_key, from_key) REFERENCES bp_rooms (bp_key, key)
            ON DELETE CASCADE
            DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE bp_exits
    ADD CONSTRAINT bp_exits_to_fk
        FOREIGN KEY (bp_key, to_key) REFERENCES bp_rooms (bp_key, key)
            ON DELETE CASCADE
            DEFERRABLE INITIALLY DEFERRED;