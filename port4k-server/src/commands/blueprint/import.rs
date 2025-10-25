//! @bp import <bp> <dir>

use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::path::Path;
use std::sync::Arc;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.len() < 4 {
        ctx.output.system(super::USAGE).await;
        return Ok(());
    }

    let bp_key = &intent.args[2];
    let subdir = &intent.args[3];

    // If you want to enforce permissions later:
    // if !ctx.sess.lock().await.is_admin() { return Ok("[bp] permission denied.\n".into()); }

    let blueprint = ctx.registry.repos.room.blueprint_by_key(bp_key).await?;

    let base_path = Path::new(ctx.registry.config.import_dir.as_str());
    match crate::import::import_blueprint_sub_dir(blueprint.id, subdir, base_path, &ctx.registry.db).await {
        Ok(()) => {
            ctx.output.system(format!(
                "[bp] imported YAML rooms from {}/{} into `{}`.",
                base_path.display(),
                subdir,
                bp_key
            )).await;
        }
        Err(e) => {
            ctx.output.system(format!("[bp] import failed: {:#}", e)).await;
        }
    }

    Ok(())
}
