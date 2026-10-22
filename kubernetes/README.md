# Linux Kubernetes foundation

These are foundation inputs, not an inventory of application sandboxes. The
Linux-only target uses K3s `v1.36.5+k3s1`, Helm `v3.22.1`, Cilium `1.20.3`,
Agent Sandbox `v1.0.6`, CloudNativePG `1.30.1` and the official SeaweedFS chart
`4.48.0` (server `4.48`). Mac remains native development and component/integration
testing; do not create a Mac cluster. Single-node PVCs provide restart persistence,
not multi-host HA or a backup. Development data may be reset and reseeded.

Use an operator-owned directory with mode `0700` for private values, rendered
files and secrets (`umask 077`). Never print resolved Helm values, connection URIs,
Secret data or container environments. The committed replacement markers must be
replaced in private inputs before admission. They are deliberately not runnable
image tags or credentials. Linux builds Linux images; publish and select immutable
`repository@sha256:...` images. Registry/account details and credential renewal
belong to the actual installation, not this generic template.

## Cluster and controllers

Verify the official release artifacts against the recorded upstream checksums.
The reference verified K3s Linux amd64 binary SHA-256 is
`d73847bcd3c5fccef0115b372e2f9a91f3032dc84bbf71518a4617565294d313`;
its [pinned official installer](https://raw.githubusercontent.com/k3s-io/k3s/v1.36.5+k3s1/install.sh)
has SHA-256 `46177d4c99440b4c0311b67233823a8e8a2fc09693f6c89af1a7161e152fbfad`.
Install the verified binary at `/usr/local/bin/k3s`, then install the configuration
before executing that installer with `INSTALL_K3S_SKIP_DOWNLOAD=true` and
`INSTALL_K3S_VERSION=v1.36.5+k3s1`. Confirm the CIDRs do not overlap host routes.
Keep kube-proxy; the configuration disables Flannel, bundled network policy,
Traefik and ServiceLB. Cilium owns policy enforcement.

```sh
sudo install -d -m 0755 /etc/rancher/k3s
sudo install -m 0600 kubernetes/k3s.yaml /etc/rancher/k3s/config.yaml
helm repo add cilium https://helm.cilium.io/
helm upgrade --install cilium cilium/cilium --namespace kube-system \
  --version 1.20.3 -f kubernetes/cilium.values.yaml -f "$PRIVATE/cilium-node.yaml"
kubectl apply -f kubernetes/namespaces.yaml
```

The private Cilium override supplies `k8sServiceHost`, the actual node API address.
First prove real ingress and egress deny/allow behavior with disposable Pods.
Install the official Agent Sandbox controller/CRDs using
`kubectl apply -k kubernetes/agent-sandbox`. This selects the reviewed `v1.0.6`
core `sandbox.yaml` release manifest (SHA-256
`b995db5839f2bdca3a24b8bbefdec405df662d6dd223eec8f5dc74098c974231`),
not the separate extensions release. The Linux amd64 controller digest is
`sha256:876f9621e0f0e168bcff13aa377880b75fd37e1c8712f2f249e8878c22d06ffd`;
do not substitute the ARM64 manifest. Wait for controller readiness before native
sandbox creation.

## PostgreSQL and native storage identities

PostgreSQL runs as a plain StatefulSet ([postgres.yaml](postgres.yaml)): one
instance, one volume, the `veoveo-postgres-rw` Service every client already names,
and TLS from the installation's own PostgreSQL CA. No operator. Build
[postgres/Dockerfile](../postgres/Dockerfile), publish its Linux image digest, and
render a private copy of `postgres.yaml` with that exact image. Verify the image's
postgres UID/GID; the sample uses Debian's `999:999`. This image supplies PostgreSQL
`18.6`, vector `0.8.7` and PostGIS `3.6.4`. Do not replace it with a plain image
that omits the application extensions.

Its first start runs three scripts from the `veoveo-postgres-initdb` ConfigMap:
[owner.sql](../postgres/initdb/owner.sql) creates `veoveo_owner`
(`LOGIN CREATEROLE CREATEDB`) and `veoveo`;
[postgres-platform.sql](postgres-platform.sql) creates the gateway's and Lore's
logins, the `openshell_gateway` database, the `lore` schema and
`max_connections = 512`; [zz-extensions.sql](../postgres/initdb/zz-extensions.sql)
enables the extensions. Everything after that is plain SQL: Hub creates server
roles and data-environment databases, and settings change with `ALTER SYSTEM` and a
reload, or a restart for the few settings that need one, such as `max_connections`.
`pg_hba.conf` admits TLS with a password to every database, so databases created at
runtime are reachable, and the `postgres` superuser only from inside the pod.

Budget connections for each Core-backed process (a pool of up to 10 plus its
dedicated instance connection, listeners and singletons), the OpenShell gateway (a
10-connection data pool plus a 4-connection mutation-lock pool per replica), Lore,
rollouts, operators and headroom.

Create `kubernetes.io/basic-auth` Secrets in `veoveo-system`, each with native
`username` and `password` keys, from private files:

| Secret | Username | Responsibility |
| --- | --- | --- |
| `veoveo-owner-db` | `veoveo_owner` | Server roles and data-environment databases |
| `openshell-gateway-role` | `openshell_gateway` | Gateway database only |
| `lore-db` | `server_lore` | Lore schema only |

Generate the password files without leading/trailing whitespace or a terminal
newline, and use the same credential in the Secret and in application input.
URI-encode it when building a database URL:

```sh
python3 -c 'import pathlib, secrets, sys; pathlib.Path(sys.argv[1]).write_bytes(secrets.token_urlsafe(48).encode("ascii"))' \
  "$PRIVATE/lore-db.password"
```

Create the installation's PostgreSQL CA and the server certificate for the Service
name. The CA key stays private; only its certificate is a trust input:

```sh
host=veoveo-postgres-rw.veoveo-system.svc.cluster.local
openssl req -x509 -new -nodes -newkey ec -pkeyopt ec_paramgen_curve:P-256 -days 3650 \
  -subj /CN=veoveo-postgres-ca -keyout "$PRIVATE/postgres-ca.key" -out "$PRIVATE/postgres-ca.crt"
openssl req -new -nodes -newkey ec -pkeyopt ec_paramgen_curve:P-256 -subj "/CN=$host" \
  -keyout "$PRIVATE/postgres-server.key" -out "$PRIVATE/postgres-server.csr"
printf 'subjectAltName=DNS:%s\nextendedKeyUsage=serverAuth\n' "$host" > "$PRIVATE/postgres-server.ext"
openssl x509 -req -in "$PRIVATE/postgres-server.csr" -CA "$PRIVATE/postgres-ca.crt" \
  -CAkey "$PRIVATE/postgres-ca.key" -CAcreateserial -days 825 \
  -extfile "$PRIVATE/postgres-server.ext" -out "$PRIVATE/postgres-server.crt"
kubectl -n veoveo-system create secret tls veoveo-postgres-tls \
  --cert "$PRIVATE/postgres-server.crt" --key "$PRIVATE/postgres-server.key"
kubectl -n veoveo-system create configmap postgres-client-ca \
  --from-file=postgres-ca.crt="$PRIVATE/postgres-ca.crt"
kubectl -n veoveo-system create configmap veoveo-postgres-initdb \
  --from-file=owner.sql=postgres/initdb/owner.sql \
  --from-file=platform.sql=kubernetes/postgres-platform.sql \
  --from-file=zz-extensions.sql=postgres/initdb/zz-extensions.sql
kubectl apply -f "$PRIVATE/postgres.yaml"
kubectl -n veoveo-system wait pod/veoveo-postgres-0 --for=condition=Ready --timeout=300s
```

Use `veoveo-postgres-rw.veoveo-system.svc.cluster.local:5432` with
`sslmode=verify-full` and `postgres-ca.crt` as the root. No passwords appear in
manifests or argv. Application migrations remain component-owned; no application
shares the gateway role.

## Logs, image builder and registry

Factory sandboxes need three installation services. [logs.yaml](logs.yaml) runs
Vector on every node and single-node VictoriaLogs, so every container's output
outlives its pod for 7 days. [builder.yaml](builder.yaml) runs BuildKit's
user-namespace variant, which Hub reaches at
`tcp://buildkitd.veoveo-system.svc.cluster.local:1234`, and a plain Distribution
registry at `registry.veoveo-system.svc.cluster.local:5000` with the fixed ClusterIP
`10.43.200.10`:

```sh
kubectl apply -f kubernetes/logs.yaml -f kubernetes/builder.yaml
```

Point the node's container runtime at the registry once, then restart K3s. The
restart takes the API away for a few seconds and restarts no container:

```sh
sudo tee /etc/rancher/k3s/registries.yaml >/dev/null <<'YAML'
mirrors:
  "registry.veoveo-system.svc.cluster.local:5000":
    endpoint:
      - "http://10.43.200.10:5000"
YAML
sudo systemctl restart k3s
```

## SeaweedFS

```sh
helm repo add seaweedfs https://seaweedfs.github.io/seaweedfs/helm
helm upgrade --install seaweedfs seaweedfs/seaweedfs --namespace veoveo-system \
  --version 4.48.0 -f kubernetes/seaweedfs.values.yaml
```

Before installation provide the chart's native external certificate Secrets in
`veoveo-system`: `seaweedfs-ca-cert`, `seaweedfs-master-cert`,
`seaweedfs-volume-cert`, `seaweedfs-filer-cert`, `seaweedfs-client-cert`, and
`seaweedfs-s3-cert`. The CA Secret **must contain the public certificate under
`tls.crt`, not `ca.crt`**: the chart's `[grpc].ca` is
`/usr/local/share/ca-certificates/ca/tls.crt`. Do not include the CA private key.
The leaf Secrets contain `tls.crt` and `tls.key`. For example:

```sh
kubectl -n veoveo-system create secret generic seaweedfs-ca-cert \
  --from-file=tls.crt="$PRIVATE/storage-ca.crt"
```

Issue the component/service SANs required by the selected chart from the
trusted installation CA using the existing TLS recipe; inspect the rendered
commands/volumes before selection. The S3 leaf additionally covers its private
Service DNS name and the unchanged public object hostname. No cert-manager is
required; private signing keys remain trusted operator inputs.

`seaweedfs-s3-identities` has the native key `seaweedfs_s3_config` containing
reviewed native identity JSON. Bootstrap with the existing native Seaweed IAM
administration, create the `artifacts` and `lore` buckets, and retain separate
Artifact/Lore identities and their existing
[Artifact](../openshell/artifact-storage-policy.json) and
[Lore](../openshell/lore-storage-policy.json) policies. Do not use a shared admin
key in applications or replace bucket grants with global permission. Store the
resulting application credentials only in their own trusted Secret inputs.

For identities configured with Seaweed's native `actions` field, the scoped
examples include bucket-root read as well as object operations:

```text
Lore:     ["Read:lore", "Read:lore/*", "Write:lore/*", "List:lore"]
Artifact: ["Admin:artifacts", "Read:artifacts", "Read:artifacts/*", "Write:artifacts/*", "List:artifacts", "Tagging:artifacts/*"]
```

Seaweed `4.48` routes authenticated `HEAD /<bucket>` through
[native `ACTION_READ`](https://github.com/seaweedfs/seaweedfs/blob/4.48/weed/s3api/s3api_server.go#L1041-L1045).
Object-only `Read:<bucket>/*` does not match that bucket-root request, and
`List:<bucket>` is a different action. Lore and Artifact check bucket availability
with HeadBucket; successful object PUT/GET/DELETE alone does not prove this check.
Keep the exact own-bucket read action in the existing identity, without global
`Read` or `Admin`. Artifact also unconditionally calls CreateBucket at startup,
then configures CORS/lifecycle. Seaweed4.48 routes CreateBucket through
[native `ACTION_ADMIN`](https://github.com/seaweedfs/seaweedfs/blob/4.48/weed/s3api/s3api_server.go#L1047-L1048)
before checking bucket existence. The native actions format therefore requires
exact `Admin:artifacts`; an already-created bucket or object-scoped Write alone
is insufficient. This permits administration of that bucket, including its bucket
operations, not IAM administration or access to other buckets. Do not add global
`Admin` or give this action to Lore. These are native action examples, not AWS IAM
statement syntax; retain the separate reviewed fine-grained policies when using
the attached-policy path.

After creating those existing IAM identities and before application use, bootstrap
each bucket once through the native TLS-enabled operator shell. Run inside the
trusted filer Pod so the chart's mounted `/etc/seaweedfs/security.toml` supplies
`[grpc].ca` and `[grpc.client].cert`/`.key`; do not disable TLS or print that file
(it also contains signing material):

```sh
kubectl -n veoveo-system exec -it seaweedfs-filer-0 -- weed shell \
  -master=seaweedfs-master.veoveo-system.svc.cluster.local:9333 \
  -filer=seaweedfs-filer.veoveo-system.svc.cluster.local:8888
```

At the native shell prompt, substitute the **existing configured IAM identity
names**, not access-key IDs or newly invented identities:

```text
s3.bucket.create -name artifacts -owner <existing_artifact_identity_name>
s3.bucket.create -name lore -owner <existing_lore_identity_name>
```

The owner must match the identity name in native S3 IAM configuration. This is
trusted installation bootstrap, not a startup Job or global application `Admin`
grant. It does not replace Artifact's own scoped startup authorization above.
The command leaves an already-existing bucket unchanged; rerunning it does not
reassign ownership. Keep the existing separate bucket policies and credentials.

S3 clients use HTTPS on `seaweedfs-s3.veoveo-system.svc.cluster.local:9000`.
Internal master/volume/filer data and metadata use their explicit local-path PVCs;
there is no hostPath. The chart's native network policies admit its peers and the
S3 port from foundation Pods/native supervisors; actual egress still requires
the workload's native policy. Preserve public SigV4 Host/method/path/query/headers
through the tunnel; never rewrite the public object hostname to Service DNS.

## Native OpenShell gateway

Use maintained OpenShell source
`53e3665811a72dd1555edb18aeb4fedef4158dbd` for the gateway/driver,
including Kubernetes destination-CA, admitted GPU propagation and terminal-runtime
recovery fixes atop the native Service-DNS work and chart/runtime base
`846de5fa01234a22827915b3f4e5b9092367078b`. Supply all three actual
registry/repository/digest overrides; do not select stock images with the same
tag. The reference gateway was built from `53e3665811a72dd1555edb18aeb4fedef4158dbd`,
its selected supervisor from `d7a990083214e8a618f05b1b471aeeac6d9aa82a`, and the unchanged
sandbox runtime from the base revision. Record each component's own source and
image identity rather than treating a shared tag as matching-source evidence.
Its deployed gateway image is
`registry.cloudflare.com/cd9522254303bfb3d2322a0d9b82764f/gateway@sha256:1bdeade25a8239b7c385924915de59f520be3c48322f79ef4be62d7ce5801564`.
The published supervisor selected for new generations is
`registry.cloudflare.com/cd9522254303bfb3d2322a0d9b82764f/supervisor@sha256:f1d8f896daa08d85f8667a595c82544d93cbe73f2d16250387ab484ee5a286d2`.
Its Linux ELF SHA-256 is
`b3e6ce5adb6915d62743d7d933dd192b42cdec254e9e1a37cf91321793c6a7bd`.
The sandbox runtime remains source `846de5fa` and image
`registry.cloudflare.com/cd9522254303bfb3d2322a0d9b82764f/openshell/sandbox@sha256:5177d0c76e2119a09a3af290ebc1da625a1e9fdf3ddcad7485bc973585f1e9bf`.
Source selection or publication does not prove that every existing generation
has adopted the selected supervisor.

The supervisor retains the Kubernetes driver's declared ServiceAccount source.
When either credential in the immutable launch bundle has expired, it uses native
`IssueSandboxToken` followed by `RefreshSandboxToken`, validates the paired
credentials and matching runtime generation, then installs them before log clients
or policy discovery. Fresh bundles skip this exchange: their first launch can
precede persistence of the compute runtime binding required by issuance. Static
Docker/standalone credentials keep their existing path. This changes neither
native Lore authentication nor its Hub-issued grants.

The earlier reboot attempts exposed driver-written suspension of running workloads.
The selected gateway is deployed at reference Helm revision 9. Temporarily Pending
Pods preserve running intent; a terminal supervisor with admitted Pod identities
and enforcement fences intact enters native `always`/`on-failure` replacement.
Before loss publication, stale v1beta1 `Suspended` status projects as unavailable
readiness rather than operator Stop. Missing/deleting Pods, identity/fence failures,
explicit Stop/Delete, `never` and canonical terminal results retain their boundaries.

Both native Pods deliberately use `Never`. Host recovery creates fresh runtime
and Pod identities, retaining PVC-backed files, sessions and durable Tasks; it is
not a same-Pod expired-bundle bootstrap test. In a scoped deployed check, an
unexpected supervisor SIGKILL moved `media-live-qa` from Starting at 12:03:56 UTC
to Ready at 12:04:09 UTC (13 seconds), with restart count 1, no inferred exit code,
a fresh runtime generation and Pods, and successful native exec. Explicit Stop
then stayed Stopped without Pods for 30.23 seconds before an explicit QA-only
Start returned Ready. The foreign GPU workload was unchanged.

The third Linux host reboot on 2026-10-20 was issued at 12:13:32 UTC. SSH returned
at 12:15:34; the 12:17:04 inventory found Hub, Time, Artifact, Media, its synthetic
provider, both Computers and both owned `always` QA runners Ready with restart
count 1, no inferred command exit and fresh runtime generations. No manual native
Start preceded that inventory; the `never` Time QA runner stayed Stopped. All 25
PVC identities and volume bindings matched the preboot inventory. The saved
browser session and authoritative control revision survived; a fresh terminal
WebSocket returned its marker in 941 ms (66 output bytes). This proves native
host recovery and these retained-state checks, not OS PTY-process persistence or
final model acceptance. The retained postboot inventory is
`/home/rozgo/.config/veoveo/k3s/evidence/openshell-gateway-53e-recovery-20261020/postboot.json`,
SHA-256 `7ff98cfe7ec82dab116615cd920fbb117e268c33f999e5858b1f7ef10212b128`.

Create the standard `registry-pull` dockerconfigjson Secret in the `openshell` namespace from the private pull-only registry config. Native managed
workspace reconciliation copies the referenced image-pull Secret. Renew registry
credentials through that registry's existing operator mechanism, not a new worker.
Create the same pull-only `registry-pull` Secret in `veoveo-system` for private
PostgreSQL and Lore images. Do not copy push/admin credentials into either namespace.

Create `openshell-gateway-db` in `openshell`, key `uri`, with the gateway-only
PostgreSQL URI using `sslmode=verify-full` and
`sslrootcert=/etc/openshell/database-ca/postgres-ca.crt`. Copy only the public
PostgreSQL CA certificate into `postgres-client-ca` ConfigMap key `postgres-ca.crt`
in `openshell`; native chart extraVolumes mount it read-only. Private
values select the actual management OIDC issuer/audience/claim mappings. Keep
TLS and mTLS enabled; the chart's idempotent native PKI hook owns gateway/client
certificates and sandbox signing keys. Keep the gateway ID stable across restarts.
Do not export private gateway/bootstrap keys into application workloads.

The selected gateway must include the complete native Kubernetes
`additional_ca_bundle` plumbing through `0299a1ec1e9f8305ebfe967c2b033f36ecc7ba18`;
the supervisor also requires the Service-DNS fix through `b849a25c`. Create the public-only
`platform-destination-roots-v1` ConfigMap in `openshell`, key `ca.crt`, from a
reviewed PEM bundle containing the retained chart gateway CA and current Lore/
Seaweed destination roots. Do not include private keys or unrelated certificates.
The example mounts it read-only at `/etc/openshell/platform-ca` through existing
chart extra volumes and sets the native driver path to `ca.crt` there:

```sh
kubectl -n openshell create configmap platform-destination-roots-v1 \
  --from-file=ca.crt="$PRIVATE/destination-roots.pem"
```

The separately scheduled supervisor does not inherit CAs installed only in a
workload image or mounted only in its filesystem. The driver validates this
bounded PEM input, stages it in the existing immutable supervisor bootstrap
Secret for each generation, and passes its existing `--additional-ca-bundle`
flag. Supervisor upstream TLS and workload trust receive these roots; exact
network policy and hostname verification still apply. No proxy or TLS bypass
is configured. A running generation retains its original roots; recreate only
an affected runtime to select changed roots. When adding private Helm overrides,
retain **both** database-CA and destination-CA mounts: Helm replaces volume lists
rather than appending them. These inputs are not deployed TLS acceptance.

```sh
helm upgrade --install openshell "$OPENSHELL_SOURCE/deploy/helm/openshell" \
  --namespace openshell -f kubernetes/openshell.values.yaml \
  -f "$PRIVATE/openshell-images-and-identity.yaml"
```

The service endpoint is `https://openshell.openshell.svc.cluster.local:8080`.
The gateway uses its distinct PostgreSQL database, outside this Helm release.
Its own authentication bootstrap may require the external issuer online. Local
Mac Keycloak remains external; no authentication bypass or startup-order engine.
Only normal Kubernetes/OpenShell controllers, application retry/reconnect and
restarts recover dependency failures. Test late and interrupted dependencies;
do not infer complete healing from a Ready Pod.

## Runtime ownership and trusted configuration

Create workspaces and versioned templates through the existing authenticated
OpenShell CLI/API, then create Hub/MCP/Computer/factory workloads through those
APIs. No per-workspace Helm release or static workload inventory belongs here.
For each trusted service config PVC, **first create the native workspace**, then
prepare its required provider custody. Native credential mint/store invokes
`ensure_workspace`, creating its managed namespace; `CreateWorkspace` alone only
persists the workspace object. Verify that actual namespace before creating the
operator-approved PVC and bootstrap Job there. Do not create the namespace yourself
or add a dummy provider. PVCs cannot be mounted across namespaces. The Job copies
only approved private config from a temporary native Secret, sets the approved
component file owner, and exits. Mount that admitted PVC read-only outside
`/sandbox`; remove the temporary Secret/Job after successful copy. Do not mount
caller-selected Secrets, ConfigMaps, host paths or protected `/run` paths through
driver configuration, and do not introduce a copy controller.

Computers use the driver's automatic per-sandbox workspace PVC at `/sandbox`.
No explicit shared `/sandbox` claim, interpolation, second copy-up or Helm-owned
Computer PVC is needed. Workload defaults use immutable versioned template/policy
references. Live Hub publication changes future provisioning without replacing
existing Computers. MCP request destinations and repository assignments remain
live typed Hub control-plane data; installation files seed only an empty catalog.
Restarting infrastructure must never overwrite the authoritative active catalog.

Continue with [native workload/bootstrap inputs](WORKLOADS.md) for Lore, tunnel,
workspace-local config/public-CA PVCs and Kubernetes Computer defaults.

## Validation boundary

Render/lint the exact upstream charts and server-side admit ordinary resources
before deployment. For an offline OpenShell render only, disable its live
`agentSandbox.preflight.enabled` check; never disable the deployed preflight.
Builder owns chart rendering, schema checks, immutable image verification and
deployed probes. These source inputs alone do not prove PostgreSQL extension-image
compatibility, Seaweed TLS/IAM, signed bytes, service recovery or a Kubernetes
Computer. Keep deterministic checks, deployed checks and final exact MiMo
acceptance separate. Remove obsolete Linux platform Compose launch paths only
after replacement acceptance; preserve Mac test fixtures and external identity.


## Reference qualification checkpoint — 2026-10-20

This dated checkpoint records the Linux reference installation, not an automatic
acceptance claim for these generic inputs. Exact selected registry image digests
belong in the installation's own record. The maintained source pins are:

| Component | Reviewed source revision |
| --- | --- |
| Hub | `9b83c26e9bdc926b054d089fd87ca26e3cba6f31` |
| Time | `d70ac8e2f9bdfb1c012ee744e5cd119ce4bca1dd` |
| Artifact server | `00c8e93f5ca3ffe9137773754a4945c6d349b3bf` |
| Media | `f01cdb0b772a87e20835f5b88be18910a7785e54` |
| Synthetic Media provider fixture | `4e8504b2c9bc72b0b249a8cf122e29983634d2e8` |
| Native Lore | `6b7a0e598b8fbbaed82c777109135ddb25873bbf` |
| OpenCode | `ed62aad52c80ecdf4297310ce84305aff5717b7d` |
| OpenShell gateway/driver | `53e3665811a72dd1555edb18aeb4fedef4158dbd` |
| OpenShell supervisor | `d7a990083214e8a618f05b1b471aeeac6d9aa82a` |
| OpenShell sandbox runtime | `846de5fa01234a22827915b3f4e5b9092367078b` |

The replacement Computer image is
`registry.cloudflare.com/cd9522254303bfb3d2322a0d9b82764f/veoveo-computer@sha256:12c2ba83bd6f3dea715d1f8ab25ee20dd81dfb8145868f9a193f73cb4ce695d4`,
built from OpenCode `ed62aad52c80ecdf4297310ce84305aff5717b7d`. Both fresh owner
homepages render. Direct mounted Settings and Back to app, adding
`/sandbox/projects`, New session, and browser Back/Forward all retain the Computer
prefix. The final bounded exact-model acceptance also passed, as recorded below.

These are maintained-fork source revisions, not upstream release versions or
reproducible-build claims. The corrected Factory/Computer build uses the UID/GID
10001 installation inputs; an earlier UID-1000 image is not accepted.

The qualified Media runtime digest is
`sha256:2da66a129757e3d54e379bb8a34c2b15da8b768681cd00702229ec44d838a920`;
its matching QA image digest is
`sha256:a6ba2faddd3b3c6320538bbc64d0a37e0e01b001bd7f4e2201685b32428b0c0f`.
Media uses Artifact client revision `58f9fbd2ceab7f267144676284b62bf756945427`,
included in Artifact main `9a535440d989e24021ae172f71f3f5c08aa2703b`.
The deployed Artifact server remains at the separately recorded revision; the
synthetic provider image remains `sha256:398f86bbda316b6cd40b2a88c332fadc6fcc809ca53f91b713fb6322227080d4`.

Deterministic/component evidence includes both Hub Docker check gates (231 passed,
one skipped per platform), focused native CA/DNS tests and strict Clippy,
upstream chart render/lint/admission, and the final OpenShell driver suite
(306 passed), strict driver Clippy (20.04 seconds), and `mise run docs`
(zero errors, three warnings, 8.58 seconds). Existing server infrastructure/restart
filters also passed before the final driver-only projection correction. Runtime
checks separately proved Cilium ingress/egress denial and allowance, CNPG extension/TLS access, private/public
scoped S3 byte round trips and cross-bucket denial. Successful render, image pull
or Pod Ready alone is not application acceptance.

The d7 supervisor source passed 16 Core authentication tests and two existing
server identity-rejection tests. Strict default-feature, all-target Clippy for
Core and the supervisor passed in 26.09 seconds; the Linux release build took
81 seconds. These are source/build gates, separate from live credential renewal
and fresh-generation host recovery. The supervisor's `telemetry` and
`defaults-without-telemetry` features are mutually exclusive, so its valid strict Clippy invocation does not use
`--all-features`.

Deployed Computer checks used two real owners, distinct automatic workspace PVCs,
web navigation, owner isolation and native Stop/Start persistence. Live template
selection affected a new Computer while the existing Computer retained its admitted
specification. Invalid configuration preserved the active revision; an interrupted
replica notification listener reconciled the missed revision without process
replacement. A real Time event subscription delivered its later notification
across a live publication (9.75-second case; 12.381-second operator sequence).
These are measured cases, not a general zero-downtime or arbitrary hot-reload claim.

Native Lore checks used real user login, explicit repository assignments,
allowed/denied clones, withdrawal beyond token expiry and ordinary refresh. An
isolated Factory cloned the immutable published revision and built a dependency-free
synthetic Rust fixture with Cargo `--offline --frozen --release` in 387 ms. Its
native provider renewed automatically; a fresh clone beyond the original expiry
plus JWT leeway passed in 107 ms. The factory had no OpenCode/model provider;
its sandbox, template, provider/profile and workspace were deleted natively while
pre-existing unrelated Pod identities remained unchanged. This proves the stated
synthetic fixture scenario, not model inference or general reproducible builds.

Time's seven routed cases passed in 10.21 seconds; Artifact's six passed in
223.15 seconds, including the 1-GiB multipart/range/release case. Media's six
synthetic deployed cases passed serially in 49.06 seconds. A separate in-flight
Task continued across scoped Hub and Media restarts in 112.42 seconds and returned
65,536 bytes, SHA-256 `c6ee57f09dbd4b0e5c6072ec59f2bbce2decf3ef9b1c695af802f26ae28931ce`.
The initial Media attempt failed two cases: a concurrent run-insert conflict and
an aborted Artifact upload reported as success on an idempotent client retry.
The fixes preserve tenant isolation and reject terminal failed uploads. Media's
policy also lacked its public signed-upload authority; that exact authority is
now admitted with TLS passthrough. The original transfer error was not retained,
so its historical cause is not proved. These are synthetic byte/Task checks,
not inference or valid media encoding. Native host recovery now passes as
recorded above. Postboot terminal verifiers passed for Time (0.94 seconds),
Artifact (1.80 seconds) and Media (1.89 seconds); the synthetic Media output retained the exact byte hash
above. The observed OpenCode Back-navigation escape from the Computer mount is
corrected in the published source pin above: titlebar history records route-relative
paths and lets the mounted router resolve destinations. Image publication, both
owner homepages and actual mounted Settings/Back/New session/browser Back/Forward
acceptance pass. A source commit alone does not replace an existing Computer's frozen
template. Do not carry historical Docker or signed-provider evidence forward as
proof of pending Kubernetes cases.

The final Computer scenario explicitly used
`deepinfra/XiaomiMiMo/MiMo-V2.6-Pro`, with an eight-step/240-second bound and no
model substitution. Session `ses_ea69c1ad3ffeKqVD2BVH1CSrMt` reached terminal idle
with succeeded status in seven recorded model steps, 81.283 seconds
(12:58:23.007–12:59:44.290 UTC). Its authenticated native shell remote repository
metadata command exited 0 and verified repository
`01a1586c89057ea3adcfc848da54077b`. Reads of the existing local checkout matched
published revision `6f6e421befee367e83606f2e0f35b2fdd09b1daab2861fce1263cab31ae11c9f`
and source SHA-256 `6a542b5339909414f1dbdc904b14b6e60e29cadfdf8d61e097fa35d52fd0b110`;
this was not a fresh model-driven remote fetch.
Two routed `resolve_time` calls and `expand_schedule` returned the expected single
09:00–10:00 UTC occurrence with `truncated=false`.

The trace was not error-free: the model first tried `tools.shell` inside CodeMode,
received `Unknown tool`, made two discovery turns, then used the native shell
correctly. The earlier attempt's remote Lore exit 16 came from operator login
using the native exec default auth path instead of the image workload's path;
repeating the native login with the explicit image paths corrected qualification
setup without an application or authorization change. Lore's secure-storage
warning remained nonfatal. These observations are distinct from the deterministic
checks, synthetic Media byte proofs and host-recovery evidence above; they do not
qualify real Media inference, general publication or the separate CUDA work.

Evidence is retained outside Git: `/tmp/k3s-foundation-20261019/` on Linux,
`/tmp/k3s-native-factory-qualification-20261020/` on Linux and
`/tmp/k3s-live-admin-20261020/` on the operator Mac. These paths identify receipts,
not portable bootstrap inputs; private artifacts must not be copied into source.
The canonical execution plan owns subsequent acceptance and closeout status.
