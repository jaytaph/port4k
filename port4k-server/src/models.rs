use crate::db::DbResult;
use crate::db::error::DbError;
use serde_json::Value;

pub mod account;
pub mod blueprint;
pub mod character;
pub mod room;
pub mod types;
pub mod zone;

pub fn json_string_vec_opt(v: Option<Value>, field: &'static str) -> DbResult<Vec<String>> {
    match v {
        None => Ok(Vec::new()),
        Some(val) => {
            let out: Vec<String> = serde_json::from_value(val).map_err(|_| DbError::Decode(field.into()))?;
            Ok(out)
        }
    }
}
