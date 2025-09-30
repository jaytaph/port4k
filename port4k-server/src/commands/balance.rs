use crate::commands::CmdCtx;
use anyhow::Result;

pub async fn who(ctx: &CmdCtx<'_>) -> Result<String> {
    let list = ctx.registry.who().await;
    Ok(if list.is_empty() {
        "No one is online.\r\n".into()
    } else {
        format!("Online ({}): {}\r\n", list.len(), list.join(", "))
    })
}

pub async fn balance(ctx: &CmdCtx<'_>) -> Result<String> {
    let name = ctx.sess.lock().await.name.clone();
    let Some(user) = name else {
        return Ok("You must `login` first.\r\n".into());
    };
    let bal = ctx.registry.db.account_balance(&user.0).await?;
    Ok(format!("Your balance: {bal} coin(s).\r\n"))
}
