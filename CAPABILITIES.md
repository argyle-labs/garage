# garage — capabilities

## Endpoint
- [x] `garage.{list,detail,create,update,delete}` — routes to the admin API (port 3903); `admin_token` is a secret (`garage.<name>.admin_token`)

## Tools (admin API v2)
- [x] cluster: `health`, `status`
- [x] layout: `detail`, `apply` (single node, zone + capacity, applied at the next version)
- [x] bucket: `list`, `detail`, `create`, `delete`, `update` (quotas), `lifecycle` (one orca-managed rule, ID `orca`: days-based object expiry and/or abort of incomplete multipart uploads after N days, 1–36500 each, optional single non-empty prefix; every other rule is preserved untouched; `clear` removes only the `orca` rule; disabled rules and fixed-date expiry are not expressible; Garage's daily worker applies rules at midnight UTC, so space is not reclaimed immediately; dry-run reads the current rules and reports the resulting list without writing), `allow`, `deny`, `cleanup_uploads`
- [x] key: `list`, `detail`, `create`, `import`, `delete` — secrets live only in the orca secret store

## ServiceBackend
- [x] `provider` / `runtimes` / `default_port` / `capabilities` / `data_paths`
- [x] `status` — `GetClusterHealth`
- [ ] `workload_spec` — refused: WorkloadSpec cannot carry a config file, secret files, `cap_drop`, `no-new-privileges` or a user; deploy `compose.yml` through the docker plugin
- [ ] `configure`

## Not yet
- [ ] ObjectStoreAdmin seam adapter (core seam pending); `api.rs` holds the Garage-domain mapping it will wrap
