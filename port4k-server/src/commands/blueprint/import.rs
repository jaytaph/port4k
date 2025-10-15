//! @bp import <bp> <dir>

use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::db::error::DbError;
use crate::input::parser::Intent;
use std::path::Path;
use std::sync::Arc;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 4 {
        out.append(super::USAGE);
        out.failure();
        return Ok(out);
    }

    let bp_key = &intent.args[2];
    let subdir = &intent.args[3];

    // If you want to enforce permissions later:
    // if !ctx.sess.lock().await.is_admin() { return Ok("[bp] permission denied.\n".into()); }

    let blueprint = ctx
        .registry
        .repos
        .room
        .blueprint_by_key(bp_key)
        .await
        .map_err(DbError::from)?;

    let base_path = Path::new(ctx.registry.config.import_dir.as_str());
    match crate::import::import_blueprint_subdir(blueprint.id, subdir, &base_path, &ctx.registry.db).await {
        Ok(()) => {
            out.append(
                format!(
                    "[bp] imported YAML rooms from {}/{} into `{}`.\n",
                    base_path.display(),
                    subdir,
                    bp_key
                )
                .as_str(),
            );
            out.success();
        }
        Err(e) => {
            out.append(format!("[bp] import failed: {:#}\n", e).as_str());
            out.failure();
        }
    }

    Ok(out)
}
