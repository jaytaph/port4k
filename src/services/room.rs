use crate::commands::CmdCtx;
use crate::db::repo::{AccountRepo, RealmRepo, RoomRepo, UserRepo};
use crate::error::{AppResult, DomainError};
use crate::lua::{LUA_CMD_TIMEOUT, LuaJob, LuaResult, ScriptHook};
use crate::models::room::{RoomView, build_room_view_impl};
use crate::models::types::{AccountId, Direction, ExitId, ObjectId, RealmId, RoomId};
use crate::state::session::Cursor;
use rand::seq::IndexedRandom;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use crate::services::inventory::LootConfig;

pub struct RoomService {
    room_repo: Arc<dyn RoomRepo>,
    realm_repo: Arc<dyn RealmRepo>,
    user_repo: Arc<dyn UserRepo>,
    account_repo: Arc<dyn AccountRepo>,
    inventory_service: Arc<crate::services::inventory::InventoryService>,
}

impl RoomService {
    pub fn new(
        room_repo: Arc<dyn RoomRepo>,
        realm_repo: Arc<dyn RealmRepo>,
        user_repo: Arc<dyn UserRepo>,
        account_repo: Arc<dyn AccountRepo>,
        inventory_service: Arc<crate::services::inventory::InventoryService>,
    ) -> Self {
        Self {
            room_repo,
            realm_repo,
            user_repo,
            account_repo,
            inventory_service,
        }
    }

    pub async fn get_by_id(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        room_id: RoomId,
    ) -> AppResult<RoomView> {
        self.build_room_view(realm_id, account_id, room_id).await
    }

    pub async fn get_room_id_by_key(
        &self,
        realm_id: RealmId,
        room_key: &str,
    ) -> AppResult<Option<RoomId>> {
        // Find realm first
        let realm = match self.realm_repo.get(realm_id).await? {
            Some(r) => r,
            None => return Err(DomainError::NotFound("Realm not found".into())),
        };

        let room_id = self.room_repo.get_room_id_by_key(realm.bp_id, room_key).await?;
        Ok(room_id)
    }

    pub async fn hint_consider(&self, cursor: &Cursor, trigger: &str) -> AppResult<Option<String>> {
        let current_visit = cursor.room.visit_count;
        let rv = &cursor.room;
        let room_id = cursor.room_id;
        let account_id = cursor.account_id;
        let realm_id = cursor.realm_id;

        let mut result = None;

        for hint in rv.blueprint.hints.iter() {
            if hint.when == trigger {
                let shown = rv.room_kv.get_bool(&format!("hint_shown_{}", hint.id), false);

                if hint.once.unwrap_or(false) && shown {
                    continue; // Already shown
                }

                // Check cooldown
                if let Some(cooldown) = hint.cooldown {
                    let last_shown_visit = rv.room_kv.get_num::<i64>(&format!("hint_last_visit_{}", hint.id), 0);
                    if current_visit - last_shown_visit < cooldown as i64 {
                        continue; // Still in cooldown
                    }
                }

                // Return the hint
                result = Some(format!("{{c:cyan:bright_cyan}}Hint: {}{{c}}", hint.text));

                // Mark as shown
                if hint.once.unwrap_or(false) {
                    self.user_repo
                        .set_room_kv(
                            realm_id,
                            room_id,
                            account_id,
                            &format!("hint_shown_{}", hint.id),
                            &serde_json::Value::Bool(true),
                        )
                        .await?;
                }

                // Update last shown visit for cooldown
                if hint.cooldown.is_some() {
                    self.user_repo
                        .set_room_kv(
                            realm_id,
                            room_id,
                            account_id,
                            &format!("hint_last_visit_{}", hint.id),
                            &serde_json::Value::Number(current_visit.into()),
                        )
                        .await?;
                }
            }
        }

        Ok(result)
    }

    pub async fn hint_trigger(&self, cursor: &Cursor, trigger: &str) -> AppResult<Option<String>> {
        let rv = &cursor.room;

        // collect all entries that have hint.when == trigger
        let hints: Vec<_> = rv.blueprint.hints.iter().filter(|hint| hint.when == trigger).collect();

        if hints.is_empty() {
            Ok(None)
        } else {
            let mut rng = rand::rng();
            let hint = hints.choose(&mut rng).unwrap();
            Ok(Some(format!("{{c:cyan:bright_cyan}}Hint: {}{{c}}", hint.text)))
        }
    }

