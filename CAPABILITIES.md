# garage — capabilities

## Endpoint
- [x] `garage.{list,detail,create,update,delete}` — routes to the admin API (port 3903); `admin_token` is a secret (`garage.<name>.admin_token`)

## Tools (admin API v2)
- [x] cluster: `health`, `status`
- [x] layout: `detail`, `apply` (single node, zone + capacity, applied at the next version)
- [x] bucket: `list`, `detail`, `create`, `delete`, `update` (quotas), `allow`, `deny`, `cleanup_uploads`
- [x] key: `list`, `detail`, `create`, `import`, `delete` — secrets live only in the orca secret store

## ServiceBackend
- [x] `provider` / `runtimes` / `default_port` / `capabilities` / `data_paths`
- [x] `status` — `GetClusterHealth`
- [ ] `workload_spec` — refused: WorkloadSpec cannot carry a config file, secret files, `cap_drop`, `no-new-privileges` or a user; deploy `compose.yml` through the docker plugin
- [ ] `configure`

## Not yet
- [ ] ObjectStoreAdmin seam adapter (core seam pending); `api.rs` holds the Garage-domain mapping it will wrap
