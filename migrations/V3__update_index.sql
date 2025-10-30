-- ALTER TABLE bp_exits
--     ADD CONSTRAINT bp_exits_dir_check
--         CHECK (dir ~ '^(n|ne|e|se|s|sw|w|nw|u|d)$');

CREATE UNIQUE INDEX IF NOT EXISTS bp_exits_from_dir_unique
    ON bp_exits (from_room_id, dir);