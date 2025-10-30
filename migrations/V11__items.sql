-- ============================================================================
-- Port4k Items System - Blueprint-Scoped (v5)
-- ============================================================================
-- Items are strictly scoped to blueprints and cannot cross blueprint boundaries
-- Each blueprint defines its own item catalog
-- Item instances exist within zones (which belong to blueprints)
-- ============================================================================

-- ============================================================================
-- BLUEPRINT-LEVEL: Item Catalog (Templates)
-- ============================================================================

-- Table: bp_items_catalog
-- Defines all items available within a blueprint
-- This is the "template" or "definition" of items
CREATE TABLE bp_items_catalog (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bp_id UUID NOT NULL REFERENCES blueprints(id) ON DELETE CASCADE,
    item_key VARCHAR(64) NOT NULL,      -- Unique identifier (e.g., "multi_spanner")
    name VARCHAR(255) NOT NULL,          -- Display name (e.g., "Multi-Spanner")
    short TEXT NOT NULL,                 -- Short description (e.g., "versatile multi-spanner")
    description TEXT NOT NULL,           -- Full description
    examine TEXT,                        -- Optional detailed examine text
    stackable BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure each item_key is unique within a blueprint
    CONSTRAINT uq_bp_items_catalog_bp_item UNIQUE(bp_id, item_key)
);

-- Table: bp_item_nouns
-- Searchable nouns for items (e.g., "spanner", "tool", "wrench")
-- Players use these nouns to reference items in commands
CREATE TABLE bp_item_nouns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bp_id UUID NOT NULL REFERENCES blueprints(id) ON DELETE CASCADE,
    item_id UUID NOT NULL REFERENCES bp_items_catalog(id) ON DELETE CASCADE,
    noun VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for blueprint catalog lookups
CREATE INDEX idx_bp_items_catalog_bp_id ON bp_items_catalog(bp_id);
CREATE INDEX idx_bp_items_catalog_item_key ON bp_items_catalog(item_key);
CREATE INDEX idx_bp_item_nouns_bp_id ON bp_item_nouns(bp_id);
CREATE INDEX idx_bp_item_nouns_item_id ON bp_item_nouns(item_id);
CREATE INDEX idx_bp_item_nouns_noun ON bp_item_nouns(noun);

-- ============================================================================
-- ZONE-LEVEL: Item Instances
-- ============================================================================

-- Table: items
-- Actual item instances that exist in the game world
-- Each item references its catalog definition and has a specific location
CREATE TABLE items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zone_id UUID NOT NULL REFERENCES zones(id) ON DELETE CASCADE,
    catalog_id UUID NOT NULL REFERENCES bp_items_catalog(id),

    -- Location: Exactly ONE of these must be set
    room_id UUID REFERENCES bp_rooms(id) ON DELETE CASCADE,
    account_id UUID REFERENCES accounts(id) ON DELETE CASCADE,
    object_id UUID REFERENCES bp_objects(id) ON DELETE CASCADE,
    container_item_id UUID REFERENCES items(id) ON DELETE CASCADE,

    -- Instance-specific properties
    quantity INTEGER NOT NULL DEFAULT 1 CHECK (quantity > 0),
    condition JSONB,  -- Durability, charges, enchantments, etc.

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraint: Exactly one location must be specified
    CONSTRAINT chk_item_single_location CHECK (
        (
            (room_id IS NOT NULL)::INTEGER +
            (account_id IS NOT NULL)::INTEGER +
            (object_id IS NOT NULL)::INTEGER +
            (container_item_id IS NOT NULL)::INTEGER
        ) = 1
    )
);

-- Core indexes
CREATE INDEX idx_items_zone_id ON items(zone_id);
CREATE INDEX idx_items_catalog_id ON items(catalog_id);

-- Location-specific indexes (partial indexes for better performance)
CREATE INDEX idx_items_room_id
    ON items(room_id)
    WHERE room_id IS NOT NULL;

CREATE INDEX idx_items_account_id
    ON items(account_id)
    WHERE account_id IS NOT NULL;

CREATE INDEX idx_items_object_id
    ON items(object_id)
    WHERE object_id IS NOT NULL;

CREATE INDEX idx_items_container_item_id
    ON items(container_item_id)
    WHERE container_item_id IS NOT NULL;

-- Composite indexes for common queries
CREATE INDEX idx_items_zone_account
    ON items(zone_id, account_id)
    WHERE account_id IS NOT NULL;

CREATE INDEX idx_items_zone_room
    ON items(zone_id, room_id)
    WHERE room_id IS NOT NULL;


-- Add to zone_object_loot_state table
CREATE TABLE zone_object_loot_state (
    zone_id UUID NOT NULL REFERENCES zones(id) ON DELETE CASCADE,
    object_id UUID NOT NULL REFERENCES bp_objects(id) ON DELETE CASCADE,
    account_id UUID,  -- NEW: NULL for shared, specific for per-player
    instantiated BOOLEAN NOT NULL DEFAULT FALSE,
    instantiated_at TIMESTAMPTZ,
    PRIMARY KEY (zone_id, object_id, account_id)
);

-- Partial indexes for performance
CREATE INDEX idx_zone_object_loot_state_shared
    ON zone_object_loot_state(zone_id, object_id)
    WHERE account_id IS NULL;

CREATE INDEX idx_zone_object_loot_state_player
    ON zone_object_loot_state(zone_id, object_id, account_id)
    WHERE account_id IS NOT NULL;