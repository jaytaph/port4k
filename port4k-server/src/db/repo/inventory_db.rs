use crate::db::{Db, DbResult, DbError};
use crate::models::inventory::{Item, ItemInstance, ItemLocation};
use crate::models::types::{AccountId, ItemId, ZoneId, ObjectId, RoomId, BlueprintId};
use std::sync::Arc;
use crate::db::repo::inventory::InventoryRepo;

pub struct InventoryRepository {
    db: Arc<Db>,
}

impl InventoryRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Helper: Get blueprint_id for a zone
    async fn get_blueprint_for_zone(&self, zone_id: ZoneId) -> DbResult<BlueprintId> {
        let client = self.db.pool.get().await?;
        let row = client
            .query_one("SELECT bp_id FROM zones WHERE id = $1", &[&zone_id])
            .await?;
        Ok(row.get(0))
    }
}

#[async_trait::async_trait]
impl InventoryRepo for InventoryRepository {

    // ========================================================================
    // CATALOG QUERIES
    // ========================================================================

    async fn get_item_by_key(&self, zone_id: ZoneId, item_key: &str) -> DbResult<Item> {
        let bp_id = self.get_blueprint_for_zone(zone_id).await?;
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
                SELECT
                    c.id, c.bp_id, c.item_key, c.name, c.short,
                    c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM bp_items_catalog c
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE c.bp_id = $1 AND c.item_key = $2
                GROUP BY c.id
                "#,
                &[&bp_id, &item_key],
            )
            .await?;

