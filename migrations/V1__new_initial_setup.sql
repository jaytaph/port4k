CREATE EXTENSION IF NOT EXISTS "citext" WITH SCHEMA public;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public;

-- =====================================================================
--  ACCOUNTS (created first to break cycles)
-- =====================================================================

CREATE TABLE public.accounts (
    id               uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    username         citext                                             NOT NULL UNIQUE,
    email            citext                                             NOT NULL UNIQUE,
    role             text                     DEFAULT 'player'::text    NOT NULL,
    created_at       timestamp with time zone DEFAULT now()             NOT NULL,
    password_hash    text,
    last_login       timestamp with time zone,

    -- current location
    current_realm_id uuid,
    current_room_id  uuid,

    -- spawn / respawn location
    spawn_realm_id   uuid,
    spawn_room_id    uuid,

    xp               integer                  DEFAULT 0                 NOT NULL
        CONSTRAINT accounts_xp_check
            CHECK (xp >= 0),

    health           integer                  DEFAULT 100               NOT NULL
        CONSTRAINT accounts_health_check
            CHECK (health >= 0 AND health <= 100),

    coins            integer                  DEFAULT 0                 NOT NULL
        CONSTRAINT accounts_coins_check
            CHECK (coins >= 0),

    locked_out       boolean                  DEFAULT false             NOT NULL,
    show_motd        boolean                  DEFAULT true              NOT NULL,

    flags            jsonb                    DEFAULT '{}'::jsonb       NOT NULL
);

ALTER TABLE public.accounts
    OWNER TO port4k;

-- indexes that don't depend on FKs
CREATE INDEX accounts_last_login_idx ON public.accounts (last_login DESC);
CREATE INDEX accounts_flags_gin      ON public.accounts USING gin (flags);
CREATE INDEX accounts_realm_idx      ON public.accounts (current_realm_id);
CREATE INDEX accounts_room_idx       ON public.accounts (current_room_id);


-- =====================================================================
--  BLUEPRINTS (needs accounts for owner_id)
-- =====================================================================

CREATE TABLE public.blueprints (
    id            uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    key           text                                               NOT NULL UNIQUE,
    title         text                                               NOT NULL,
    owner_id      uuid                                               NOT NULL REFERENCES public.accounts,
    status        text                     DEFAULT 'draft'::text     NOT NULL,
    entry_room_id uuid,
    created_at    timestamp with time zone DEFAULT now()             NOT NULL
);

ALTER TABLE public.blueprints
    OWNER TO port4k;


-- =====================================================================
--  REALMS (needs blueprints; now also references accounts for owner_id)
-- =====================================================================

