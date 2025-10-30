-- =============================================================================
-- Extensions
-- =============================================================================
CREATE EXTENSION IF NOT EXISTS pgcrypto;  -- gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS citext;    -- case-insensitive usernames


-- =============================================================================
-- Accounts (UUID PK; username as CITEXT UNIQUE)
-- =============================================================================
CREATE TABLE public.accounts
(
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    username         citext NOT NULL UNIQUE,
    email            citext NOT NULL UNIQUE,
    role             text   NOT NULL DEFAULT 'player',
    created_at       timestamptz NOT NULL DEFAULT now(),
    password_hash    text,                  -- Argon2id PHC string
    last_login       timestamptz,

    -- Position (see FKs added after dependent tables exist)
    zone_id          uuid,
    current_room_id  uuid,

    -- Progression & stats
    xp               integer  NOT NULL DEFAULT 0 CHECK (xp >= 0),
    health           integer  NOT NULL DEFAULT 100 CHECK (health BETWEEN 0 AND 100),
    coins            integer  NOT NULL DEFAULT 0 CHECK (coins >= 0),

    -- Free-form player state
    inventory        jsonb    NOT NULL DEFAULT '[]'::jsonb,
    flags            jsonb    NOT NULL DEFAULT '{}'::jsonb
);
ALTER TABLE public.accounts OWNER TO port4k;

CREATE INDEX accounts_last_login_idx ON public.accounts (last_login DESC);
CREATE INDEX accounts_inventory_gin  ON public.accounts USING gin (inventory);
CREATE INDEX accounts_flags_gin      ON public.accounts USING gin (flags);