    // Travel to the given room
    pub async fn enter_room(&self, ctx: Arc<CmdCtx>, c: &Cursor) -> AppResult<()> {
        // Enter the current room
        ctx.sess.write().set_cursor(Some(c.clone()));

        // Increase visit count and last visit timestamp
        self.user_repo
            .inc_room_kv(ctx.realm_id()?, ctx.room_id()?, ctx.account_id()?, "__visit_count", 1)
            .await?;
        let ts = serde_json::Value::Number(serde_json::Number::from(chrono::Utc::now().timestamp()));
        self.user_repo
            .set_room_kv(
                ctx.realm_id()?,
                ctx.room_id()?,
                ctx.account_id()?,
                "__last_visited_at",
                &ts,
            )
            .await?;

        // self.reload_cursor(ctx.cursor()?).await?;

        // Spawn loot found in the room
        for object in &ctx.cursor()?.room.objects {
            if let Some(loot_config) = &object.loot {
                let realm_id = ctx.realm_id()?;
                let account_id = ctx.account_id()?;

                let loot_config = LootConfig {
                    items: loot_config.items.clone(),
                    credits: loot_config.credits,
                    once: loot_config.once,
                    shared: loot_config.shared,
                };

                // Instantiate if not already done
                self.inventory_service.instantiate_loot(
                    realm_id,
                    object.id,
                    account_id,
                    &loot_config
                ).await?;
            }
        }

        // Enter or First enter lua hooks
        self.lua_on_enter(ctx.clone()).await?;

        // self.reload_cursor(&ctx.cursor()?).await?;

        Ok(())
    }

    pub async fn create_cursor(&self, realm_id: RealmId, room_id: RoomId, account_id: AccountId) -> AppResult<Cursor> {
        let Some(account) = self.account_repo.get_by_id(account_id).await? else {
            return Err(DomainError::NotFound("Account not found".into()));
        };

        let Some(realm) = self.realm_repo.get(realm_id).await? else {
            return Err(DomainError::NotFound("Realm not found".into()));
        };
        let room_view = self.build_room_view(realm_id, account_id, room_id).await?;

        let cursor = Cursor::new(realm, room_view, account);
        Ok(cursor)
    }

    pub async fn exit_room(&self, ctx: Arc<CmdCtx>) -> AppResult<()> {
        // Exit the current room
        self.lua_on_exit(ctx.clone()).await?;

        Ok(())
    }

    /// Called when we enter the room. Either calls on_enter or on_first_enter lua hooks.
    async fn lua_on_enter(&self, ctx: Arc<CmdCtx>) -> AppResult<()> {
        let rv = ctx.room_view()?;

        let (tx, rx) = oneshot::channel();

        let output_handle = ctx.output.clone();
        let first_enter_hook = rv.scripts.get(&ScriptHook::OnFirstEnter);

        if rv.visit_count == 1 && first_enter_hook.is_some() {
            // Enter first time hook if there is such a script
            ctx.lua_tx
                .send(LuaJob::OnFirstEnter {
                    output_handle,
                    cursor: Box::new(ctx.cursor()?),
                    account_id: ctx.account_id()?,
                    reply: tx,
                })
                .await
                .map_err(Box::from)?;
        } else {
            // Run on the first entry when there is no first enter hook
            // Run on each subsequent times
            ctx.lua_tx
                .send(LuaJob::OnEnter {
                    output_handle,
                    cursor: Box::new(ctx.cursor()?),
                    account_id: ctx.account_id()?,
                    reply: tx,
                })
                .await
                .map_err(Box::from)?
                // .map_err(|e| {
                //     panic!("{}", e.to_string());
                //     Box::from(e)
                // })?
            ;
        }

        match timeout(LUA_CMD_TIMEOUT, rx).await {
            Ok(Ok(lua_result)) => match lua_result {
                LuaResult::Failed(msg) => {
                    let s = format!("{{c:yellow:bright_red}}Lua script failure: {msg}{{c}}");
                    ctx.output.system(s).await;
                    return Ok(());
                }
                LuaResult::Success(_) => {
                    let s = "{c:yellow:bright_green}Lua script completed without issues{c}";
                    ctx.output.system(s).await;
                }
            },
            Ok(Err(e)) => {
                let s = format!("{{c:yellow:bright_red}}Internal system error: {e}{{c}}");
                ctx.output.system(s).await;
            }
            Err(_elapsed) => {
                let s = "{c:yellow:bright_red}The room doesn't react (script timed out){c}";
                ctx.output.system(s).await;
            }
        }

        Ok(())
    }

