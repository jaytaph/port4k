use crate::commands::CmdCtx;
use anyhow::Result;

#[allow(unused)]
pub async fn balance(ctx: &CmdCtx) -> Result<String> {
    let name = ctx.sess.read().unwrap().name.clone();
    let Some(user) = name else {
        return Ok("You must `login` first.\r\n".into());
    };
    let bal = ctx.registry.db.account_balance(&user.0).await?;
    Ok(format!("Your balance: {bal} coin(s).\r\n"))
}
