# Native workload bootstrap and live operations

These are ordinary trusted bootstrap commands, not a launcher or runtime inventory.
Use the authenticated maintained-fork CLI pointed at the selected Kubernetes
gateway. Select individually reviewed, compatible source and image pins for the
gateway, supervisor and sandbox runtime; a shared tag alone does not prove that
compatibility. Keep existing native roles/scopes and mediation: Kubernetes
placement does not add a new authentication or Lore permission system.

## Private service names and trust

| Input | Kubernetes value |
| --- | --- |
| Gateway | `https://openshell.openshell.svc.cluster.local:8080` |
| PostgreSQL | `veoveo-postgres-rw.veoveo-system.svc.cluster.local:5432` |
| Private S3 | `https://seaweedfs-s3.veoveo-system.svc.cluster.local:9000` |
| Native Lore | `grpcs://lore.veoveo-system.svc.cluster.local:41337` |
| Log store (console only) | `http://victorialogs.veoveo-system.svc.cluster.local:9428` |
| Image builder (Hub only) | `tcp://buildkitd.veoveo-system.svc.cluster.local:1234` |
| Local registry, every Veoveo image | `registry.veoveo-system.svc.cluster.local:5000` |
| Hub public origin / registered-server issuer | `https://veoveo.example.com` |
| Browser and native Lore authorization-server issuer | `https://veoveo.example.com/oauth` |
| Public signed objects | `https://veoveo.example.com/artifacts/` |

Render public origins to the actual installation. Service DNS replaces old host
loopback/Docker aliases only for this Linux placement. Keep every endpoint's
exact binary/port/method/path/provider rule. The `policies/` files use native
policy schema and complete canonicalization, not a second policy language.
Computers reach Lore through the `vv-lore` profile's credential-bound endpoint and
factory sandboxes through `vv-factory-lore`'s, which admits reads only; no policy has a passthrough Lore rule. Do not add aliases
or disable TLS to make trust pass.

The private Lore leaf must cover `lore.veoveo-system.svc.cluster.local` and its
native Hub deployment endpoint must use that same recipient. The S3 leaf must
cover its private Service DNS, which the tunnel verifies. Build
the maintained [Lore image](../factory/Lore.Dockerfile) with reviewed **public**
CA files only in its existing `ca` context; include storage and database public
roots needed by its selected clients. Build Factory/Computer with the necessary
public Lore root. Private CA keys never enter images or user workloads. The
gateway's chart-generated workload trust remains native supervisor material.

## Lore and tunnel foundation workloads

Render `lore.toml` privately with the actual Hub issuer/audience/client; Lore runs
no hook. Create `lore-settings` from the native TOML,
`lore-server-tls` as a TLS Secret, and `lore-storage` from a private env file with
exact keys `LORE_POSTGRES_URL`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`.
This `--from-env-file` input uses raw `KEY=value` lines without shell quotes;
URL-encode the PostgreSQL password inside the single-line URI, using the
[no-whitespace credential](README.md#postgresql-and-native-storage-identities).
Hub/Artifact env
files sourced by their shell entrypoints retain ordinary shell quoting.
Use `server_lore`, schema/bucket `lore`, and the dedicated native S3 identity.
Create that bucket once with its existing IAM identity name using the
[native TLS operator bootstrap](README.md#seaweedfs) before application use; do
not grant global `Admin` permission. With Seaweed's native actions format,
Artifact alone needs exact `Admin:artifacts` for its startup CreateBucket and
bucket configuration; Lore does not. See the [scoped identity inputs](README.md#seaweedfs).
Its PostgreSQL URI uses the Service DNS with `sslmode=verify-full` and
`sslrootcert=/etc/lore/postgres-ca/postgres-ca.crt`. Create `postgres-client-ca`
ConfigMap key `postgres-ca.crt` in `veoveo-system` from only the PostgreSQL CA certificate;
the ordinary Lore Deployment mounts it read-only. Native SQLx parses these URI
options without a Lore source change.
No source grants are inferred from the storage credential or Hub administrator.

```sh
kubectl -n veoveo-system create configmap lore-settings \
  --from-file=local.toml="$PRIVATE/lore.toml"
kubectl -n veoveo-system create secret tls lore-server-tls \
  --cert="$PRIVATE/lore.crt" --key="$PRIVATE/lore.key"
kubectl -n veoveo-system create secret generic lore-storage \
  --from-env-file="$PRIVATE/lore-storage.env"