-- =============================================================================
-- Zones (runtime instances)
-- =============================================================================
CREATE TABLE public.zones
(
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    key        text NOT NULL UNIQUE,                  -- e.g. 'live', 'playtest:joshua'
    title      text NOT NULL,
    kind       text NOT NULL DEFAULT 'live',          -- 'live' | 'playtest' | 'event' | ...
    created_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.zones OWNER TO port4k;

-- =============================================================================
-- Blueprints (authoring packages)
-- =============================================================================
CREATE TABLE public.blueprints
(
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    key            text NOT NULL UNIQUE,              -- human-friendly identifier
    title          text NOT NULL,
    owner_id       uuid NOT NULL REFERENCES public.accounts(id),
    status         text NOT NULL DEFAULT 'draft',     -- 'draft' | 'live' | ...
    entry_room_id  uuid,                              -- FK added after bp_rooms exists
    created_at     timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.blueprints OWNER TO port4k;

-- =============================================================================
-- Blueprint Rooms (static content)
-- =============================================================================
CREATE TABLE public.bp_rooms
(
    id       uuid PRIMARY KEY DEFAULT gen_random_uuid(), -- room_id (UUID)
    bp_id    uuid NOT NULL REFERENCES public.blueprints(id) ON DELETE CASCADE,
    key      text NOT NULL,                               -- room key inside the blueprint
    title    text NOT NULL,
    body     text NOT NULL,
    lockdown boolean NOT NULL DEFAULT false,              -- default room-level lockdown
    short    text,
    hints    jsonb,
    objects  jsonb,
    scripts  jsonb,
    CONSTRAINT bp_rooms_bp_key_uidx UNIQUE (bp_id, key)
);
ALTER TABLE public.bp_rooms OWNER TO port4k;

CREATE INDEX bp_rooms_short_gin ON public.bp_rooms
    USING gin (to_tsvector('simple', COALESCE(short, '')));
CREATE INDEX bp_rooms_body_gin ON public.bp_rooms
    USING gin (to_tsvector('simple', COALESCE(body, '')));

-- Link blueprint.entry_room_id → bp_rooms(id) (do it after bp_rooms exists)
ALTER TABLE public.blueprints
    ADD CONSTRAINT blueprints_entry_room_fk
        FOREIGN KEY (entry_room_id) REFERENCES public.bp_rooms(id)
            DEFERRABLE INITIALLY DEFERRED;

-- =============================================================================
-- Blueprint Exits (static; by room UUIDs)
-- =============================================================================
CREATE TABLE public.bp_exits
(
    from_room_id        uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    dir                 text NOT NULL,
    to_room_id          uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    locked              boolean NOT NULL DEFAULT false,   -- default exit lock
    description         text,
    visible_when_locked boolean NOT NULL DEFAULT true,
    PRIMARY KEY (from_room_id, dir)
);
ALTER TABLE public.bp_exits OWNER TO port4k;

-- =============================================================================
-- Blueprint room scripts (simple dual fields)
-- =============================================================================
CREATE TABLE public.bp_room_scripts
(
    room_id       uuid REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    hook          VARCHAR(50),
    script        text,
    updated_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (room_id, hook)
);
ALTER TABLE public.bp_room_scripts OWNER TO port4k;


-- Per-event script sources (draft/live)
CREATE TABLE public.bp_scripts_draft
(
    room_id    uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    event      text NOT NULL,
    source     text NOT NULL,
    author_id  uuid NOT NULL REFERENCES public.accounts(id),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (room_id, event)
);
ALTER TABLE public.bp_scripts_draft OWNER TO port4k;

CREATE TABLE public.bp_scripts_live
(
    room_id    uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    event      text NOT NULL,
    source     text NOT NULL,
    author_id  uuid NOT NULL REFERENCES public.accounts(id),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (room_id, event)
);
ALTER TABLE public.bp_scripts_live OWNER TO port4k;

-- =============================================================================
-- Blueprint objects & nouns (static)
-- =============================================================================
CREATE TABLE public.bp_objects
(
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id     uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    name        text NOT NULL,                               -- object identifier within room
    short       text NOT NULL,
    description text NOT NULL,
    examine     text,
    state       jsonb NOT NULL DEFAULT '{}'::jsonb,          -- static default state
    use_lua     text,
    position    integer,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT bp_objects_room_name_uidx UNIQUE (room_id, name)
);
ALTER TABLE public.bp_objects OWNER TO port4k;

CREATE INDEX bp_objects_desc_gin ON public.bp_objects
    USING gin (to_tsvector('simple', COALESCE(short,'') || ' ' || COALESCE(description,'')));
CREATE INDEX bp_object_state_gin ON public.bp_objects USING gin (state);

CREATE TABLE public.bp_object_nouns
(
    room_id uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    obj_id  uuid NOT NULL REFERENCES public.bp_objects(id) ON DELETE CASCADE,
    noun    text NOT NULL,
    PRIMARY KEY (room_id, noun)
);
ALTER TABLE public.bp_object_nouns OWNER TO port4k;

CREATE INDEX bp_object_noun_btree ON public.bp_object_nouns (room_id, noun);

-- =============================================================================
-- Arbitrary KV scoped to rooms/players (by UUIDs)
-- =============================================================================
CREATE TABLE public.bp_room_kv
(
    room_id uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    key     text NOT NULL,
    value   jsonb NOT NULL,
    PRIMARY KEY (room_id, key)
);
ALTER TABLE public.bp_room_kv OWNER TO port4k;

CREATE TABLE public.bp_player_kv
(
    room_id      uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    account_id   uuid NOT NULL REFERENCES public.accounts(id) ON DELETE CASCADE,
    key          text NOT NULL,
    value        jsonb NOT NULL,
    PRIMARY KEY (room_id, account_id, key)
);
ALTER TABLE public.bp_player_kv OWNER TO port4k;

-- =============================================================================
-- Runtime overlay: per-zone room state (locks/objects/flags)
-- =============================================================================
CREATE TABLE public.zone_room_state
(
    zone_id uuid NOT NULL REFERENCES public.zones(id) ON DELETE CASCADE,
    room_id uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    state   jsonb NOT NULL DEFAULT '{}'::jsonb,  -- {"locks":{}, "objects":{}, "flags":{}}
    PRIMARY KEY (zone_id, room_id)
);
ALTER TABLE public.zone_room_state OWNER TO port4k;

CREATE INDEX zone_room_state_state_gin ON public.zone_room_state USING gin (state);

-- =============================================================================
-- Loot (runtime) scoped to (zone, room)
-- =============================================================================
CREATE TABLE public.loot_spawns
(
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    zone_id       uuid NOT NULL REFERENCES public.zones(id) ON DELETE CASCADE,
    room_id       uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    item          text NOT NULL,
    qty_min       integer NOT NULL DEFAULT 1,
    qty_max       integer NOT NULL DEFAULT 1,
    interval_ms   integer NOT NULL DEFAULT 10000,
    max_instances integer NOT NULL DEFAULT 5,
    next_spawn_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.loot_spawns OWNER TO port4k;

CREATE INDEX loot_spawns_zone_room_idx ON public.loot_spawns (zone_id, room_id);

CREATE TABLE public.room_loot
(
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    zone_id    uuid NOT NULL REFERENCES public.zones(id) ON DELETE CASCADE,
    room_id    uuid NOT NULL REFERENCES public.bp_rooms(id) ON DELETE CASCADE,
    item       text NOT NULL,
    qty        integer NOT NULL CHECK (qty > 0),
    spawned_at timestamptz NOT NULL DEFAULT now(),
    picked_by  uuid REFERENCES public.accounts(id),
    picked_at  timestamptz
);
ALTER TABLE public.room_loot OWNER TO port4k;

CREATE INDEX idx_room_loot_available
    ON public.room_loot (zone_id, room_id)
    WHERE picked_by IS NULL;

-- =============================================================================
-- Characters (avatars) positioned by zone+room UUIDs
-- =============================================================================
CREATE TABLE public.characters
(
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   uuid NOT NULL REFERENCES public.accounts(id) ON DELETE CASCADE,
    name         text NOT NULL UNIQUE,

    zone_id      uuid REFERENCES public.zones(id) ON DELETE SET NULL,
    room_id      uuid REFERENCES public.bp_rooms(id) ON DELETE SET NULL
        DEFERRABLE INITIALLY DEFERRED,

    stats_json   jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at   timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.characters OWNER TO port4k;

CREATE INDEX characters_zone_room_idx ON public.characters (zone_id, room_id);

-- =============================================================================
-- Back-fill FKs on accounts now that targets exist
-- =============================================================================
ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_zone_fk
        FOREIGN KEY (zone_id) REFERENCES public.zones(id)
            ON DELETE SET NULL;

ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_current_room_fk
        FOREIGN KEY (current_room_id) REFERENCES public.bp_rooms(id)
            ON DELETE SET NULL
            DEFERRABLE INITIALLY DEFERRED;

CREATE INDEX accounts_zone_idx ON public.accounts(zone_id);
CREATE INDEX accounts_room_idx ON public.accounts(current_room_id);
