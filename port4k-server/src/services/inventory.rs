use std::sync::Arc;
use crate::db::repo::InventoryRepo;
use crate::error::{AppResult, DomainError};
use crate::models::inventory::{Item, ItemInstance, ItemLocation};
use crate::models::types::{AccountId, ItemId, ZoneId, ObjectId, RoomId};

pub struct InventoryService {
    repo: Arc<dyn InventoryRepo>,
}

impl InventoryService {
    pub fn new(repo: Arc<dyn InventoryRepo>) -> Self {
        Self { repo }
    }

    // ========================================================================
    // ITEM CATALOG QUERIES (Blueprint-level templates)
    // ========================================================================

    /// Get item definition from catalog by item_key
    pub async fn get_item_by_key(&self, zone_id: ZoneId, item_key: &str) -> AppResult<Item> {
        let item = self.repo.get_item_by_key(zone_id, item_key).await?;
        Ok(item)
    }

    /// Get item definition from catalog by catalog_id
    pub async fn get_item_by_id(&self, item_id: ItemId) -> AppResult<Item> {
        let item = self.repo.get_item_by_id(item_id).await?;
        Ok(item)
    }

    /// Search for item by noun (returns catalog entry)
    pub async fn find_item_by_noun(&self, zone_id: ZoneId, noun: &str) -> AppResult<Option<Item>> {
        let item = self.repo.find_item_by_noun(zone_id, noun).await?;
        Ok(item)
    }

    /// Get all items available in blueprint catalog for a zone
    pub async fn get_zone_catalog(&self, zone_id: ZoneId) -> AppResult<Vec<Item>> {
        let catalog = self.repo.get_zone_catalog(zone_id).await?;
        Ok(catalog)
    }

    // ========================================================================
    // ITEM INSTANCE QUERIES (Zone-level instances)
    // ========================================================================

    /// Get a specific item instance by its instance ID
    pub async fn get_item_instance(&self, instance_id: ItemId) -> AppResult<ItemInstance> {
        let instance = self.repo.get_item_instance(instance_id).await?;
        Ok(instance)
    }

    /// Check if player has a specific item instance in their inventory
    pub async fn has_item(&self, zone_id: ZoneId, account_id: AccountId, instance_id: ItemId) -> AppResult<bool> {
        let b = self.repo.has_item(zone_id, account_id, instance_id).await?;
        Ok(b)
    }

    /// Check if player has any item matching the item_key
    pub async fn has_item_by_key(&self, zone_id: ZoneId, account_id: AccountId, item_key: &str) -> AppResult<bool> {
        let b = self.repo.has_item_by_key(zone_id, account_id, item_key).await?;
        Ok(b)
    }

    /// Check if player has item matching noun
    pub async fn has_item_by_noun(&self, zone_id: ZoneId, account_id: AccountId, noun: &str) -> AppResult<bool> {
        let b = self.repo.has_item_by_noun(zone_id, account_id, noun).await?;
        Ok(b)
    }

    // ========================================================================
    // PLAYER INVENTORY
    // ========================================================================

    /// Get all items in player's inventory
    pub async fn get_player_inventory(&self, zone_id: ZoneId, account_id: AccountId) -> AppResult<Vec<ItemInstance>> {
        let items = self.repo.get_player_inventory(zone_id, account_id).await?;
        Ok(items)
    }

    /// Get player inventory grouped by item type (for display)
    pub async fn get_player_inventory_summary(&self, zone_id: ZoneId, account_id: AccountId) -> AppResult<Vec<InventorySummaryItem>> {
        let instances = self.get_player_inventory(zone_id, account_id).await?;

        // Group by catalog_id
        let mut summary: std::collections::HashMap<ItemId, InventorySummaryItem> = std::collections::HashMap::new();

        for instance in instances {
            summary.entry(instance.catalog_id)
                .and_modify(|e| e.quantity += instance.quantity)
                .or_insert(InventorySummaryItem {
                    catalog_id: instance.catalog_id,
                    item_key: instance.item_key.clone(),
                    name: instance.name.clone(),
                    short: Some(instance.short.clone()),
                    quantity: instance.quantity,
                    stackable: instance.stackable,
                });
        }

        Ok(summary.into_values().collect())
    }

    /// Find specific item in player inventory by noun
    pub async fn find_in_inventory(&self, zone_id: ZoneId, account_id: AccountId, noun: &str) -> AppResult<Option<ItemInstance>> {
        let instance = self.repo.find_item_in_player_inventory(zone_id, account_id, noun).await?;
        Ok(instance)
    }

