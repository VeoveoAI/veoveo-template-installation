# Native workload bootstrap and live operations

These are ordinary trusted bootstrap commands, not a launcher or runtime inventory.
Use the authenticated maintained-fork CLI pointed at the selected Kubernetes
gateway. Keep the gateway, supervisor and sandbox-runtime images on the same
reviewed revision. Keep existing native roles/scopes and mediation: Kubernetes
placement does not add a new authentication or Lore permission system.

## Private service names and trust

| Input | Kubernetes value |
| --- | --- |
| Gateway | `https://openshell.openshell.svc.cluster.local:8080` |
| PostgreSQL | `veoveo-postgres-rw.veoveo-system.svc.cluster.local:5432` |
| Private S3 | `https://seaweedfs-s3.veoveo-system.svc.cluster.local:9000` |
| Native Lore | `grpcs://lore.veoveo-system.svc.cluster.local:41337` |
| Hub public origin/issuer | `https://veoveo.example.com`, `/oauth` |
| Public signed objects | `https://objects.veoveo.example.com` |

Render public origins to the actual installation. Service DNS replaces old host
loopback/Docker aliases only for this Linux placement. Keep every endpoint's
exact binary/port/method/path/provider rule. The `policies/` files use native
policy schema and complete canonicalization, not a second policy language.
Lore's user path remains TLS passthrough; factories retain their existing native
credential-bound gRPC rules. Do not add aliases or disable TLS to make trust pass.

The private Lore leaf must cover `lore.veoveo-system.svc.cluster.local` and its
native Hub deployment endpoint must use that same recipient. The S3 leaf must
cover both its private Service DNS and unchanged public signing authority. Build
the maintained [Lore image](../factory/Lore.Dockerfile) with reviewed **public**
CA files only in its existing `ca` context; include storage and database public
roots needed by its selected clients. Build Factory/Computer with the necessary
public Lore root. Private CA keys never enter images or user workloads. The
gateway's chart-generated workload trust remains native supervisor material.

## Lore and tunnel foundation workloads

Render `lore.toml` privately with the actual Hub issuer/audience/client and existing
trusted hook assignments. The hook is unchanged; an empty map does not prove a
factory protected-branch scenario. Create `lore-settings` from the native TOML,
`lore-server-tls` as a TLS Secret, and `lore-storage` from a private env file with
exact keys `LORE_POSTGRES_URL`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`.
Use `server_lore`, schema/bucket `lore`, and the dedicated native S3 identity.
Its PostgreSQL URI uses the Service DNS with `sslmode=verify-full` and
`sslrootcert=/etc/lore/postgres-ca/postgres-ca.crt`. Create `postgres-client-ca`
ConfigMap key `postgres-ca.crt` in `veoveo-system` from only the CNPG public CA;
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

The private manifest replaces only the explicit image marker with the verified
Linux image digest. A single Lore process owns its native local locks; the small
PVC holds its HOME/temp/local lock paths. Repository storage stays in native
S3/PostgreSQL. A TCP readiness probe proves only a listening socket. Ordinary
Deployment restart handles startup exit; test real native operations and late/
interrupted issuer/storage dependencies separately. No service-order init Job.

For the existing tunnel, create `public-tunnel` ConfigMap from the privately
rendered `tunnel-config.yaml` and a same-name Secret with `credentials.json`.
Create `public-tunnel-ca` ConfigMap with **public** `gateway.crt` and `storage.crt`.
The gateway public CA comes from the chart's retained native PKI output; only its
public certificate is copied. `tunnel.yaml` pins the verified official cloudflared
`2026.10.1` Linux amd64 digest; apply that selected manifest. Never use `noTLSVerify`. Hub routing intentionally
sets its native exposure Host; S3 routing must **not** set `httpHostHeader`.

## Workspace-local trusted service configuration

```sh
openshell workspace create vv-control
openshell workspace create vv-services
openshell workspace create installation
```

Native `CreateWorkspace` persists the workspace, but does not itself instantiate
the Kubernetes namespace. Prepare the **required** provider profiles/custody using
[native control](../computer/NATIVE_CONTROL.md#trusted-preparation-and-attachment)
and the existing registration-provider instructions. The initial native credential
mint/store calls `ensure_workspace`, which creates the managed namespace. Verify
the actual resulting namespace before PVC/Job admission; do not create it yourself
or add a dummy provider merely to trigger it. With gateway ID `veoveo-k3s` and
workspace `vv-control`, the native name is `openshell-veoveo-k3s-vv-control`.
The namespace is OpenShell-owned; no per-workspace Helm release.

Hub's platform-admin service identity needs the existing six workspace/sandbox/
provider permissions plus **`config:read`** for native `GetGatewayInfo` backend
discovery. Both Computer-enabled Hub policy examples now allow that exact POST.
No broader native role, wildcard RPC, credential export or new token worker.

Create `hub-config-bootstrap` in that actual namespace from only reviewed private
`hub.json`, `hub.env`, `computer-policy-v1.yaml` and the selected public
`authorization-server.jwks.json`, using native Secret `--from-file` arguments.
The latter preserves the file-backed JWKS path in `identity/entra.example.json`;
it contains public verification keys, never signing keys. Match these four names
to `hub-config-pvc.yaml`. Before
running the one-time Job, replace its copy-image marker with the verified immutable
Linux utility image, and confirm native sandbox UID/GID are `1000:1000`. It copies
only those files without output and refuses an existing target. The PVC labels
match native default resource admission for workspace `vv-control`.

```sh
kubectl apply -f "$PRIVATE/hub-config-pvc.yaml"
kubectl -n openshell-veoveo-k3s-vv-control wait job/seed-hub-config-v1 \
  --for=condition=Complete --timeout=120s
