ALTER TABLE bp_exits
    ADD COLUMN id UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE bp_exits
    DROP CONSTRAINT bp_exits_pkey;
ALTER TABLE bp_exits
    ADD PRIMARY KEY (id);

CREATE TABLE zone_exits
(
    exit_id UUID    NOT NULL,
    zone_id UUID    NOT NULL,
    locked  BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (exit_id, zone_id),
    FOREIGN KEY (exit_id) REFERENCES bp_exits (id) ON DELETE CASCADE
);

CREATE TABLE user_exits
(
    account_id UUID    NOT NULL,
    exit_id    UUID    NOT NULL,
    locked     BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (account_id, exit_id),
    FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
    FOREIGN KEY (exit_id) REFERENCES bp_exits (id) ON DELETE CASCADE
);

DROP TABLE account_zone_items;
DROP TABLE account_zone_state;