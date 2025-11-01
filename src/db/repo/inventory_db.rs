use crate::db::repo::inventory::InventoryRepo;
use crate::db::{map_row, map_row_opt, Db, DbError, DbResult};
use crate::models::inventory::{Item, ItemInstance, ItemLocation};
use crate::models::types::{AccountId, BlueprintId, ItemId, ObjectId, RoomId, RealmId};
use std::sync::Arc;

pub struct InventoryRepository {
    db: Arc<Db>,
}

impl InventoryRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Helper: Get blueprint_id for a realm
    async fn get_blueprint_for_realm(&self, realm_id: RealmId) -> DbResult<BlueprintId> {
        let client = self.db.pool.get().await?;
        let row = client
            .query_one("SELECT bp_id FROM realms WHERE id = $1", &[&realm_id])
            .await?;
        Ok(row.get(0))
    }
}

#[async_trait::async_trait]
impl InventoryRepo for InventoryRepository {
    // ========================================================================
    // CATALOG QUERIES
    // ========================================================================

    async fn get_item_by_key(&self, realm_id: RealmId, item_key: &str) -> DbResult<Item> {
        let bp_id = self.get_blueprint_for_realm(realm_id).await?;
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

        map_row(
            &row,
            Item::try_from_row,
            &format!("InventoryRepo::get_item_by_key realm_id={} item_key={}", realm_id, item_key),
        )
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

        map_row(
            &row,
            Item::try_from_row,
            &format!("InventoryRepo::get_item_by_id item_id={}", catalog_id),
        )
    }

    async fn find_item_by_noun(&self, realm_id: RealmId, noun: &str) -> DbResult<Option<Item>> {
        let bp_id = self.get_blueprint_for_realm(realm_id).await?;
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

        map_row_opt(
            row,
            Item::try_from_row,
            &format!("InventoryRepo::find_item_by_noun realm_id={} noun={}", realm_id, noun),
        )
    }

    async fn get_realm_catalog(&self, realm_id: RealmId) -> DbResult<Vec<Item>> {
        let bp_id = self.get_blueprint_for_realm(realm_id).await?;
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


        let items: DbResult<Vec<Item>> = rows
            .into_iter()
            .map(|row| { map_row(
                &row,
                Item::try_from_row,
                &format!("InventoryRepo::get_realm_catalog realm_id={}", realm_id))
            })
            .collect();

        items
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
                    i.id, i.realm_id, i.catalog_id,
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

        let location = ItemLocation::from_db_columns(row.get(3), row.get(4), row.get(5), row.get(6))
            .map_err(|e| DbError::DataError(e))?;

        Ok(ItemInstance {
            instance_id: row.get(0),
            realm_id: row.get(1),
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

    async fn has_item(&self, realm_id: RealmId, account_id: AccountId, instance_id: ItemId) -> DbResult<bool> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM item_instances WHERE instance_id = $1 AND realm_id = $2 AND account_id = $3)",
                &[&instance_id, &realm_id, &account_id],
            )
            .await?;

        Ok(row.get(0))
    }

