use crate::commands::{CmdCtx, CommandError, CommandResult};
use crate::error::{AppResult, DomainError, LoginError};
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::models::realm::Realm;
use crate::models::room::RoomView;
use crate::models::types::{RealmId, RoomId};
use std::sync::Arc;

const DEFAULT_REALM_KEY: &'static str = "live_world";
const DEFAULT_ROOM_KEY: &'static str = "cell_block";

const MOTD: &str = r#"

** ==============  PORT4K INCOMING MESSAGE =================
**
**   Welcome back, {c:yellow}{v:account.name}{c}!  (last login: {v:last_login:Never logged in before})
**   Server time: {c:white}{v:wall_time}{c}
**
**   Account:
**      HP    : {c:green:bold}{v:account.health:0}/100{c}
**      XP    : {c:green:bold}{v:account.xp:0}{c} (Level {v:account.xp_level:1}: {v:account.xp_level_name})
**      Coins : {c:green:bold}{v:account.coins:0}{c}
**
**   News:
**    - New vault area unlocked in The Hub.
**    - Type 'help' or 'commands' to get started.
**    - Use 'who' to see who's online.
**
**   Tips:
**    - Most rooms have hidden nouns. Try: {c:blue}'examine terminal'{c}, {c:blue}'open crate'{c}.
**    - Use cardinal directions or verbs like {c:blue}'in'{c}/{c:blue}'out'{c} to move.
**    - Stuck? Try {c:blue}'look'{c}, {c:blue}'hint'{c}, or {c:blue}'scan'{c}.
**
**   Enjoy your stay, {v:account.role} {v:account.name}.
**
** ====================  END OF MESSAGE ====================

"#;

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    // Step 1: Validate input
    if intent.args.len() < 3 {
        ctx.output.line("Login failed. Check your username and password.").await;
        ctx.output
            .system("Not enough arguments. Usage: login <user> [pass]")
            .await;
        return Ok(());
    }

    // Step 2: Attempt to login
    let account = match ctx
        .registry
        .services
        .account
        .login(intent.args[1].as_str(), intent.args[2].as_str())
        .await
    {
        Ok(account) => account,
        Err(err) => {
            match err {
                LoginError::UserNotFound => {
                    ctx.output
                        .system("Login failed. Check your username and password.")
                        .await;
                }
                LoginError::InvalidPassword => {
                    ctx.output
                        .system("Login failed. Check your username and password.")
                        .await;
                }
                LoginError::AccountLocked => {
                    ctx.output
                        .system("This account has been locked. Please contact admin for support")
                        .await;
                }
                LoginError::TooManyAttempts => {
                    ctx.output
                        .system("This account has been tried too many times. Please contact admin for support")
                        .await;
                }
                LoginError::InternalError(e) => {
                    ctx.output
                        .system(format!("Login failed due to server error. Contact admin. Error: {}", e))
                        .await;
                }
            }
            return Ok(());
        }
    };

    // Step 3: find realm and room to spawn into
    let realm_id = resolve_realm_id(&ctx, &account).await.map_err(|e| {
        CommandError::Custom(e.to_string())
        // CommandError::Custom("Failed to resolve starting realm.".to_string())
    })?;
    let realm = load_realm(&ctx, realm_id)
        .await
        .map_err(|_| CommandError::Custom("Failed to load starting realm.".to_string()))?;
    let room_id = resolve_room_id(&ctx, &account, realm.id)
        .await
        .map_err(|_| CommandError::Custom("Failed to resolve starting room.".to_string()))?;
    let room = load_room(&ctx, &account, realm.id, room_id)
        .await
        .map_err(|_| CommandError::Custom("Failed to load starting room.".to_string()))?;

    // Step 4: Log into the session at the realm/room
    ctx.sess.write().login(account, realm, room);

    ctx.output
        .system("You are logged in. Welcome to port4k!".to_string())
        .await;

    ctx.output.line("You have successfully logged in.").await;

    // Step 5: Show MOTD if needed
    if ctx.account()?.show_motd {
        ctx.output.system(MOTD).await;
    }

    // Step 6: "enter" the room
    ctx.registry
        .services
        .room
        .enter_room(ctx.clone(), &ctx.cursor()?)
        .await?;

    Ok(())
}

async fn resolve_realm_id(ctx: &Arc<CmdCtx>, account: &Account) -> AppResult<RealmId> {
    if let Some(rid) = account.current_realm_id {
        return Ok(rid);
    }

    // fall back to default realm
    match ctx.registry.services.realm.get_by_key(DEFAULT_REALM_KEY).await {
        Ok(Some(realm)) => Ok(realm.id),
        // Err(e) => panic!("{}", e.to_string()),
        _ => fail_login(ctx, "Default realm not found.").await,
    }
}

async fn load_realm(ctx: &Arc<CmdCtx>, realm_id: RealmId) -> AppResult<Realm> {
    match ctx.registry.services.realm.get_by_id(realm_id).await {
        Ok(Some(realm)) => Ok(realm),
        Ok(None) => fail_login::<Realm>(ctx, "Starting realm not found.").await,
        Err(_) => fail_login::<Realm>(ctx, "Starting realm not found.").await,
    }
}

async fn resolve_room_id(ctx: &Arc<CmdCtx>, account: &Account, realm_id: RealmId) -> AppResult<RoomId> {
    if let Some(rid) = account.current_room_id {
        return Ok(rid);
    }

    // fall back to default room in this realm
    match ctx
        .registry
        .services
        .room
        .get_room_id_by_key(realm_id, DEFAULT_ROOM_KEY)
        .await
    {
        Ok(Some(rid)) => Ok(rid),
        _ => fail_login(ctx, "Default room not found.").await,
    }
}

async fn load_room(ctx: &Arc<CmdCtx>, account: &Account, realm_id: RealmId, room_id: RoomId) -> AppResult<RoomView> {
    match ctx
        .registry
        .services
        .room
        .get_by_id(realm_id, account.id, room_id)
        .await
    {
        Ok(room) => Ok(room),
        Err(_) => fail_login(ctx, "Starting room not found.").await?,
    }
}

async fn fail_login<T>(ctx: &Arc<CmdCtx>, internal_msg: &str) -> AppResult<T> {
    ctx.output
        .line("Login failed due to server error. Contact admin.")
        .await;
    ctx.output.system(format!("Error: {internal_msg}")).await;
    Err(DomainError::LoginError(internal_msg.to_string()))
}