        Ok(Item {
            id: row.get(0),
            bp_id: row.get(1),
            item_key: row.get(2),
            name: row.get(3),
            short: row.get(4),
            description: row.get(5),
            examine: row.get(6),
            stackable: row.get(7),
            nouns: row.get(8),
        })
    }

    async fn get_item_by_id(&self, catalog_id: ItemId) -> DbResult<Item> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
                SELECT
                    c.id, c.bp_id, c.item_key, c.name, c.short,
                    c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM bp_items_catalog c
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE c.id = $1
                GROUP BY c.id
                "#,
                &[&catalog_id],
            )
            .await?;

        Ok(Item {
            id: row.get(0),
            bp_id: row.get(1),
            item_key: row.get(2),
            name: row.get(3),
            short: row.get(4),
            description: row.get(5),
            examine: row.get(6),
            stackable: row.get(7),
            nouns: row.get(8),
        })
    }

    async fn find_item_by_noun(&self, zone_id: ZoneId, noun: &str) -> DbResult<Option<Item>> {
        let bp_id = self.get_blueprint_for_zone(zone_id).await?;
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
                SELECT
                    c.id, c.bp_id, c.item_key, c.name, c.short,
                    c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n2.noun ORDER BY n2.noun) FILTER (WHERE n2.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM bp_items_catalog c
                JOIN bp_item_nouns n ON n.item_id = c.id AND LOWER(n.noun) = LOWER($2)
                LEFT JOIN bp_item_nouns n2 ON n2.item_id = c.id
                WHERE c.bp_id = $1
                GROUP BY c.id
                "#,
                &[&bp_id, &noun],
            )
            .await?;

        Ok(row.map(|r| Item {
            id: r.get(0),
            bp_id: r.get(1),
            item_key: r.get(2),
            name: r.get(3),
            short: r.get(4),
            description: r.get(5),
            examine: r.get(6),
            stackable: r.get(7),
            nouns: r.get(8),
        }))
    }

    async fn get_zone_catalog(&self, zone_id: ZoneId) -> DbResult<Vec<Item>> {
        let bp_id = self.get_blueprint_for_zone(zone_id).await?;
        let client = self.db.pool.get().await?;

        let rows = client
            .query(
                r#"
                SELECT
                    c.id, c.bp_id, c.item_key, c.name, c.short,
                    c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM bp_items_catalog c
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE c.bp_id = $1
                GROUP BY c.id
                ORDER BY c.name
                "#,
                &[&bp_id],
            )
            .await?;

        Ok(rows.into_iter().map(|row| Item {
            id: row.get(0),
            bp_id: row.get(1),
            item_key: row.get(2),
            name: row.get(3),
            short: row.get(4),
            description: row.get(5),
            examine: row.get(6),
            stackable: row.get(7),
            nouns: row.get(8),
        }).collect())
    }

    // ========================================================================
    // ITEM INSTANCE QUERIES
    // ========================================================================

    async fn get_item_instance(&self, instance_id: ItemId) -> DbResult<ItemInstance> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE i.id = $1
                GROUP BY i.id, c.id
                "#,
                &[&instance_id],
            )
            .await?;

        let location = ItemLocation::from_db_columns(
            row.get(3),
            row.get(4),
            row.get(5),
            row.get(6),
        ).map_err(|e| DbError::DataError(e))?;

        Ok(ItemInstance {
            instance_id: row.get(0),
            zone_id: row.get(1),
            catalog_id: row.get(2),
            location,
            quantity: row.get(7),
            condition: row.get(8),
            created_at: row.get(9),
            updated_at: row.get(10),
            item_key: row.get(11),
            name: row.get(12),
            short: row.get(13),
            description: row.get(14),
            examine: row.get(15),
            stackable: row.get(16),
            nouns: row.get(17),
        })
    }

    async fn has_item(&self, zone_id: ZoneId, account_id: AccountId, instance_id: ItemId) -> DbResult<bool> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM items WHERE id = $1 AND zone_id = $2 AND account_id = $3)",
                &[&instance_id, &zone_id, &account_id],
            )
            .await?;

        Ok(row.get(0))
    }

    async fn has_item_by_key(&self, zone_id: ZoneId, account_id: AccountId, item_key: &str) -> DbResult<bool> {
        let bp_id = self.get_blueprint_for_zone(zone_id).await?;
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM items i
                    JOIN bp_items_catalog c ON i.catalog_id = c.id
                    WHERE i.zone_id = $1
                      AND i.account_id = $2
                      AND c.bp_id = $3
                      AND c.item_key = $4
                )
                "#,
                &[&zone_id, &account_id, &bp_id, &item_key],
            )
            .await?;

        Ok(row.get(0))
    }

    async fn has_item_by_noun(&self, zone_id: ZoneId, account_id: AccountId, noun: &str) -> DbResult<bool> {
        let bp_id = self.get_blueprint_for_zone(zone_id).await?;
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM items i
                    JOIN bp_items_catalog c ON i.catalog_id = c.id
                    JOIN bp_item_nouns n ON n.item_id = c.id
                    WHERE i.zone_id = $1
                      AND i.account_id = $2
                      AND c.bp_id = $3
                      AND LOWER(n.noun) = LOWER($4)
                )
                "#,
                &[&zone_id, &account_id, &bp_id, &noun],
            )
            .await?;

        Ok(row.get(0))
    }

    // ========================================================================
    // INVENTORY QUERIES
    // ========================================================================

    async fn get_player_inventory(&self, zone_id: ZoneId, account_id: AccountId) -> DbResult<Vec<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let rows = client
            .query(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE i.zone_id = $1 AND i.account_id = $2
                GROUP BY i.id, c.id
                ORDER BY c.name
                "#,
                &[&zone_id, &account_id],
            )
            .await?;

        rows.into_iter().map(|row| {
            let location = ItemLocation::from_db_columns(
                row.get(3),
                row.get(4),
                row.get(5),
                row.get(6),
            ).map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: row.get(0),
                zone_id: row.get(1),
                catalog_id: row.get(2),
                location,
                quantity: row.get(7),
                condition: row.get(8),
                created_at: row.get(9),
                updated_at: row.get(10),
                item_key: row.get(11),
                name: row.get(12),
                short: row.get(13),
                description: row.get(14),
                examine: row.get(15),
                stackable: row.get(16),
                nouns: row.get(17),
            })
        }).collect()
    }

    async fn find_item_in_player_inventory(&self, zone_id: ZoneId, account_id: AccountId, noun: &str) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n2.noun ORDER BY n2.noun) FILTER (WHERE n2.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                JOIN bp_item_nouns n ON n.item_id = c.id AND LOWER(n.noun) = LOWER($3)
                LEFT JOIN bp_item_nouns n2 ON n2.item_id = c.id
                WHERE i.zone_id = $1 AND i.account_id = $2
                GROUP BY i.id, c.id
                LIMIT 1
                "#,
                &[&zone_id, &account_id, &noun],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(
                r.get(3),
                r.get(4),
                r.get(5),
                r.get(6),
            ).map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                zone_id: r.get(1),
                catalog_id: r.get(2),
                location,
                quantity: r.get(7),
                condition: r.get(8),
                created_at: r.get(9),
                updated_at: r.get(10),
                item_key: r.get(11),
                name: r.get(12),
                short: r.get(13),
                description: r.get(14),
                examine: r.get(15),
                stackable: r.get(16),
                nouns: r.get(17),
            })
        }).transpose()
    }

    async fn find_item_by_key_in_inventory(&self, zone_id: ZoneId, account_id: AccountId, item_key: &str) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE i.zone_id = $1 AND i.account_id = $2 AND c.item_key = $3
                GROUP BY i.id, c.id
                LIMIT 1
                "#,
                &[&zone_id, &account_id, &item_key],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(
                r.get(3),
                r.get(4),
                r.get(5),
                r.get(6),
            ).map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                zone_id: r.get(1),
                catalog_id: r.get(2),
                location,
                quantity: r.get(7),
                condition: r.get(8),
                created_at: r.get(9),
                updated_at: r.get(10),
                item_key: r.get(11),
                name: r.get(12),
                short: r.get(13),
                description: r.get(14),
                examine: r.get(15),
                stackable: r.get(16),
                nouns: r.get(17),
            })
        }).transpose()
    }

    // ========================================================================
    // ROOM QUERIES
    // ========================================================================

    async fn get_room_items(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<Vec<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let rows = client
            .query(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE i.zone_id = $1 AND i.room_id = $2
                GROUP BY i.id, c.id
                ORDER BY c.name
                "#,
                &[&zone_id, &room_id],
            )
            .await?;

        rows.into_iter().map(|row| {
            let location = ItemLocation::from_db_columns(
                row.get(3),
                row.get(4),
                row.get(5),
                row.get(6),
            ).map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: row.get(0),
                zone_id: row.get(1),
                catalog_id: row.get(2),
                location,
                quantity: row.get(7),
                condition: row.get(8),
                created_at: row.get(9),
                updated_at: row.get(10),
                item_key: row.get(11),
                name: row.get(12),
                short: row.get(13),
                description: row.get(14),
                examine: row.get(15),
                stackable: row.get(16),
                nouns: row.get(17),
            })
        }).collect()
    }

    async fn find_item_in_room(&self, zone_id: ZoneId, room_id: RoomId, noun: &str) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n2.noun ORDER BY n2.noun) FILTER (WHERE n2.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                JOIN bp_item_nouns n ON n.item_id = c.id AND LOWER(n.noun) = LOWER($3)
                LEFT JOIN bp_item_nouns n2 ON n2.item_id = c.id
                WHERE i.zone_id = $1 AND i.room_id = $2
                GROUP BY i.id, c.id
                LIMIT 1
                "#,
                &[&zone_id, &room_id, &noun],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(
                r.get(3),
                r.get(4),
                r.get(5),
                r.get(6),
            ).map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                zone_id: r.get(1),
                catalog_id: r.get(2),
                location,
                quantity: r.get(7),
                condition: r.get(8),
                created_at: r.get(9),
                updated_at: r.get(10),
                item_key: r.get(11),
                name: r.get(12),
                short: r.get(13),
                description: r.get(14),
                examine: r.get(15),
                stackable: r.get(16),
                nouns: r.get(17),
            })
        }).transpose()
    }

    // ========================================================================
    // OBJECT/CONTAINER QUERIES
    // ========================================================================

    async fn get_object_items(&self, zone_id: ZoneId, object_id: ObjectId) -> DbResult<Vec<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let rows = client
            .query(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                LEFT JOIN bp_item_nouns n ON n.item_id = c.id
                WHERE i.zone_id = $1 AND i.object_id = $2
                GROUP BY i.id, c.id
                ORDER BY c.name
                "#,
                &[&zone_id, &object_id],
            )
            .await?;

        rows.into_iter().map(|row| {
            let location = ItemLocation::from_db_columns(
                row.get(3),
                row.get(4),
                row.get(5),
                row.get(6),
            ).map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: row.get(0),
                zone_id: row.get(1),
                catalog_id: row.get(2),
                location,
                quantity: row.get(7),
                condition: row.get(8),
                created_at: row.get(9),
                updated_at: row.get(10),
                item_key: row.get(11),
                name: row.get(12),
                short: row.get(13),
                description: row.get(14),
                examine: row.get(15),
                stackable: row.get(16),
                nouns: row.get(17),
            })
        }).collect()
    }

    async fn find_item_in_object(&self, zone_id: ZoneId, object_id: ObjectId, noun: &str) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
                SELECT
                    i.id, i.zone_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n2.noun ORDER BY n2.noun) FILTER (WHERE n2.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                JOIN bp_item_nouns n ON n.item_id = c.id AND LOWER(n.noun) = LOWER($3)
                LEFT JOIN bp_item_nouns n2 ON n2.item_id = c.id
                WHERE i.zone_id = $1 AND i.object_id = $2
                GROUP BY i.id, c.id
                LIMIT 1
                "#,
                &[&zone_id, &object_id, &noun],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(
                r.get(3),
                r.get(4),
                r.get(5),
                r.get(6),
            ).map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                zone_id: r.get(1),
                catalog_id: r.get(2),
                location,
                quantity: r.get(7),
                condition: r.get(8),
                created_at: r.get(9),
                updated_at: r.get(10),
                item_key: r.get(11),
                name: r.get(12),
                short: r.get(13),
                description: r.get(14),
                examine: r.get(15),
                stackable: r.get(16),
                nouns: r.get(17),
            })
        }).transpose()
    }

    // ========================================================================
    // LOOT STATE
    // ========================================================================

    async fn is_loot_instantiated(&self, zone_id: ZoneId, object_id: ObjectId, account_id: Option<AccountId>) -> DbResult<bool> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
                SELECT COALESCE(instantiated, FALSE)
                FROM zone_object_loot_state
                WHERE zone_id = $1
                  AND object_id = $2
                  AND (account_id = $3 OR (account_id IS NULL AND $3 IS NULL))
                "#,
                &[&zone_id, &object_id, &account_id],
            )
            .await;

        match row {
            Ok(r) => Ok(r.get(0)),
            Err(_) => Ok(false),  // Not found = not instantiated
        }
    }

    async fn mark_loot_instantiated(&self, zone_id: ZoneId, object_id: ObjectId, account_id: Option<AccountId>) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        client
            .execute(
                r#"
                INSERT INTO zone_object_loot_state (zone_id, object_id, account_id, instantiated, instantiated_at)
                VALUES ($1, $2, $3, TRUE, NOW())
                ON CONFLICT (zone_id, object_id, account_id)
                DO UPDATE SET instantiated = TRUE, instantiated_at = NOW()
                "#,
                &[&zone_id, &object_id, &account_id],
            )
            .await?;

        Ok(())
    }

    // ========================================================================
    // ITEM SPAWNING
    // ========================================================================

    async fn spawn_item(
        &self,
        zone_id: ZoneId,
        item_key: &str,
        location: ItemLocation,
        quantity: i32,
    ) -> DbResult<ItemId> {
        let client = self.db.pool.get().await?;

        // Call the database spawn_item function
        let (room_id, account_id, object_id, container_item_id) = location.to_db_columns();

        let row = client
            .query_one(
                "SELECT spawn_item($1, $2, $3, $4, $5, $6, $7)",
                &[&zone_id, &item_key, &room_id, &account_id, &object_id, &container_item_id, &quantity],
            )
            .await?;

        Ok(row.get(0))
    }

    // ========================================================================
    // ITEM MOVEMENT
    // ========================================================================

    async fn move_item(&self, instance_id: ItemId, new_location: ItemLocation) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        let (room_id, account_id, object_id, container_item_id) = new_location.to_db_columns();

        client
            .execute(
                "SELECT move_item($1, $2, $3, $4, $5)",
                &[&instance_id, &room_id, &account_id, &object_id, &container_item_id],
            )
            .await?;

        Ok(())
    }

    // ========================================================================
    // ITEM MODIFICATION
    // ========================================================================

    async fn consume_item(&self, _zone_id: ZoneId, _account_id: AccountId, instance_id: ItemId) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        // Get current quantity
        let row = client
            .query_one("SELECT quantity FROM items WHERE id = $1", &[&instance_id])
            .await?;

        let quantity: i32 = row.get(0);

        if quantity <= 1 {
            // Delete item
            client
                .execute("DELETE FROM items WHERE id = $1", &[&instance_id])
                .await?;
        } else {
            // Reduce quantity
            client
                .execute(
                    "UPDATE items SET quantity = quantity - 1, updated_at = NOW() WHERE id = $1",
                    &[&instance_id],
                )
                .await?;
        }

        Ok(())
    }

    async fn set_item_quantity(&self, instance_id: ItemId, quantity: i32) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        client
            .execute(
                "UPDATE items SET quantity = $1, updated_at = NOW() WHERE id = $2",
                &[&quantity, &instance_id],
            )
            .await?;

        Ok(())
    }

    async fn set_item_condition(&self, instance_id: ItemId, condition: serde_json::Value) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        client
            .execute(
                "UPDATE items SET condition = $1, updated_at = NOW() WHERE id = $2",
                &[&condition, &instance_id],
            )
            .await?;

        Ok(())
    }

    async fn delete_item(&self, instance_id: ItemId) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        client
            .execute("DELETE FROM items WHERE id = $1", &[&instance_id])
            .await?;

        Ok(())
    }
}