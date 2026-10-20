# Native Lore storage in existing PostgreSQL and SeaweedFS

This template selects the `s3_postgres` plugin in the maintained Lore fork.
Reviewed source `7785408e70ed302296a24383ae302c3a606e78c3` passes native
immutable/mutable conformance against real PostgreSQL/SeaweedFS, focused
concurrency/failure/reopen scenarios, plugin configuration checks and operator
fixture compilation. Deployed server, dependency restart and factory acceptance
remain pending. Follow the
[execution checklist](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md)
and qualify the exact source revision before replacing the live server. The build
examples in [native setup](README.md) pin that plugin revision for both CLI and
server and reuse one fresh source export. The earlier `8be7e6e6` upstream-merge
baseline has no plugin.

The server connects to the **existing `veoveo` database** with dedicated login
`server_lore`, owning only schema `lore`. There is no additional database or
database instance. SeaweedFS holds a dedicated private bucket `lore` and dedicated
signing identity. Artifact retains its own schema, bucket and credentials. These
storage credentials stay in the trusted Lore server; no Computer or factory gets
database access or S3 signing keys. Native repository protocol/authentication and
permissions remain Lore responsibilities. User-facing access through Hub is a
separate transport qualification, not direct database or storage access.

## Trusted fresh preparation

Inspect existing roles, schema, bucket and identity before preparation. Preserve
matching credentials and refuse conflicting ownership. Use native PostgreSQL
operator sessions for a new installation; there is no bootstrap normalizer or
migration. The database owner cannot necessarily `SET ROLE server_lore`, so do
not assume it can create another login's schema with `AUTHORIZATION` directly.

Create the dedicated login through trusted role administration, reading the
password privately through psql's `\getenv` rather than shell arguments:

```sql
\getenv lore_password LORE_DATABASE_PASSWORD
CREATE ROLE server_lore LOGIN PASSWORD :'lore_password'
    NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
```

In the **existing `veoveo` database**, its owner grants `CONNECT` and a temporary
schema-creation capability:

```sql
GRANT CONNECT, CREATE ON DATABASE veoveo TO server_lore;
```

Connect privately as `server_lore` to that same database and execute:

```sql
CREATE SCHEMA lore;
```

Return immediately to the trusted owner session and revoke the temporary grant:

```sql
REVOKE CREATE ON DATABASE veoveo FROM server_lore;
```

Read back schema ownership and role memberships. If role creation gave
`veoveo_owner` implicit administrative membership in `server_lore`, revoke that
membership using the trusted role-administration session; do not leave it as an
installation dependency. Do not alter unrelated memberships. Existing matching
definitions require inspection rather than rerunning creation statements.
Runtime startup creates native tables inside the already-owned `lore` schema.

Verify `server_lore` can connect and use/own `lore`, cannot read unrelated
application tables, cannot create a database or role, and has no administrator or
cross-schema grants. Do not change shared application grants or database-wide
PUBLIC privileges to implement this slice. Ordinary restarts preserve schema
ownership and committed data. A trusted URL must name `veoveo`, for example:

```text
postgres://server_lore:<URL-encoded-password>@vv-postgres.internal:5432/veoveo?sslmode=disable
```

This example follows the existing private bootstrap network. Select verified TLS
when the installation's PostgreSQL endpoint provides it; never copy credentials
into tracked TOML. URL-encode the password instead of interpolating raw special
characters into a URL.

Provision a dedicated `lore` bucket and signing identity using SeaweedFS's native
IAM interfaces. The bucket must never have enabled versioning: the plugin rejects
both enabled and suspended versioning. Attach/read back the reviewed
[bucket policy](lore-storage-policy.json). It permits only `ListBucket`
(`HeadBucket`), `GetBucketVersioning` and object GET/PUT/DELETE on `lore`. Bucket
creation and IAM administration remain trusted bootstrap operations. No wildcard
administration, other bucket access, multipart upload or version listing is needed
by this backend. Verify bootstrap administration still works and this signing
identity is denied access to `artifacts`, unrelated buckets and IAM administration.

Create ignored directory `state/openshell/lore-direct/` with mode 0700 and file
`storage.env` with mode 0600. It contains only standard native credentials:

