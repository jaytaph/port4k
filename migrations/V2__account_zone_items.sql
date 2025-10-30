
-- Per-account, per-zone hero state
CREATE TABLE account_zone_state (
    account_id       UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    zone_id          UUID NOT NULL REFERENCES zones(id)    ON DELETE CASCADE,
    coins            INT  NOT NULL DEFAULT 0,
    health           INT  NOT NULL DEFAULT 100,
    xp               INT  NOT NULL DEFAULT 0,
    current_room_id  UUID NULL REFERENCES bp_rooms(id),
    PRIMARY KEY (account_id, zone_id)
);

CREATE INDEX account_zone_state__zone ON account_zone_state(zone_id);


-- Per-account, per-zone inventory (bag)
CREATE TABLE account_zone_items (
    account_id  UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    zone_id     UUID NOT NULL REFERENCES zones(id)    ON DELETE CASCADE,
    object_id   UUID NOT NULL REFERENCES bp_objects(id)  ON DELETE CASCADE,
    qty         INT  NOT NULL,
    PRIMARY KEY (account_id, zone_id, object_id),
    CHECK (qty >= 0)
);

CREATE INDEX account_zone_items__zone ON account_zone_items(zone_id);
