//! @bp import <bp> <dir>

use std::path::Path;
use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::{failure, success};
use crate::db::error::DbError;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 4 {
        return Ok(failure!(super::USAGE));
    }

    let bp_key = &intent.args[2];
    let subdir = &intent.args[3];

    // If you want to enforce permissions later:
    // if !ctx.sess.lock().await.is_admin() { return Ok("[bp] permission denied.\n".into()); }

    let blueprint = ctx.registry.repos.room.blueprint_by_key(bp_key).await.map_err(DbError::from)?;

    let base_path = Path::new(ctx.registry.config.import_dir.as_str());
    match crate::import::import_blueprint_subdir(blueprint.id, subdir, &base_path, &ctx.registry.db).await {
        Ok(()) => Ok(success!(format!(
            "[bp] imported YAML rooms from {}/{} into `{}`.\n",
            base_path.display(), subdir, bp_key
        ))),
        Err(e) => Ok(failure!(format!("[bp] import failed: {:#}\n", e))),
    }
}
