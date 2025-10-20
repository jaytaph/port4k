CREATE TABLE zone_room_kv (
    zone_id UUID NOT NULL,
    room_id UUID NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    PRIMARY KEY (zone_id, room_id, key),
    FOREIGN KEY (zone_id) REFERENCES zones(id) ON DELETE CASCADE,
    FOREIGN KEY (room_id) REFERENCES bp_rooms(id) ON DELETE CASCADE
);

CREATE TABLE user_room_kv (
    account_id UUID NOT NULL,
    zone_id UUID NOT NULL,
    room_id UUID NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    PRIMARY KEY (account_id, zone_id, room_id, key),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (zone_id) REFERENCES zones(id) ON DELETE CASCADE,
    FOREIGN KEY (room_id) REFERENCES bp_rooms(id) ON DELETE CASCADE
);

CREATE TABLE bp_objects_kv (
    object_id UUID NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    PRIMARY KEY (object_id, key),
    FOREIGN KEY (object_id) REFERENCES bp_objects(id) ON DELETE CASCADE
);

CREATE TABLE zone_object_kv (
    zone_id UUID NOT NULL,
    object_id UUID NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    PRIMARY KEY (zone_id, object_id, key),
    FOREIGN KEY (zone_id) REFERENCES zones(id) ON DELETE CASCADE,
    FOREIGN KEY (object_id) REFERENCES bp_objects(id) ON DELETE CASCADE
);

CREATE TABLE user_object_kv (
    account_id UUID NOT NULL,
    object_id UUID NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    PRIMARY KEY (account_id, object_id, key),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (object_id) REFERENCES bp_objects(id) ON DELETE CASCADE
);