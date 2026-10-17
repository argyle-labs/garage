use plugin_toolkit::prelude::*;
use plugin_toolkit::secrets;

use crate::api::{
    Bucket, BucketRef, BucketSummary, ClusterHealth, ClusterStatus, Garage, Key, KeySummary,
    Layout, LayoutApplied, Perms, Quotas,
};
use crate::endpoint_db;

pub const PROVIDER: &str = "garage";

pub async fn connect(name: &str) -> Result<Garage> {
    let token = secrets::resolve_scoped(PROVIDER, name, "admin_token")?;
    let row = endpoint_db::require(name)?;
    require_routes(name, &row.routes)?;
    let base_url = route::resolve_reachable(name, &row.routes, false).await?;
    Ok(Garage::new(&base_url, token.expose_secret()))
}

pub fn require_routes<T>(name: &str, routes: &[T]) -> Result<()> {
    if routes.is_empty() {
        bail!("endpoint {name} has no routes; set routes to the Garage admin API (port 3903)");
    }
    Ok(())
}

pub fn key_secret_name(endpoint: &str, key_id: &str) -> Result<String> {
    Ok(
        secrets::scoped_name(PROVIDER, endpoint, &format!("key_{key_id}"))?
            .as_str()
            .to_string(),
    )
}

#[orca_struct(args)]
pub struct EndpointArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
}

/// Cluster health: node and partition quorum counts (`GetClusterHealth`).
#[orca_tool(domain = "garage", verb = "cluster.health", role = "read")]
async fn garage_cluster_health(args: EndpointArgs, _ctx: &ToolCtx) -> Result<ClusterHealth> {
    connect(&args.name).await?.health().await
}

/// Nodes with address, version, role and free space (`GetClusterStatus`).
#[orca_tool(domain = "garage", verb = "cluster.status", role = "read")]
async fn garage_cluster_status(args: EndpointArgs, _ctx: &ToolCtx) -> Result<ClusterStatus> {
    connect(&args.name).await?.status().await
}

/// Current cluster layout and staged changes (`GetClusterLayout`).
#[orca_tool(domain = "garage", verb = "layout.detail", role = "read")]
async fn garage_layout_detail(args: EndpointArgs, _ctx: &ToolCtx) -> Result<Layout> {
    connect(&args.name).await?.layout().await
}

#[orca_struct(args)]
pub struct LayoutApplyArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Full node id from `garage.cluster.status`.
    #[arg(long)]
    pub node_id: String,
    /// Zone to place the node in.
    #[arg(long)]
    pub zone: String,
    /// Storage capacity in bytes.
    #[arg(long)]
    pub capacity: u64,
    /// Node tags. Comma-separated on the CLI.
    #[arg(long, value_delimiter = ',')]
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Assign a node a zone and capacity and apply the layout at the next version
/// (`UpdateClusterLayout`, `ApplyClusterLayout`).
#[orca_tool(
    domain = "garage",
    verb = "layout.apply",
    role = "admin",
    execute_gated = true
)]
async fn garage_layout_apply(args: LayoutApplyArgs, _ctx: &ToolCtx) -> Result<LayoutApplied> {
    connect(&args.name)
        .await?
        .assign_node(&args.node_id, &args.zone, args.capacity, &args.tags)
        .await
}

/// Buckets with their aliases (`ListBuckets`).
#[orca_tool(domain = "garage", verb = "bucket.list", role = "read")]
async fn garage_bucket_list(args: EndpointArgs, _ctx: &ToolCtx) -> Result<Vec<BucketSummary>> {
    connect(&args.name).await?.buckets().await
}

#[orca_struct(args)]
pub struct BucketArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Bucket id (64 hex chars) or global alias.
    #[arg(long)]
    pub bucket: String,
}

/// Bucket usage, quotas and key grants (`GetBucketInfo`).
#[orca_tool(domain = "garage", verb = "bucket.detail", role = "read")]
async fn garage_bucket_detail(args: BucketArgs, _ctx: &ToolCtx) -> Result<Bucket> {
    connect(&args.name)
        .await?
        .bucket(BucketRef::parse(&args.bucket))
        .await
}

#[orca_struct(args)]
pub struct BucketCreateArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Global alias (the S3 bucket name).
    #[arg(long)]
    pub alias: String,
}

/// Create a bucket under a global alias (`CreateBucket`).
#[orca_tool(
    domain = "garage",
    verb = "bucket.create",
    role = "admin",
    execute_gated = true
)]
async fn garage_bucket_create(args: BucketCreateArgs, _ctx: &ToolCtx) -> Result<Bucket> {
    connect(&args.name).await?.create_bucket(&args.alias).await
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
pub struct Deleted {
    pub id: String,
    pub deleted: bool,
}

/// Delete an empty bucket (`DeleteBucket`).
#[orca_tool(
    domain = "garage",
    verb = "bucket.delete",
    role = "admin",
    execute_gated = true
)]
async fn garage_bucket_delete(args: BucketArgs, _ctx: &ToolCtx) -> Result<Deleted> {
    let g = connect(&args.name).await?;
    let id = g.bucket_id(&args.bucket).await?;
    g.delete_bucket(&id).await?;
    Ok(Deleted { id, deleted: true })
}

