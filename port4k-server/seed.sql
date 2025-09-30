-- =========================
-- ACCOUNTS (with balances)
-- =========================
INSERT INTO accounts (username, role, password_hash, balance)
VALUES
  ('admin',  'admin',  '$argon2id$v=19$m=4096,t=3,p=1$YTRucTM0d2JrMmYwMDAwMA$ys7sXXH6ETEFmIVysP4fW6YQo5s6V/hy2VLrNF7CDEM', 100000),
  ('alice',  'player', '$argon2id$v=19$m=4096,t=3,p=1$MWIya3JwNmNnZTQwMDAwMA$jbsb0ayARAcFOHJ+tLIIR/mhd7ocQpOp0gTrW8cKPoQ',  2500),
  ('bob',    'player', '$argon2id$v=19$m=4096,t=3,p=1$ajlzMXB4Nm5sMHIwMDAwMA$msXwjUslddp3j8B7vRcPRXn84cAsXH2oPbqEjwl2yw4',  1500),
  ('carol',  'player', '$argon2id$v=19$m=4096,t=3,p=1$c3NudWZsM3UycWgwMDAwMA$rmhw1AzK4zZtbAEJyzKWaAMV56I5H4fF5qDvOWSGYPM',  500)
ON CONFLICT (username) DO NOTHING;

-- ======================================
-- ZONES (the migration already adds 'start')
-- ======================================
INSERT INTO zones (key, title)
VALUES
  ('hub',     'The Central Hub'),
  ('crypt',   'Forgotten Crypt')
ON CONFLICT (key) DO NOTHING;

-- ==============================
-- ROOMS for HUB zone (3 rooms)
-- ==============================
WITH z AS (SELECT id FROM zones WHERE key='hub')
INSERT INTO rooms (zone_id, key, title, body, flags)
SELECT z.id, v.key, v.title, v.body, v.flags::jsonb
FROM z, (VALUES
  ('plaza',  'Hub Plaza',        'A circular plaza with flickering holo-signs.', '[]'),
  ('west',   'Workshop Alley',   'Clanks and sparks echo from half-open garages.', '[]'),
  ('east',   'Bazaar Row',       'Vendors hawk wares from neon-lit stalls.', '[]')
) AS v(key,title,body,flags)
ON CONFLICT (zone_id, key) DO NOTHING;

-- Exits HUB
WITH z AS (SELECT id FROM zones WHERE key='hub'),
r AS (
  SELECT
    MAX(CASE WHEN key='plaza' THEN id END) plaza,
    MAX(CASE WHEN key='west'  THEN id END) west,
    MAX(CASE WHEN key='east'  THEN id END) east
  FROM rooms WHERE zone_id = (SELECT id FROM z)
)
INSERT INTO exits (from_room, dir, to_room, flags)
SELECT r.plaza, 'west',  r.west, '{}'::jsonb FROM r
UNION ALL
SELECT r.west,  'east',  r.plaza, '{}'::jsonb FROM r
UNION ALL
SELECT r.plaza, 'east',  r.east,  '{}'::jsonb FROM r
UNION ALL
SELECT r.east,  'west',  r.plaza, '{}'::jsonb FROM r
ON CONFLICT DO NOTHING;

-- ==============================
-- ROOMS for CRYPT zone (3 rooms)
-- ==============================
WITH z AS (SELECT id FROM zones WHERE key='crypt')
INSERT INTO rooms (zone_id, key, title, body, flags)
SELECT z.id, v.key, v.title, v.body, v.flags::jsonb
FROM z, (VALUES
  ('gate',    'Crypt Gate',      'A rusted gate guards a stairwell descending into dark.', '[]'),
  ('cat1',    'Catacomb I',      'Rows of niches hold ancient urns.', '[]'),
  ('shrine',  'Moon Shrine',     'A pale sigil glows on broken tiles.', '[]')
) AS v(key,title,body,flags)
ON CONFLICT (zone_id, key) DO NOTHING;

-- Exits CRYPT
WITH z AS (SELECT id FROM zones WHERE key='crypt'),
r AS (
  SELECT
    MAX(CASE WHEN key='gate'   THEN id END) gate,
    MAX(CASE WHEN key='cat1'   THEN id END) cat1,
    MAX(CASE WHEN key='shrine' THEN id END) shrine
  FROM rooms WHERE zone_id = (SELECT id FROM z)
)
INSERT INTO exits (from_room, dir, to_room, flags)
SELECT r.gate,   'down', r.cat1,  '{}'::jsonb FROM r
UNION ALL
SELECT r.cat1,   'up',   r.gate,  '{}'::jsonb FROM r
UNION ALL
SELECT r.cat1,   'east', r.shrine,'{}'::jsonb FROM r
UNION ALL
SELECT r.shrine, 'west', r.cat1, '{}'::jsonb FROM r
ON CONFLICT DO NOTHING;

