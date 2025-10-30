ALTER TABLE public.bp_rooms
    ADD COLUMN IF NOT EXISTS o text;

ALTER TABLE public.bp_objects
    ADD COLUMN IF NOT EXISTS flags    jsonb DEFAULT '[]'::jsonb NOT NULL,
    ADD COLUMN IF NOT EXISTS visible  text,
    ADD COLUMN IF NOT EXISTS controls jsonb DEFAULT '[]'::jsonb NOT NULL,
    ADD COLUMN IF NOT EXISTS loot     jsonb DEFAULT '{}'::jsonb NOT NULL;

ALTER TABLE public.bp_objects
    ADD CONSTRAINT bp_objects_visible_chk
        CHECK (
            visible IS NULL OR visible IN ('always','when_revealed','when_unlocked','script')
            );

CREATE INDEX IF NOT EXISTS bp_objects_flags_gin    ON public.bp_objects USING gin (flags);
CREATE INDEX IF NOT EXISTS bp_objects_controls_gin ON public.bp_objects USING gin (controls);
CREATE INDEX IF NOT EXISTS bp_objects_loot_gin     ON public.bp_objects USING gin (loot);


CREATE TABLE IF NOT EXISTS public.bp_items_catalog (
    id     uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    bp_id  uuid NOT NULL REFERENCES public.blueprints ON DELETE CASCADE,
    item_id   text NOT NULL,               -- e.g. "microcell"
    name      text NOT NULL,               -- "Microcell"
    stackable boolean NOT NULL DEFAULT false,
    UNIQUE (bp_id, item_id)
);

CREATE INDEX IF NOT EXISTS bp_items_catalog_bp_idx ON public.bp_items_catalog (bp_id);

ALTER TABLE bp_rooms DROP COLUMN objects;
ALTER TABLE bp_rooms DROP COLUMN scripts;