kubectl -n openshell-veoveo-k3s-vv-control delete job seed-hub-config-v1
kubectl -n openshell-veoveo-k3s-vv-control delete secret hub-config-bootstrap
```

`hub-driver-config.json` mounts this exact admitted claim read-only at
`/etc/veoveo/hub`; do not mount the foundation namespace's PVC. Use separate
workspace-local config claims and exact file lists for Time/Artifact/Media if
needed. No config-copy controller, hostPath, protected `/run` mount, caller
Secret/ConfigMap reference or shared user workspace. Server native config files
may hold their own trusted storage secrets; user workloads receive mediated
provider references instead.

`service-storage.yaml` and `seed-service-configs.yaml` provide the same initial
native `vv-services` bootstrap for Time/Artifact/Media. Use only Artifact's
`storage.env` and Media's `provider.env` in their separate temporary Secrets;
the Jobs copy exact files, not operator directories. Their `*-driver-config.json`
inputs mount each service's own writable state and read-only configuration.
Delete completed seed Jobs and their temporary Secrets. This is initial trusted
storage preparation, never a Helm inventory or runtime reconciliation process.

Artifact's existing native `storage.env` separates its private backend from signed
client URLs (actual access/secret keys remain only in that private file):

```text
VEOVEO_STORAGE_ENDPOINT=https://seaweedfs-s3.veoveo-system.svc.cluster.local:9000
VEOVEO_STORAGE_SIGNING_ENDPOINT=https://objects.veoveo.example.com
VEOVEO_STORAGE_REGION=us-east-1
VEOVEO_STORAGE_BUCKET=artifacts
```

The public signing authority must match the Computer curl policy/tunnel route and
TLS certificate. Never substitute the private Service authority into a signed URL.

### PostgreSQL client trust shared within each workspace

Create `postgres-client-ca` ConfigMap in each actual native workspace namespace
with `postgres-ca.crt` (CNPG public CA) and `storage-ca.crt` (Seaweed public CA),
never their keys. Use the public Seaweed CA certificate from the
`seaweedfs-ca-cert` Secret's **`tls.crt`** key. That chart input key is distinct
from the copied `storage-ca.crt` filename; a Secret containing only `ca.crt` does
not satisfy the chart's native `[grpc].ca` path. Render the utility image in
`postgres-client-ca.yaml`, apply its workspace-local PVCs/one-time Jobs, wait for
completion, then delete those Jobs and temporary source ConfigMaps. The native
driver-config examples mount these approved CA-only claims read-only at
`/etc/veoveo/tls`. They contain no Hub signing/configuration secret and are reused
only inside their named workspace.

Hub's own DB URI **and its `--server-database-url` / corresponding environment
setting used for application registration grants** must retain:

```text
host=veoveo-postgres-rw.veoveo-system.svc.cluster.local
sslmode=verify-full
sslrootcert=/etc/veoveo/tls/postgres-ca.crt
```

Use their distinct actual users/passwords in private URI files; do not place
credentials in argv. Hub issues narrowed application DB grants from that configured
server URI, so fixing only Hub's own URI is insufficient. The application mount
path must match the grant URI. The `database` policy's `tls: skip` means OpenShell
passes PostgreSQL's native TLS through; the PostgreSQL client still verifies the
chain **and Service hostname** with `verify-full`. It does not disable TLS checks.

For Artifact, set native nonsecret `--env SSL_CERT_DIR=/etc/veoveo/tls`. Its existing
AWS rustls client loads that public CA directory **alongside** the injected
`SSL_CERT_FILE` system/supervisor bundle. Keep the injected file intact. The
CA-only claim therefore supplies Seaweed trust without a derivative image or
weaker certificate validation. Prove the deployed S3 handshake/operations; the
mount/environment configuration alone is not transport acceptance.

## Hub and automatic private Computers

`hub.example.json` is the existing typed Hub input with Kubernetes gateway/Lore
addresses, versioned Computer template/policy and empty repository/assignment
lists. Merge installation identity/settings/signing/services by their typed keys;
never concatenate duplicate IDs or replace an existing active catalog with this
bootstrap sample. Run the Hub binary's native validate/import commands against
the **actual complete private input**, not a partial JSON subtree. Initial files
seed only an empty installation; routine updates use the authenticated live
`GET`/`PUT /admin/{profile}/control-plane` path and its current revision.

```sh
openshell profile lint --global --from computer/providers
openshell profile import --global --from computer/providers
openshell sandbox template create computer-v1 --workspace installation \
  --image "$COMPUTER_IMAGE_AT_DIGEST" --cpu 2 --memory 4Gi \
  --env SSL_CERT_DIR=/etc/ssl/certs \
  --env VEOVEO_LORE_REMOTE=grpcs://lore.veoveo-system.svc.cluster.local:41337
