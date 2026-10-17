#![allow(clippy::disallowed_types)]

use plugin_toolkit::api_client::ApiClientBuilder;
use plugin_toolkit::prelude::*;
use plugin_toolkit::progenitor_client::QueryParam;
use plugin_toolkit::reqwest::{Client as HttpClient, RequestBuilder};
use plugin_toolkit::secrets::Secret;
use plugin_toolkit::serde::de::DeserializeOwned;
use plugin_toolkit::serde_json::{self, Value};
use std::time::Duration;

pub struct Garage {
    base_url: String,
    token: String,
    http: HttpClient,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClusterHealth {
    pub status: String,
    pub known_nodes: u64,
    pub connected_nodes: u64,
    pub storage_nodes: u64,
    pub storage_nodes_up: u64,
    pub partitions: u64,
    pub partitions_quorum: u64,
    pub partitions_all_ok: u64,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FreeSpace {
    pub available: u64,
    pub total: u64,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeRole {
    pub zone: String,
    #[serde(default)]
    pub capacity: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub is_up: bool,
    pub draining: bool,
    #[serde(default)]
    pub addr: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub garage_version: Option<String>,
    #[serde(default)]
    pub last_seen_secs_ago: Option<u64>,
    #[serde(default)]
    pub role: Option<NodeRole>,
    #[serde(default)]
    pub data_partition: Option<FreeSpace>,
    #[serde(default)]
    pub metadata_partition: Option<FreeSpace>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClusterStatus {
    pub layout_version: u64,
    pub nodes: Vec<Node>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRole {
    pub id: String,
    pub zone: String,
    #[serde(default)]
    pub capacity: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub stored_partitions: Option<u64>,
    #[serde(default)]
    pub usable_capacity: Option<u64>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub version: u64,
    pub partition_size: u64,
    pub roles: Vec<LayoutRole>,
    #[serde(default)]
    pub staged_role_changes: Vec<LayoutRoleChange>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRoleChange {
    pub id: String,
    #[serde(default)]
    pub remove: Option<bool>,
    #[serde(default)]
    pub zone: Option<String>,
    #[serde(default)]
    pub capacity: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutApplied {
    pub message: Vec<String>,
    pub layout: Layout,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalAlias {
    pub access_key_id: String,
    pub alias: String,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BucketSummary {
    pub id: String,
    pub created: String,
    pub global_aliases: Vec<String>,
    #[serde(default)]
    pub local_aliases: Vec<LocalAlias>,
}

#[orca_struct]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Perms {
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub write: bool,
    #[serde(default)]
    pub owner: bool,
}

#[orca_struct]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Quotas {
    #[serde(default)]
    pub max_size: Option<u64>,
    #[serde(default)]
    pub max_objects: Option<u64>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BucketKey {
    pub access_key_id: String,
    pub name: String,
    pub permissions: Perms,
    #[serde(default)]
    pub bucket_local_aliases: Vec<String>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub id: String,
    pub created: String,
    pub global_aliases: Vec<String>,
    pub website_access: bool,
    pub keys: Vec<BucketKey>,
    pub objects: u64,
    pub bytes: u64,
    pub unfinished_uploads: u64,
    pub unfinished_multipart_uploads: u64,
    pub unfinished_multipart_upload_parts: u64,
    pub unfinished_multipart_upload_bytes: u64,
    pub quotas: Quotas,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeySummary {
    pub id: String,
    pub name: String,
    pub expired: bool,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub expiration: Option<String>,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyBucket {
    pub id: String,
    pub global_aliases: Vec<String>,
    pub local_aliases: Vec<String>,
    pub permissions: Perms,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyPerm {
    #[serde(default)]
    pub create_bucket: bool,
}

#[orca_struct]
#[derive(Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub access_key_id: String,
    pub name: String,
    pub expired: bool,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub expiration: Option<String>,
    pub permissions: KeyPerm,
    pub buckets: Vec<KeyBucket>,
}

pub enum BucketRef<'a> {
    Id(&'a str),
    Alias(&'a str),
}

impl<'a> BucketRef<'a> {
    pub fn parse(bucket: &'a str) -> Self {
        if bucket.len() == 64 && bucket.bytes().all(|b| b.is_ascii_hexdigit()) {
            BucketRef::Id(bucket)
        } else {
            BucketRef::Alias(bucket)
        }
    }
}

impl Garage {
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let http = ApiClientBuilder::new()
            .bearer(token)
            .map_err(|_| anyhow!("admin token is not a valid header value"))?
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http,
        })
    }

    fn request(&self, method: &str, op: &str, query: &[(&str, &str)]) -> RequestBuilder {
        let url = format!("{}/v2/{op}", self.base_url);
        let req = match method {
            "GET" => self.http.get(url),
            _ => self.http.post(url),
        };
        query
            .iter()
            .fold(req, |r, (k, v)| r.query(&QueryParam::new(k, v)))
    }

    async fn exchange(&self, req: RequestBuilder, op: &str) -> Result<(u16, String)> {
        let resp = req
            .send()
            .await
            .map_err(|e| anyhow!("{op}: {}", self.excerpt(&e.to_string())))?;
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| anyhow!("{op}: {}", self.excerpt(&e.to_string())))?;
        Ok((status, body))
    }

    async fn send(&self, req: RequestBuilder, op: &str) -> Result<String> {
        let (status, body) = self.exchange(req, op).await?;
        if !(200..300).contains(&status) {
            bail!("{op}: HTTP {status} {}", self.excerpt(&body));
        }
        Ok(body)
    }

    fn decode<T: DeserializeOwned>(&self, op: &str, body: &str) -> Result<T> {
        serde_json::from_str(body).map_err(|e| anyhow!("{op}: unexpected response: {e}"))
    }

    async fn get<T: DeserializeOwned>(&self, op: &str, query: &[(&str, &str)]) -> Result<T> {
        let body = self.send(self.request("GET", op, query), op).await?;
        self.decode(op, &body)
    }

    async fn post_raw(
        &self,
        op: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<String> {
        let req = self.request("POST", op, query);
        let req = match body {
            Some(b) => req.json(b),
            None => req,
        };
        self.send(req, op).await
    }

    async fn post<T: DeserializeOwned>(
        &self,
        op: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<T> {
        let body = self.post_raw(op, query, Some(body)).await?;
        self.decode(op, &body)
    }

    pub async fn health(&self) -> Result<ClusterHealth> {
        self.get("GetClusterHealth", &[]).await
    }

    pub async fn status(&self) -> Result<ClusterStatus> {
        self.get("GetClusterStatus", &[]).await
    }

    pub async fn layout(&self) -> Result<Layout> {
        self.get("GetClusterLayout", &[]).await
    }

    pub async fn assign_node(
        &self,
        node_id: &str,
        zone: &str,
        capacity: u64,
        tags: &[String],
        apply_all_staged: bool,
    ) -> Result<LayoutApplied> {
        check_id(node_id)?;
        let staged: Layout = self
            .post(
                "UpdateClusterLayout",
                &[],
                &json!({"roles": [{"id": node_id, "zone": zone, "capacity": capacity, "tags": tags}]}),
            )
            .await?;
        let foreign: Vec<&str> = staged
            .staged_role_changes
            .iter()
            .map(|c| c.id.as_str())
            .filter(|id| *id != node_id)
            .collect();
        if !apply_all_staged && !foreign.is_empty() {
            let all: Vec<&str> = staged
                .staged_role_changes
                .iter()
                .map(|c| c.id.as_str())
                .collect();
            bail!(
                "layout has staged changes for other nodes; refusing to apply. staged: {}. \
                 pass --apply-all-staged to apply them all",
                all.join(", ")
            );
        }
        self.post(
            "ApplyClusterLayout",
            &[],
            &json!({"version": staged.version + 1}),
        )
        .await
    }

    pub async fn buckets(&self) -> Result<Vec<BucketSummary>> {
        self.get("ListBuckets", &[]).await
    }

    pub async fn bucket(&self, bucket: BucketRef<'_>) -> Result<Bucket> {
        match bucket {
            BucketRef::Id(id) => self.get("GetBucketInfo", &[("id", id)]).await,
            BucketRef::Alias(alias) => self.get("GetBucketInfo", &[("globalAlias", alias)]).await,
        }
    }

    pub async fn bucket_id(&self, bucket: &str) -> Result<String> {
        match BucketRef::parse(bucket) {
            BucketRef::Id(id) => Ok(id.to_string()),
            alias => Ok(self.bucket(alias).await?.id),
        }
    }

    pub async fn create_bucket(&self, alias: &str) -> Result<Bucket> {
        self.post("CreateBucket", &[], &json!({"globalAlias": alias}))
            .await
    }

    pub async fn delete_bucket(&self, id: &str) -> Result<()> {
        self.post_raw("DeleteBucket", &[("id", id)], None).await?;
        Ok(())
    }

    pub async fn set_quotas(&self, id: &str, quotas: Quotas) -> Result<Bucket> {
        self.post("UpdateBucket", &[("id", id)], &json!({"quotas": quotas}))
            .await
    }

    pub async fn keys(&self) -> Result<Vec<KeySummary>> {
        self.get("ListKeys", &[]).await
    }

    pub async fn key(&self, id: &str) -> Result<Key> {
        check_id(id)?;
        self.get("GetKeyInfo", &[("id", id)]).await
    }

    pub async fn create_key(&self, name: &str) -> Result<(Key, Secret<String>)> {
        let body = self
            .post_raw("CreateKey", &[], Some(&json!({"name": name})))
            .await?;
        let mut raw: Value = self.decode("CreateKey", &body)?;
        let secret = raw
            .as_object_mut()
            .and_then(|o| o.remove("secretAccessKey"))
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| anyhow!("CreateKey: response carried no secret access key"))?;
        let key: Key = serde_json::from_value(raw)
            .map_err(|e| anyhow!("CreateKey: unexpected response: {e}"))?;
        check_id(&key.access_key_id)?;
        Ok((key, Secret::new(secret)))
    }

    pub async fn import_key(&self, id: &str, secret: &str, name: Option<&str>) -> Result<Key> {
        check_id(id)?;
        let body = self
            .post_raw(
                "ImportKey",
                &[],
                Some(&json!({"accessKeyId": id, "secretAccessKey": secret, "name": name})),
            )
            .await
            .map_err(|e| anyhow!(redact(&format!("{e:#}"), secret)))?;
        let mut raw: Value = self.decode("ImportKey", &body)?;
        if let Some(o) = raw.as_object_mut() {
            o.remove("secretAccessKey");
        }
        serde_json::from_value(raw).map_err(|e| anyhow!("ImportKey: unexpected response: {e}"))
    }

    pub async fn delete_key(&self, id: &str) -> Result<bool> {
        check_id(id)?;
        let req = self.request("POST", "DeleteKey", &[("id", id)]);
        match self.exchange(req, "DeleteKey").await? {
            (200..=299, _) => Ok(true),
            (404, _) => Ok(false),
            (status, body) => bail!("DeleteKey: HTTP {status} {}", self.excerpt(&body)),
        }
    }

    pub async fn grant(
        &self,
        allow: bool,
        bucket_id: &str,
        key_id: &str,
        perms: Perms,
    ) -> Result<Bucket> {
        check_id(key_id)?;
        let op = if allow {
            "AllowBucketKey"
        } else {
            "DenyBucketKey"
        };
        self.post(
            op,
            &[],
            &json!({"bucketId": bucket_id, "accessKeyId": key_id, "permissions": perms}),
        )
        .await
    }

    pub async fn cleanup_uploads(&self, bucket_id: &str, older_than_secs: u64) -> Result<u64> {
        let v: Value = self
            .post(
                "CleanupIncompleteUploads",
                &[],
                &json!({"bucketId": bucket_id, "olderThanSecs": older_than_secs}),
            )
            .await?;
        v["uploadsDeleted"]
            .as_u64()
            .ok_or_else(|| anyhow!("CleanupIncompleteUploads: response carried no uploadsDeleted"))
    }

    fn excerpt(&self, text: &str) -> String {
        let text = redact(text.trim(), &self.token);
        let text = plugin_toolkit::logging::scrub(&text);
        text.char_indices()
            .nth(ERROR_EXCERPT)
            .map_or(&*text, |(i, _)| &text[..i])
            .to_string()
    }
}

const ERROR_EXCERPT: usize = 256;
const REQUEST_TIMEOUT_SECS: u64 = 30;

pub fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_string()
    } else {
        text.replace(secret, "***")
    }
}

pub fn check_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        bail!("invalid garage id {id:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_toolkit::serde_json::json;

    #[test]
    fn bucket_refs_split_ids_from_aliases() {
        let id = "a".repeat(64);
        assert!(matches!(BucketRef::parse(&id), BucketRef::Id(_)));
        assert!(matches!(BucketRef::parse("sccache"), BucketRef::Alias(_)));
        assert!(matches!(
            BucketRef::parse(&"z".repeat(64)),
            BucketRef::Alias(_)
        ));
    }

    #[test]
    fn ids_reject_non_alphanumerics() {
        assert!(check_id("GK31c2f218a2e44f485b94239e").is_ok());
        assert!(check_id("GK1&id=x").is_err());
        assert!(check_id("a.b").is_err());
        assert!(check_id("").is_err());
    }

    #[test]
    fn excerpt_scrubs_before_truncating() {
        let g = Garage::new("http://g.test", "t").unwrap();
        let e = g.excerpt(&format!(
            r#"{}"password":"hunter2hunter2"}}"#,
            "x".repeat(ERROR_EXCERPT - 16)
        ));
        assert!(!e.contains("hunt"), "{e}");
    }

    #[test]
    fn key_and_summary_decoding_drop_a_stray_secret() {
        let k: Key = serde_json::from_value(json!({
            "accessKeyId": "GK1", "name": "ci", "expired": false, "secretAccessKey": "s3cr3t",
            "permissions": {}, "buckets": []
        }))
        .unwrap();
        assert!(!serde_json::to_string(&k).unwrap().contains("s3cr3t"));
        let s: KeySummary = serde_json::from_value(
            json!({"id": "GK1", "name": "ci", "expired": false, "secretAccessKey": "s3cr3t"}),
        )
        .unwrap();
        assert!(!serde_json::to_string(&s).unwrap().contains("s3cr3t"));
    }

    #[test]
    fn excerpt_redacts_the_token_and_truncates() {
        let g = Garage::new("http://g.test", "tok-0123456789abcdef").unwrap();
        let e = g.excerpt(&format!("Bearer tok-0123456789abcdef {}", "x".repeat(1000)));
        assert!(!e.contains("tok-0123456789abcdef"), "{e}");
        assert!(e.len() < 300, "{}", e.len());
    }
}