    // ========================================================================
    // ROOM ITEMS
    // ========================================================================

    /// Get all items in a room (on the ground)
    pub async fn get_room_items(&self, zone_id: ZoneId, room_id: RoomId) -> AppResult<Vec<ItemInstance>> {
        let items = self.repo.get_room_items(zone_id, room_id).await?;
        Ok(items)
    }

    /// Find item in room by noun
    pub async fn find_in_room(&self, zone_id: ZoneId, room_id: RoomId, noun: &str) -> AppResult<Option<ItemInstance>> {
        let items = self.repo.find_item_in_room(zone_id, room_id, noun).await?;
        Ok(items)
    }

    // ========================================================================
    // OBJECT ITEMS (Containers/Loot)
    // ========================================================================

    /// Get all items inside an object/container
    pub async fn get_object_items(&self, zone_id: ZoneId, object_id: ObjectId) -> AppResult<Vec<ItemInstance>> {
        let items = self.repo.get_object_items(zone_id, object_id).await?;
        Ok(items)
    }

    /// Find item inside object by noun
    pub async fn find_in_object(&self, zone_id: ZoneId, object_id: ObjectId, noun: &str) -> AppResult<Option<ItemInstance>> {
        let items = self.repo.find_item_in_object(zone_id, object_id, noun).await?;
        Ok(items)
    }

    // ========================================================================
    // LOOT INSTANTIATION
    // ========================================================================

    /// Check if object's loot has been instantiated for a player (or globally)
    pub async fn is_loot_instantiated(&self, zone_id: ZoneId, object_id: ObjectId, account_id: Option<AccountId>) -> AppResult<bool> {
        let b = self.repo.is_loot_instantiated(zone_id, object_id, account_id).await?;
        Ok(b)
    }

    /// Instantiate loot from an object's loot definition
    /// This is called when player first examines/searches an object
    ///
    /// If loot is shared (global): spawns items inside the object
    /// If loot is per-player: spawns items directly to player inventory
    pub async fn instantiate_loot(
        &self,
        zone_id: ZoneId,
        object_id: ObjectId,
        account_id: AccountId,
        loot_config: &LootConfig,
    ) -> AppResult<LootInstantiationResult> {
        // Determine account_id for state tracking
        let state_account_id = if loot_config.shared {
            None  // Global loot
        } else {
            Some(account_id)  // Per-player loot
        };

        // Check if already instantiated
        if self.is_loot_instantiated(zone_id, object_id, state_account_id).await? {
            return Ok(LootInstantiationResult::AlreadyInstantiated);
        }

        let mut spawned_items = Vec::new();

        // Spawn items
        for item_key in &loot_config.items {
            let location = if loot_config.shared {
                // Shared: spawn inside object (no owner yet)
                ItemLocation::Object(object_id)
            } else {
                // Per-player: spawn directly to player inventory
                ItemLocation::Player(account_id)
            };

            let instance_id = self.spawn_item(
                zone_id,
                item_key,
                location,
                1,  // quantity
            ).await?;

            spawned_items.push(instance_id);
        }

        // Mark as instantiated
        self.repo.mark_loot_instantiated(zone_id, object_id, state_account_id).await?;

        Ok(LootInstantiationResult::Instantiated {
            items: spawned_items,
            credits: loot_config.credits,
            shared: loot_config.shared,
        })
    }

    // ========================================================================
    // ITEM SPAWNING
    // ========================================================================

    /// Spawn a new item instance from catalog
    /// Automatically handles stacking if item is stackable
    pub async fn spawn_item(
        &self,
        zone_id: ZoneId,
        item_key: &str,
        location: ItemLocation,
        quantity: i32,
    ) -> AppResult<ItemId> {
        if quantity <= 0 {
            return Err(DomainError::Validation {
                field: "quantity",
                message: "Quantity must be positive".to_string(),
            });
        }

        let item_id = self.repo.spawn_item(zone_id, item_key, location, quantity).await?;
        Ok(item_id)
    }

    /// Spawn multiple items at once
    pub async fn spawn_items_bulk(
        &self,
        zone_id: ZoneId,
        items: Vec<(String, ItemLocation, i32)>, // (item_key, location, quantity)
    ) -> AppResult<Vec<ItemId>> {
        let mut spawned = Vec::new();

        for (item_key, location, quantity) in items {
            let instance_id = self.spawn_item(zone_id, &item_key, location, quantity).await?;
            spawned.push(instance_id);
        }

        Ok(spawned)
    }

