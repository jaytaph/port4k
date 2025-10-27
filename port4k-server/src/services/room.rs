use std::collections::HashMap;
use crate::db::repo::{ZoneRepo, RoomRepo, UserRepo};
use crate::models::types::{AccountId, Direction, ExitId, RoomId, ZoneId};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use crate::commands::CmdCtx;
use crate::error::{AppResult, DomainError};
use crate::lua::{LuaJob, LuaResult, ScriptHook, LUA_CMD_TIMEOUT};
use crate::models::room::{build_room_view_impl, RoomView};
use crate::models::zone::ZoneContext;
use crate::state::session::Cursor;

pub struct RoomService {
    room_repo: Arc<dyn RoomRepo>,
    zone_repo: Arc<dyn ZoneRepo>,
    user_repo: Arc<dyn UserRepo>,
}

impl RoomService {
    pub fn new(
        room_repo: Arc<dyn RoomRepo>,
        zone_repo: Arc<dyn ZoneRepo>,
        user_repo: Arc<dyn UserRepo>,
    ) -> Self {
        Self {
            room_repo,
            zone_repo,
            user_repo,
        }
    }

    pub async fn hint_consider(&self, ctx: Arc<CmdCtx>, trigger: &str) -> AppResult<()> {
        let room_view = ctx.room_view()?;
        let current_visit = room_view.visit_count;

        for hint in room_view.blueprint.hints.iter() {
            if hint.when == trigger {
                let account_id = ctx.account_id()?;
                let room_id = ctx.room_id()?;
                let zone_id = ctx.zone_id()?;

                let kv = self.user_repo.room_kv(zone_id, room_id, account_id).await?;
                let shown = kv.get_bool(&format!("hint_shown_{}", hint.id), false);

                if hint.once.unwrap_or(false) && shown {
                    continue; // Already shown
                }

                // Check cooldown
                if let Some(cooldown) = hint.cooldown {
                    let last_shown_visit = kv.get_num::<i64>(&format!("hint_last_visit_{}", hint.id), 0);
                    if current_visit - last_shown_visit < cooldown as i64 {
                        continue; // Still in cooldown
                    }
                }

                // Show the hint
                ctx.output.system(format!("{{c:cyan:bright_cyan}}Hint: {}{{c}}", hint.text)).await;

                // Mark as shown
                if hint.once.unwrap_or(false) {
                    self.user_repo.set_room_kv(
                        zone_id,
                        room_id,
                        account_id,
                        &format!("hint_shown_{}", hint.id),
                        &serde_json::Value::Bool(true),
                    ).await?;
                }

                // Update last shown visit for cooldown
                if hint.cooldown.is_some() {
                    self.user_repo.set_room_kv(
                        zone_id,
                        room_id,
                        account_id,
                        &format!("hint_last_visit_{}", hint.id),
                        &serde_json::Value::Number(current_visit.into()),
                    ).await?;
                }
            }
        }

        Ok(())
    }

    // Travel to the given room
    pub async fn enter_room(&self, ctx: Arc<CmdCtx>, c: &Cursor) -> AppResult<()> {
        // Enter the current room

        ctx.sess.write().cursor = Some(c.clone());

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
        let account_id = ctx.account_id()?;
        let room_id = ctx.room_id()?;
        let zone_id = ctx.zone_id()?;
        let room = ctx.room_view()?;

        let kv = self.user_repo.room_kv(zone_id, room_id, account_id).await?;
        let cnt = kv.get_num::<i32>("has_entered", 0);

        // Key does not exist. This is the first time we enter, so we only update the counter
        self.user_repo.set_room_kv(
            zone_id,
            room_id,
            account_id,
            "has_entered",
            &serde_json::Value::Number(serde_json::Number::from(cnt + 1)),
        ).await?;

        let (tx, rx) = oneshot::channel();

        let output_handle = ctx.output.clone();

        let first_enter_hook = room.scripts.get(&ScriptHook::OnFirstEnter);

        if cnt == 0 && first_enter_hook.is_some() {
            // Enter first time hook if there is such a script
            ctx.lua_tx.send(LuaJob::OnFirstEnter {
                output_handle,
                cursor: Box::new(ctx.cursor()?),
                account: ctx.account()?,
                reply: tx,
            }).await.map_err(Box::from)?;
        } else {
            // Run on the first entry when there is no first enter hook
            // Run on each subsequent times
            ctx.lua_tx.send(LuaJob::OnEnter {
                output_handle,
                cursor: Box::new(ctx.cursor()?),
                account: ctx.account()?,
                reply: tx,
            }).await.map_err(Box::from)?;
        }

        match timeout(LUA_CMD_TIMEOUT, rx).await {
            Ok(Ok(lua_result)) => {
                match lua_result {
                    LuaResult::Failed(msg) => {
                        let s = format!("{{c:yellow:bright_red}}Lua script failuer: {msg}{{c}}");
                        ctx.output.system(s).await;
                        return Ok(());
                    }
                    LuaResult::Success(_) => {
                        let s = "{c:yellow:bright_green}Lua script completed without issues{c}";
                        ctx.output.system(s).await;
                    }
                }
            }
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
        // let account_id = ctx.account_id()?;
        // let room_id = ctx.room_id()?;
        // let zone_id = ctx.zone_id()?;

        let (tx, rx) = oneshot::channel();

        let output_handle = ctx.output.clone();

        // Enter first time hook
        ctx.lua_tx.send(LuaJob::OnLeave {
            output_handle,
            cursor: Box::new(ctx.cursor()?),
            account: ctx.account()?,
            reply: tx,
        }).await.map_err(Box::from)?;

        match timeout(LUA_CMD_TIMEOUT, rx).await {
            Err(e) => { ctx.output.system(format!("The room doesn't react ({e})")).await; }
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

    pub async fn set_exit_locked(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId, exit_dir: Direction, locked: bool) -> AppResult<()>{
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                self.user_repo.set_exit_locked(zone_id, room_id, account_id, exit_id, locked).await?;
                Ok(())
            },
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn set_exit_locked_shared(&self, zone_id: ZoneId, room_id: RoomId, exit_dir: Direction, locked: bool) -> AppResult<()> {
        match self.exit_by_direction(room_id, exit_dir).await? {
            Some(exit_id) => {
                self.zone_repo.set_exit_locked(zone_id, room_id, exit_id, locked).await?;
                Ok(())
            },
            None => Err(DomainError::NotFound("Exit not found".into())),
        }
    }

    pub async fn build_room_view(&self, zone_ctx: &ZoneContext, account_id: AccountId, room_id: RoomId) -> AppResult<RoomView> {
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
            &user_qty
        );

        Ok(rv)
    }
}
