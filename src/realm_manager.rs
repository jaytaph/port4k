// use chrono::Utc;
// use crate::error::AppResult;
// use crate::models::realm::{Realm, RealmKind};
// use crate::models::types::{AccountId, BlueprintId, RealmId};
//
// pub struct RealmManager {
//
// }
//
// impl RealmManager {
//     pub async fn create_test_realm(&self, owner: AccountId, bp_id: BlueprintId) -> AppResult<RealmId> {
//         let realm = Realm {
//             id: RealmId::new(),
//             bp_id,
//             title: format!("Test Realm for {}", owner),
//             kind: RealmKind::Test { owner },
//             created_at: Utc::now(),
//         };
//
//         Ok(realm.id);
//     }
//
//     pub async fn create_persistent_realm(&self, title: &str, owner: AccountId, bp_id: BlueprintId) -> AppResult<RealmId> {
//         let realm_id = RealmId::new();
//
//         client.execute(
//             "INSERT INTO realms (id, bp_id, kind, name, created_at) VALUES ($1, $2, $3, $4, $5)",
//             &[
//                 &realm_id,
//                 &bp_id,
//                 title,
//                 &"persistent",
//                 &Utc::now(),
//             ],
//         );
//
//         let realm = Realm {
//             id: RealmId::new(),
//             bp_id,
//             kind: RealmKind::Persistent { name },
//             created_at: Utc::now(),
//         };
//
//         Ok(realm.id);
//     }
// }
