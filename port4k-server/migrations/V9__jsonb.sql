ALTER TABLE bp_room_kv
    ADD CONSTRAINT chk_bp_room_kv_value_string_or_string_array
        CHECK (
            jsonb_typeof(value) = 'string'
                OR (
                jsonb_typeof(value) = 'array'
                    AND NOT EXISTS (
                    SELECT 1
                    FROM jsonb_array_elements(value) AS e
                    WHERE jsonb_typeof(e) <> 'string'
                )
                )
            );