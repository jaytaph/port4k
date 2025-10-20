pub struct InventoryItem {
    /// Object ID of the item
    pub object_id: String,
    /// Quantity of the item
    pub quantity: u32,
}

pub struct Inventory {
    /// List of inventory items
    pub items: Vec<InventoryItem>,
    /// Maximum number of distinct item types (quantities) allowed. This means that 1 box with 10 items count as 11 (10 items + 1 box)
    pub max_item_count: u32,
}