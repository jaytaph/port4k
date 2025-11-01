use crate::commands::{CmdCtx, CommandResult};
use std::sync::Arc;
use crate::input::parser::Intent;

pub async fn inventory(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult {
    let realm_id = ctx.realm_id()?;
    let account_id = ctx.account_id()?;

    let items = ctx.registry.services.inventory.get_player_inventory(realm_id, account_id).await?;
    if items.is_empty() {
        ctx.output.line("Your inventory is empty.").await;
        return Ok(());
    }

    let headers = vec!["Quantity".to_string(), "Item".to_string(), "Description".to_string()];
    let rows: Vec<Vec<String>> = items.iter()
        .map(|item| vec![item.quantity.to_string(), item.name.clone(), item.short.clone()])
        .collect();
    ctx.output.table(headers, rows).await;

    Ok(())
}
