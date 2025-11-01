// use crate::error::{AppResult, DomainError};
// use crate::models::types::{AccountId, Direction, RoomId};
// use crate::state::session::Cursor;
// use std::sync::Arc;
//
// #[allow(unused)]
// #[derive(Clone)]
// struct ResolvedExit {
//     from: RoomId,
//     to_room: RoomId,
//     locked_message: Option<String>,
// }
//
// pub struct NavigatorService {
//     pub zone_router: Arc<ZoneRouter>,
// }
//
// impl NavigatorService {
//     pub fn new(zone_router: Arc<ZoneRouter>) -> Self {
//         Self { zone_router }
//     }
//
//     // pub async fn go(&self, cursor: &Cursor, account_id: AccountId, dir: Direction) -> AppResult<(RoomId, RoomId)> {
//     //     let zone_id = cursor.zone_ctx.zone.id;
//     //     let from_id = cursor.room_view.room.id;
//     //
//     //     let (_exit, to_id) = self.resolve_exit_checked(cursor, account_id, from_id, dir).await?;
//     //
//     //     let state = self.zone_router.storage_for(&cursor.zone_ctx);
//     //     state.set_current_room(zone_id, account_id, to_id).await?;
//     //     // state.record_travel(account_id, from_id, to_id).await?;
//     //     // If you have tolls, deduct coins here:
//     //     // uow.update_coins(account_id, -toll_amount).await?;
//     //     // If you want to award travel XP:
//     //     // state.update_xp(account_id, 5).await?;
//     //     // uow.commit().await?;
//     //
//     //     Ok((from_id, to_id))
//     // }
//
//     #[allow(unused)]
//     async fn resolve_exit_checked(
//         &self,
//         cursor: &Cursor,
//         _account_id: AccountId,
//         from: RoomId,
//         dir: Direction,
//     ) -> AppResult<(ResolvedExit, RoomId)> {
//         // 1) Find matching exit in the current room view
//         let want = dir.to_short();
//         let exit_row = cursor
//             .room_view
//             .exits
//             .iter()
//             .find(|e| e.direction.to_short() == want && e.from_room_id == from)
//             .cloned();
//         // .map_e(|| DomainError::InvalidDirection);
//
//         let exit_row = match exit_row {
//             Some(e) => e,
//             None => return Err(DomainError::InvalidDirection(format!("No exit '{want}' from here."))),
//         };
//
//         // 2) Pull the player’s per-zone state for checks (xp/coins/inventory)
//         // let state_repo = self.zone_router.state_for(&cursor.zone_ctx);
//         // let zstate = state_repo
//         //     .zone_room_state(&cursor.zone_ctx, from, account_id)
//         //     .await?;
//
//         // // 3) Parse constraints/flags from the exit. Adjust field access to your model.
//         // // Here we assume ExitRow has an optional `state: serde_json::Value`.
//         let (locked, locked_msg) = (exit_row.flags.locked, "The way is locked.".to_string());
//         // parse_exit_state(&exit_row.state);
//
//         if locked {
//             return Err(DomainError::LockedExit(locked_msg));
//         }
//
//         // if let Some(mx) = min_xp {
//         //     if zstate.xp < mx {
//         //         return Err(DomainError::LockedExit(format!(
//         //             "You need at least {mx} XP to go that way."
//         //         )));
//         //     }
//         // }
//
//         // if let Some(item) = req_item {
//         //     let need = req_qty.unwrap_or(1);
//         //     let have = zstate
//         //         .items
//         //         .iter()
//         //         .find(|(oid, _)| *oid == item)
//         //         .map(|(_, qty)| *qty)
//         //         .unwrap_or(0);
//         //     if have < need {
//         //         return Err(DomainError::LockedExit("You’re missing something important.".into()));
//         //     }
//         // }
//
//         // if let Some(cost) = toll {
//         //     if zstate.coins < cost {
//         //         return Err(DomainError::LockedExit(format!(
//         //             "You need {cost} coins to pass."
//         //         )));
//         //     }
//         // }
//
//         // 4) Optional: Lua pre-hook (can_go)
//         // TODO: call into your Lua worker with (zone_ctx, account_id, exit_row).
//         // If it returns false or a message, surface that as a user error.
//
//         // 5) Success
//         let resolved = ResolvedExit {
//             from,
//             to_room: exit_row.to_room_id,
//             locked_message: locked_msg.into(),
//         };
//         Ok((resolved, exit_row.to_room_id))
//     }
// }
