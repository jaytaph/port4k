CREATE TABLE IF NOT EXISTS blueprint_scripts_draft
(
    bp_key     TEXT        NOT NULL,
    room_key   TEXT        NOT NULL,
    event      TEXT        NOT NULL, -- e.g. 'on_command','on_enter','on_timer'
    source     TEXT        NOT NULL,
    author     TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key, event)
);

CREATE TABLE IF NOT EXISTS blueprint_scripts_live
(
    bp_key     TEXT        NOT NULL,
    room_key   TEXT        NOT NULL,
    event      TEXT        NOT NULL,
    source     TEXT        NOT NULL,
    author     TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (bp_key, room_key, event)
);