    async fn has_item_by_key(&self, realm_id: RealmId, account_id: AccountId, item_key: &str) -> DbResult<bool> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
            SELECT EXISTS(
                SELECT 1 FROM item_instances ii
                JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
                WHERE ii.realm_id = $1
                    AND ii.account_id = $2
                    AND bp.item_key = $3
            )
            "#,
                &[&realm_id, &account_id, &item_key],
            )
            .await?;

        Ok(row.get(0))
    }

    async fn has_item_by_noun(&self, realm_id: RealmId, account_id: AccountId, noun: &str) -> DbResult<bool> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
            SELECT EXISTS(
                SELECT 1 FROM item_instances ii
                JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
                JOIN bp_item_nouns n ON n.item_id = bp.id
                WHERE ii.realm_id = $1
                    AND ii.account_id = $2
                    AND LOWER(n.noun) = LOWER($3)
            )
            "#,
                &[&realm_id, &account_id, &noun],
            )
            .await?;

        Ok(row.get(0))
    }

    // ========================================================================
    // INVENTORY QUERIES
    // ========================================================================

    async fn get_player_inventory(&self, realm_id: RealmId, account_id: AccountId) -> DbResult<Vec<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let rows = client
            .query(
                r#"
            SELECT
                ii.instance_id, ii.realm_id, ii.catalog_id,
                ii.room_id, ii.account_id, ii.object_id, ii.container_item_id,
                ii.quantity, ii.condition, ii.created_at, ii.updated_at,
                bp.item_key, bp.name, bp.short, bp.description, bp.examine, bp.stackable,
                COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
            FROM item_instances ii
            JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
            LEFT JOIN bp_item_nouns n ON n.item_id = bp.id
            WHERE ii.realm_id = $1 AND ii.account_id = $2
            GROUP BY ii.instance_id, bp.id
            ORDER BY bp.name
            "#,
                &[&realm_id, &account_id],
            )
            .await?;

        rows.into_iter()
            .map(|row| {
                let location = ItemLocation::from_db_columns(row.get(3), row.get(4), row.get(5), row.get(6))
                    .map_err(|e| DbError::DataError(e))?;

                Ok(ItemInstance {
                    instance_id: row.get(0),
                    realm_id: row.get(1),
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
            })
            .collect()
    }

    async fn find_item_in_player_inventory(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        noun: &str,
    ) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
                SELECT
                    i.id, i.realm_id, i.catalog_id,
                    i.room_id, i.account_id, i.object_id, i.container_item_id,
                    i.quantity, i.condition, i.created_at, i.updated_at,
                    c.item_key, c.name, c.short, c.description, c.examine, c.stackable,
                    COALESCE(array_agg(n2.noun ORDER BY n2.noun) FILTER (WHERE n2.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
                FROM items i
                JOIN bp_items_catalog c ON i.catalog_id = c.id
                JOIN bp_item_nouns n ON n.item_id = c.id AND LOWER(n.noun) = LOWER($3)
                LEFT JOIN bp_item_nouns n2 ON n2.item_id = c.id
                WHERE i.realm_id = $1 AND i.account_id = $2
                GROUP BY i.id, c.id
                LIMIT 1
                "#,
                &[&realm_id, &account_id, &noun],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(r.get(3), r.get(4), r.get(5), r.get(6))
                .map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                realm_id: r.get(1),
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
        })
        .transpose()
    }

    async fn find_item_by_key_in_inventory(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        item_key: &str,
    ) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
            SELECT
                ii.instance_id, ii.realm_id, ii.catalog_id,
                ii.room_id, ii.account_id, ii.object_id, ii.container_item_id,
                ii.quantity, ii.condition, ii.created_at, ii.updated_at,
                bp.item_key, bp.name, bp.short, bp.description, bp.examine, bp.stackable,
                COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
            FROM item_instances ii
            JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
            LEFT JOIN bp_item_nouns n ON n.item_id = bp.id
            WHERE ii.realm_id = $1 AND ii.account_id = $2 AND bp.item_key = $3
            GROUP BY ii.instance_id, bp.id
            LIMIT 1
            "#,
                &[&realm_id, &account_id, &item_key],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(r.get(3), r.get(4), r.get(5), r.get(6))
                .map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                realm_id: r.get(1),
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
        })
            .transpose()
    }
    // ========================================================================
    // ROOM QUERIES
    // ========================================================================

    async fn get_room_items(&self, realm_id: RealmId, room_id: RoomId) -> DbResult<Vec<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let rows = client
            .query(
                r#"
            SELECT
                ii.instance_id, ii.realm_id, ii.catalog_id,
                ii.room_id, ii.account_id, ii.object_id, ii.container_item_id,
                ii.quantity, ii.condition, ii.created_at, ii.updated_at,
                bp.item_key, bp.name, bp.short, bp.description, bp.examine, bp.stackable,
                COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
            FROM item_instances ii
            JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
            LEFT JOIN bp_item_nouns n ON n.item_id = bp.id
            WHERE ii.realm_id = $1 AND ii.room_id = $2
            GROUP BY ii.instance_id, bp.id
            ORDER BY bp.name
            "#,
                &[&realm_id, &room_id],
            )
            .await?;

        rows.into_iter()
            .map(|row| {
                let location = ItemLocation::from_db_columns(row.get(3), row.get(4), row.get(5), row.get(6))
                    .map_err(|e| DbError::DataError(e))?;

                Ok(ItemInstance {
                    instance_id: row.get(0),
                    realm_id: row.get(1),
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
            })
            .collect()
    }

    async fn find_item_in_room(&self, realm_id: RealmId, room_id: RoomId, noun: &str) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
            SELECT
                ii.instance_id, ii.realm_id, ii.catalog_id,
                ii.room_id, ii.account_id, ii.object_id, ii.container_item_id,
                ii.quantity, ii.condition, ii.created_at, ii.updated_at,
                bp.item_key, bp.name, bp.short, bp.description, bp.examine, bp.stackable,
                COALESCE(array_agg(n2.noun ORDER BY n2.noun) FILTER (WHERE n2.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
            FROM item_instances ii
            JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
            JOIN bp_item_nouns n ON n.item_id = bp.id AND LOWER(n.noun) = LOWER($3)
            LEFT JOIN bp_item_nouns n2 ON n2.item_id = bp.id
            WHERE ii.realm_id = $1 AND ii.room_id = $2
            GROUP BY ii.instance_id, bp.id
            LIMIT 1
            "#,
                &[&realm_id, &room_id, &noun],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(r.get(3), r.get(4), r.get(5), r.get(6))
                .map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                realm_id: r.get(1),
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
        })
            .transpose()
    }

    // ========================================================================
    // OBJECT/CONTAINER QUERIES
    // ========================================================================

    async fn get_object_items(&self, realm_id: RealmId, object_id: ObjectId) -> DbResult<Vec<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let rows = client
            .query(
                r#"
            SELECT
                ii.instance_id, ii.realm_id, ii.catalog_id,
                ii.room_id, ii.account_id, ii.object_id, ii.container_item_id,
                ii.quantity, ii.condition, ii.created_at, ii.updated_at,
                bp.item_key, bp.name, bp.short, bp.description, bp.examine, bp.stackable,
                COALESCE(array_agg(n.noun ORDER BY n.noun) FILTER (WHERE n.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
            FROM item_instances ii
            JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
            LEFT JOIN bp_item_nouns n ON n.item_id = bp.id
            WHERE ii.realm_id = $1 AND ii.object_id = $2
            GROUP BY ii.instance_id, bp.id
            ORDER BY bp.name
            "#,
                &[&realm_id, &object_id],
            )
            .await?;

        rows.into_iter()
            .map(|row| {
                let location = ItemLocation::from_db_columns(row.get(3), row.get(4), row.get(5), row.get(6))
                    .map_err(|e| DbError::DataError(e))?;

                Ok(ItemInstance {
                    instance_id: row.get(0),
                    realm_id: row.get(1),
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
            })
            .collect()
    }

    async fn find_item_in_object(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        noun: &str,
    ) -> DbResult<Option<ItemInstance>> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_opt(
                r#"
            SELECT
                ii.instance_id, ii.realm_id, ii.catalog_id,
                ii.room_id, ii.account_id, ii.object_id, ii.container_item_id,
                ii.quantity, ii.condition, ii.created_at, ii.updated_at,
                bp.item_key, bp.name, bp.short, bp.description, bp.examine, bp.stackable,
                COALESCE(array_agg(n2.noun ORDER BY n2.noun) FILTER (WHERE n2.noun IS NOT NULL), ARRAY[]::TEXT[]) as nouns
            FROM item_instances ii
            JOIN bp_items_catalog bp ON ii.catalog_id = bp.id
            JOIN bp_item_nouns n ON n.item_id = bp.id AND LOWER(n.noun) = LOWER($3)
            LEFT JOIN bp_item_nouns n2 ON n2.item_id = bp.id
            WHERE ii.realm_id = $1 AND ii.object_id = $2
            GROUP BY ii.instance_id, bp.id
            LIMIT 1
            "#,
                &[&realm_id, &object_id, &noun],
            )
            .await?;

        row.map(|r| {
            let location = ItemLocation::from_db_columns(r.get(3), r.get(4), r.get(5), r.get(6))
                .map_err(|e| DbError::DataError(e))?;

            Ok(ItemInstance {
                instance_id: r.get(0),
                realm_id: r.get(1),
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
        })
            .transpose()
    }

    // ========================================================================
    // LOOT STATE
    // ========================================================================

    async fn is_loot_instantiated(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        account_id: Option<AccountId>,
    ) -> DbResult<bool> {
        let client = self.db.pool.get().await?;

        let row = client
            .query_one(
                r#"
            SELECT EXISTS(
                SELECT 1 FROM loot_instantiation_state
                WHERE realm_id = $1
                    AND object_id = $2
                    AND account_id IS NOT DISTINCT FROM $3
            )
            "#,
                &[&realm_id, &object_id, &account_id],
            )
            .await?;

        Ok(row.get(0))
    }

    async fn mark_loot_instantiated(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        account_id: Option<AccountId>,
    ) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        client
            .execute(
                r#"
            INSERT INTO loot_instantiation_state (realm_id, object_id, account_id, instantiated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (realm_id, object_id, account_id)
            DO UPDATE SET instantiated_at = NOW()
            "#,
                &[&realm_id, &object_id, &account_id],
            )
            .await?;

        Ok(())
    }
    // ========================================================================
    // ITEM SPAWNING
    // ========================================================================

    async fn spawn_item(
        &self,
        realm_id: RealmId,
        item_key: &str,
        location: ItemLocation,
        quantity: i32,
    ) -> DbResult<ItemId> {
        let mut client = self.db.pool.get().await?;
        let transaction = client.transaction().await?;

        // 1. Get bp_id for the realm
        let bp_id: BlueprintId = transaction
            .query_one("SELECT bp_id FROM realms WHERE id = $1", &[&realm_id])
            .await?
            .get(0);

        // 1. Get item definition from bp_items_catalog
        let catalog_row = transaction
            .query_one(
                "SELECT id, name, short, stackable FROM bp_items_catalog WHERE bp_id = $1 AND item_key = $2",
                &[&bp_id, &item_key],
            )
            .await?;

        let catalog_id: ItemId = catalog_row.get("id");
        let stackable: bool = catalog_row.get("stackable");

        let (room_id, account_id, object_id, container_item_id) = location.to_db_columns();

        // 2. If stackable, try to find existing stack at this location
        if stackable {
            let existing = transaction
                .query_opt(
                    "SELECT instance_id, quantity
                FROM item_instances
                WHERE realm_id = $1
                    AND catalog_id = $2
                    AND room_id IS NOT DISTINCT FROM $3
                    AND account_id IS NOT DISTINCT FROM $4
                    AND object_id IS NOT DISTINCT FROM $5
                    AND container_item_id IS NOT DISTINCT FROM $6
                LIMIT 1",
                    &[&realm_id, &catalog_id, &room_id, &account_id, &object_id, &container_item_id],
                )
                .await?;

            if let Some(row) = existing {
                // Stack exists - update quantity
                let instance_id: ItemId = row.get(0);
                let current_quantity: i32 = row.get(1);
                let new_quantity = current_quantity + quantity;

                transaction
                    .execute(
                        "UPDATE item_instances
                    SET quantity = $1, updated_at = NOW()
                    WHERE instance_id = $2",
                        &[&new_quantity, &instance_id],
                    )
                    .await?;

                transaction.commit().await?;
                return Ok(instance_id);
            }
        }

        // 3. No existing stack (or not stackable) - create new instance
        let row = transaction
            .query_one(
                "INSERT INTO item_instances (
                realm_id, catalog_id, item_key,
                room_id, account_id, object_id, container_item_id,
                quantity, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            RETURNING instance_id",
                &[
                    &realm_id,
                    &catalog_id,
                    &item_key,
                    &room_id,
                    &account_id,
                    &object_id,
                    &container_item_id,
                    &quantity,
                ],
            )
            .await?;

        let instance_id: ItemId = row.get(0);

        transaction.commit().await?;
        Ok(instance_id)
    }

    // ========================================================================
    // ITEM MOVEMENT
    // ========================================================================

    async fn move_item(&self, instance_id: ItemId, new_location: ItemLocation) -> DbResult<()> {
        let mut client = self.db.pool.get().await?;
        let transaction = client.transaction().await?;

        // Get item info
        let item_row = transaction
            .query_one(
                "SELECT realm_id, catalog_id, quantity FROM item_instances WHERE instance_id = $1",
                &[&instance_id],
            )
            .await?;

        let realm_id: RealmId = item_row.get(0);
        let catalog_id: ItemId = item_row.get(1);
        let quantity: i32 = item_row.get(2);

        // Check if stackable
        let stackable: bool = transaction
            .query_one(
                "SELECT stackable FROM bp_items_catalog WHERE id = $1",
                &[&catalog_id],
            )
            .await?
            .get(0);

        let (room_id, account_id, object_id, container_item_id) = new_location.to_db_columns();

        // If stackable, try to merge with existing stack at destination
        if stackable {
            let existing = transaction
                .query_opt(
                    "SELECT instance_id, quantity
                FROM item_instances
                WHERE realm_id = $1
                    AND catalog_id = $2
                    AND instance_id != $3
                    AND room_id IS NOT DISTINCT FROM $4
                    AND account_id IS NOT DISTINCT FROM $5
                    AND object_id IS NOT DISTINCT FROM $6
                    AND container_item_id IS NOT DISTINCT FROM $7
                LIMIT 1",
                    &[&realm_id, &catalog_id, &instance_id, &room_id, &account_id, &object_id, &container_item_id],
                )
                .await?;

            if let Some(row) = existing {
                // Merge into existing stack
                let existing_id: ItemId = row.get(0);
                let existing_quantity: i32 = row.get(1);

                // Update existing stack
                transaction
                    .execute(
                        "UPDATE item_instances SET quantity = $1, updated_at = NOW() WHERE instance_id = $2",
                        &[&(existing_quantity + quantity), &existing_id],
                    )
                    .await?;

                // Delete moved item
                transaction
                    .execute("DELETE FROM item_instances WHERE instance_id = $1", &[&instance_id])
                    .await?;

                transaction.commit().await?;
                return Ok(());
            }
        }

        // No merge - just update location
        transaction
            .execute(
                "UPDATE item_instances
            SET room_id = $1, account_id = $2, object_id = $3, container_item_id = $4, updated_at = NOW()
            WHERE instance_id = $5",
                &[&room_id, &account_id, &object_id, &container_item_id, &instance_id],
            )
            .await?;

        transaction.commit().await?;
        Ok(())
    }

    // ========================================================================
    // ITEM MODIFICATION
    // ========================================================================

    async fn consume_item(&self, _realm_id: RealmId, _account_id: AccountId, instance_id: ItemId) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        // Get current quantity
        let row = client
            .query_one("SELECT quantity FROM item_instances WHERE instance_id = $1", &[&instance_id])
            .await?;

        let quantity: i32 = row.get(0);

        if quantity <= 1 {
            // Delete item
            client
                .execute("DELETE FROM item_instances WHERE instance_id = $1", &[&instance_id])
                .await?;
        } else {
            // Reduce quantity
            client
                .execute(
                    "UPDATE item_instances SET quantity = quantity - 1, updated_at = NOW() WHERE instance_id = $1",
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
                "UPDATE item_instances SET quantity = $1, updated_at = NOW() WHERE instance_id = $2",
                &[&quantity, &instance_id],
            )
            .await?;

        Ok(())
    }

    async fn set_item_condition(&self, instance_id: ItemId, condition: serde_json::Value) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        client
            .execute(
                "UPDATE item_instances SET condition = $1, updated_at = NOW() WHERE instance_id = $2",
                &[&condition, &instance_id],
            )
            .await?;

        Ok(())
    }

    async fn delete_item(&self, instance_id: ItemId) -> DbResult<()> {
        let client = self.db.pool.get().await?;

        client
            .execute("DELETE FROM item_instances WHERE instance_id = $1", &[&instance_id])
            .await?;

        Ok(())
    }
}
