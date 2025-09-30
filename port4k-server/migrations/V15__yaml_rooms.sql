
ALTER TABLE bp_rooms
    ADD COLUMN IF NOT EXISTS short TEXT,
    ADD COLUMN IF NOT EXISTS hints JSONB,
    ADD COLUMN IF NOT EXISTS objects JSONB,
    ADD COLUMN IF NOT EXISTS scripts JSONB;

-- Optional helpful index for text search later
CREATE INDEX IF NOT EXISTS bp_rooms_short_gin ON bp_rooms USING gin (to_tsvector('simple', coalesce(short,'')));
CREATE INDEX IF NOT EXISTS bp_rooms_body_gin  ON bp_rooms USING gin (to_tsvector('simple', coalesce(body,'')));

-- Exits: add cosmetic + visibility flags (you already have locked)
ALTER TABLE bp_exits
    ADD COLUMN IF NOT EXISTS description TEXT,
    ADD COLUMN IF NOT EXISTS visible_when_locked BOOLEAN NOT NULL DEFAULT TRUE;
