use crate::db::DbResult;
use crate::models::inventory::{Item, ItemInstance, ItemLocation};
use crate::models::types::{AccountId, ItemId, ObjectId, RealmId, RoomId};

#[async_trait::async_trait]
pub trait InventoryRepo: Send + Sync {
    // ========================================================================
    // CATALOG QUERIES (Blueprint-level item definitions)
    // ========================================================================

    /// Get item definition from catalog by item_key
    async fn get_item_by_key(&self, realm_id: RealmId, item_key: &str) -> DbResult<Item>;

    /// Get item definition from catalog by catalog_id
    async fn get_item_by_id(&self, catalog_id: ItemId) -> DbResult<Item>;

    /// Find item in catalog by noun
    async fn find_item_by_noun(&self, realm_id: RealmId, noun: &str) -> DbResult<Option<Item>>;

    /// Get all items in realm's blueprint catalog
    async fn get_realm_catalog(&self, realm_id: RealmId) -> DbResult<Vec<Item>>;

    // ========================================================================
    // ITEM INSTANCE QUERIES
    // ========================================================================

    /// Get a specific item instance by its ID
    async fn get_item_instance(&self, instance_id: ItemId) -> DbResult<ItemInstance>;

    /// Check if player has specific item instance
    async fn has_item(&self, realm_id: RealmId, account_id: AccountId, instance_id: ItemId) -> DbResult<bool>;

    /// Check if player has any item with given item_key
    async fn has_item_by_key(&self, realm_id: RealmId, account_id: AccountId, item_key: &str) -> DbResult<bool>;

    /// Check if player has any item matching noun
    async fn has_item_by_noun(&self, realm_id: RealmId, account_id: AccountId, noun: &str) -> DbResult<bool>;

    // ========================================================================
    // INVENTORY QUERIES
    // ========================================================================

    /// Get all items in player's inventory
    async fn get_player_inventory(&self, realm_id: RealmId, account_id: AccountId) -> DbResult<Vec<ItemInstance>>;

    /// Find item in player inventory by noun
    async fn find_item_in_player_inventory(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        noun: &str,
    ) -> DbResult<Option<ItemInstance>>;

    /// Find item in player inventory by item_key
    async fn find_item_by_key_in_inventory(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        item_key: &str,
    ) -> DbResult<Option<ItemInstance>>;

    // ========================================================================
    // ROOM QUERIES
    // ========================================================================

    /// Get all items in a room
    async fn get_room_items(&self, realm_id: RealmId, room_id: RoomId) -> DbResult<Vec<ItemInstance>>;

    /// Find item in room by noun
    async fn find_item_in_room(&self, realm_id: RealmId, room_id: RoomId, noun: &str)
    -> DbResult<Option<ItemInstance>>;

    // ========================================================================
    // OBJECT/CONTAINER QUERIES
    // ========================================================================

    /// Get all items inside an object
    async fn get_object_items(&self, realm_id: RealmId, object_id: ObjectId) -> DbResult<Vec<ItemInstance>>;

    /// Find item in object by noun
    async fn find_item_in_object(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        noun: &str,
    ) -> DbResult<Option<ItemInstance>>;

    // ========================================================================
    // LOOT STATE
    // ========================================================================

    /// Check if object's loot has been instantiated
    /// account_id is None for shared/global loot, Some for per-player loot
    async fn is_loot_instantiated(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        account_id: Option<AccountId>,
    ) -> DbResult<bool>;

    /// Mark object's loot as instantiated
    async fn mark_loot_instantiated(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        account_id: Option<AccountId>,
    ) -> DbResult<()>;

    // ========================================================================
    // ITEM SPAWNING
    // ========================================================================

    /// Spawn a new item instance from catalog
    /// Automatically handles stacking if item is stackable
    /// Returns the instance_id (either new or existing stack)
    async fn spawn_item(
        &self,
        realm_id: RealmId,
        item_key: &str,
        location: ItemLocation,
        quantity: i32,
    ) -> DbResult<ItemId>;

    // ========================================================================
    // ITEM MOVEMENT
    // ========================================================================

    /// Move item to new location
    /// Automatically merges with existing stacks if applicable
    async fn move_item(&self, instance_id: ItemId, new_location: ItemLocation) -> DbResult<()>;

    // ========================================================================
    // ITEM MODIFICATION
    // ========================================================================

    /// Consume one item (reduces quantity by 1, or deletes if quantity becomes 0)
    async fn consume_item(&self, realm_id: RealmId, account_id: AccountId, instance_id: ItemId) -> DbResult<()>;

    /// Set item quantity (for stackable items)
    async fn set_item_quantity(&self, instance_id: ItemId, quantity: i32) -> DbResult<()>;

    /// Update item condition (durability, charges, custom state)
    async fn set_item_condition(&self, instance_id: ItemId, condition: serde_json::Value) -> DbResult<()>;

    /// Delete item instance entirely
    async fn delete_item(&self, instance_id: ItemId) -> DbResult<()>;

    // ========================================================================
    // CONVENIENCE METHODS (can have default implementations)
    // ========================================================================

    /// Add item to player inventory (spawn + move)
    async fn add_item(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        item_key: &str,
        quantity: i32,
    ) -> DbResult<ItemId> {
        self.spawn_item(realm_id, item_key, ItemLocation::Player(account_id), quantity)
            .await
    }
}
