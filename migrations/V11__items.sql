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

-- Item instances (actual spawned items)
CREATE TABLE item_instances (
    instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    zone_id UUID NOT NULL,
    catalog_id UUID NOT NULL REFERENCES bp_items_catalog(id),
    item_key VARCHAR(100) NOT NULL,

    -- Location (only ONE should be set)
    room_id UUID,
    account_id UUID,
    object_id UUID,
    container_item_id UUID,

    -- Item state
    quantity INTEGER NOT NULL DEFAULT 1,
    condition JSONB DEFAULT '{}',

    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Ensure only one location is set
    CHECK (
        (room_id IS NOT NULL)::int +
        (account_id IS NOT NULL)::int +
        (object_id IS NOT NULL)::int +
        (container_item_id IS NOT NULL)::int = 1
    )
);

CREATE INDEX idx_item_instances_zone ON item_instances(zone_id);
CREATE INDEX idx_item_instances_room ON item_instances(zone_id, room_id) WHERE room_id IS NOT NULL;
CREATE INDEX idx_item_instances_player ON item_instances(zone_id, account_id) WHERE account_id IS NOT NULL;
CREATE INDEX idx_item_instances_object ON item_instances(zone_id, object_id) WHERE object_id IS NOT NULL;
CREATE INDEX idx_item_instances_container ON item_instances(zone_id, container_item_id) WHERE container_item_id IS NOT NULL;
CREATE INDEX idx_item_instances_stacking ON item_instances(zone_id, catalog_id, room_id, account_id, object_id, container_item_id);

-- Track loot instantiation state
CREATE TABLE loot_instantiation_state (
    zone_id UUID NOT NULL,
    object_id UUID NOT NULL,
    account_id UUID,
    instantiated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    PRIMARY KEY (zone_id, object_id, account_id)
);

CREATE INDEX idx_loot_state_object ON loot_instantiation_state(zone_id, object_id);
