-- Room-level scripts (optional)
CREATE TABLE public.bp_room_scripts
(
    bp_key         text        NOT NULL REFERENCES public.blueprints ON DELETE CASCADE,
    room_key       text        NOT NULL,
    on_enter_lua   text, -- room "on_enter"
    on_command_lua text, -- room "on_command"
    updated_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);

-- Objects that live in a room
CREATE TABLE public.bp_objects
(
    bp_key      text        NOT NULL REFERENCES public.blueprints ON DELETE CASCADE,
    room_key    text        NOT NULL,
    id          text        NOT NULL, -- e.g. 'blast_door' (stable per room)
    short       text        NOT NULL, -- one-liner
    description text        NOT NULL,
    examine     text,                 -- optional extra
    state       jsonb       NOT NULL DEFAULT '{}'::jsonb,
    use_lua     text,                 -- per-object verb handler
    position    integer,              -- optional author-specified order
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key, id),
    FOREIGN KEY (bp_key, room_key) REFERENCES public.bp_rooms (bp_key, key) ON DELETE CASCADE
);

-- Nouns/aliases for object targeting
CREATE TABLE public.bp_object_nouns
(
    bp_key   text NOT NULL,
    room_key text NOT NULL,
    obj_id   text NOT NULL,
    noun     text NOT NULL, -- store lowercased
    PRIMARY KEY (bp_key, room_key, noun),
    FOREIGN KEY (bp_key, room_key, obj_id)
        REFERENCES public.bp_objects (bp_key, room_key, id)
        ON DELETE CASCADE
);

-- Helpful indexes
CREATE INDEX bp_objects_desc_gin
    ON public.bp_objects
        USING gin (to_tsvector('simple', COALESCE(short, '') || ' ' || COALESCE(description, '')));

CREATE INDEX bp_object_state_gin
    ON public.bp_objects
        USING gin (state);

CREATE INDEX bp_object_noun_btree
    ON public.bp_object_nouns (bp_key, room_key, noun);