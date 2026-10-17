# garage

[Garage](https://garagehq.deuxfleurs.fr/) is a self-hosted, S3-compatible object store.

A first-party [orca](https://github.com/argyle-labs/orca) plugin (service-backend + `garage.*` tools). It is a translation layer over Garage's official [admin API v2](https://garagehq.deuxfleurs.fr/api/garage-admin-v2.json) (Garage v2.4.x): every tool is one or two admin API calls, sent through orca's delegated HTTP with the endpoint's `admin_token` as a Bearer token.

This repo is **self-contained** — the steps below run Garage **by hand, without orca**.

---

## Run it without orca

```sh
mkdir -p meta data secrets
openssl rand -hex 32 > secrets/rpc_secret
openssl rand -base64 32 > secrets/admin_token
openssl rand -base64 32 > secrets/metrics_token
chmod 600 secrets/*
chown -R 1000:1000 meta data secrets
GARAGE_BIND_ADDR=10.0.0.5 docker compose up -d
```

[`compose.yml`](compose.yml) runs `dxflrs/garage:v2.4.1` (digest-pinned) as UID 1000 with a read-only root filesystem, `cap_drop: ALL`, `no-new-privileges`, `mem_limit` (`GARAGE_MEM_LIMIT`, default `2g`) and `pids_limit: 512`, on its own network, with a `garage health` healthcheck. It mounts only [`garage.toml`](garage.toml) (read-only), `meta`, `data`, and the three secret files.

`GARAGE_BIND_ADDR` binds the S3 API (3900). The admin API (3903) binds separately to `GARAGE_ADMIN_BIND_ADDR`, default `127.0.0.1`; never set it to the S3 bind or any wide address. Reach the admin API through the TLS ingress (Caddy) on the host, which proxies to `127.0.0.1:3903`.

### Config

| Key | Value |
|---|---|
| `replication_factor` | `1` (single node) |
| `db_engine` | `lmdb` |
| `metadata_fsync` / `data_fsync` | `true` |
| `s3_api.s3_region` | `garage` |
| `s3_api.api_bind_addr` | `[::]:3900` |
| `admin.api_bind_addr` | `[::]:3903` |
| `rpc_secret_file` / `admin.admin_token_file` / `admin.metrics_token_file` | `/run/secrets/rpc_secret` / `/run/secrets/admin_token` / `/run/secrets/metrics_token` |

Secrets never go in `garage.toml`; Garage refuses secret files readable by others.

Both fsync settings are on so an acknowledged write survives a power loss on a single node; this costs write throughput and latency, especially for many small objects.

### Ports & data

| | |
|---|---|
| S3 API | `3900` |
| Admin API | `3903` (loopback; via the TLS ingress) |
| Metadata | `/var/lib/garage/meta` |
| Data | `/var/lib/garage/data` |

### Backup & restore

Back up `meta` (Garage snapshots it every 6h into `meta/snapshots`) and `data`, plus the three secret files.

## With orca

```sh
# store secrets/admin_token as the orca secret garage.main.admin_token
orca garage.create --name main --route lan=https://garage-admin.example.lan
orca garage.cluster.status --name main             # note the node id
orca garage.layout.apply --name main --node-id <id> --zone dc1 --capacity 500000000000
orca garage.bucket.create --name main --alias sccache
orca garage.key.create --name main --key-name ci   # secret stored as garage.main.key_<id>
orca garage.bucket.allow --name main --bucket sccache --key-id <id> --read --write
orca garage.bucket.update --name main --bucket sccache --max-size 200000000000
```

Writes run by default; pass `dryRun=true` to preview. Reads need `read`, writes need `admin`.

| Tool | Role | Admin API |
|---|---|---|
| `garage.cluster.health` | read | `GET /v2/GetClusterHealth` |
| `garage.cluster.status` | read | `GET /v2/GetClusterStatus` |
| `garage.layout.detail` | read | `GET /v2/GetClusterLayout` |
| `garage.layout.apply` | admin | `POST /v2/UpdateClusterLayout`, `POST /v2/ApplyClusterLayout` (next version); refuses if other nodes have staged changes unless `--apply-all-staged` |
| `garage.bucket.list` | read | `GET /v2/ListBuckets` |
| `garage.bucket.detail` | read | `GET /v2/GetBucketInfo` |
| `garage.bucket.create` | admin | `POST /v2/CreateBucket` |
| `garage.bucket.delete` | admin | `POST /v2/DeleteBucket` |
| `garage.bucket.update` | admin | `POST /v2/UpdateBucket` (quotas; needs `--max-size`, `--max-objects` or `--clear`) |
| `garage.bucket.allow` | admin | `POST /v2/AllowBucketKey` |
| `garage.bucket.deny` | admin | `POST /v2/DenyBucketKey` |
| `garage.bucket.cleanup_uploads` | admin | `POST /v2/CleanupIncompleteUploads` |
| `garage.key.list` | read | `GET /v2/ListKeys` |
| `garage.key.detail` | read | `GET /v2/GetKeyInfo` (never `showSecretKey`) |
| `garage.key.create` | admin | `POST /v2/CreateKey` |
| `garage.key.import` | admin | `POST /v2/ImportKey` |
| `garage.key.delete` | admin | `POST /v2/DeleteKey` |

Buckets are addressed by id (64 hex characters) or global alias; an alias is resolved with `GetBucketInfo?globalAlias=` before a write.

A created key's secret access key is written to the orca secret store as `garage.<endpoint>.key_<access key id>` and is never returned; tools return that name. `garage.key.import` reads the secret from an orca secret named by `secret_ref`, which must be `garage.<endpoint>.*` and not the admin token. If storing a created key's secret fails, the key is deleted from Garage. `garage.key.delete` also removes the stored secret, and still does so when the key is already gone from Garage.

`service.status garage` reports `GetClusterHealth`.