kubectl apply -f "$PRIVATE/lore.yaml"
```

The private manifest is `lore.yaml` with its image
[filled](README.md#naming-each-image-once) from the lock, the same Lore image that Hub,
Computers and factory sandboxes mount. A single Lore process owns its native local locks; the small
PVC holds its HOME/temp/local lock paths. Repository storage stays in native
S3/PostgreSQL. A TCP readiness probe proves only a listening socket.

Lore reads Hub's OIDC discovery document at startup, so it cannot become Ready
before Hub serves its public issuer: until then it exits with `OIDC discovery
endpoint returned error` and its Deployment restarts it. Do not wait for Lore here;
continue with the workspaces and Hub, and check Lore once Hub serves (below). The
ordinary Deployment restart picks Hub up, and deleting the crash-looping Pod skips
the remaining back-off (measured 2026-10-22: Ready 1 s after the Pod's deletion once
Hub served). Test real native operations and late or interrupted issuer/storage
dependencies separately. No service-order init Job.

The existing tunnel is remotely managed: cloudflared runs with `--token-file` and
reads its routes from Cloudflare. [tunnel-routes.yaml](tunnel-routes.yaml) records
those routes, in order, as the dashboard holds them; enter them under the tunnel's
routes, rendered to the public host. The first route whose host and path match
serves a request, so `/artifacts/` comes before Hub's route for the same host.
Create the `public-tunnel` Secret with the tunnel token as its `token` key, which
`tunnel.yaml` mounts at `/etc/cloudflared-credentials`, and the `public-tunnel-ca`
ConfigMap with **public** `gateway.crt` and `storage.crt`, mounted at
`/etc/cloudflared-ca`, the files the routes' CA pools name. The gateway public CA comes from the chart's
retained native PKI output; only its public certificate is copied. `tunnel.yaml`
pins the verified official cloudflared `2026.10.1` Linux amd64 digest; apply that
selected manifest. Never use `noTLSVerify`. Hub routing intentionally sets its
native exposure Host; S3 routing must **not** set `httpHostHeader`, since SigV4
binds the public Host.

## Workspace-local trusted service configuration

```sh
openshell workspace create --name vv-control
openshell workspace create --name vv-services
openshell workspace create --name installation
openshell workspace create --name vv-factory
```

Native `CreateWorkspace` persists the workspace, but does not itself instantiate
the Kubernetes namespace. Prepare the **required** provider profiles/custody using
[native control](../computer/NATIVE_CONTROL.md#trusted-preparation-and-attachment)
and the existing registration-provider instructions. The initial native credential
mint/store calls `ensure_workspace`, which creates the managed namespace. Verify
the actual resulting namespace before PVC/Job admission; do not create it yourself
or add a dummy provider merely to trigger it. With gateway ID `openshell` and
workspace `vv-control`, the native name is `openshell-openshell-vv-control`.
The namespace is OpenShell-owned; no per-workspace Helm release. These example
values select gateway ID `openshell`; it is installation data, not a fixed namespace
contract. Native namespaces follow `openshell-<gateway_id>-<workspace>`. The chart
uses explicit `gatewayConfig["openshell.drivers.kubernetes"].gateway_id` when set,
otherwise `server.sandboxJwt.gatewayId`, otherwise its fullname. Render the PVC/Job
namespaces to the actual selected ID and verify the native namespace before use.
Changing these examples does not rename or restart an existing gateway.

Hub's platform-admin service identity needs the existing six workspace/sandbox/
provider permissions plus **`config:read`** for native `GetGatewayInfo` backend
discovery. Both Computer-enabled Hub policy examples now allow that exact POST.
Factory sandboxes add `DeleteSandbox` and `DeleteProvider` under the same sandbox and
provider permissions; Hub recreates a sandbox whose policy changed instead of
updating it, so it needs no `config:write`
([NATIVE_CONTROL.md](../computer/NATIVE_CONTROL.md#exact-network-boundary-and-qualification)).
No broader native role, wildcard RPC, credential export or new token worker.

Create `hub-config-bootstrap` in that actual namespace from only reviewed private
`hub.json`, `hub.env`, `computer-policy-v1.yaml`, `factory-base-v1.yaml` (the
[factory base policy](policies/factory-base.yaml) rendered with Hub's host) and the
selected public `authorization-server.jwks.json`, using native Secret `--from-file`
arguments. The JWKS file preserves the file-backed JWKS path in
`identity/entra.example.json`; it contains public verification keys, never signing
keys. Match these five names to `hub-config-pvc.yaml`. Before
running the seed Job, replace its copy-image marker with the verified immutable
Linux utility image. Hub and MCP runtime images use UID `10001`; the config
seed Job uses that UID so mode `0600` files remain readable to the component.
The Job's group `10001` does not require the image's primary GID to match: Time
and Artifact images can use GID `999`, while the measured native Hub uses GID
`10001`. Owner UID controls these private files. Factory and Computer images also
use UID/GID `10001`, matching `server.sandboxUid`/`sandboxGid`. Native workspace
seeding runs as that identity and must be able to read the image's private
`/sandbox` directories. Confirm actual identities against the selected runtime.
The Job copies only those files without output, each to a hidden temporary name in
the claim, and renames them into place once every copy succeeded. A rerun therefore
replaces the files of the existing claim, which keeps the claim and the UID OpenShell
pinned when it admitted the Hub sandbox. To change a file, recreate the bootstrap
Secret, apply the manifest again (its claim stays unchanged), wait for and delete the
Job as below, and restart Hub through its native lifecycle so it reads the files
again. `hub.json` seeds only an empty installation; a live control plane changes
through Hub's control-plane route. The PVC labels match native default resource
admission for workspace `vv-control`.

```sh
kubectl apply -f "$PRIVATE/hub-config-pvc.yaml"
kubectl -n openshell-openshell-vv-control wait job/seed-hub-config-v1 \
  --for=condition=Complete --timeout=120s
