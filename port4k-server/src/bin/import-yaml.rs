use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Context;
use clap::Parser;
use uuid::Uuid;
use port4k_server::config;
use port4k_server::import::import_blueprint_sub_dir;
use port4k_server::models::types::BlueprintId;

/// Adjust this to however you construct your Db wrapper.
use port4k_server::db::Db;

#[derive(Debug, Parser)]
#[command(name = "import-yaml", version, about = "Import YAML rooms into a blueprint")]
struct Args {
    /// Blueprint UUID (mutually exclusive with --bp-key)
    #[arg(long)]
    bp_id: Option<Uuid>,

    /// Blueprint key (creates if missing when --owner is provided)
    #[arg(long, conflicts_with = "bp_id")]
    bp_key: Option<String>,

    /// Owner username for creating a new blueprint (with --bp-key)
    #[arg(long)]
    owner: Option<String>,

    /// Subdirectory under content_base that contains the YAML files
    #[arg(long)]
    subdir: Option<String>,

    /// Optionally set the blueprint’s entry room by key after import
    #[arg(long)]
    entry_room: Option<String>,

    /// DB URL (defaults to $DATABASE_URL)
    #[arg(long)]
    database_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let cfg = Arc::new(config::Config::from_env()?);

    let db = Arc::new(Db::new(&cfg.database_url)?);
    db.init().await?;

    // Resolve or create blueprint
    let bp_id = match (args.bp_id, args.bp_key.as_deref()) {
        (Some(id), _) => id,
        (None, Some(key)) => ensure_blueprint(&db, key, args.owner.as_deref()).await?,
        _ => anyhow::bail!("Provide either --bp-id or --bp-key (with optional --owner)"),
    };

    let content_base = PathBuf::from(cfg.import_dir.clone());
    let sub_dir = args.subdir.unwrap_or_else(|| ".".to_string());

    // Run importer
    import_blueprint_sub_dir(BlueprintId::from(bp_id), &sub_dir, content_base.as_path(), &db).await
        .map_err(|e| anyhow::anyhow!("import failed: {e}"))?;

    // Optionally set entry room
    if let Some(entry_key) = args.entry_room.as_deref() {
        set_entry_room(&db, bp_id, entry_key).await?;
    }

    println!("✓ Import complete into blueprint {bp_id}");
    println!("  subdir:       {sub_dir}");
    if let Some(k) = args.bp_key.as_deref() {
        println!("  blueprint key: {k}");
    }
    if let Some(entry) = args.entry_room.as_deref() {
        println!("  entry room set to: {entry}");
    }

    Ok(())
}

async fn ensure_blueprint(db: &Db, key: &str, owner_username: Option<&str>) -> anyhow::Result<Uuid> {
    // Try existing
    if let Some(id) = query_opt_uuid(
        db,
        "SELECT id FROM blueprints WHERE key = $1",
        &[&key],
    ).await? {
        return Ok(id);
    }
    let owner = owner_username
        .context("Blueprint not found; to create it, pass --owner <username>")?;
    let owner_id = query_opt_uuid(
        db,
        "SELECT id FROM accounts WHERE username = $1",
        &[&owner],
    ).await?
        .context("Owner username not found in accounts")?;

    let client = db.get_client().await?;
    let row = client.query_one(
            "INSERT INTO blueprints (key, title, owner_id, status)
             VALUES ($1,$2,$3,'draft')
             ON CONFLICT (key) DO UPDATE SET title = EXCLUDED.title
             RETURNING id",
            &[&key, &key, &owner_id],
        )
        .await?;
    Ok(row.get(0))
}

async fn set_entry_room(db: &Db, bp_id: Uuid, room_key: &str) -> anyhow::Result<()> {
    let client = db.get_client().await?;
    let row = client
        .query_opt(
            "SELECT id FROM bp_rooms WHERE bp_id = $1 AND key = $2",
            &[&bp_id, &room_key],
        )
        .await?;
    let room_id: Uuid = row
        .context("entry room key not found in blueprint")?
        .get(0);
    client
        .execute(
            "UPDATE blueprints SET entry_room_id = $1 WHERE id = $2",
            &[&room_id, &bp_id],
        )
        .await?;
    Ok(())
}

async fn query_opt_uuid(
    db: &Db,
    sql: &str,
    params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
) -> anyhow::Result<Option<Uuid>> {
    let client = db.get_client().await?;
    let row = client.query_opt(sql, params).await?;
    Ok(row.map(|r| r.get::<_, Uuid>(0)))
}