```text
LORE_POSTGRES_URL=<secret URL to existing veoveo database as server_lore>
AWS_ACCESS_KEY_ID=<dedicated lore bucket access key>
AWS_SECRET_ACCESS_KEY=<dedicated lore bucket signing key>
```

Compose injects that Docker-addressed file only into `vv-lore`; the native AWS SDK uses its provider
chain. Do not reuse `.env` bootstrap storage administrator credentials. No operator
directory or credential file enters the image or author workspace. The tracked
[server template](../lore/server.toml) contains only the environment variable name,
schema and nonsecret S3 settings. Its private backend endpoint is an installation
choice and creates no additional public origin. If the S3 listener is configured
for TLS, select its verified TLS endpoint and trust in the private configuration.
Host-native operator commands use a separate private `storage-native.env` with
the host-reachable PostgreSQL URL. Never inject the host loopback URL into the
Docker server. The keys have the same names; only the trusted connection address
changes with the process's network location.

## Qualification and disposable development cutover

Before changing the live server, run the native component and real-backend gates
documented in the reviewed source's `lore-s3-postgres/README.md`. Use dedicated
login `server_lore_test`, schema `lore_test`, bucket `lore-test` and bucket identity
inside the same `veoveo` database and SeaweedFS service. Neither test identity gets
runtime schema/bucket grants. The explicit integration harness resets only its
native `lore_test` tables and fails if configuration is absent. Do not run two
harnesses against the same fixture. Its parameters are:

```text
LORE_S3_POSTGRES_TEST_URL=<private URL as server_lore_test to existing veoveo DB>
LORE_S3_POSTGRES_TEST_SCHEMA=lore_test
LORE_S3_POSTGRES_TEST_BUCKET=lore-test
LORE_S3_POSTGRES_TEST_ENDPOINT=<reachable private S3 endpoint>
AWS_ACCESS_KEY_ID=<test bucket access key>
AWS_SECRET_ACCESS_KEY=<test bucket signing key>
```

In rapid development the backend switch starts with fresh disposable Lore state.
Stop the owning Lore server, retain canonical source and trusted identity/hook
inputs, reset only approved Lore state, then reseed from retained Git/published
source using native Lore operations. Do not migrate local packfiles or reset
Artifact, application, identity or other schema/bucket state. Backups and a
compatibility layer are not prerequisites for this fresh development cutover.
Revisit retention before production data becomes part of the scope.

Build CLI, server, factory package and stopped-server fixture from the same
reviewed source. The fixture now takes a **native config directory containing
`local.toml`**, not `/data`. Run it only in an isolated trusted operator process
on the private network with the selected configuration, public CA and native
storage credential environment. It constructs the selected backend and emits
the existing final JSON shape: `repository`, `forbidden`, `main`, `assigned`,
`other`. Save that JSON privately as fixture evidence. For actual repository
registrations/assignments use Hub's existing typed control-plane publication;
review native hook bindings separately in trusted TOML. This is not a runtime seed
service or a grant-provisioning helper. Do not run it concurrently with the
owning Lore server or give its storage credentials to the qualifier factory.

Keep the existing private native address, alias, static IP, TLS identity and volume
definitions. The local `vv-lore` volume is retained by this template; switching the
configured backend does not convert or delete it. The unqualified loopback port
proposal is separate from changing storage and does not establish Hub forwarding.
After the explicit cutover, probe native create/clone/edit/push, branch protection
and permission denials, then verify exact payload bytes in `lore` and index/ref
rows in `veoveo.lore`. Restart the owning process and dependencies and repeat
byte/hash retrieval. These are deployed probes, separate from component tests.

## Operational limits

The immutable and mutable stores share a pool capped at 16 connections. Native
resource locks remain local to one process; multi-server HA is not qualified.
PostgreSQL records publication only after an immutable S3 generation is uploaded.
Unique keys and exact-generation recursive retirement prevent a late upload or
old deletion from overwriting/removing a new published generation. This is not a
two-resource atomic commit. Failed physical deletion returns an error and retains
retirement state for native repeat/`verify(heal)` cleanup. Interrupted unpublished
uploads may retain unreachable bytes; no concurrent pending-object collector,
cleanup daemon or migration system is included. Follow the source backend contract
for exact behavior and do not claim automatic complete orphan reclamation.
