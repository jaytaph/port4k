use anyhow::Context;
use clap::Parser;
use port4k::config;
use port4k::models::types::BlueprintId;
use std::sync::Arc;
use uuid::Uuid;

use port4k::db::Db;

// cargo run --bin create-realm -- --bp-key hub --title "The Hub" --key "the_hub" --owner system --kind live --entry "main_square"

#[derive(Debug, Parser)]
#[command(name = "create-realm", version, about = "Create a new realm based on a blueprint")]
struct Args {
    /// Key of the blueprint to base the realm on (eg: "hub")
    #[arg(long)]
    bp_key: String,

    /// Human readable title of the realm (eg: "The Hub")
    #[arg(long)]
    title: String,

    /// Machine key / slug for the realm (eg: "the_hub")
    #[arg(long)]
    key: String,

    /// Owner account / system actor
    #[arg(long)]
    owner: String,

    /// Kind of realm (eg: "live", "staging", "template")
    #[arg(long)]
    kind: String,

    /// Override database URL (if omitted, use env/config)
    #[arg(long)]
    database_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // load config from env
    let cfg = Arc::new(config::Config::from_env()?);

    // allow overriding the DSN from CLI
    let database_url = args
        .database_url
        .as_deref()
        .unwrap_or(&cfg.database_url);

    let db = Arc::new(Db::new(database_url)?);
    db.init().await?;

    // 1. make sure the blueprint exists (or import it)
    let blueprint_id = ensure_blueprint_exists(db.clone(), &args.bp_key).await?;

    // 2. create realm in DB
    let realm_id = create_realm(
        db.clone(),
        blueprint_id,
        &args.key,
        &args.title,
        &args.owner,
        &args.kind,
    )
        .await?;

    println!(
        "✅ Realm created:\n  id: {}\n  key: {}\n  title: {}\n  bp_id: {}",
        realm_id, args.key, args.title, blueprint_id
    );

    Ok(())
}

/// Ensure there is a blueprint with this key in the database.
/// If it doesn't exist yet, attempt to import it from the given dir.
async fn ensure_blueprint_exists(
    db: Arc<Db>,
    bp_key: &str,
) -> anyhow::Result<BlueprintId> {
    let client = db.get_client().await?;

    // Try to find blueprint by key
    if let Some(row) = client
        .query_opt(
            r#"SELECT id FROM blueprints WHERE key = $1"#,
            &[&bp_key],
        )
        .await?
    {
        let id: Uuid = row.get("id");
        return Ok(BlueprintId(id));
    }

    Err(anyhow::anyhow!("Blueprint not found"))
}

/// Actually inserts the realm into the database.
/// Adjust table/column names here to match your schema.
async fn create_realm(
    db: Arc<Db>,
    blueprint_id: BlueprintId,
    key: &str,
    title: &str,
    owner: &str,
    kind: &str,
) -> anyhow::Result<Uuid> {
    let realm_id = Uuid::new_v4();
    let client = db.get_client().await?;

    // Get owner id from username
    let owner_row = client
        .query_one(
            r#"SELECT id FROM accounts WHERE username = $1"#,
            &[&owner],
        )
        .await
        .with_context(|| format!("failed to find owner account '{}'", owner))?;
    let owner_id: Uuid = owner_row.get("id");

    // You might have extra columns: created_by, updated_at, jsonb config, etc.
    client
        .execute(
            r#"
            INSERT INTO realms (id, bp_id, key, title, owner_id, kind)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            &[
                &realm_id,
                &blueprint_id.0, // assuming BlueprintId(Uuid)
                &key,
                &title,
                &owner_id,
                &kind,
            ],
        )
        .await
        .with_context(|| "failed to insert realm")?;

    Ok(realm_id)
}
