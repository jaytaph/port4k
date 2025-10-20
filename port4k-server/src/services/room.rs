use std::collections::HashMap;
use crate::db::repo::{ZoneRepo, RoomRepo, UserRepo};
use crate::models::types::{AccountId, RoomId};
use std::sync::Arc;
use crate::error::AppResult;
use crate::models::room::{build_room_view_impl, RoomView};
use crate::models::zone::ZoneContext;

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

    pub async fn build_room_view(&self, zone_ctx: &ZoneContext, account_id: AccountId, room_id: RoomId) -> AppResult<RoomView> {
        // Get blueprint room data
        let bp_room = self.room_repo.room_by_id(zone_ctx.blueprint.id, room_id).await.unwrap();
        let bp_exits = self.room_repo.room_exits(room_id).await.unwrap();
        let bp_objs = self.room_repo.room_objects(room_id).await.unwrap();
        let bp_room_kv = self.room_repo.room_kv(room_id).await.unwrap();

        // Get zone info
        let zone_room_kv = self.zone_repo.room_kv(zone_ctx.zone.id, room_id).await.unwrap();
        let zone_obj_kv = self.zone_repo.obj_kv(zone_ctx.zone.id, room_id).await.unwrap();

        // get account info
        let user_room_kv = self.user_repo.room_kv(zone_ctx.zone.id, room_id, account_id).await.unwrap();
        let user_obj_kv = self.user_repo.obj_kv(zone_ctx.zone.id, room_id, account_id).await.unwrap();

        // @todo: not filled yet
        let zone_qty = HashMap::new();
        let user_qty = HashMap::new();

        let rv = build_room_view_impl(
            &bp_room,
            &bp_exits.as_slice(),
            &bp_objs.as_slice(),
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

    // pub async fn room_kv_get_all(&self, room_id: RoomId) -> AppResult<serde_json::Map<String, serde_json::Value>> {
    //     let kv_pairs = self.kv_repo.room_kv_get_all(room_id).await?;
    //     Ok(kv_pairs)
    // }
    //
    // /// Get room key-value data
    // pub async fn room_kv_get(&self, room_id: RoomId, object_key: &str) -> AppResult<serde_json::Value> {
    //     let v = self.kv_repo.room_kv_get(room_id, object_key).await?;
    //     Ok(v)
    // }

    // /// Set room key-value data
    // pub async fn room_kv_set(&self, room_id: RoomId, object_key: &str, v: &serde_json::Value) -> AppResult<()> {
    //     self.kv_repo.room_kv_set(room_id, object_key, v.clone()).await?;
    //     Ok(())
    // }

    // /// Get player key-value data for specific room
    // pub async fn player_kv_get(
    //     &self,
    //     room_id: RoomId,
    //     account_id: AccountId,
    //     object_key: &str,
    // ) -> AppResult<Option<serde_json::Value>> {
    //     let v = self.kv_repo.player_kv_get(room_id, account_id, object_key).await?;
    //     Ok(v)
    // }

    // /// Set player key-value data for specific room
    // pub async fn player_kv_set(
    //     &self,
    //     room_id: RoomId,
    //     account_id: AccountId,
    //     object_key: &str,
    //     v: &serde_json::Value,
    // ) -> AppResult<()> {
    //     self.kv_repo
    //         .player_kv_set(room_id, account_id, object_key, v.clone())
    //         .await?;
    //     Ok(())
    // }

    // /// Creates a RoomView for the given cursor in the given zone context
    // pub async fn build_room_view(
    //     &self,
    //     router: Arc<ZoneRouter>,
    //     zone_ctx: &ZoneContext,
    //     account_id: AccountId,
    //     room_id: RoomId,
    // ) -> AppResult<RoomView> {
    //     let zone_storage: Arc<dyn StateStorage> = router.storage_for(zone_ctx);
    //
    //     let room_fut = async {
    //         self.room_repo
    //             .room_by_id(zone_ctx.blueprint.id, room_id)
    //             .await
    //             .map_err(DomainError::from)
    //     };
    //     let exits_fut = async { self.room_repo.room_exits(room_id).await.map_err(DomainError::from) };
    //     let objects_fut = async { self.room_repo.room_objects(room_id).await.map_err(DomainError::from) };
    //     let scripts_fut = async {
    //         self.room_repo
    //             .room_scripts(room_id, ScriptSource::Live)
    //             .await
    //             .map_err(DomainError::from)
    //     };
    //     let kv_fut = async { self.room_repo.room_kv(room_id).await.map_err(DomainError::from) };
    //     let state_fut = async { zone_state.zone_room_state(zone_ctx, room_id, account_id).await };
    //
    //     let (room, exits, objects, scripts, room_kv, zone_state) =
    //         try_join!(room_fut, exits_fut, objects_fut, scripts_fut, kv_fut, state_fut)?;
    //
    //     Ok(RoomView {
    //         room,
    //         objects,
    //         scripts,
    //         room_kv,
    //         exits,
    //         // zone_state: Some(zone_state),
    //     })
    // }

    // pub async fn object_kv_get(&self, room_id: RoomId, obj_name: &str, key: &str) -> AppResult<serde_json::Value> {
    //     println!("********** OBJEC_KV_KEY)");
    //     dbg!(&obj_name, &key);
    //     let full_key = format!("object:{}:{}", obj_name, key);
    //     let v = self.kv_repo.room_kv_get(room_id, &full_key).await?;
    //     Ok(v)
    // }
}