CREATE TABLE public.realms (
    id         uuid NOT NULL PRIMARY KEY,
    bp_id      uuid NOT NULL
        REFERENCES public.blueprints
            ON DELETE CASCADE,
    key        varchar UNIQUE,
    title      varchar,
    kind       varchar,
    owner_id   uuid
        REFERENCES public.accounts
            ON DELETE SET NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE public.realms
    OWNER TO port4k;

-- now that realms exists, we can hook accounts → realms
ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_current_realm_fk
        FOREIGN KEY (current_realm_id)
        REFERENCES public.realms
        ON DELETE SET NULL;

ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_spawn_realm_fk
        FOREIGN KEY (spawn_realm_id)
        REFERENCES public.realms
        ON DELETE SET NULL;


-- =====================================================================
--  BLUEPRINT ROOMS (needs blueprints)
-- =====================================================================

CREATE TABLE public.bp_rooms (
    id       uuid    DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    bp_id    uuid                              NOT NULL
        REFERENCES public.blueprints
            ON DELETE CASCADE,
    key      text                              NOT NULL,
    title    text                              NOT NULL,
    body     text                              NOT NULL,
    lockdown boolean DEFAULT false             NOT NULL,
    short    text,
    hints    jsonb,
    CONSTRAINT bp_rooms_bp_key_uidx
        UNIQUE (bp_id, key)
);

ALTER TABLE public.bp_rooms
    OWNER TO port4k;

-- add the room FKs to accounts now that bp_rooms exists
ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_current_room_fk
        FOREIGN KEY (current_room_id)
        REFERENCES public.bp_rooms
        ON DELETE SET NULL
        DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE public.accounts
    ADD CONSTRAINT accounts_spawn_room_fk
        FOREIGN KEY (spawn_room_id)
        REFERENCES public.bp_rooms
        ON DELETE SET NULL
        DEFERRABLE INITIALLY DEFERRED;

-- blueprints → entry room
ALTER TABLE public.blueprints
    ADD CONSTRAINT blueprints_entry_room_fk
        FOREIGN KEY (entry_room_id)
        REFERENCES public.bp_rooms
        DEFERRABLE INITIALLY DEFERRED;

-- text search indexes for rooms
CREATE INDEX bp_rooms_short_gin ON public.bp_rooms
    USING gin (to_tsvector('simple', COALESCE(short, '')));
CREATE INDEX bp_rooms_body_gin ON public.bp_rooms
    USING gin (to_tsvector('simple', COALESCE(body, '')));


-- =====================================================================
--  BLUEPRINT EXITS
-- =====================================================================

CREATE TABLE public.bp_exits (
    id                  uuid    DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    from_room_id        uuid                              NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    dir                 text                              NOT NULL,
    to_room_id          uuid                              NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    locked              boolean DEFAULT false             NOT NULL,
    description         text,
    visible_when_locked boolean DEFAULT true              NOT NULL
);

ALTER TABLE public.bp_exits
    OWNER TO port4k;

CREATE UNIQUE INDEX bp_exits_from_dir_unique
    ON public.bp_exits (from_room_id, dir);


-- =====================================================================
--  ROOM SCRIPTS
-- =====================================================================

CREATE TABLE public.bp_room_scripts (
    room_id    uuid                                   NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    hook       varchar(50)                            NOT NULL,
    script     text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    PRIMARY KEY (room_id, hook)
);

ALTER TABLE public.bp_room_scripts
    OWNER TO port4k;


-- =====================================================================
--  OBJECTS
-- =====================================================================

CREATE TABLE public.bp_objects (
    id          uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    room_id     uuid                                               NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    name        text                                               NOT NULL,
    short       text                                               NOT NULL,
    description text                                               NOT NULL,
    examine     text,
    state       jsonb                    DEFAULT '{}'::jsonb       NOT NULL,
    use_lua     text,
    position    integer,
    updated_at  timestamp with time zone DEFAULT now()             NOT NULL,
    flags       jsonb                    DEFAULT '[]'::jsonb       NOT NULL,
    visible     text
        CONSTRAINT bp_objects_visible_chk
            CHECK (
                visible IS NULL
                OR visible = ANY (
                    ARRAY[
                        'always'::text,
                        'when_revealed'::text,
                        'when_unlocked'::text,
                        'script'::text
                    ]
                )
            ),
    controls    jsonb                    DEFAULT '[]'::jsonb       NOT NULL,
    loot        jsonb                    DEFAULT '{}'::jsonb       NOT NULL,
    CONSTRAINT bp_objects_room_name_uidx
        UNIQUE (room_id, name)
);

ALTER TABLE public.bp_objects
    OWNER TO port4k;

CREATE INDEX bp_objects_desc_gin
    ON public.bp_objects
    USING gin (
        to_tsvector(
            'simple'::regconfig,
            COALESCE(short, ''::text) || ' '::text || COALESCE(description, ''::text)
        )
    );

CREATE INDEX bp_object_state_gin
    ON public.bp_objects
    USING gin (state);

CREATE INDEX bp_objects_flags_gin
    ON public.bp_objects
    USING gin (flags);

CREATE INDEX bp_objects_controls_gin
    ON public.bp_objects
    USING gin (controls);

CREATE INDEX bp_objects_loot_gin
    ON public.bp_objects
    USING gin (loot);


-- =====================================================================
--  OBJECT NOUNS
-- =====================================================================

CREATE TABLE public.bp_object_nouns (
    room_id uuid NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    obj_id  uuid NOT NULL
        REFERENCES public.bp_objects
            ON DELETE CASCADE,
    noun    text NOT NULL,
    PRIMARY KEY (room_id, noun)
);

ALTER TABLE public.bp_object_nouns
    OWNER TO port4k;

CREATE INDEX bp_object_noun_btree
    ON public.bp_object_nouns (room_id, noun);


-- =====================================================================
--  ROOM-LEVEL KV (on blueprint)
-- =====================================================================

CREATE TABLE public.bp_room_kv (
    room_id uuid  NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    key     text  NOT NULL,
    value   jsonb NOT NULL,
    PRIMARY KEY (room_id, key)
);

ALTER TABLE public.bp_room_kv
    OWNER TO port4k;


-- =====================================================================
--  LOOT SPAWNS
-- =====================================================================

CREATE TABLE public.loot_spawns (
    id            uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    realm_id      uuid                                               NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    room_id       uuid                                               NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    item          text                                               NOT NULL,
    qty_min       integer                  DEFAULT 1                 NOT NULL,
    qty_max       integer                  DEFAULT 1                 NOT NULL,
    interval_ms   integer                  DEFAULT 10000             NOT NULL,
    max_instances integer                  DEFAULT 5                 NOT NULL,
    next_spawn_at timestamp with time zone DEFAULT now()             NOT NULL
);

ALTER TABLE public.loot_spawns
    OWNER TO port4k;

CREATE INDEX loot_spawns_realm_room_idx
    ON public.loot_spawns (realm_id, room_id);


-- =====================================================================
--  ROOM LOOT
-- =====================================================================

CREATE TABLE public.room_loot (
    id         uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    realm_id   uuid                                               NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    room_id    uuid                                               NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    item       text                                               NOT NULL,
    qty        integer                                            NOT NULL
        CONSTRAINT room_loot_qty_check
            CHECK (qty > 0),
    spawned_at timestamp with time zone DEFAULT now()             NOT NULL,
    picked_by  uuid
        REFERENCES public.accounts,
    picked_at  timestamp with time zone
);

ALTER TABLE public.room_loot
    OWNER TO port4k;

CREATE INDEX idx_room_loot_available
    ON public.room_loot (realm_id, room_id)
    WHERE (picked_by IS NULL);


-- =====================================================================
--  CHARACTERS
-- =====================================================================

CREATE TABLE public.characters (
    id         uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    account_id uuid                                               NOT NULL
        REFERENCES public.accounts
            ON DELETE CASCADE,
    name       text                                               NOT NULL UNIQUE,
    realm_id   uuid
        REFERENCES public.realms
            ON DELETE SET NULL,
    room_id    uuid
        REFERENCES public.bp_rooms
            ON DELETE SET NULL
            DEFERRABLE INITIALLY DEFERRED,
    stats_json jsonb                    DEFAULT '{}'::jsonb       NOT NULL,
    created_at timestamp with time zone DEFAULT now()             NOT NULL
);

ALTER TABLE public.characters
    OWNER TO port4k;

CREATE INDEX characters_realm_room_idx
    ON public.characters (realm_id, room_id);


-- =====================================================================
--  REALM ROOM KV
-- =====================================================================

CREATE TABLE public.realm_room_kv (
    realm_id uuid  NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    room_id  uuid  NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    key      text  NOT NULL,
    value    jsonb NOT NULL,
    PRIMARY KEY (realm_id, room_id, key)
);

ALTER TABLE public.realm_room_kv
    OWNER TO port4k;


-- =====================================================================
--  USER ROOM KV
-- =====================================================================

CREATE TABLE public.user_room_kv (
    account_id uuid  NOT NULL
        REFERENCES public.accounts
            ON DELETE CASCADE,
    realm_id   uuid  NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    room_id    uuid  NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    key        text  NOT NULL,
    value      jsonb NOT NULL,
    PRIMARY KEY (account_id, realm_id, room_id, key)
);

ALTER TABLE public.user_room_kv
    OWNER TO port4k;


-- =====================================================================
--  BP OBJECTS KV
-- =====================================================================

CREATE TABLE public.bp_objects_kv (
    object_id uuid  NOT NULL
        REFERENCES public.bp_objects
            ON DELETE CASCADE,
    key       text  NOT NULL,
    value     jsonb NOT NULL,
    PRIMARY KEY (object_id, key)
);

ALTER TABLE public.bp_objects_kv
    OWNER TO port4k;


-- =====================================================================
--  REALM OBJECT KV
-- =====================================================================

CREATE TABLE public.realm_object_kv (
    realm_id  uuid  NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    object_id uuid  NOT NULL
        REFERENCES public.bp_objects
            ON DELETE CASCADE,
    key       text  NOT NULL,
    value     jsonb NOT NULL,
    PRIMARY KEY (realm_id, object_id, key)
);

ALTER TABLE public.realm_object_kv
    OWNER TO port4k;


-- =====================================================================
--  USER OBJECT KV  (FIXED: realm_id now references public.realms)
-- =====================================================================

CREATE TABLE public.user_object_kv (
    account_id uuid  NOT NULL
        REFERENCES public.accounts
            ON DELETE CASCADE,
    realm_id   uuid  NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    object_id  uuid  NOT NULL
        REFERENCES public.bp_objects
            ON DELETE CASCADE,
    key        text  NOT NULL,
    value      jsonb NOT NULL,
    PRIMARY KEY (realm_id, account_id, object_id, key)
);

ALTER TABLE public.user_object_kv
    OWNER TO port4k;


-- =====================================================================
--  REALM EXITS
-- =====================================================================

CREATE TABLE public.realm_exits (
    realm_id uuid                  NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    room_id  uuid                  NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    exit_id  uuid                  NOT NULL
        REFERENCES public.bp_exits
            ON DELETE CASCADE,
    locked   boolean DEFAULT false NOT NULL,
    PRIMARY KEY (realm_id, room_id, exit_id)
);

ALTER TABLE public.realm_exits
    OWNER TO port4k;


-- =====================================================================
--  USER EXITS
-- =====================================================================

CREATE TABLE public.user_exits (
    realm_id   uuid                  NOT NULL
        REFERENCES public.realms
            ON DELETE CASCADE,
    room_id    uuid                  NOT NULL
        REFERENCES public.bp_rooms
            ON DELETE CASCADE,
    exit_id    uuid                  NOT NULL
        REFERENCES public.bp_exits
            ON DELETE CASCADE,
    account_id uuid                  NOT NULL
        REFERENCES public.accounts
            ON DELETE CASCADE,
    locked     boolean DEFAULT false NOT NULL,
    PRIMARY KEY (realm_id, room_id, exit_id, account_id)
);

ALTER TABLE public.user_exits
    OWNER TO port4k;


-- =====================================================================
--  BP ITEMS CATALOG
-- =====================================================================

CREATE TABLE public.bp_items_catalog (
    id          uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    bp_id       uuid                                               NOT NULL
        REFERENCES public.blueprints
            ON DELETE CASCADE,
    item_key    varchar(64)                                        NOT NULL,
    name        varchar(255)                                       NOT NULL,
    short       text                                               NOT NULL,
    description text                                               NOT NULL,
    examine     text,
    is_external boolean                  DEFAULT false,
    stackable   boolean                  DEFAULT false             NOT NULL,
    created_at  timestamp with time zone DEFAULT now()             NOT NULL,
    updated_at  timestamp with time zone DEFAULT now()             NOT NULL,
    CONSTRAINT uq_bp_items_catalog_bp_item
        UNIQUE (bp_id, item_key)
);

ALTER TABLE public.bp_items_catalog
    OWNER TO port4k;

CREATE INDEX idx_bp_items_catalog_bp_id
    ON public.bp_items_catalog (bp_id);

CREATE INDEX idx_bp_items_catalog_item_key
    ON public.bp_items_catalog (item_key);


-- =====================================================================
--  BP ITEM NOUNS
-- =====================================================================

CREATE TABLE public.bp_item_nouns (
    id         uuid                     DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    bp_id      uuid                                               NOT NULL
        REFERENCES public.blueprints
            ON DELETE CASCADE,
    item_id    uuid                                               NOT NULL
        REFERENCES public.bp_items_catalog
            ON DELETE CASCADE,
    noun       varchar(64)                                        NOT NULL,
    created_at timestamp with time zone DEFAULT now()             NOT NULL
);

ALTER TABLE public.bp_item_nouns
    OWNER TO port4k;

CREATE INDEX idx_bp_item_nouns_bp_id
    ON public.bp_item_nouns (bp_id);

CREATE INDEX idx_bp_item_nouns_item_id
    ON public.bp_item_nouns (item_id);

CREATE INDEX idx_bp_item_nouns_noun
    ON public.bp_item_nouns (noun);


-- =====================================================================
--  ITEM INSTANCES
-- =====================================================================

CREATE TABLE public.item_instances (
    instance_id       uuid      DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    realm_id          uuid                                NOT NULL,
    catalog_id        uuid                                NOT NULL
        REFERENCES public.bp_items_catalog,
    item_key          varchar(100)                        NOT NULL,
    room_id           uuid,
    account_id        uuid,
    object_id         uuid,
    container_item_id uuid,
    quantity          integer   DEFAULT 1                 NOT NULL,
    condition         jsonb     DEFAULT '{}'::jsonb,
    created_at        timestamp with time zone DEFAULT now()             NOT NULL,
    updated_at        timestamp with time zone DEFAULT now()             NOT NULL,
    CONSTRAINT item_instances_check
        CHECK (
            (
                (room_id IS NOT NULL)::integer
                + (account_id IS NOT NULL)::integer
                + (object_id IS NOT NULL)::integer
                + (container_item_id IS NOT NULL)::integer
            ) = 1
        )
);

ALTER TABLE public.item_instances
    OWNER TO port4k;

CREATE INDEX idx_item_instances_realm
    ON public.item_instances (realm_id);

CREATE INDEX idx_item_instances_room
    ON public.item_instances (realm_id, room_id)
    WHERE (room_id IS NOT NULL);

CREATE INDEX idx_item_instances_player
    ON public.item_instances (realm_id, account_id)
    WHERE (account_id IS NOT NULL);

CREATE INDEX idx_item_instances_object
    ON public.item_instances (realm_id, object_id)
    WHERE (object_id IS NOT NULL);

CREATE INDEX idx_item_instances_container
    ON public.item_instances (realm_id, container_item_id)
    WHERE (container_item_id IS NOT NULL);

CREATE INDEX idx_item_instances_stacking
    ON public.item_instances (realm_id, catalog_id, room_id, account_id, object_id, container_item_id);


-- =====================================================================
--  LOOT INSTANTIATION STATE
-- =====================================================================

CREATE TABLE public.loot_instantiation_state (
    realm_id        uuid                    NOT NULL,
    object_id       uuid                    NOT NULL,
    account_id      uuid                    NOT NULL,
    instantiated_at timestamp with time zone DEFAULT now() NOT NULL,
    PRIMARY KEY (realm_id, object_id, account_id)
);

ALTER TABLE public.loot_instantiation_state
    OWNER TO port4k;

CREATE INDEX idx_loot_state_object
    ON public.loot_instantiation_state (realm_id, object_id);
