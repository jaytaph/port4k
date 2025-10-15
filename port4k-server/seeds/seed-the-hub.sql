BEGIN;

-- 1) Zone: "The Hub"
INSERT INTO public.zones (key, title)
VALUES ('hub', 'The Hub')
ON CONFLICT (key) DO NOTHING;

-- 2) Owner account (needed by blueprints.owner_id)
INSERT INTO public.accounts (username, email, role)
VALUES ('system', 'system@example.com', 'admin')
ON CONFLICT (username) DO NOTHING;

-- 3) Blueprint for the Hub
INSERT INTO public.blueprints (key, title, owner_id, status)
SELECT 'hub', 'The Hub', a.id, 'live'
FROM public.accounts a
WHERE a.username = 'system'
ON CONFLICT (key) DO NOTHING;

-- 4) Rooms (entry + a small loop + a locked vault)
--    keys: entry, north_corridor, east_corridor, vault
INSERT INTO public.bp_rooms (bp_id, key, title, body, short, lockdown, hints, objects, scripts)
SELECT b.id, 'entry', 'The Hub',
       'You stand in a circular chamber buzzing with quiet energy. Corridors lead north and east. A heavy door hints at something valuable nearby.',
       'The central hub hums softly.',
       FALSE, '[]'::jsonb, '[]'::jsonb, '[]'::jsonb
FROM public.blueprints b
WHERE b.key = 'hub'
ON CONFLICT (bp_id, key) DO NOTHING;

INSERT INTO public.bp_rooms (bp_id, key, title, body, short, lockdown, hints, objects, scripts)
SELECT b.id, 'north_corridor', 'North Corridor',
       'A dim hallway with exposed conduits along the wall. The hub lies to the south.',
       'A dim north corridor.',
       FALSE, '[]'::jsonb, '[]'::jsonb, '[]'::jsonb
FROM public.blueprints b
WHERE b.key = 'hub'
ON CONFLICT (bp_id, key) DO NOTHING;

INSERT INTO public.bp_rooms (bp_id, key, title, body, short, lockdown, hints, objects, scripts)
SELECT b.id, 'east_corridor', 'East Corridor',
       'A narrow passage cluttered with crates. The hub is back to the west. A reinforced hatch lies further on.',
       'A cluttered east corridor.',
       FALSE, '[]'::jsonb, '[]'::jsonb, '[]'::jsonb
FROM public.blueprints b
WHERE b.key = 'hub'
ON CONFLICT (bp_id, key) DO NOTHING;

INSERT INTO public.bp_rooms (bp_id, key, title, body, short, lockdown, hints, objects, scripts)
SELECT b.id, 'vault', 'Maintenance Vault',
       'A compact vault with neatly arranged supplies. The blast door to the west looks sturdy.',
       'A tidy maintenance vault.',
       FALSE, '[]'::jsonb, '[]'::jsonb, '[]'::jsonb
FROM public.blueprints b
WHERE b.key = 'hub'
ON CONFLICT (bp_id, key) DO NOTHING;

-- 5) Set blueprint.entry_room_id to the Hub (entry)
UPDATE public.blueprints b
SET entry_room_id = r.id
FROM public.bp_rooms r
WHERE b.key = 'hub'
  AND r.key = 'entry'
  AND r.bp_id = b.id;

-- Helper subselects for exits/objects to keep things readable
-- (Postgres inlines these just fine.)
-- 6) Exits between rooms (vault door is locked)
INSERT INTO public.bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
SELECT r_from.id, 'north', r_to.id, FALSE, 'A corridor leading north from the hub.', TRUE
FROM public.blueprints b
         JOIN public.bp_rooms r_from ON r_from.bp_id = b.id AND r_from.key = 'entry'
         JOIN public.bp_rooms r_to   ON r_to.bp_id   = b.id AND r_to.key   = 'north_corridor'
WHERE b.key = 'hub'
ON CONFLICT (from_room_id, dir) DO NOTHING;

INSERT INTO public.bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
SELECT r_from.id, 'south', r_to.id, FALSE, 'Back to the hub.', TRUE
FROM public.blueprints b
         JOIN public.bp_rooms r_from ON r_from.bp_id = b.id AND r_from.key = 'north_corridor'
         JOIN public.bp_rooms r_to   ON r_to.bp_id   = b.id AND r_to.key   = 'entry'
WHERE b.key = 'hub'
ON CONFLICT (from_room_id, dir) DO NOTHING;

INSERT INTO public.bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
SELECT r_from.id, 'east', r_to.id, FALSE, 'A narrow passage heading east.', TRUE
FROM public.blueprints b
         JOIN public.bp_rooms r_from ON r_from.bp_id = b.id AND r_from.key = 'entry'
         JOIN public.bp_rooms r_to   ON r_to.bp_id   = b.id AND r_to.key   = 'east_corridor'
WHERE b.key = 'hub'
ON CONFLICT (from_room_id, dir) DO NOTHING;

INSERT INTO public.bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
SELECT r_from.id, 'west', r_to.id, FALSE, 'Back to the hub.', TRUE
FROM public.blueprints b
         JOIN public.bp_rooms r_from ON r_from.bp_id = b.id AND r_from.key = 'east_corridor'
         JOIN public.bp_rooms r_to   ON r_to.bp_id   = b.id AND r_to.key   = 'entry'
