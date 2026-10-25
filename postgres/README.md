# PostgreSQL extensions

**Mac development fixture.** The Compose steps below build and run the Mac's
application PostgreSQL. On Linux the same [Dockerfile](Dockerfile) builds the image
the [K3s foundation](../kubernetes/README.md) publishes to its registry and runs as a
StatefulSet.

The application database uses the existing pinned PostgreSQL 18.6 image with
pinned PGDG pgvector 0.8.7 and PostGIS 3.6.4 packages. Keycloak's separate database is unchanged.
The build context is only this directory; no installation credentials enter it.

```sh
docker compose build vv-postgres
docker compose up -d --no-deps --no-build --pull never vv-postgres
```

Keep the existing volume, credentials, connection URLs and loopback listener. The
image keeps the upstream entrypoint and PostgreSQL layout. It holds the existing
server/client packages while installing the extensions, rather than upgrading the
server as a side effect. Neither extension needs a new service or preload setting.

On an empty data directory, `initdb/owner.sql` creates `veoveo`, then
[`initdb/zz-extensions.sql`](initdb/zz-extensions.sql) enables `vector` and `postgis`
in one transaction. For an **existing** database, image replacement does not run
init files: apply exactly the same SQL through the trusted bootstrap administrator:

```sh
docker compose exec -T vv-postgres \
  psql -X --username postgres --dbname veoveo --set=ON_ERROR_STOP=1 \
  < postgres/initdb/zz-extensions.sql
```

This can be rerun; it reports installed extension versions and refuses an existing
extension outside `_extensions` or a schema/extension not owned by `postgres`.
It refuses unexpected installed versions and does not relocate, upgrade or change
owners of existing objects. Review a version mismatch separately; `CREATE EXTENSION IF NOT EXISTS` is not an
extension upgrade. No application-state reset is required for enablement.

`_extensions` is trusted bootstrap-owned, with `USAGE TO PUBLIC` and no public
`CREATE`. Its leading underscore cannot be derived from a registered service slug.
Existing and future service logins inherit access to the extensions' standard
public type/function/reference-data grants without new role memberships. PostGIS's
`spatial_ref_sys` is shared read-only reference data; applications do not own or
modify it. Service schemas, tables, roles, URLs and search paths are unchanged.
Service migrations resolve unqualified application objects in their own schema;
qualify shared extension types/functions/operators. Installing extensions does not add component SQL or
change the component-owned SQL-confinement test contract.

## Service-role verification

Run [`verify.sql`](verify.sql) through an **existing non-superuser service login**,
with its own schema explicitly selected. Use standard private `PGHOST`, `PGPORT`,
`PGDATABASE=veoveo`, `PGUSER` and `PGPASSWORD`/`PGPASSFILE`; never put a password in
argv or retain credentials in the check output:

```sh
psql -X --set=ON_ERROR_STOP=1 --set=service_schema=time -f postgres/verify.sql
```

Repeat for the actual `artifact` and `media` logins, and the existing installation
owner with `service_schema=hub`. A trusted administrator may instead `SET ROLE`
to the existing service group before including this SQL; label that receipt as
group-role authorization, not an authenticated service-login test. Do not grant
new rights merely to make verification pass.

The transaction creates a small table and HNSW/GiST indexes **only in the selected
owned schema**, checks exact vector distance/nearest result and spatial bounds,
transforms EPSG:4326 to EPSG:3857, and demonstrates denied `CREATE` in `_extensions`.
It checks read-only access to `spatial_ref_sys`. Retain EXPLAIN output showing
`_extension_verification_hnsw` and `_extension_verification_gist` selected with
`enable_seqscan=off`: this is forced-plan usability proof, not performance or
approximate-search recall qualification. Everything rolls back; on an error,
`ON_ERROR_STOP` exits and closing the connection also rolls back. Existing tables
and extension objects are not modified.

Validate both fresh initialization and explicit existing-database application on
Linux ARM64 and AMD64, including rerunning enablement and confirming service-role
verification leaves no `_extension_verification` table or indexes behind. Read back
`pg_extension` versions and `postgres --version`; the PostgreSQL server must remain
18.6. Source builds/checks and runtime rollout are separate receipts.
