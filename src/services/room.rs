use crate::commands::CmdCtx;
use crate::db::repo::{RoomRepo, UserRepo, ZoneRepo};
use crate::error::{AppResult, DomainError};
use crate::lua::{LUA_CMD_TIMEOUT, LuaJob, LuaResult, ScriptHook};
use crate::models::room::{RoomView, build_room_view_impl};
use crate::models::types::{AccountId, Direction, ExitId, ObjectId, RoomId, ZoneId};
use crate::models::zone::ZoneContext;
use crate::state::session::Cursor;
use rand::seq::IndexedRandom;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::timeout;

pub struct RoomService {
    room_repo: Arc<dyn RoomRepo>,
    zone_repo: Arc<dyn ZoneRepo>,
    user_repo: Arc<dyn UserRepo>,
}

impl RoomService {
    pub fn new(room_repo: Arc<dyn RoomRepo>, zone_repo: Arc<dyn ZoneRepo>, user_repo: Arc<dyn UserRepo>) -> Self {
        Self {
            room_repo,
            zone_repo,
            user_repo,
        }
    }

    pub async fn hint_consider(&self, cursor: &Cursor, trigger: &str) -> AppResult<Option<String>> {
        let current_visit = cursor.room_view.visit_count;
        let rv = &cursor.room_view;
        let room_id = cursor.room_id;
        let account_id = cursor.account_id;
        let zone_id = cursor.zone_id;

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
                            zone_id,
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
                            zone_id,
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
        let rv = &cursor.room_view;

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
        ctx.sess.write().cursor = Some(c.clone());

        // Increase visit count and last visit timestamp
        self.user_repo
            .inc_room_kv(ctx.zone_id()?, ctx.room_id()?, ctx.account_id()?, "__visit_count", 1)
            .await?;
        let ts = serde_json::Value::Number(serde_json::Number::from(chrono::Utc::now().timestamp()));
        self.user_repo
            .set_room_kv(
                ctx.zone_id()?,
                ctx.room_id()?,
                ctx.account_id()?,
                "__last_visited_at",
                &ts,
            )
            .await?;

        // reload room view after updating visit count
        let rv = self
            .build_room_view(&ctx.zone_ctx()?, ctx.account_id()?, ctx.room_id()?)
            .await?;
        {
            let mut sess = ctx.sess.write();
            let cursor = sess
                .cursor
                .as_mut()
                .ok_or_else(|| DomainError::InternalError("Cursor not able to mutate".into()))?;
            cursor.room_view = rv;
        }

        // Enter or First enter lua hooks
        self.lua_on_enter(ctx.clone()).await?;

        Ok(())
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
                    account: ctx.account()?,
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
                    account: ctx.account()?,
                    reply: tx,
                })
                .await
                .map_err(Box::from)?;
        }

        match timeout(LUA_CMD_TIMEOUT, rx).await {
            Ok(Ok(lua_result)) => match lua_result {
                LuaResult::Failed(msg) => {
                    let s = format!("{{c:yellow:bright_red}}Lua script failuer: {msg}{{c}}");
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
                account: ctx.account()?,
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
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        exit_dir: Direction,
        locked: bool,
    ) -> AppResult<()> {
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                self.user_repo
                    .set_exit_locked(zone_id, room_id, account_id, exit_id, locked)
                    .await?;
                Ok(())
            }
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn is_exit_locked(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        exit_dir: Direction,
    ) -> AppResult<bool> {
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                let locked = self.user_repo
                    .is_exit_locked(zone_id, room_id, account_id, exit_id)
                    .await?;
                Ok(locked)
            }
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn set_exit_locked_shared(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        exit_dir: Direction,
        locked: bool,
    ) -> AppResult<()> {
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                self.zone_repo
                    .set_exit_locked(zone_id, room_id, exit_id, locked)
                    .await?;
                Ok(())
            }
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn build_room_view(
        &self,
        zone_ctx: &ZoneContext,
        account_id: AccountId,
        room_id: RoomId,
    ) -> AppResult<RoomView> {
        // Get blueprint room data
        let bp_room = self.room_repo.room_by_id(zone_ctx.blueprint.id, room_id).await?;
        let bp_exits = self.room_repo.room_exits(room_id).await?;
        let bp_objs = self.room_repo.room_objects(room_id).await?;
        let bp_room_kv = self.room_repo.room_kv(room_id).await?;
        let bp_scripts = self.room_repo.room_scripts(room_id).await?;

        // Get zone info
        let zone_room_kv = self.zone_repo.room_kv(zone_ctx.zone.id, room_id).await?;
        let zone_obj_kv = self.zone_repo.obj_kv(zone_ctx.zone.id, room_id).await?;

        // get account info
        let user_room_kv = self.user_repo.room_kv(zone_ctx.zone.id, room_id, account_id).await?;
        let user_obj_kv = self.user_repo.obj_kv(zone_ctx.zone.id, room_id, account_id).await?;

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
        zone_id: ZoneId,
        account_id: AccountId,
        object_id: ObjectId,
        key: &str,
        val: &serde_json::Value
    ) -> AppResult<()> {
        self.user_repo.set_object_kv(zone_id, account_id, object_id, key, val).await?;
        Ok(())
    }

    pub async fn set_object_state_shared(
        &self,
        zone_id: ZoneId,
        object_id: ObjectId,
        key: &str,
        val: &serde_json::Value,
    ) -> AppResult<()> {
        self.zone_repo.set_object_kv(zone_id, object_id, key, val).await?;
        Ok(())
    }

}