kubectl -n openshell-openshell-vv-control delete job seed-hub-config-v1
kubectl -n openshell-openshell-vv-control delete secret hub-config-bootstrap
```

`hub-driver-config.json` mounts this exact admitted claim read-only at
`/etc/veoveo/hub`; do not mount the foundation namespace's PVC. Use separate
workspace-local config claims and exact file lists for console/Artifact/Media if
needed. No config-copy controller, hostPath, protected `/run` mount, caller
Secret/ConfigMap reference or shared user workspace. Server native config files
may hold their own trusted storage secrets; user workloads receive mediated
provider references instead.

`service-storage.yaml` and `seed-service-configs.yaml` provide the same initial
native `vv-services` bootstrap for console/Artifact/Media. Use only Artifact's
`storage.env` and Media's `provider.env` in their separate temporary Secrets;
the config Jobs run as UID `10001` with seed group `10001`, copy exact files as
mode `0600`, replace them in an existing claim as Hub's seed does, and never copy
operator directories. The component's owner UID must
match; its primary GID need not match for these owner-only files. Confirm actual
identities against the selected Artifact/Media runtime images. Their `*-driver-config.json`
inputs mount each service's own writable state and read-only configuration.
Delete completed seed Jobs and their temporary Secrets. This is trusted storage
preparation, never a Helm inventory or runtime reconciliation process.

Artifact's existing native `storage.env` separates its private backend from signed
client URLs (actual access/secret keys remain only in that private file):

```text
VEOVEO_STORAGE_ENDPOINT=https://seaweedfs-s3.veoveo-system.svc.cluster.local:9000
VEOVEO_STORAGE_SIGNING_ENDPOINT=https://veoveo.example.com
VEOVEO_STORAGE_REGION=us-east-1
VEOVEO_STORAGE_BUCKET=artifacts
```

Artifact addresses the bucket by path, so it signs
`https://veoveo.example.com/artifacts/<key>` and SigV4 binds that host and path;
the tunnel's `/artifacts/` route carries them to the private S3 unchanged. Computer
curl and Media's Artifact client send signed requests to the public host through
OpenShell's inspected `artifact_bytes` endpoint for `/artifacts/**`, which allows
only their methods and forwards Host, path, query, headers and body as signed, with
no credential binding. That host's other endpoints in the same policies are
inspected too, so a passthrough rule for it would be ambiguous. The embedding
runtime passes TLS through to the host instead (see [EMBEDDING.md](EMBEDDING.md)).
Never substitute the private Service authority into a signed URL.

### PostgreSQL client trust shared within each workspace

