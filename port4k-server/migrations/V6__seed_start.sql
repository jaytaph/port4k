INSERT INTO zones (key, title) VALUES ('start','The Starting Zone')
    ON CONFLICT (key) DO NOTHING;

WITH z AS (SELECT id FROM zones WHERE key='start')
INSERT INTO rooms (zone_id, key, title, body)
SELECT z.id, 'entry', 'Entry Hall', 'A small, quiet room lit by phosphor glow.'
FROM z
    ON CONFLICT DO NOTHING;

WITH z AS (SELECT id FROM zones WHERE key='start')
INSERT INTO rooms (zone_id, key, title, body)
SELECT z.id, 'north', 'North Alcove', 'Dusty shelves line the walls.'
FROM z
    ON CONFLICT DO NOTHING;

WITH z AS (SELECT id FROM zones WHERE key='start')
INSERT INTO rooms (zone_id, key, title, body)
SELECT z.id, 'east', 'East Walkway', 'A narrow passage stretches on.'
FROM z
    ON CONFLICT DO NOTHING;

-- exits
WITH z AS (SELECT id FROM zones WHERE key='start'),
     e AS (SELECT r1.id AS entry, r2.id AS north, r3.id AS east
           FROM z
                    JOIN rooms r1 ON r1.zone_id=z.id AND r1.key='entry'
                    JOIN rooms r2 ON r2.zone_id=z.id AND r2.key='north'
                    JOIN rooms r3 ON r3.zone_id=z.id AND r3.key='east')
INSERT INTO exits (from_room, dir, to_room)
SELECT entry, 'north', north FROM e
ON CONFLICT DO NOTHING;

WITH z AS (SELECT id FROM zones WHERE key='start'),
     e AS (SELECT r1.id AS entry, r2.id AS north, r3.id AS east
           FROM z
                    JOIN rooms r1 ON r1.zone_id=z.id AND r1.key='entry'
                    JOIN rooms r2 ON r2.zone_id=z.id AND r2.key='north'
                    JOIN rooms r3 ON r3.zone_id=z.id AND r3.key='east')
INSERT INTO exits (from_room, dir, to_room)
SELECT north, 'south', entry FROM e
ON CONFLICT DO NOTHING;

WITH z AS (SELECT id FROM zones WHERE key='start'),
     e AS (SELECT r1.id AS entry, r2.id AS north, r3.id AS east
           FROM z
                    JOIN rooms r1 ON r1.zone_id=z.id AND r1.key='entry'
                    JOIN rooms r2 ON r2.zone_id=z.id AND r2.key='north'
                    JOIN rooms r3 ON r3.zone_id=z.id AND r3.key='east')
INSERT INTO exits (from_room, dir, to_room)
SELECT entry, 'east', east FROM e
ON CONFLICT DO NOTHING;

WITH z AS (SELECT id FROM zones WHERE key='start'),
     e AS (SELECT r1.id AS entry, r2.id AS north, r3.id AS east
           FROM z
                    JOIN rooms r1 ON r1.zone_id=z.id AND r1.key='entry'
                    JOIN rooms r2 ON r2.zone_id=z.id AND r2.key='north'
                    JOIN rooms r3 ON r3.zone_id=z.id AND r3.key='east')
INSERT INTO exits (from_room, dir, to_room)
SELECT east, 'west', entry FROM e
ON CONFLICT DO NOTHING;