WHERE b.key = 'hub'
ON CONFLICT (from_room_id, dir) DO NOTHING;

-- Locked door from east_corridor -> vault
INSERT INTO public.bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
SELECT r_from.id, 'in', r_to.id, TRUE, 'A heavy blast door blocks the way in.', TRUE
FROM public.blueprints b
         JOIN public.bp_rooms r_from ON r_from.bp_id = b.id AND r_from.key = 'east_corridor'
         JOIN public.bp_rooms r_to   ON r_to.bp_id   = b.id AND r_to.key   = 'vault'
WHERE b.key = 'hub'
ON CONFLICT (from_room_id, dir) DO NOTHING;

-- And from vault -> east_corridor (usually locked from outside)
INSERT INTO public.bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
SELECT r_from.id, 'out', r_to.id, TRUE, 'The blast door can be opened from here.', TRUE
FROM public.blueprints b
         JOIN public.bp_rooms r_from ON r_from.bp_id = b.id AND r_from.key = 'vault'
         JOIN public.bp_rooms r_to   ON r_to.bp_id   = b.id AND r_to.key   = 'east_corridor'
WHERE b.key = 'hub'
ON CONFLICT (from_room_id, dir) DO NOTHING;

-- 7) Objects + nouns
-- Hub terminal
WITH hub_room AS (
    SELECT r.id AS room_id
    FROM public.blueprints b
             JOIN public.bp_rooms r ON r.bp_id = b.id AND r.key = 'entry'
    WHERE b.key = 'hub'
),
     ins AS (
         INSERT INTO public.bp_objects (room_id, name, short, description, examine, state, position)
             SELECT room_id, 'terminal', 'a flickering terminal',
                    'An aging console with a phosphor display.',
                    'The terminal shows a blinking cursor: READY _',
                    '[]'::jsonb, 1
             FROM hub_room
             ON CONFLICT DO NOTHING
             RETURNING id, room_id
     )
INSERT INTO public.bp_object_nouns (room_id, obj_id, noun)
SELECT i.room_id, i.id, n
FROM ins i
         JOIN LATERAL (VALUES ('terminal'), ('console'), ('computer')) AS v(n) ON TRUE
ON CONFLICT DO NOTHING;

-- North corridor poster
WITH n_room AS (
    SELECT r.id AS room_id
    FROM public.blueprints b
             JOIN public.bp_rooms r ON r.bp_id = b.id AND r.key = 'north_corridor'
    WHERE b.key = 'hub'
),
     ins AS (
         INSERT INTO public.bp_objects (room_id, name, short, description, examine, state, position)
             SELECT room_id, 'poster', 'a peeling poster',
                    'A faded poster warns: "AUTHORIZED PERSONNEL ONLY".',
                    'The date is smudged beyond recognition.',
                    '[]'::jsonb, 1
             FROM n_room
             ON CONFLICT DO NOTHING
             RETURNING id, room_id
     )
INSERT INTO public.bp_object_nouns (room_id, obj_id, noun)
SELECT i.room_id, i.id, n
FROM ins i
         JOIN LATERAL (VALUES ('poster'), ('sign')) AS v(n) ON TRUE
ON CONFLICT DO NOTHING;

-- East corridor crate
WITH e_room AS (
    SELECT r.id AS room_id
    FROM public.blueprints b
             JOIN public.bp_rooms r ON r.bp_id = b.id AND r.key = 'east_corridor'
    WHERE b.key = 'hub'
),
     ins AS (
         INSERT INTO public.bp_objects (room_id, name, short, description, examine, state, position)
             SELECT room_id, 'crate', 'a sealed crate',
                    'A sturdy cargo crate sealed with a magnetic clasp.',
                    'There''s a small notch that looks like it fits a card.',
                    '[]'::jsonb, 1
             FROM e_room
             ON CONFLICT DO NOTHING
             RETURNING id, room_id
     )
INSERT INTO public.bp_object_nouns (room_id, obj_id, noun)
SELECT i.room_id, i.id, n
FROM ins i
         JOIN LATERAL (VALUES ('crate'), ('box')) AS v(n) ON TRUE
ON CONFLICT DO NOTHING;

-- Vault chest
WITH v_room AS (
    SELECT r.id AS room_id
    FROM public.blueprints b
             JOIN public.bp_rooms r ON r.bp_id = b.id AND r.key = 'vault'
    WHERE b.key = 'hub'
),
     ins AS (
         INSERT INTO public.bp_objects (room_id, name, short, description, examine, state, position)
             SELECT room_id, 'chest', 'a compact supply chest',
                    'A compact chest with emergency supplies.',
                    'It''s locked but the mechanism looks simple.',
                    '[]'::jsonb, 1
             FROM v_room
             ON CONFLICT DO NOTHING
             RETURNING id, room_id
     )
INSERT INTO public.bp_object_nouns (room_id, obj_id, noun)
SELECT i.room_id, i.id, n
FROM ins i
         JOIN LATERAL (VALUES ('chest'), ('box'), ('supplies')) AS v(n) ON TRUE
ON CONFLICT DO NOTHING;

COMMIT;