Create `postgres-client-ca` ConfigMap in each actual native workspace namespace
with `postgres-ca.crt` (PostgreSQL CA certificate), `storage-ca.crt` (Seaweed public CA)
and `lore-ca.crt` (the selected native Lore service public CA),
never their keys. Use the public Seaweed CA certificate from the
`seaweedfs-ca-cert` Secret's **`tls.crt`** key. That chart input key is distinct
from the copied `storage-ca.crt` filename; a Secret containing only `ca.crt` does
not satisfy the chart's native `[grpc].ca` path. Render the utility image in
`postgres-client-ca.yaml`, apply its workspace-local PVCs and seed Jobs, which
replace the CA files of an existing claim in place, wait for completion, then delete
those Jobs and temporary source ConfigMaps. The native
driver-config examples mount these approved CA-only claims read-only at
`/etc/veoveo/tls`. They contain no Hub signing/configuration secret and are reused
only inside their named workspace.

Hub's own DB URI must retain:

```text
host=veoveo-postgres-rw.veoveo-system.svc.cluster.local
sslmode=verify-full
sslrootcert=/etc/veoveo/tls/postgres-ca.crt
```

Its `--server-database-url` / `VEOVEO_SERVER_DATABASE_URL`, which Hub copies into
every registration grant, keeps the same host and `sslmode=verify-full` but has **no
`sslrootcert`**. Factory sandboxes mount no CA claim; every server instead verifies
against the PostgreSQL CA in [`platform-destination-roots-v1`](README.md#native-openshell-gateway),
which the supervisor injects as `SSL_CERT_FILE`. Core's sqlx client uses native roots
(`tls-rustls-ring-native-roots`), which read that file: an inference from source that a
factory instance's first database connection must prove.

Set `VEOVEO_DATABASE_URL` with the actual owner username but **no password**;
supply the matching [no-whitespace managed-role password](README.md#postgresql-and-native-storage-identities)
separately through shell-sourced `VEOVEO_DATABASE_PASSWORD`.
Set `VEOVEO_SERVER_DATABASE_URL` with **neither username nor password**. Hub supplies
the narrowed application role and password through its existing registration grant
contract. Keep secrets in private configuration, not argv. Both configured URLs
must retain the Service DNS and `sslmode=verify-full`, so fixing only Hub's own URI
is insufficient. The `database` policy's `tls: skip` means OpenShell
passes PostgreSQL's native TLS through; the PostgreSQL client still verifies the
chain **and Service hostname** with `verify-full`. It does not disable TLS checks.

For Hub native workspace provisioning and Artifact, set native nonsecret
`--env SSL_CERT_DIR=/etc/veoveo/tls` (or the same literal in Hub's trusted shell-sourced env). The native gRPC and existing
AWS rustls clients load that public CA directory **alongside** the injected
`SSL_CERT_FILE` system/supervisor bundle. Keep the injected file intact. The
CA-only claim therefore supplies Seaweed trust without a derivative image or
weaker certificate validation. Prove the deployed S3 handshake/operations; the
mount/environment configuration alone is not transport acceptance.

## Hub and automatic private Computers

`hub.example.json` is the existing typed Hub input with Kubernetes gateway/Lore
addresses, versioned Computer template/policy, the private bindings of Artifact, Media,
Embedding and console, empty repository/assignment lists and the `execution` and
`factory` sections. Merged by typed keys with
[identity/entra.example.json](../identity/entra.example.json), it is a complete seed
catalog that `veoveo-hub validate` admits, with the names the
[installation design](../DESIGN.md#identity) lists. Both use one set of example
identifiers, which an installation replaces with its own: Entra tenant
`11111111-1111-4111-8111-111111111111`, the OpenShell resource application
`22222222-2222-4222-8222-222222222222` (the gateway's audience), Hub's broker
application `33333333-3333-4333-8333-333333333333` (the `entra` client) and the example
people `44444444-…` (member), `55555555-…` (administrator) and `66666666-…` (billed).
`execution.baseBranch` (`main`) is the branch whose instance serves a server a user's selection does not name. `factory` names the
same gateway as `nativeUpstreams`, the `vv-factory` workspace, the base policy at
`/etc/veoveo/hub/factory-base-v1.yaml`, the
[factory template](README.md#factory-sandbox-policy-and-providers) `factory-v1` in
workspace `installation`, which sizes every factory sandbox and mounts Lore, the
loopback port, Hub's issuer, the Lore server, the two global provider profiles, the
image builder (BuildKit and the local registry) and soft caps (`instancesPerUser: 4`,
`enforce: false`: exceeding the cap warns in the instance's status). These are Hub
control-plane values: the file seeds them and the live route below changes them.
Its `oauthClients` also register `veoveo-hub-sdk-cli`, the public client whose
`login` signs a person in from a terminal: the authorization code grant with PKCE and
refresh-token rotation, no client secret, the person's `local` MCP profile and the
loopback redirect `http://127.0.0.1:3211/callback`. Hub matches a redirect URI
exactly, port included, so the CLI listens on that port; 3211 stays clear of
OpenCode's `http://127.0.0.1:3210/callback`. The client grants nothing by itself:
the person's own memberships and grants decide what the CLI may do.
Merge installation identity/settings/signing/services by their typed keys;
never concatenate duplicate IDs or replace an existing active catalog with this
bootstrap sample. Run the Hub binary's native validate/import commands against
the **actual complete private input**, not a partial JSON subtree. Initial files
seed only an empty installation; routine updates use the authenticated live
`GET`/`PUT /admin/{profile}/control-plane` path and its current revision. That route
needs an explicit rule in the complete input's `policies[].rules`, naming the route's
`{profile}` and the Entra `administrator` app role, which Hub carries in the token's
`roles` claim. The identity example's `reference-policy` holds it:

```json
{"id": "installation-administration", "effect": "allow",
 "actions": ["admin_read", "admin_write"], "profiles": ["local"],
 "roles": ["administrator"], "tenantIds": ["reference"]}
```

Making someone an administrator is assigning them that Entra app role; Hub's `admin`
installation role, with its risks, governs only server tool permissions.

There are no named environments and no enrollment step: a service that registers
serves. Start Hub from the trusted config claim, then start the services; each
registration routes its slug. When a new deployment of a slug registers beside an
older one, the newest registration serves and Hub logs one WARN naming the deployment
it set aside, so moving a service to a new deployment needs no gap. A redeploy that
keeps its `VEOVEO_DEPLOYMENT_ID` stays the one deployment.

```sh
openshell profile lint --global --from computer/providers
openshell profile import --global --from computer/providers
openshell sandbox template create computer-v1 --workspace installation \
  --image "$COMPUTER_IMAGE_AT_DIGEST" --cpu 2 --memory 4Gi \
  --driver-config-json "$(cat "$PRIVATE/computer-driver-config.json")" \
  --env SSL_CERT_DIR=/etc/ssl/certs \
  --env VEOVEO_LORE_REMOTE=grpcs://lore.veoveo-system.svc.cluster.local:41337 \
  --env VEOVEO_CLI_HUB="$HUB_ORIGIN" --env VEOVEO_CLI_PROFILE=computer \
  --env VEOVEO_CLI_BEARER_ENV=COMPUTER_HUB_REFERENCE \
  --annotation "veoveo.ai/release-note=$COMPUTER_RELEASE_NOTE"
```

The template's driver configuration, [filled](README.md#naming-each-image-once) from
the lock, mounts the Lore and Hub SDK CLI images read-only at `/opt/veoveo/lore` and
`/opt/veoveo/hub-sdk-cli`, whose binary directories the Computer image puts on `PATH`.
`HUB_ORIGIN` is the Hub's public origin, such as `https://veoveo.example.com`. The
three `VEOVEO_CLI_*` variables make `veoveo-hub-sdk-cli tools` and `call` act as the
Computer's owner with no flags, exactly as OpenCode does: each request goes to
`$HUB_ORIGIN/mcp/computer` with the `COMPUTER_HUB_REFERENCE` placeholder, which the
proxy replaces with the owner's Hub token, and the
[Computer policy](policies/computer-v1.yaml)'s `hub` rule admits that endpoint for the
CLI's mounted binary. The CLI's `login`, `logout` and `ask` say that they do not apply
in a Computer, and no rule admits the model endpoint for the CLI.
The profiles and the template may come before or after Hub starts: Hub reads the
template when a Computer is entered, and until it and the profiles exist, `/computer`
says that Computers are unavailable and why. The template, the
[policy](policies/computer-v1.yaml) and Hub's `computers` settings make the Computer
version, a twelve-digit digest that `/computer/status`, the console's `computer` tool
and each sandbox's `COMPUTER_VERSION` name. `GET /computer/status`, `POST
/computer/restart` and `POST /computer/previous` serve the owner's browser session and
a bearer of Hub's Computers profile: the [Computer policy](policies/computer-v1.yaml)
lets any program in a Computer call them with the owner's Hub token, and
[console's policy](policies/console.yaml) lets console call status and restart for its
caller and read the public `GET /computer/version`. The optional `veoveo.ai/release-note`
annotation is the version's release note; its first line is the summary agents tell
the person. To publish a new version, delete the template and create it again under
the same name with the new image, environment, mounts or note (OpenShell has no
template update), or publish a new policy file or template name through the live control
plane. Running Computers keep their version until their owners restart them; a new
sandbox, such as the first one, starts at the current version.

`computers.collectAfterSeconds` (the example: 7200, two hours) collects idle
Computers: Hub deletes the sandbox of a Computer with no browser request through Hub
and no network traffic from its sandbox for that long, keeping `~`, and the next entry
creates it at the current version, telling of the update when the version changed.
Collection uses the `GetSandbox` and `DeleteSandbox` rights a restart already needs,
and OpenShell's recorded `last_activity_time`, so the gateway must run a release that
records it. A job that makes no network traffic for that long is collected; omit the
setting to never collect. Hub counts both from its own start, so restarting Hub delays
collection by up to that long.

Each Computer's `computer-lore` provider uses the global `vv-lore` profile that
`computers.loreProvider` names, imported with the
[factory profiles](README.md#factory-sandbox-policy-and-providers). Hub sets
`LORE_IDENTITY` in each owner's template clone; the template sets no Lore variable.

**No Docker `persistent_workspace` flag or explicit `/sandbox` volume is passed.**
The selected Kubernetes driver creates and seeds the private workspace PVC for
each clone with native ownership. Hub checks the actual selected driver through
`GetGatewayInfo`; it rejects reusable static mounts at/below `/sandbox`. Versioned
read-only config claims outside `/sandbox` remain supported.

Start Hub using the reviewed Hub image/policy/provider and
`--driver-config-json "$(cat "$PRIVATE/hub-driver-config.json")"`, the
[filled](README.md#naming-each-image-once) configuration that also mounts the Lore image
Hub reads pushed revisions with, retaining the
existing trusted native command that loads `/etc/veoveo/hub/hub.env` and execs the
Hub binary. Long-running Hub/MCP sandboxes explicitly select
`--restart-policy always`; native default `Never` is unsuitable. Expose the same
Hub `http` service on `8080` with native `bearer_passthrough`. No Helm inventory
owns it. Once Hub serves at its public issuer, Lore becomes Ready:

```sh
kubectl -n veoveo-system wait deployment/lore --for=condition=Available --timeout=300s
```
 Console/Artifact/Media retain their actual native commands/configs, using
their own trusted claims, Kubernetes policies and distinct native registration
providers; do not share Hub's control identity. Time and every other server built
from a pushed Lore branch run as factory sandboxes that Hub creates; the
installation has no Time sandbox.

Console, the Hub MCP server, is a platform sandbox in `vv-services` on this
namespace registration path, not a factory instance: it reads instance output from
the log store, which the factory base policy withholds from branches. Its inputs are
[its policy](policies/console.yaml), the
[registration profile](../openshell/providers/console-registration.yaml), the
`console-state-v1` claim in `service-storage.yaml` for its saved registration key
and the [driver config](console-driver-config.json). It needs no CA claim: its
database grant verifies against the platform roots like every server's. Choose a
UUIDv7 `CONSOLE_DEPLOYMENT_ID` and keep it.

For example, after native provider custody/config preparation, the Hub and console
launch path uses these installation inputs:

```sh
openshell sandbox create --workspace vv-control --name hub --from "$HUB_IMAGE_AT_DIGEST" \
  --policy kubernetes/policies/hub-computer-entra.example.yaml \
  --provider vv-hub-native-control --no-auto-providers \
  --driver-config-json "$(cat "$PRIVATE/hub-driver-config.json")" \
  --restart-policy always --detach --env SSL_CERT_DIR=/etc/veoveo/tls -- \
  /bin/sh -ec 'set -a; . /etc/veoveo/hub/hub.env; set +a; exec /usr/local/bin/veoveo-hub serve --allow-loopback-hosts'
openshell service expose --workspace vv-control hub 8080 http \
  --authorization-mode bearer-passthrough
openshell profile import --workspace vv-services --file openshell/providers/console-registration.yaml
openshell provider create --workspace vv-services --name vv-console-registration \
  --type vv-console-registration --credential VEOVEO_REGISTRATION_SECRET
openshell sandbox create --workspace vv-services --name console --from "$CONSOLE_IMAGE_AT_DIGEST" \
  --policy kubernetes/policies/console.yaml --provider vv-console-registration --no-auto-providers \
  --driver-config-json "$(cat kubernetes/console-driver-config.json)" \
  --restart-policy always --detach \
  --env VEOVEO_DEPLOYMENT_ID="$CONSOLE_DEPLOYMENT_ID" --env VEOVEO_HUB_ISSUER="$HUB_ORIGIN" \
  --env VEOVEO_PUBLIC_BASE_URL=http://console.internal:8805 --env VEOVEO_PORT=8805 -- \
  /usr/local/bin/veoveo-mcp-console
```

`provider create --credential VEOVEO_REGISTRATION_SECRET` reads the namespace
registration secret from the operator's environment, never from argv. Console serves
once it registers, like the other services.

Set `HUB_ORIGIN` to the Hub's actual public origin, such as
`https://veoveo.example.com`. Console, Artifact and Media use that origin for
`VEOVEO_HUB_ISSUER`: the native registered-server/forwarded-token issuer publishes
`/.well-known/oauth-authorization-server`, `/jwks.json` and `/oauth/register`.
The SDK verifies that discovery returns the same issuer before registration.
The Kubernetes policies admit those exact public-origin routes.

Do not append `/oauth` to this service setting. The separate browser/native Lore
authorization-server issuer is `https://veoveo.example.com/oauth`; its metadata
at `/.well-known/oauth-authorization-server/oauth` does not advertise server
registration. Preserve that issuer for human/Lore clients. Do not replace either
issuer with a native exposure alias. Artifact/Media use their corresponding
policies/driver-config files and existing native trusted shell commands loading
only their own config file. Verify discovery, registration and JWKS before
claiming MCP readiness.

Computer creation remains browser admission → native per-owner clone/providers →
ForwardTcp. Hub injects `COMPUTER_HUB_URL` from the selected profile's protected
resource; the image does not hardcode an installation origin. Native provider
custody supplies the user's Hub refresh grant and the approved model credential.
Explicit native Lore login remains user-owned. After native `repository create`
emits a canonical ID, publish that ID and only the acting principal's intended
assignment through Hub's existing typed live control plane; no automatic grants.
A factory sandbox reads its repository as the branch's latest pusher through its own
per-instance Lore provider.

Change templates/defaults by native template replacement or versioned references and
live Hub publication. Existing Computers keep their recorded version until their owner
restarts them; adding/removing/registering services or changing grants does not
restart Hub or unrelated workloads. Drain/reconnect active operations explicitly. Never replay a
non-idempotent mutation automatically or promise zero downtime.

### Replacing one service image

The current native API has no in-place sandbox image update. Stop/Start reuses
the admitted image. To replace Media's image, first drain or explicitly qualify
its active Tasks and retain its complete native creation inputs privately.

Export the authored policy with `openshell --workspace vv-services policy get media
--base` for round-tripping. Effective `sandbox get --policy-only` output includes
reserved provider overlays and is not the authored replacement input. Preserve
the exact provider attachments separately; exporting a base policy does not
transfer their custody or replace their attachment configuration.

Then run:

```sh
openshell --workspace vv-services sandbox stop media
openshell --workspace vv-services sandbox delete media
```

Repeat the reviewed `sandbox create --name media --from "$MEDIA_IMAGE_AT_DIGEST"`
invocation with the replacement digest, preserving the workspace, logical name,
provider attachments, environment, command, resources, restart policy and supplied
PVC configuration. Apply only intentionally reviewed policy changes. Keep
`media-state-v1`, `media-config-v1` and `postgres-client-ca-v1`; do not reseed or
delete these claims, their providers or the workspace. The supplied state claim
retains Media's registration key; this is not a promise that Delete preserves
the automatically owned `/sandbox` PVC. Preserve/recreate only this service's
exposures if it has any. Gate requests on fresh Hub registration and application
listener/readiness, not merely native phase Ready. The same logical destination
and registration key do not require a Hub or unrelated workload restart. Never
automatically replay a non-idempotent request across replacement.

## Isolated synthetic Media provider

The existing testing-only fixture image needs `MEDIA_PROVIDER_WEBHOOK_SECRET`
even for status-check operation. Its native CLI accepts an environment value,
not a config-file flag. Do not put that secret in native `--env` arguments or
mount Media's service configuration into the fixture. Prepare a separate private
shell env file `$PRIVATE/media-provider.env` containing only the fake webhook
HMAC value agreed with the synthetic Media service. Never copy real provider
credentials or real webhook keys into this fixture.

Create `media-provider-config-bootstrap` from that exact file in the actual
`vv-services` namespace, then use the fixture's separate claim and seed Job in
`media-provider-config-pvc.yaml`. The Job runs as UID 10001 and copies only
`provider.env` as mode 0600, replacing an existing one in place. Replace only its copy-image marker with the
reviewed utility digest. Remove the completed Job and bootstrap Secret after
seeding. `media-provider-driver-config.json` mounts only that claim, read-only at
`/etc/veoveo/media`; no service state, database/storage credentials, or service
configuration are shared.

```sh
kubectl -n openshell-openshell-vv-services create secret generic media-provider-config-bootstrap \
  --from-file=provider.env="$PRIVATE/media-provider.env"
kubectl apply -f "$PRIVATE/media-provider-config-pvc.yaml"
kubectl -n openshell-openshell-vv-services wait job/seed-media-provider-config-v1 \
  --for=condition=Complete --timeout=120s
kubectl -n openshell-openshell-vv-services delete job seed-media-provider-config-v1
kubectl -n openshell-openshell-vv-services delete secret media-provider-config-bootstrap
openshell sandbox create --workspace vv-services --name media-provider \
  --from "$MEDIA_PROVIDER_IMAGE_AT_DIGEST" --policy kubernetes/policies/media-provider.yaml \
  --driver-config-json "$(cat kubernetes/media-provider-driver-config.json)" \
  --no-auto-providers --restart-policy always --detach \
  --env MEDIA_FAKE_PROVIDER_LISTEN=127.0.0.1:8803 \
  --env MEDIA_FAKE_PROVIDER_API_BEARER=fake-generation-key \
  --env MEDIA_FAKE_PROVIDER_ADVERTISED_BASE=https://vv-services--media-provider--http.openshell.localhost:8080 -- \
  /bin/sh -ec 'set -a; . /etc/veoveo/media/provider.env; set +a; exec /usr/local/bin/veoveo-mcp-media-fake-provider'
openshell service expose --workspace vv-services media-provider 8803 http \
  --authorization-mode bearer-passthrough
```

Use the native workload alias with the configured gateway Service port (`8080`
in this example) for both the fixture's advertised HTTPS origin and Media's
`MEDIA_PROVIDER_URL`. The CLI formats exposure URLs using its operator gateway
connection port: an operator port-forward at `https://localhost:18443` therefore
prints `:18443`. That operator-local display port is not the in-sandbox origin;
keep the workload alias and exact policy port aligned with the gateway Service. The fixture's workload policy grants no outgoing
callbacks; use status-check operation with Media's `VEOVEO_PUBLIC_ORIGIN` unset.
This fixture produces synthetic data, not model inference or valid media proof.
Its native process readiness does not prove Media registration or a Task result.
No fixture image rebuild is required for these configuration inputs.

Media with a real provider may complete its runs by the provider's webhooks instead.
Its trusted configuration then sets `VEOVEO_PUBLIC_ORIGIN` to Hub's public origin and
`VEOVEO_ALLOW_LOOPBACK_HOSTS=true`, since native HTTP ingress reaches it as
`127.0.0.1:8802`, beside the provider's webhook secret. Expose Media natively and add
the Media callback route of [tunnel-routes.yaml](tunnel-routes.yaml), which publishes
only `/media/operations/{operation_id}/{token}`:

```sh
openshell service expose --workspace vv-services media 8802 http \
  --authorization-mode bearer-passthrough
```

With an existing native Hub bearer/reference, verify registration and actual MCP
routing without invoking a model:

```sh
veoveo-hub-sdk-cli --hub "$HUB_ORIGIN" --profile local \
  --bearer-env HUB_MEMBER_REFERENCE --json tools
veoveo-hub-sdk-cli --hub "$HUB_ORIGIN" --profile local \
  --bearer-env HUB_MEMBER_REFERENCE --json resources
```

The CLI owns discovery and MCP requests; keep its credential in the named env or
native provider, never an argv token. Inspect both items and reported per-server
degradation: an empty/partial listing is not all-server readiness. For registration
health, the existing authenticated `GET /admin/local/server-health` reports the
registered slug and live kernel-instance state. It requires the existing Hub
`admin_read` policy; the non-MCP fixture does not register as an MCP server.