#[orca_struct(args)]
pub struct BucketUpdateArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Bucket id (64 hex chars) or global alias.
    #[arg(long)]
    pub bucket: String,
    /// Maximum total bytes. Omit for no limit.
    #[arg(long)]
    pub max_size: Option<u64>,
    /// Maximum object count. Omit for no limit.
    #[arg(long)]
    pub max_objects: Option<u64>,
}

/// Set a bucket's size and object quotas; an omitted quota is removed
/// (`UpdateBucket`).
#[orca_tool(
    domain = "garage",
    verb = "bucket.update",
    role = "admin",
    execute_gated = true
)]
async fn garage_bucket_update(args: BucketUpdateArgs, _ctx: &ToolCtx) -> Result<Bucket> {
    let g = connect(&args.name).await?;
    let id = g.bucket_id(&args.bucket).await?;
    g.set_quotas(
        &id,
        Quotas {
            max_size: args.max_size,
            max_objects: args.max_objects,
        },
    )
    .await
}

#[orca_struct(args)]
pub struct GrantArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Bucket id (64 hex chars) or global alias.
    #[arg(long)]
    pub bucket: String,
    /// Access key id.
    #[arg(long)]
    pub key_id: String,
    #[arg(long)]
    #[serde(default)]
    pub read: bool,
    #[arg(long)]
    #[serde(default)]
    pub write: bool,
    #[arg(long)]
    #[serde(default)]
    pub owner: bool,
}

async fn grant(allow: bool, args: &GrantArgs) -> Result<Bucket> {
    if !(args.read || args.write || args.owner) {
        bail!("pass at least one of read, write, owner");
    }
    let g = connect(&args.name).await?;
    let id = g.bucket_id(&args.bucket).await?;
    let perms = Perms {
        read: args.read,
        write: args.write,
        owner: args.owner,
    };
    g.grant(allow, &id, &args.key_id, perms).await
}

/// Grant a key read, write or owner on a bucket (`AllowBucketKey`).
#[orca_tool(
    domain = "garage",
    verb = "bucket.allow",
    role = "admin",
    execute_gated = true
)]
async fn garage_bucket_allow(args: GrantArgs, _ctx: &ToolCtx) -> Result<Bucket> {
    grant(true, &args).await
}

/// Revoke a key's read, write or owner on a bucket (`DenyBucketKey`).
#[orca_tool(
    domain = "garage",
    verb = "bucket.deny",
    role = "admin",
    execute_gated = true
)]
async fn garage_bucket_deny(args: GrantArgs, _ctx: &ToolCtx) -> Result<Bucket> {
    grant(false, &args).await
}

#[orca_struct(args)]
pub struct CleanupArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Bucket id (64 hex chars) or global alias.
    #[arg(long)]
    pub bucket: String,
    /// Abort incomplete uploads older than this many seconds.
    #[arg(long, default_value_t = 86400)]
    #[serde(default = "one_day")]
    pub older_than_secs: u64,
}

fn one_day() -> u64 {
    86400
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
pub struct CleanupReport {
    pub bucket_id: String,
    pub uploads_deleted: u64,
}

/// Abort a bucket's incomplete multipart uploads older than a threshold
/// (`CleanupIncompleteUploads`).
#[orca_tool(
    domain = "garage",
    verb = "bucket.cleanup_uploads",
    role = "admin",
    execute_gated = true
)]
async fn garage_bucket_cleanup_uploads(args: CleanupArgs, _ctx: &ToolCtx) -> Result<CleanupReport> {
    let g = connect(&args.name).await?;
    let bucket_id = g.bucket_id(&args.bucket).await?;
    let uploads_deleted = g.cleanup_uploads(&bucket_id, args.older_than_secs).await?;
    Ok(CleanupReport {
        bucket_id,
        uploads_deleted,
    })
}

/// Access keys (`ListKeys`).
#[orca_tool(domain = "garage", verb = "key.list", role = "read")]
async fn garage_key_list(args: EndpointArgs, _ctx: &ToolCtx) -> Result<Vec<KeySummary>> {
    connect(&args.name).await?.keys().await
}

#[orca_struct(args)]
pub struct KeyArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Access key id.
    #[arg(long)]
    pub key_id: String,
}

/// An access key's permissions and bucket grants, never its secret
/// (`GetKeyInfo`).
#[orca_tool(domain = "garage", verb = "key.detail", role = "read")]
async fn garage_key_detail(args: KeyArgs, _ctx: &ToolCtx) -> Result<Key> {
    connect(&args.name).await?.key(&args.key_id).await
}