    // ========================================================================
    // ITEM MOVEMENT
    // ========================================================================

    /// Move item to a new location
    /// Automatically merges with existing stacks if applicable
    pub async fn move_item(&self, instance_id: ItemId, new_location: ItemLocation) -> AppResult<()> {
        self.repo.move_item(instance_id, new_location).await?;
        Ok(())
    }

    /// Take item from room/object and put in player inventory
    pub async fn take_item(&self, instance_id: ItemId, account_id: AccountId) -> AppResult<()> {
        self.move_item(instance_id, ItemLocation::Player(account_id)).await
    }

    /// Drop item from inventory to room
    pub async fn drop_item(&self, instance_id: ItemId, room_id: RoomId) -> AppResult<()> {
        self.move_item(instance_id, ItemLocation::Room(room_id)).await
    }

    /// Put item into object/container
    pub async fn put_item_in_object(&self, instance_id: ItemId, object_id: ObjectId) -> AppResult<()> {
        self.move_item(instance_id, ItemLocation::Object(object_id)).await
    }

    /// Put item inside another item (nested containers)
    pub async fn put_item_in_container(&self, instance_id: ItemId, container_id: ItemId) -> AppResult<()> {
        self.move_item(instance_id, ItemLocation::Container(container_id)).await
    }

    /// Transfer item from one player to another
    pub async fn transfer_item(
        &self,
        zone_id: ZoneId,
        instance_id: ItemId,
        from_account: AccountId,
        to_account: AccountId,
    ) -> AppResult<()> {
        // Verify ownership
        if !self.has_item(zone_id, from_account, instance_id).await? {
            return Err(DomainError::NotFound("Item not found in player inventory".to_string()));
        }

        self.move_item(instance_id, ItemLocation::Player(to_account)).await
    }

    // ========================================================================
    // ITEM MODIFICATION
    // ========================================================================

    /// Consume/remove item (e.g., eating food, using consumable)
    /// For stackable items, reduces quantity by 1
    /// For non-stackable items, removes the item entirely
    pub async fn consume_item(&self, zone_id: ZoneId, account_id: AccountId, instance_id: ItemId) -> AppResult<()> {
        // Verify ownership
        if !self.has_item(zone_id, account_id, instance_id).await? {
            return Err(DomainError::NotFound("Item not found in player inventory".to_string()));
        }

        self.repo.consume_item(zone_id, account_id, instance_id).await?;
        Ok(())
    }

    /// Update item quantity (for stackable items)
    pub async fn set_item_quantity(&self, instance_id: ItemId, quantity: i32) -> AppResult<()> {
        if quantity <= 0 {
            return Err(DomainError::Validation {
                field: "quantity",
                message: "Quantity must be positive".to_string(),
            });
        }

        self.repo.set_item_quantity(instance_id, quantity).await?;
        Ok(())
    }

    /// Update item condition (durability, charges, custom state)
    pub async fn set_item_condition(&self, instance_id: ItemId, condition: serde_json::Value) -> AppResult<()> {
        self.repo.set_item_condition(instance_id, condition).await?;
        Ok(())
    }

    /// Delete item instance entirely
    pub async fn delete_item(&self, instance_id: ItemId) -> AppResult<()> {
        self.repo.delete_item(instance_id).await?;
        Ok(())
    }

    // ========================================================================
    // CONVENIENCE METHODS
    // ========================================================================

    /// Add item to player inventory by item_key
    pub async fn add_item(&self, zone_id: ZoneId, account_id: AccountId, item_key: &str, quantity: i32) -> AppResult<ItemId> {
        self.spawn_item(
            zone_id,
            item_key,
            ItemLocation::Player(account_id),
            quantity,
        ).await
    }

    /// Remove item from player inventory by item_key
    pub async fn remove_item_by_key(&self, zone_id: ZoneId, account_id: AccountId, item_key: &str, quantity: i32) -> AppResult<()> {
        // Find item in inventory
        let item = self.repo.find_item_by_key_in_inventory(zone_id, account_id, item_key).await?
            .ok_or_else(|| DomainError::NotFound(format!("Item '{}' not found in inventory", item_key)))?;

        if item.quantity < quantity {
            return Err(DomainError::Validation {
                field: "quantity",
                message: format!("Not enough items. Have {}, need {}", item.quantity, quantity),
            });
        }

        if item.quantity == quantity {
            // Delete entire stack
            self.delete_item(item.instance_id).await
        } else {
            // Reduce quantity
            self.set_item_quantity(item.instance_id, item.quantity - quantity).await
        }
    }

