use crate::commands::{CmdCtx, CommandResult};
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};

#[allow(unused)]
pub async fn balance(ctx: &CmdCtx) -> Result<CommandResult> {
    let name = ctx.sess.read().unwrap().name.clone();
    let Some(user) = name else {
        return Ok(Failure("You must `login` first.\n".into()));
    };

    let bal = ctx.registry.db.account_balance(&user.0).await?;

    Ok(Success(format!("Your balance: {bal} coin(s).\n")))
}