    /// Called when we exit the room.
    async fn lua_on_exit(&self, ctx: Arc<CmdCtx>) -> AppResult<()> {
        let (tx, rx) = oneshot::channel();

        let output_handle = ctx.output.clone();

        // Enter first time hook
        ctx.lua_tx
            .send(LuaJob::OnLeave {
                output_handle,
                cursor: Box::new(ctx.cursor()?),
                account_id: ctx.account_id()?,
                reply: tx,
            })
            .await
            .map_err(Box::from)?;

        match timeout(LUA_CMD_TIMEOUT, rx).await {
            Err(e) => {
                ctx.output.system(format!("The room doesn't react ({e})")).await;
            }
            Ok(_) => {}
        }

        Ok(())
    }

    pub async fn exit_by_direction(&self, room_id: RoomId, direction: Direction) -> AppResult<Option<ExitId>> {
        let exits = self.room_repo.room_exits(room_id).await?;
        for exit in exits {
            if exit.dir == direction {
                return Ok(Some(exit.id));
            }
        }
        Ok(None)
    }

    pub async fn set_exit_locked(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        exit_dir: Direction,
        locked: bool,
    ) -> AppResult<()> {
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                self.user_repo
                    .set_exit_locked(realm_id, room_id, account_id, exit_id, locked)
                    .await?;
                Ok(())
            }
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn is_exit_locked(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        exit_dir: Direction,
    ) -> AppResult<bool> {
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                let locked = self
                    .user_repo
                    .is_exit_locked(realm_id, room_id, account_id, exit_id)
                    .await?;
                Ok(locked)
            }
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn set_exit_locked_shared(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        exit_dir: Direction,
        locked: bool,
    ) -> AppResult<()> {
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                self.realm_repo
                    .set_exit_locked(realm_id, room_id, exit_id, locked)
                    .await?;
                Ok(())
            }
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn build_room_view(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        room_id: RoomId,
    ) -> AppResult<RoomView> {
        // Get blueprint room data
        let Some(realm) = self.realm_repo.get(realm_id).await? else {
            return Err(DomainError::NotFound("Realm not found".into()));
        };

        let bp_room = self.room_repo.room_by_id(realm.bp_id, room_id).await?;
        let bp_exits = self.room_repo.room_exits(room_id).await?;
        let bp_objs = self.room_repo.room_objects(room_id).await?;
        let bp_room_kv = self.room_repo.room_kv(room_id).await?;
        let bp_scripts = self.room_repo.room_scripts(room_id).await?;

        // Get zone info
        let zone_room_kv = self.realm_repo.room_kv(realm_id, room_id).await?;
        let zone_obj_kv = self.realm_repo.obj_kv(realm_id, room_id).await?;

        // get account info
        let user_room_kv = self.user_repo.room_kv(realm_id, room_id, account_id).await?;
        let user_obj_kv = self.user_repo.obj_kv(realm_id, room_id, account_id).await?;

        // @todo: not filled yet
        let zone_qty = HashMap::new();
        let user_qty = HashMap::new();

        let rv = build_room_view_impl(
            &bp_room,
            &bp_exits.as_slice(),
            &bp_objs.as_slice(),
            &bp_scripts,
            &bp_room_kv,
            &zone_room_kv,
            &zone_obj_kv,
            &zone_qty,
            &user_room_kv,
            &user_obj_kv,
            &user_qty,
        );

        Ok(rv)
    }

    pub async fn set_object_state(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        object_id: ObjectId,
        key: &str,
        val: &serde_json::Value,
    ) -> AppResult<()> {
        self.user_repo
            .set_object_kv(realm_id, account_id, object_id, key, val)
            .await?;
        Ok(())
    }

    pub async fn set_object_state_shared(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        key: &str,
        val: &serde_json::Value,
    ) -> AppResult<()> {
        self.realm_repo.set_object_kv(realm_id, object_id, key, val).await?;
        Ok(())
    }
}