-- ==================================================
-- LINK between START zone and HUB (optional travel)
-- ==================================================
WITH start_z AS (SELECT id FROM zones WHERE key='start'),
     hub_z   AS (SELECT id FROM zones WHERE key='hub'),
     r AS (
       SELECT
         (SELECT id FROM rooms WHERE zone_id=(SELECT id FROM start_z) AND key='entry') AS start_entry,
         (SELECT id FROM rooms WHERE zone_id=(SELECT id FROM hub_z)   AND key='plaza') AS hub_plaza
     )
INSERT INTO exits (from_room, dir, to_room)
SELECT start_entry, 'portal', hub_plaza FROM r
UNION ALL
SELECT hub_plaza, 'portal', start_entry FROM r
ON CONFLICT DO NOTHING;

-- ===================================
-- SCRIPTS (example Lua) + ROOM HOOKS
-- ===================================
-- Simple on_enter script: gives a short description
INSERT INTO scripts (lang, source, sha256)
VALUES
('lua',
$LUA$
function on_enter(ctx)
  ctx:send("A cool breeze passes through. You sense hidden stories here.")
end
$LUA$,
'c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0')
ON CONFLICT DO NOTHING;

-- Simple on_command script: handles 'look' and 'pray'
INSERT INTO scripts (lang, source, sha256)
VALUES
('lua',
$LUA$
function on_command(ctx, verb, args)
  if verb == "look" then
    ctx:send("You take a careful look around...")
    return
  elseif verb == "pray" then
    ctx:send("A faint chime answers your prayer.")
    return
  end
  -- Let engine handle unknowns
end
$LUA$,
'5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f')
ON CONFLICT DO NOTHING;

-- Attach scripts to specific rooms
WITH hub AS (SELECT id FROM zones WHERE key='hub'),
     plaza_room AS (SELECT id FROM rooms WHERE zone_id=(SELECT id FROM hub) AND key='plaza'),
     s_enter AS (SELECT id FROM scripts WHERE sha256='c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0c9b1e9a0'),
     s_cmd   AS (SELECT id FROM scripts WHERE sha256='5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f')
INSERT INTO room_scripts (room_id, event, script_id)
SELECT (SELECT id FROM plaza_room), 'on_enter',  (SELECT id FROM s_enter)
ON CONFLICT DO NOTHING;

WITH hub AS (SELECT id FROM zones WHERE key='hub'),
     plaza_room AS (SELECT id FROM rooms WHERE zone_id=(SELECT id FROM hub) AND key='plaza'),
     s_cmd   AS (SELECT id FROM scripts WHERE sha256='5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f5c2c2d1f')
INSERT INTO room_scripts (room_id, event, script_id)
SELECT (SELECT id FROM plaza_room), 'on_command', (SELECT id FROM s_cmd)
ON CONFLICT DO NOTHING;

-- ======================
-- LOOT SPAWNS / ROOM LOOT
-- ======================
-- Add an extra spawn to Hub Plaza
WITH hub AS (SELECT id FROM zones WHERE key='hub'),
     plaza AS (SELECT id AS room_id FROM rooms WHERE zone_id=(SELECT id FROM hub) AND key='plaza')
INSERT INTO loot_spawns (room_id, item, qty_min, qty_max, interval_ms, max_instances, next_spawn_at)
SELECT room_id, 'coin', 1, 4, 8000, 10, now()
FROM plaza
ON CONFLICT DO NOTHING;

-- Seed a couple of coins already lying around in START/entry (unpicked)
WITH entry_room AS (
  SELECT r.id AS room_id
  FROM rooms r
  JOIN zones z ON z.id=r.zone_id
  WHERE z.key='start' AND r.key='entry'
)
INSERT INTO room_loot (room_id, item, qty)
SELECT room_id, 'coin', 2 FROM entry_room
UNION ALL
SELECT room_id, 'coin', 1 FROM entry_room
ON CONFLICT DO NOTHING;

-- ===========================
-- CHARACTERS (start in START)
-- ===========================
WITH entry_room AS (
  SELECT r.id AS room_id
  FROM rooms r JOIN zones z ON z.id=r.zone_id
  WHERE z.key='start' AND r.key='entry'
)
INSERT INTO characters (account_name, name, location_id, stats_json)
SELECT 'alice', 'Aliza',  room_id, '{"level":1,"hp":10,"mp":4}'::jsonb FROM entry_room
UNION ALL
SELECT 'bob',   'Boric',  room_id, '{"level":1,"hp":12,"mp":1}'::jsonb FROM entry_room
UNION ALL
SELECT 'carol', 'Carys',  room_id, '{"level":1,"hp":8,"mp":8}' ::jsonb FROM entry_room
ON CONFLICT DO NOTHING;