```

**No Docker `persistent_workspace` flag or explicit `/sandbox` volume is passed.**
The selected Kubernetes driver creates and seeds the private workspace PVC for
each clone with native ownership. Hub checks the actual selected driver through
`GetGatewayInfo`; it rejects reusable static mounts at/below `/sandbox`. Versioned
read-only config claims outside `/sandbox` remain supported.

Start Hub using the reviewed Hub image/policy/provider and
`--driver-config-json "$(cat kubernetes/hub-driver-config.json)"`, retaining the
existing trusted native command that loads `/etc/veoveo/hub/hub.env` and execs the
Hub binary. Long-running Hub/MCP sandboxes explicitly select
`--restart-policy always`; native default `Never` is unsuitable. Expose the same
Hub `http` service on `8080` with native `bearer_passthrough`. No Helm inventory
owns it. Time/Artifact/Media retain their actual native commands/configs, using
their own trusted read-only claims, Kubernetes policies and distinct native
registration providers; do not share Hub's control identity.

For example, after native provider custody/config preparation, the existing
Hub/Time launch path uses these installation inputs:

```sh
openshell sandbox create --workspace vv-control --name hub --from "$HUB_IMAGE_AT_DIGEST" \
  --policy kubernetes/policies/hub-computer-entra.example.yaml \
  --provider vv-hub-native-control --no-auto-providers \
  --driver-config-json "$(cat kubernetes/hub-driver-config.json)" \
  --restart-policy always --detach -- \
  /bin/sh -ec 'set -a; . /etc/veoveo/hub/hub.env; set +a; exec /usr/local/bin/veoveo-hub serve --allow-loopback-hosts'
openshell service expose --workspace vv-control hub 8080 http \
  --authorization-mode bearer-passthrough
openshell sandbox create --workspace vv-services --name time --from "$TIME_IMAGE_AT_DIGEST" \
  --policy kubernetes/policies/time.yaml --provider vv-time-registration --no-auto-providers \
  --driver-config-json "$(cat kubernetes/time-driver-config.json)" \
  --restart-policy always --detach \
  --env VEOVEO_HUB_ISSUER="$HUB_ISSUER" \
  --env VEOVEO_PUBLIC_BASE_URL=http://time.internal:8800 --env VEOVEO_PORT=8800 -- \
  /usr/local/bin/veoveo-mcp-time
```

Render the actual selected issuer/registration transport together with the exact
native policy authority; the Kubernetes policies select the unchanged example public authority on HTTPS.
`HUB_ISSUER` is the actual existing public issuer, including its configured path;
it is not a new native exposure alias issuer. Verify discovered issuer equality, registration and JWKS before claiming
MCP readiness. Artifact/Media use their corresponding policies/driver-config files
and existing native trusted shell commands loading only their own config file.

Computer creation remains browser admission → native per-owner clone/providers →
ForwardTcp. Hub injects `COMPUTER_HUB_URL` from the selected profile's protected
resource; the image does not hardcode an installation origin. Native provider
custody supplies the user's Hub refresh grant and the approved model credential.
Explicit native Lore login remains user-owned. After native `repository create`
emits a canonical ID, publish that ID and only the acting principal's intended
assignment through Hub's existing typed live control plane; no automatic grants.
Factory repository reads use their distinct configured native provider identity.

Change templates/defaults by versioned native API references and live Hub
publication. Existing Computers keep admitted snapshots unless explicitly replaced;
adding/removing/registering services or changing grants does not restart Hub or
unrelated workloads. Drain/reconnect active operations explicitly. Never replay a
non-idempotent mutation automatically or promise zero downtime.
