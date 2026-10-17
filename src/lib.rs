#![allow(clippy::disallowed_types)]

use plugin_toolkit::service::{
    BoxFuture, Routes, Runtime, ServiceBackend, ServiceCapability, ServiceError, ServiceStatus,
    WorkloadSpec,
};

pub mod api;
pub mod tools;

use plugin_toolkit::clap;

#[plugin_toolkit::endpoint_resource(plugin = "garage")]
pub struct GarageEndpoint {
    pub name: String,
    #[secret]
    pub admin_token: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct GarageBackend {
    provider: &'static str,
}

impl GarageBackend {
    pub fn new(provider: &'static str) -> Self {
        Self { provider }
    }
}

impl ServiceBackend for GarageBackend {
    fn provider(&self) -> &str {
        self.provider
    }

    fn runtimes(&self) -> Vec<Runtime> {
        vec![Runtime::Docker, Runtime::Podman]
    }

    fn capabilities(&self) -> Vec<ServiceCapability> {
        vec![ServiceCapability::Status]
    }

    fn default_port(&self) -> u16 {
        3903
    }

    fn data_paths(&self) -> Vec<String> {
        vec!["/var/lib/garage/meta".to_string()]
    }

    fn workload_spec<'a>(
        &'a self,
        _runtime: Runtime,
        _instance: &'a str,
        _routes: &'a Routes,
    ) -> BoxFuture<'a, Result<WorkloadSpec, ServiceError>> {
        Box::pin(async move {
            Err(ServiceError::Other(
                "garage needs a config file, secret files, cap_drop, no-new-privileges and a \
                 non-root user, which WorkloadSpec cannot express; deploy compose.yml through \
                 the docker plugin"
                    .into(),
            ))
        })
    }

    fn status<'a>(
        &'a self,
        instance: &'a str,
        _routes: &'a Routes,
    ) -> BoxFuture<'a, Result<ServiceStatus, ServiceError>> {
        Box::pin(async move {
            let g = tools::connect(instance)
                .await
                .map_err(|e| ServiceError::Other(format!("{e:#}")))?;
            let h = g
                .health()
                .await
                .map_err(|e| ServiceError::Transport(format!("{e:#}")))?;
            Ok(health_status(&h))
        })
    }
}