#[orca_struct(args)]
pub struct KeyCreateArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Key name.
    #[arg(long)]
    pub key_name: String,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
pub struct StoredKey {
    pub key: Key,
    pub secret_ref: String,
}

pub async fn create_key(g: &Garage, endpoint: &str, key_name: &str) -> Result<StoredKey> {
    let (key, secret) = g.create_key(key_name).await?;
    let secret_ref = key_secret_name(endpoint, &key.access_key_id)?;
    secrets::set(
        &secret_ref,
        secret.expose_secret(),
        Some(&format!("garage secret access key {}", key.access_key_id)),
    )
    .with_context(|| {
        format!(
            "key {} was created but its secret could not be stored; delete it with garage.key.delete",
            key.access_key_id
        )
    })?;
    Ok(StoredKey { key, secret_ref })
}

/// Create an access key; its secret goes to the orca secret store and only the
/// secret's name is returned (`CreateKey`).
#[orca_tool(
    domain = "garage",
    verb = "key.create",
    role = "admin",
    execute_gated = true
)]
async fn garage_key_create(args: KeyCreateArgs, _ctx: &ToolCtx) -> Result<StoredKey> {
    create_key(&connect(&args.name).await?, &args.name, &args.key_name).await
}

#[orca_struct(args)]
pub struct KeyImportArgs {
    /// Registered garage endpoint name.
    #[arg(long)]
    pub name: String,
    /// Access key id to import.
    #[arg(long)]
    pub key_id: String,
    /// Orca secret holding the secret access key.
    #[arg(long)]
    pub secret_ref: String,
    /// Key name.
    #[arg(long)]
    pub key_name: Option<String>,
}

pub async fn import_key(g: &Garage, args: &KeyImportArgs) -> Result<StoredKey> {
    let secret = secrets::get_required(&args.secret_ref)?;
    let key = g
        .import_key(
            &args.key_id,
            secret.expose_secret(),
            args.key_name.as_deref(),
        )
        .await?;
    Ok(StoredKey {
        key,
        secret_ref: args.secret_ref.clone(),
    })
}

/// Import an existing access key whose secret is read from the orca secret
/// store (`ImportKey`).
#[orca_tool(
    domain = "garage",
    verb = "key.import",
    role = "admin",
    execute_gated = true
)]
async fn garage_key_import(args: KeyImportArgs, _ctx: &ToolCtx) -> Result<StoredKey> {
    import_key(&connect(&args.name).await?, &args).await
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
pub struct KeyDeleted {
    pub id: String,
    pub deleted: bool,
    pub secret_removed: bool,
}

pub async fn delete_key(g: &Garage, endpoint: &str, key_id: &str) -> Result<KeyDeleted> {
    g.delete_key(key_id).await?;
    let secret_removed = secrets::delete(&key_secret_name(endpoint, key_id)?)?;
    Ok(KeyDeleted {
        id: key_id.to_string(),
        deleted: true,
        secret_removed,
    })
}

/// Delete an access key and the secret `garage.key.create` stored for it
/// (`DeleteKey`).
#[orca_tool(
    domain = "garage",
    verb = "key.delete",
    role = "admin",
    execute_gated = true
)]
async fn garage_key_delete(args: KeyArgs, _ctx: &ToolCtx) -> Result<KeyDeleted> {
    delete_key(&connect(&args.name).await?, &args.name, &args.key_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const READS: &[&str] = &[
        "garage.cluster.health",
        "garage.cluster.status",
        "garage.layout.detail",
        "garage.bucket.list",
        "garage.bucket.detail",
        "garage.key.list",
        "garage.key.detail",
    ];

    pub const WRITES: &[&str] = &[
        "garage.layout.apply",
        "garage.bucket.create",
        "garage.bucket.delete",
        "garage.bucket.update",
        "garage.bucket.allow",
        "garage.bucket.deny",
        "garage.bucket.cleanup_uploads",
        "garage.key.create",
        "garage.key.import",
        "garage.key.delete",
    ];

    #[test]
    fn empty_routes_give_a_clear_error() {
        assert!(require_routes::<String>("g", &[]).is_err());
        assert!(require_routes("g", &["http://g.test:3903".to_string()]).is_ok());
    }

    #[test]
    fn key_secrets_live_in_the_plugin_namespace() {
        assert_eq!(
            key_secret_name("main", "GK31c2").unwrap(),
            "garage.main.key_GK31c2"
        );
    }

    #[test]
    fn tools_declare_roles_and_gating() {
        let role = |n| plugin_toolkit::dispatch::required_role(n);
        for n in READS {
            assert_eq!(role(n), Some("read"), "{n}");
        }
        let gated = plugin_toolkit::dispatch::execute_gated_names();
        for n in WRITES {
            assert_eq!(role(n), Some("admin"), "{n}");
            assert!(gated.contains(n), "{n}");
        }
    }
}
