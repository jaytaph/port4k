WITH entry_room AS (
    SELECT r.id AS room_id
    FROM rooms r
        JOIN zones z ON z.id = r.zone_id
    WHERE z.key = 'start' AND r.key = 'entry'
)
INSERT INTO loot_spawns (room_id, item, qty_min, qty_max, interval_ms, max_instances, next_spawn_at)
SELECT room_id, 'coin', 1, 3, 10000, 5, now()
FROM entry_room
ON CONFLICT DO NOTHING;
