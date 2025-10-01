-- ============================================================================
-- Zones (runtime instances: 'live', 'playtest:<user>', 'event:<slug>', ...)
-- ============================================================================
CREATE TABLE public.zones
(
    id         bigserial PRIMARY KEY,
    key        text NOT NULL UNIQUE,
    title      text NOT NULL,
    kind       text NOT NULL DEFAULT 'live',         -- 'live' | 'playtest' | 'event' | ...
    created_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.zones OWNER TO port4k;


-- ============================================================================
-- Accounts (global player profile; position is bp/room + zone)
-- ============================================================================
CREATE TABLE public.accounts
(
    username         text PRIMARY KEY,
    role             text                      NOT NULL DEFAULT 'player',
    created_at       timestamptz               NOT NULL DEFAULT now(),
    password_hash    text,                                 -- Argon2id PHC string
    last_login       timestamptz,

    -- Zone + position (FKs added later once targets exist)
    zone_key         text,
    current_room_bp  text,
    current_room_key text,

    -- Progression & stats
    xp               integer                   NOT NULL DEFAULT 0 CHECK (xp >= 0),
    health           integer                   NOT NULL DEFAULT 100 CHECK (health BETWEEN 0 AND 100),
    coins            integer                   NOT NULL DEFAULT 0 CHECK (coins >= 0),

    -- Free-form player state
    inventory        jsonb                     NOT NULL DEFAULT '[]'::jsonb,
    flags            jsonb                     NOT NULL DEFAULT '{}'::jsonb
);
ALTER TABLE public.accounts OWNER TO port4k;

CREATE INDEX accounts_last_login_idx ON public.accounts (last_login DESC);
CREATE INDEX accounts_inventory_gin  ON public.accounts USING gin (inventory);
CREATE INDEX accounts_flags_gin      ON public.accounts USING gin (flags);


-- ============================================================================
-- Blueprints (authoring packages)
-- ============================================================================
CREATE TABLE public.blueprints
(
    key            text PRIMARY KEY,
    title          text NOT NULL,
    owner          text NOT NULL REFERENCES public.accounts(username),
    status         text NOT NULL DEFAULT 'draft',   -- 'draft' | 'live' | ...
    entry_room_key text,                             -- optional default spawn inside this bp
    created_at     timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.blueprints OWNER TO port4k;


-- Rooms inside a blueprint (static content)
CREATE TABLE public.bp_rooms
(
    bp_key   text NOT NULL REFERENCES public.blueprints(key) ON DELETE CASCADE,
    key      text NOT NULL,
    title    text NOT NULL,
    body     text NOT NULL,
    lockdown boolean NOT NULL DEFAULT false,        -- default room-level lockdown
    short    text,
    hints    jsonb,
    objects  jsonb,
    scripts  jsonb,
    PRIMARY KEY (bp_key, key)
);
ALTER TABLE public.bp_rooms OWNER TO port4k;

CREATE INDEX bp_rooms_short_gin ON public.bp_rooms
    USING gin (to_tsvector('simple', COALESCE(short, '')));
CREATE INDEX bp_rooms_body_gin ON public.bp_rooms
    USING gin (to_tsvector('simple', COALESCE(body, '')));

-- (Optional) Link blueprint.entry_room_key to an existing room (composite FK).
-- Done with ALTER to avoid creation order issues.
ALTER TABLE public.blueprints
    ADD CONSTRAINT blueprints_entry_room_fk
        FOREIGN KEY (key, entry_room_key)
            REFERENCES public.bp_rooms (bp_key, key)
            DEFERRABLE INITIALLY DEFERRED;


-- Exits between rooms (static; authoring time)
CREATE TABLE public.bp_exits
(
    bp_key              text    NOT NULL,
    from_key            text    NOT NULL,
    dir                 text    NOT NULL,
    to_key              text    NOT NULL,
    locked              boolean NOT NULL DEFAULT false,  -- default exit lock
    description         text,
    visible_when_locked boolean NOT NULL DEFAULT true,
    PRIMARY KEY (bp_key, from_key, dir),

    CONSTRAINT bp_exits_from_fk
        FOREIGN KEY (bp_key, from_key) REFERENCES public.bp_rooms(bp_key, key)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT bp_exits_to_fk
        FOREIGN KEY (bp_key, to_key)   REFERENCES public.bp_rooms(bp_key, key)
            ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
);
ALTER TABLE public.bp_exits OWNER TO port4k;

CREATE UNIQUE INDEX bp_exits_from_dir_uidx ON public.bp_exits (bp_key, from_key, dir);


-- Blueprint scripts (two models: single row per room; and per-event draft/live)
CREATE TABLE public.bp_room_scripts
(
    bp_key         text NOT NULL,
    room_key       text NOT NULL,
    on_enter_lua   text,
    on_command_lua text,
    updated_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.bp_room_scripts OWNER TO port4k;

CREATE TABLE public.bp_scripts_draft
(
    bp_key     text NOT NULL,
    room_key   text NOT NULL,
    event      text NOT NULL,
    source     text NOT NULL,
    author     text NOT NULL REFERENCES public.accounts(username),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key, event),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.bp_scripts_draft OWNER TO port4k;

CREATE TABLE public.bp_scripts_live
(
    bp_key     text NOT NULL,
    room_key   text NOT NULL,
    event      text NOT NULL,
    source     text NOT NULL,
    author     text NOT NULL REFERENCES public.accounts(username),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key, event),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.bp_scripts_live OWNER TO port4k;


-- Blueprint objects & nouns (static content)
CREATE TABLE public.bp_objects
(
    bp_key      text NOT NULL,
    room_key    text NOT NULL,
    id          text NOT NULL,
    short       text NOT NULL,
    description text NOT NULL,
    examine     text,
    state       jsonb NOT NULL DEFAULT '{}'::jsonb, -- static default state
    use_lua     text,
    position    integer,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key, id),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.bp_objects OWNER TO port4k;

CREATE INDEX bp_objects_desc_gin ON public.bp_objects
    USING gin (to_tsvector('simple', COALESCE(short,'') || ' ' || COALESCE(description,'')));
CREATE INDEX bp_object_state_gin ON public.bp_objects USING gin (state);

CREATE TABLE public.bp_object_nouns
(
    bp_key   text NOT NULL,
    room_key text NOT NULL,
    obj_id   text NOT NULL,
    noun     text NOT NULL,
    PRIMARY KEY (bp_key, room_key, noun),
    FOREIGN KEY (bp_key, room_key, obj_id) REFERENCES public.bp_objects (bp_key, room_key, id) ON DELETE CASCADE
);
ALTER TABLE public.bp_object_nouns OWNER TO port4k;

CREATE INDEX bp_object_noun_btree ON public.bp_object_nouns (bp_key, room_key, noun);


-- Arbitrary KV for rooms/players scoped to blueprints (authoring/test data)
CREATE TABLE public.bp_room_kv
(
    bp_key   text  NOT NULL,
    room_key text  NOT NULL,
    key      text  NOT NULL,
    value    jsonb NOT NULL,
    PRIMARY KEY (bp_key, room_key, key),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.bp_room_kv OWNER TO port4k;

CREATE TABLE public.bp_player_kv
(
    bp_key       text NOT NULL,
    account_name text NOT NULL REFERENCES public.accounts(username) ON DELETE CASCADE,
    room_key     text NOT NULL,
    key          text NOT NULL,
    value        jsonb NOT NULL,
    PRIMARY KEY (bp_key, account_name, room_key, key),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.bp_player_kv OWNER TO port4k;


-- ============================================================================
-- Runtime overlay: per-zone room state (locks, flags, object runtime state)
-- ============================================================================
CREATE TABLE public.zone_room_state
(
    zone_key text NOT NULL REFERENCES public.zones(key) ON DELETE CASCADE,
    bp_key   text NOT NULL,
    room_key text NOT NULL,
    state    jsonb NOT NULL DEFAULT '{}'::jsonb,  -- {"locks":{}, "objects":{}, "flags":{}}
    PRIMARY KEY (zone_key, bp_key, room_key),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.zone_room_state OWNER TO port4k;

CREATE INDEX zone_room_state_state_gin ON public.zone_room_state USING gin (state);


-- ============================================================================
-- Loot (runtime) now scoped to (zone, bp, room)
-- ============================================================================
CREATE TABLE public.loot_spawns
(
    id            bigserial PRIMARY KEY,
    zone_key      text NOT NULL REFERENCES public.zones(key) ON DELETE CASCADE,
    bp_key        text NOT NULL,
    room_key      text NOT NULL,
    item          text NOT NULL,
    qty_min       integer NOT NULL DEFAULT 1,
    qty_max       integer NOT NULL DEFAULT 1,
    interval_ms   integer NOT NULL DEFAULT 10000,
    max_instances integer NOT NULL DEFAULT 5,
    next_spawn_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.loot_spawns OWNER TO port4k;

CREATE INDEX loot_spawns_zone_room_idx ON public.loot_spawns (zone_key, bp_key, room_key);

CREATE TABLE public.room_loot
(
    id         bigserial PRIMARY KEY,
    zone_key   text NOT NULL REFERENCES public.zones(key) ON DELETE CASCADE,
    bp_key     text NOT NULL,
    room_key   text NOT NULL,
    item       text NOT NULL,
    qty        integer NOT NULL CHECK (qty > 0),
    spawned_at timestamptz NOT NULL DEFAULT now(),
    picked_by  text REFERENCES public.accounts(username),
    picked_at  timestamptz,
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);
ALTER TABLE public.room_loot OWNER TO port4k;

CREATE INDEX idx_room_loot_available
    ON public.room_loot (zone_key, bp_key, room_key)
    WHERE picked_by IS NULL;


-- ============================================================================
-- Characters (optional per-account avatar; positioned in zone/bp/room)
-- ============================================================================
CREATE TABLE public.characters
(
    id           bigserial PRIMARY KEY,
    account_name text NOT NULL REFERENCES public.accounts(username) ON DELETE CASCADE,
    name         text NOT NULL UNIQUE,

    zone_key     text REFERENCES public.zones(key) ON DELETE SET NULL,
    bp_key       text,
    room_key     text,
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,

    stats_json   jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at   timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.characters OWNER TO port4k;

CREATE INDEX characters_zone_room_idx ON public.characters (zone_key, bp_key, room_key);


-- ============================================================================
-- Add the FKs on accounts now that targets exist
-- ============================================================================
ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_zone_fk
        FOREIGN KEY (zone_key) REFERENCES public.zones(key)
            ON DELETE SET NULL;

ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_current_room_fk
        FOREIGN KEY (current_room_bp, current_room_key)
            REFERENCES public.bp_rooms (bp_key, key)
            ON DELETE SET NULL
            DEFERRABLE INITIALLY DEFERRED;

CREATE INDEX accounts_zone_idx ON public.accounts(zone_key);
CREATE INDEX accounts_room_idx ON public.accounts(current_room_bp, current_room_key);
