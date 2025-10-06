//! @bp import <bp> <dir>

use std::path::Path;
use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::commands::CommandResult::{Failure, Success};
use crate::error::AppResult;
use crate::input::parser::Intent;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> AppResult<CommandResult> {
    if intent.args.len() < 4 {
        return Ok(Failure(super::USAGE.into()));
    }

    let bp     = &intent.args[2];
    let subdir = &intent.args[3];

    // If you want to enforce permissions later:
    // if !ctx.sess.lock().await.is_admin() { return Ok("[bp] permission denied.\n".into()); }

    let base_path = Path::new(ctx.state.registry.config.import_dir.as_str());
    match crate::import::import_blueprint_subdir(bp, subdir, &base_path, &ctx.state.registry.db).await {
        Ok(()) => Ok(Success(format!(
            "[bp] imported YAML rooms from {}/{} into `{}`.\n",
            base_path.display(), subdir, bp
        ))),
        Err(e) => Ok(Failure(format!("[bp] import failed: {:#}\n", e))),
    }
}
