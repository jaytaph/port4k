use std::sync::Arc;
use crate::db::repo::kv::KvRepo;
use crate::error::AppResult;
use crate::models::account::Account;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RoomId};
use crate::models::zone::ZoneContext;
use crate::state::session::Cursor;

pub struct RoomService {
    repo: Arc<dyn KvRepo>,
}

impl RoomService {
    pub fn new(repo: Arc<dyn KvRepo>) -> Self {
        Self { repo }
    }

    pub async fn room_kv_get(&self, room_id: RoomId, object_key: &str) -> AppResult<serde_json::Value> {
        let v = self.repo.room_kv_get(room_id, object_key).await?;
        Ok(v)
    }

    pub async fn room_kv_set(&self, room_id: RoomId, object_key: &str, v: &serde_json::Value) -> AppResult<()> {
        self.repo.room_kv_set(room_id, object_key, v.clone()).await?;
        Ok(())
    }

    pub async fn player_kv_get(&self, room_id: RoomId, account_id: AccountId, object_key: &str) -> AppResult<Option<serde_json::Value>> {
        let v = self.repo.player_kv_get(room_id, account_id, object_key).await?;
        Ok(v)
    }

    pub async fn player_kv_set(&self, room_id: RoomId, account_id: AccountId, object_key: &str, v: &serde_json::Value) -> AppResult<()> {
        self.repo.player_kv_set(room_id, account_id, object_key, v.clone()).await?;
        Ok(())
    }

    pub async fn create_view(&self, zone_ctx: &ZoneContext, account: &Account, cursor: &Cursor) -> AppResult<RoomView> {
        let room = zone_ctx.get_room(cursor.room_id).ok_or(crate::error::DomainError::NotFound)?;

        let mut objects = vec![];
        for obj in &room.objects {
            let mut obj_view = obj.clone();
            if let Some(kv) = self.room_kv_get(room.id, &obj.key).await.ok() {
                obj_view.kv = kv;
            }
            objects.push(obj_view);
        }

        let mut players = vec![];
        for other_cursor in zone_ctx.list_cursors_in_room(room.id).await {
            if other_cursor.account_id != account.id {
                if let Some(other_account) = zone_ctx.get_account(other_cursor.account_id).await {
                    let mut player_view = crate::models::room::PlayerView {
                        id: other_account.id,
                        username: other_account.username.clone(),
                        kv: serde_json::Value::Null,
                    };
                    if let Some(kv) = self.player_kv_get(room.id, other_account.id, "profile").await.ok().flatten() {
                        player_view.kv = kv;
                    }
                    players.push(player_view);
                }
            }
        }

        Ok(RoomView {
            room: room.clone(),
            objects,
            scripts: Default::default(),
            room_kv: Default::default(),
            exits: room.exits.clone(),
            zone_state: None,
        })
    }
}