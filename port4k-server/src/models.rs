use serde_json::Value;
use crate::db::DbResult;
use crate::db::error::DbError;

pub mod account;
pub mod character;
pub mod blueprint;
pub mod zone;
pub mod room;
pub mod types;

pub fn json_string_vec_opt(v: Option<Value>, field: &'static str) -> DbResult<Vec<String>> {
    match v {
        None => Ok(Vec::new()),
        Some(val) => {
            let out: Vec<String> = serde_json::from_value(val)
                .map_err(|_| DbError::Decode(field.into()))?;
            Ok(out)
        }
    }
}