pub fn health_status(h: &api::ClusterHealth) -> ServiceStatus {
    ServiceStatus {
        healthy: h.status == "healthy",
        detail: format!(
            "{}: {}/{} storage nodes up, {}/{} partitions with quorum",
            h.status, h.storage_nodes_up, h.storage_nodes, h.partitions_quorum, h.partitions
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_toolkit::serde_json::{self, json, Value};
    use std::sync::{Arc, Mutex};

    const TOKEN: &str = "admintok-0123456789abcdef";
    const SECRET: &str = "b892c0665f0ada8a4755dae98baa3b133590e11dae3bcc1f9d769d67f16c3835";
    const BUCKET: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

    #[derive(Debug, Clone)]
    struct Call {
        method: String,
        path: String,
        body: Value,
    }

    fn serve<F>(route: impl Fn(&str, &str) -> Option<(u16, String)> + 'static, body: F) -> Vec<Call>
    where
        F: FnOnce(),
    {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, req: &str| {
                assert_eq!(cap, "http.request");
                let req: Value = serde_json::from_str(req).unwrap();
                assert!(
                    req["headers"]
                        .to_string()
                        .contains(&format!("Bearer {TOKEN}")),
                    "{req}"
                );
                let url = req["url"].as_str().unwrap();
                let path = url
                    .strip_prefix("http://g.test/v2/")
                    .unwrap_or_else(|| panic!("unexpected url {url}"))
                    .to_string();
                let method = req["method"].as_str().unwrap().to_string();
                let body = match req["body"].as_array() {
                    Some(b) if !b.is_empty() => {
                        let bytes: Vec<u8> = b.iter().map(|x| x.as_u64().unwrap() as u8).collect();
                        serde_json::from_slice(&bytes).unwrap()
                    }
                    _ => Value::Null,
                };
                log.lock().unwrap().push(Call {
                    method: method.clone(),
                    path: path.clone(),
                    body,
                });
                let (status, body) = route(&method, &path).unwrap_or((404, "Not Found".into()));
                Ok(json!({"status": status, "headers": [], "body": body.into_bytes()}).to_string())
            }),
            body,
        );
        let out = seen.lock().unwrap().clone();
        out
    }

    fn block<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(f)
    }

    fn g() -> api::Garage {
        api::Garage::new("http://g.test/", TOKEN).unwrap()
    }

    fn layout(version: u64) -> String {
        json!({
            "version": version, "partitionSize": 1000, "parameters": {"zoneRedundancy": "maximum"},
            "roles": [{"id": "n1", "zone": "dc1", "capacity": 1000, "tags": []}],
            "stagedRoleChanges": [{"id": "n1", "zone": "dc1", "capacity": 1000, "tags": []}]
        })
        .to_string()
    }

    fn bucket_info() -> String {
        json!({
            "id": BUCKET, "created": "2026-10-17T00:00:00Z", "globalAliases": ["sccache"],
            "websiteAccess": false, "objects": 3, "bytes": 42, "unfinishedUploads": 0,
            "unfinishedMultipartUploads": 1, "unfinishedMultipartUploadParts": 2,
            "unfinishedMultipartUploadBytes": 9, "quotas": {"maxSize": null, "maxObjects": null},
            "keys": [{"accessKeyId": "GK1", "name": "ci", "permissions": {"read": true, "write": true, "owner": false}, "bucketLocalAliases": []}]
        })
        .to_string()
    }

    fn key_info(with_secret: bool) -> String {
        let mut v = json!({
            "accessKeyId": "GK1", "name": "ci", "expired": false, "created": "2026-10-17T00:00:00Z",
            "permissions": {"createBucket": false}, "buckets": []
        });
        if with_secret {
            v["secretAccessKey"] = json!(SECRET);
        }
        v.to_string()
    }

    #[test]
    fn health_reads_get_cluster_health_with_bearer_token() {
        let mut got = None;
        let calls = serve(
            |_, p| {
                (p == "GetClusterHealth").then(|| {
                    (200, json!({"status": "healthy", "knownNodes": 1, "connectedNodes": 1, "storageNodes": 1,
                        "storageNodesUp": 1, "partitions": 256, "partitionsQuorum": 256, "partitionsAllOk": 256}).to_string())
                })
            },
            || got = Some(block(g().health()).unwrap()),
        );
        assert_eq!(calls[0].method, "GET");
        assert_eq!(got.unwrap().partitions_quorum, 256);
    }

    #[test]
    fn layout_apply_stages_the_role_then_applies_the_next_version() {
        let calls = serve(
            |_, p| {
                match p {
                "UpdateClusterLayout" => Some((200, layout(4))),
                "ApplyClusterLayout" => Some((200, json!({"message": ["ok"], "layout": serde_json::from_str::<Value>(&layout(5)).unwrap()}).to_string())),
                _ => None,
            }
            },
            || {
                let r = block(g().assign_node("n1", "dc1", 1000, &[], false)).unwrap();
                assert_eq!(r.layout.version, 5);
            },
        );
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].path, "UpdateClusterLayout");
        assert_eq!(
            calls[0].body,
            json!({"roles": [{"id": "n1", "zone": "dc1", "capacity": 1000, "tags": []}]})
        );
        assert_eq!(calls[1].body, json!({"version": 5}));
    }

    fn foreign_layout() -> String {
        let mut v: Value = serde_json::from_str(&layout(4)).unwrap();
        v["stagedRoleChanges"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "n2", "remove": true}));
        v.to_string()
    }

    fn applied(path: &str) -> Option<(u16, String)> {
        match path {
            "UpdateClusterLayout" => Some((200, foreign_layout())),
            "ApplyClusterLayout" => Some((200, json!({"message": [], "layout": serde_json::from_str::<Value>(&layout(5)).unwrap()}).to_string())),
            _ => None,
        }
    }

    #[test]
    fn layout_apply_refuses_foreign_staged_changes() {
        let calls = serve(
            |_, p| applied(p),
            || {
                let e = block(g().assign_node("n1", "dc1", 1000, &[], false)).unwrap_err();
                let e = format!("{e:#}");
                assert!(
                    e.contains("n1, n2") && e.contains("--apply-all-staged"),
                    "{e}"
                );
            },
        );
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn layout_apply_all_staged_applies_foreign_changes() {
        let calls = serve(
            |_, p| applied(p),
            || {
                block(g().assign_node("n1", "dc1", 1000, &[], true)).unwrap();
            },
        );
        assert_eq!(calls[1].path, "ApplyClusterLayout");
    }

    #[test]
    fn invalid_key_and_node_ids_make_no_request() {
        let calls = serve(
            |_, _| Some((200, String::new())),
            || {
                assert!(block(g().assign_node("n1&x", "dc1", 1, &[], false)).is_err());
                assert!(block(g().key("GK1/../x")).is_err());
                assert!(block(g().delete_key("GK1?id=GK2")).is_err());
                let perms = api::Perms::default();
                assert!(block(g().grant(true, BUCKET, "GK 1", perms)).is_err());
            },
        );
        assert!(calls.is_empty(), "{calls:?}");
    }

    #[test]
    fn alias_queries_are_encoded() {
        let calls = serve(
            |_, _| Some((200, bucket_info())),
            || {
                block(g().bucket_id("a b&id=x")).unwrap();
            },
        );
        assert_eq!(calls[0].path, "GetBucketInfo?globalAlias=a%20b%26id%3Dx");
    }

    #[test]
    fn health_maps_to_service_status() {
        let h = |status: &str| api::ClusterHealth {
            status: status.into(),
            known_nodes: 1,
            connected_nodes: 1,
            storage_nodes: 2,
            storage_nodes_up: 1,
            partitions: 256,
            partitions_quorum: 128,
            partitions_all_ok: 0,
        };
        let ok = health_status(&h("healthy"));
        assert!(ok.healthy);
        assert_eq!(
            ok.detail,
            "healthy: 1/2 storage nodes up, 128/256 partitions with quorum"
        );
        assert!(!health_status(&h("degraded")).healthy);
        assert!(!health_status(&h("unavailable")).healthy);
    }

    #[test]
    fn bucket_aliases_resolve_before_writes() {
        let calls = serve(
            |_, p| {
                if p.starts_with("GetBucketInfo") || p.starts_with("UpdateBucket") {
                    Some((200, bucket_info()))
                } else {
                    None
                }
            },
            || {
                let id = block(g().bucket_id("sccache")).unwrap();
                let quotas = api::Quotas {
                    max_size: Some(10),
                    max_objects: None,
                };
                block(g().set_quotas(&id, quotas)).unwrap();
            },
        );
        assert_eq!(calls[0].path, "GetBucketInfo?globalAlias=sccache");
        assert_eq!(calls[1].path, format!("UpdateBucket?id={BUCKET}"));
        assert_eq!(
            calls[1].body,
            json!({"quotas": {"maxSize": 10, "maxObjects": null}})
        );
    }

    #[test]
    fn bucket_ids_skip_the_lookup() {
        let calls = serve(
            |_, _| Some((200, String::new())),
            || {
                let id = block(g().bucket_id(BUCKET)).unwrap();
                block(g().delete_bucket(&id)).unwrap();
            },
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].method, "POST");
        assert_eq!(calls[0].path, format!("DeleteBucket?id={BUCKET}"));
    }

    #[test]
    fn grants_send_bucket_key_and_permissions() {
        let calls = serve(
            |_, _| Some((200, bucket_info())),
            || {
                let perms = api::Perms {
                    read: true,
                    write: true,
                    owner: false,
                };
                block(g().grant(true, BUCKET, "GK1", perms)).unwrap();
                block(g().grant(false, BUCKET, "GK1", perms)).unwrap();
            },
        );
        assert_eq!(calls[0].path, "AllowBucketKey");
        assert_eq!(calls[1].path, "DenyBucketKey");
        assert_eq!(
            calls[0].body,
            json!({"bucketId": BUCKET, "accessKeyId": "GK1", "permissions": {"read": true, "write": true, "owner": false}})
        );
    }

    #[test]
    fn cleanup_uploads_reports_the_count() {
        let calls = serve(
            |_, _| Some((200, json!({"uploadsDeleted": 7}).to_string())),
            || assert_eq!(block(g().cleanup_uploads(BUCKET, 60)).unwrap(), 7),
        );
        assert_eq!(
            calls[0].body,
            json!({"bucketId": BUCKET, "olderThanSecs": 60})
        );
    }

    #[test]
    fn key_create_stores_the_secret_and_never_returns_it() {
        let caps = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let log = caps.clone();
        let out = plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, req: &str| {
                log.lock().unwrap().push((cap.to_string(), req.to_string()));
                match cap {
                    "http.request" => Ok(
                        json!({"status": 200, "headers": [], "body": key_info(true).into_bytes()})
                            .to_string(),
                    ),
                    "secret.op" => Ok(json!({"value": null, "found": false}).to_string()),
                    _ => Err(format!("unexpected {cap}")),
                }
            }),
            || block(tools::create_key(&g(), "main", "ci")).unwrap(),
        );
        assert_eq!(out.secret_ref, "garage.main.key_GK1");
        assert_eq!(out.key.access_key_id, "GK1");
        let rendered = serde_json::to_string(&out).unwrap();
        assert!(!rendered.contains(SECRET), "{rendered}");
        let caps = caps.lock().unwrap();
        let (cap, req) = &caps[1];
        assert_eq!(cap, "secret.op");
        assert!(
            req.contains("garage.main.key_GK1") && req.contains(SECRET),
            "{req}"
        );
    }

    #[test]
    fn key_create_store_failure_deletes_the_key() {
        let caps = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = caps.clone();
        let e = plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, req: &str| {
                log.lock().unwrap().push(format!("{cap} {req}"));
                match cap {
                    "http.request" if req.contains("CreateKey") => Ok(
                        json!({"status": 200, "headers": [], "body": key_info(true).into_bytes()})
                            .to_string(),
                    ),
                    "http.request" => {
                        Ok(json!({"status": 200, "headers": [], "body": []}).to_string())
                    }
                    _ => Err("store offline".into()),
                }
            }),
            || block(tools::create_key(&g(), "main", "ci")).unwrap_err(),
        );
        let e = format!("{e:#}");
        assert!(
            e.contains("store offline") && e.contains("deleted from garage"),
            "{e}"
        );
        assert!(!e.contains(SECRET), "{e}");
        let caps = caps.lock().unwrap();
        assert!(caps[2].contains("DeleteKey?id=GK1"), "{caps:?}");
    }

    #[test]
    fn key_import_errors_redact_the_secret() {
        let e = plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, _: &str| match cap {
                "secret.op" => Ok(json!({"value": SECRET, "found": true}).to_string()),
                "http.request" => Ok(json!({"status": 400, "headers": [],
                    "body": format!("bad key {SECRET}").into_bytes()})
                .to_string()),
                _ => Err(format!("unexpected {cap}")),
            }),
            || {
                let args = tools::KeyImportArgs {
                    name: "main".into(),
                    key_id: "GK1".into(),
                    secret_ref: "garage.main.imported".into(),
                    key_name: None,
                };
                block(tools::import_key(&g(), &args)).unwrap_err()
            },
        );
        let e = format!("{e:#}");
        assert!(e.contains("HTTP 400") && !e.contains(SECRET), "{e}");
    }

    #[test]
    fn key_detail_never_asks_for_the_secret() {
        let calls = serve(
            |_, _| Some((200, key_info(false))),
            || {
                let k = block(g().key("GK1")).unwrap();
                assert_eq!(k.name, "ci");
            },
        );
        assert_eq!(calls[0].path, "GetKeyInfo?id=GK1");
    }

    #[test]
    fn key_import_reads_the_secret_from_the_store() {
        let mut out = None;
        let caps = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let log = caps.clone();
        plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, req: &str| {
                log.lock().unwrap().push((cap.to_string(), req.to_string()));
                match cap {
                    "secret.op" => Ok(json!({"value": SECRET, "found": true}).to_string()),
                    "http.request" => Ok(
                        json!({"status": 200, "headers": [], "body": key_info(true).into_bytes()})
                            .to_string(),
                    ),
                    _ => Err(format!("unexpected {cap}")),
                }
            }),
            || {
                let args = tools::KeyImportArgs {
                    name: "main".into(),
                    key_id: "GK1".into(),
                    secret_ref: "garage.main.imported".into(),
                    key_name: Some("ci".into()),
                };
                out = Some(block(tools::import_key(&g(), &args)).unwrap());
            },
        );
        let out = out.unwrap();
        assert_eq!(out.secret_ref, "garage.main.imported");
        assert!(!serde_json::to_string(&out).unwrap().contains(SECRET));
        assert_eq!(caps.lock().unwrap()[0].0, "secret.op");
    }

    #[test]
    fn key_delete_removes_the_stored_secret() {
        let caps = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = caps.clone();
        let out = plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, req: &str| {
                log.lock().unwrap().push(format!("{cap} {req}"));
                match cap {
                    "http.request" => {
                        Ok(json!({"status": 200, "headers": [], "body": []}).to_string())
                    }
                    "secret.op" => Ok(json!({"value": null, "found": true}).to_string()),
                    _ => Err(format!("unexpected {cap}")),
                }
            }),
            || block(tools::delete_key(&g(), "main", "GK1")).unwrap(),
        );
        assert!(out.deleted && out.secret_removed);
        let caps = caps.lock().unwrap();
        assert!(caps[0].contains("DeleteKey?id=GK1"), "{caps:?}");
        assert!(caps[1].contains("garage.main.key_GK1"), "{caps:?}");
    }

    fn delete_with(
        status: u16,
        secret: Result<String, String>,
    ) -> (
        plugin_toolkit::anyhow::Result<tools::KeyDeleted>,
        Vec<String>,
    ) {
        let caps = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = caps.clone();
        let out = plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, req: &str| {
                log.lock().unwrap().push(format!("{cap} {req}"));
                match cap {
                    "http.request" => Ok(json!({"status": status, "headers": [],
                        "body": "Not Found".as_bytes()})
                    .to_string()),
                    _ => secret.clone(),
                }
            }),
            || block(tools::delete_key(&g(), "main", "GK1")),
        );
        let caps = caps.lock().unwrap().clone();
        (out, caps)
    }

    #[test]
    fn key_delete_of_a_missing_key_still_removes_the_secret() {
        let (out, caps) = delete_with(404, Ok(json!({"value": null, "found": true}).to_string()));
        let out = out.unwrap();
        assert!(!out.deleted && out.secret_removed);
        assert!(caps[1].contains("garage.main.key_GK1"), "{caps:?}");
    }

    #[test]
    fn key_delete_reports_a_failed_secret_delete() {
        let (out, _) = delete_with(200, Err("store offline".into()));
        let e = format!("{:#}", out.unwrap_err());
        assert!(e.contains("store offline") && e.contains("GK1"), "{e}");
    }

    #[test]
    fn key_delete_surfaces_other_garage_errors() {
        let (out, caps) = delete_with(500, Err("unreached".into()));
        assert!(format!("{:#}", out.unwrap_err()).contains("HTTP 500"));
        assert_eq!(caps.len(), 1);
    }

    #[test]
    fn error_bodies_are_truncated_scrubbed_and_tokenless() {
        serve(
            |_, _| {
                Some((
                    500,
                    format!(
                        r#"{{"token":"{TOKEN}","password":"hunter2hunter2","pad":"{}"}}"#,
                        "x".repeat(2000)
                    ),
                ))
            },
            || {
                let e = block(g().buckets()).unwrap_err().to_string();
                assert!(e.starts_with("ListBuckets: HTTP 500"), "{e}");
                assert!(e.len() < 400, "{}", e.len());
                assert!(!e.contains(TOKEN), "{e}");
                assert!(!e.contains("hunter2hunter2"), "{e}");
            },
        );
    }

    #[test]
    fn connect_fails_when_the_admin_token_secret_is_missing() {
        let caps = Arc::new(Mutex::new(Vec::new()));
        let log = caps.clone();
        let e = plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, _: &str| {
                log.lock().unwrap().push(cap.to_string());
                match cap {
                    "secret.op" => Ok(json!({"value": null}).to_string()),
                    _ => Err(format!("unexpected {cap}")),
                }
            }),
            || block(tools::connect("main")).err().unwrap(),
        );
        assert!(
            format!("{e:#}").contains("garage.main.admin_token"),
            "{e:#}"
        );
        assert_eq!(*caps.lock().unwrap(), vec!["secret.op"]);
    }

    #[test]
    fn dry_run_writes_make_no_request() {
        let cfg = Arc::new(plugin_toolkit::contract::config::Config::load().unwrap());
        let ctx = plugin_toolkit::contract::ToolCtx::new(cfg);
        let caps = Arc::new(Mutex::new(Vec::new()));
        let log = caps.clone();
        plugin_toolkit::capsink::with_cap_sink(
            Box::new(move |cap: &str, _: &str| {
                log.lock().unwrap().push(cap.to_string());
                Err(format!("unexpected {cap}"))
            }),
            || {
                for (tool, args) in [
                    (
                        "garage.layout.apply",
                        json!({"name": "main", "node_id": "n1", "zone": "dc1", "capacity": 1, "dryRun": true}),
                    ),
                    (
                        "garage.bucket.create",
                        json!({"name": "main", "alias": "sccache", "dryRun": true}),
                    ),
                    (
                        "garage.bucket.delete",
                        json!({"name": "main", "bucket": "sccache", "dryRun": true}),
                    ),
                    (
                        "garage.bucket.update",
                        json!({"name": "main", "bucket": "sccache", "max_size": 1, "dryRun": true}),
                    ),
                    (
                        "garage.bucket.allow",
                        json!({"name": "main", "bucket": "sccache", "key_id": "GK1", "read": true, "dryRun": true}),
                    ),
                    (
                        "garage.bucket.deny",
                        json!({"name": "main", "bucket": "sccache", "key_id": "GK1", "read": true, "dryRun": true}),
                    ),
                    (
                        "garage.bucket.cleanup_uploads",
                        json!({"name": "main", "bucket": "sccache", "dryRun": true}),
                    ),
                    (
                        "garage.key.create",
                        json!({"name": "main", "key_name": "ci", "dryRun": true}),
                    ),
                    (
                        "garage.key.import",
                        json!({"name": "main", "key_id": "GK1", "secret_ref": "garage.main.x", "dryRun": true}),
                    ),
                    (
                        "garage.key.delete",
                        json!({"name": "main", "key_id": "GK1", "dryRun": true}),
                    ),
                ] {
                    let out = block(plugin_toolkit::dispatch::dispatch(tool, args, &ctx))
                        .unwrap_or_else(|e| panic!("{tool}: {e:#}"));
                    assert_eq!(out["dryRun"], json!(true), "{tool}: {out}");
                }
            },
        );
        assert!(
            caps.lock().unwrap().is_empty(),
            "{:?}",
            caps.lock().unwrap()
        );
    }

    #[test]
    fn generic_workload_spec_is_refused() {
        let b = GarageBackend::new("garage");
        let e = block(b.workload_spec(Runtime::Docker, "garage", &Routes::default())).unwrap_err();
        assert!(e.to_string().contains("compose.yml"), "{e}");
        assert_eq!(b.default_port(), 3903);
    }
}