    /// Get item count in player inventory
    pub async fn get_item_count(&self, zone_id: ZoneId, account_id: AccountId, item_key: &str) -> AppResult<i32> {
        let inventory = self.get_player_inventory(zone_id, account_id).await?;

        let total = inventory.iter()
            .filter(|item| item.item_key == item_key)
            .map(|item| item.quantity)
            .sum();

        Ok(total)
    }

    /// Check if player has minimum quantity of item
    pub async fn has_minimum_quantity(&self, zone_id: ZoneId, account_id: AccountId, item_key: &str, min_quantity: i32) -> AppResult<bool> {
        let count = self.get_item_count(zone_id, account_id, item_key).await?;
        Ok(count >= min_quantity)
    }
}

// ============================================================================
// SUPPORTING TYPES
// ============================================================================

/// Configuration for object loot
#[derive(Debug, Clone)]
pub struct LootConfig {
    pub items: Vec<String>,      // item_keys from catalog
    pub credits: i32,
    pub once: bool,               // Can only be looted once
    pub shared: bool,             // false = per-player, true = global
}

/// Result of loot instantiation
#[derive(Debug)]
pub enum LootInstantiationResult {
    /// Loot was already instantiated
    AlreadyInstantiated,
    /// Loot was successfully instantiated
    Instantiated {
        items: Vec<ItemId>,
        credits: i32,
        shared: bool,
    },
}

/// Summary item for inventory display (grouped/stacked)
#[derive(Debug, Clone)]
pub struct InventorySummaryItem {
    pub catalog_id: ItemId,
    pub item_key: String,
    pub name: String,
    pub short: Option<String>,
    pub quantity: i32,
    pub stackable: bool,
}

// ============================================================================
// EXAMPLES
// ============================================================================

#[cfg(test)]
mod example_usage {
    use super::*;

    async fn example_inventory_operations(service: &InventoryService) -> AppResult<()> {
        let zone_id = ZoneId::new();
        let account_id = AccountId::new();
        let room_id = RoomId::new();

        // 1. Get item from catalog
        let spanner = service.get_item_by_key(zone_id, "multi_spanner").await?;
        println!("Found item: {}", spanner.name);

        // 2. Spawn item to player inventory
        let instance_id = service.add_item(zone_id, account_id, "multi_spanner", 1).await?;
        println!("Spawned item instance: {:?}", instance_id);

        // 3. Check if player has item
        if service.has_item_by_key(zone_id, account_id, "multi_spanner").await? {
            println!("Player has spanner!");
        }

        // 4. Get player inventory
        let inventory = service.get_player_inventory(zone_id, account_id).await?;
        println!("Player has {} items", inventory.len());

        // 5. Drop item in room
        service.drop_item(instance_id, room_id).await?;
        println!("Item dropped in room");

        // 6. Find item in room by noun
        if let Some(item) = service.find_in_room(zone_id, room_id, "spanner").await? {
            println!("Found in room: {}", item.name);

            // 7. Pick it back up
            service.take_item(item.instance_id, account_id).await?;
            println!("Picked up item");
        }

        // 8. Consume item
        service.consume_item(zone_id, account_id, instance_id).await?;
        println!("Item consumed");

        Ok(())
    }

    async fn example_loot_instantiation(service: &InventoryService) -> AppResult<()> {
        let zone_id = ZoneId::new();
        let account_id = AccountId::new();
        let object_id = ObjectId::new();

        // Loot configuration from object
        let loot_config = LootConfig {
            items: vec!["multi_spanner".to_string(), "fiber_probe".to_string()],
            credits: 50,
            once: true,
            shared: false,  // Per-player
        };

        // Player examines object - instantiate loot
        let result = service.instantiate_loot(
            zone_id,
            object_id,
            account_id,
            &loot_config,
        ).await?;

        match result {
            LootInstantiationResult::Instantiated { items, credits, shared } => {
                println!("Spawned {} items and {} credits (shared: {})", items.len(), credits, shared);
            },
            LootInstantiationResult::AlreadyInstantiated => {
                println!("Loot already taken");
            },
        }

        Ok(())
    }
}