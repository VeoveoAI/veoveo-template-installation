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
sandbox creation. Install CloudNativePG from its official pinned operator chart:

```sh
helm repo add cnpg https://cloudnative-pg.github.io/charts
helm upgrade --install cnpg cnpg/cloudnative-pg --namespace cnpg-system \
  --create-namespace --version 0.29.1
```

The official chart index pairs `0.29.1` with operator `1.30.1`; its archive SHA-256
is `b53d3991fe84bcf38767e7702cae78666265427a127a26fff168ab4207d2b1df`.
Confirm that pairing with the downloaded chart before selection. Ordinary controller
readiness is not application dependency ordering.

## PostgreSQL and native storage identities

Build [postgres/Dockerfile](../postgres/Dockerfile), publish its Linux image digest,
and render a private copy of `postgres.yaml` with that exact image. Verify the
image's actual postgres UID/GID; the sample explicitly uses Debian's `999:999`
and must match the selected artifact. This image supplies PostgreSQL `18.6`,
vector `0.8.7` and PostGIS `3.6.4`. Do not replace it with a plain image that omits
the application extensions. Qualify CNPG init/start/restart with this image.

Create `kubernetes.io/basic-auth` Secrets in `veoveo-system`, each with native
`username` and `password` keys, from private files:

| Secret | Username | Responsibility |
| --- | --- | --- |
| `veoveo-owner-db` | `veoveo_owner` | Existing app schema/role bootstrap |
| `openshell-gateway-role` | `openshell_gateway` | Gateway database only |
| `lore-db` | `server_lore` | Lore schema only |

Generate the initial password files without leading/trailing whitespace or a
terminal newline. For example, before creating the corresponding Secret:

```sh
python3 -c 'import pathlib, secrets, sys; pathlib.Path(sys.argv[1]).write_bytes(secrets.token_urlsafe(48).encode("ascii"))' \
  "$PRIVATE/lore-db.password"
```

Apply the same file convention to all managed roles. CNPG `1.30.1` explicitly
calls `strings.TrimSpace(passwordFromSecret)` during
[native managed-role reconciliation](https://github.com/cloudnative-pg/cloudnative-pg/blob/v1.30.1/internal/management/controller/roles/runnable.go#L340-L375),
before applying the password to PostgreSQL. Kubernetes Secret bytes can therefore
contain a terminal LF while the actual database password does not. Do not use
`echo` to write these password files; use a no-newline write and use that same
credential in the Secret and application input. URI-encode the credential when
building a database URL; encoding a stray LF as `%0A` does not remove the mismatch.

The native CNPG Cluster/Database resources own these roles/databases/schema.
Application migrations remain component-owned. No application shares the gateway
role. `veoveo_owner` retains the existing trusted `CREATEROLE` bootstrap contract;
it is not a sandbox or human identity. No passwords appear in manifests or argv.

```sh
kubectl apply -f "$PRIVATE/postgres.yaml"
kubectl -n veoveo-system wait cluster/veoveo-postgres \
  --for=condition=Ready --timeout=300s
```

Use `veoveo-postgres-rw.veoveo-system.svc.cluster.local:5432`. TLS/client trust
and native connection URI settings must be explicit in each private client input.
CNPG's private CA is a public trust input for clients, never its signing key.

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
`b849a25c79177e9306c52a93de82d9dfc879cb92`, published on the fork's `main`,
including the Kubernetes destination-CA
and native Service-DNS fixes atop chart/runtime base
`846de5fa01234a22827915b3f4e5b9092367078b`. Supply all three actual
registry/repository/digest overrides; do not select stock images with the same
tag. The reference gateway was built from `0299a1ec1e9f8305ebfe967c2b033f36ecc7ba18`,
its supervisor from `b849a25c79177e9306c52a93de82d9dfc879cb92`, and the unchanged
sandbox runtime from the base revision. Record each component's own source and
image identity rather than treating a shared tag as matching-source evidence.
Create the standard `registry-pull` dockerconfigjson Secret in the `openshell` namespace from the private pull-only registry config. Native managed
workspace reconciliation copies the referenced image-pull Secret. Renew registry
credentials through that registry's existing operator mechanism, not a new worker.
Create the same pull-only `registry-pull` Secret in `veoveo-system` for private
PostgreSQL and Lore images. Do not copy push/admin credentials into either namespace.

Create `openshell-gateway-db` in `openshell`, key `uri`, with the gateway-only
PostgreSQL URI using `sslmode=verify-full` and
`sslrootcert=/etc/openshell/database-ca/postgres-ca.crt`. Copy only the public
CNPG CA certificate into `postgres-client-ca` ConfigMap key `postgres-ca.crt` in
`openshell`; native chart extraVolumes mount it read-only. This is separate from
CNPG's role-password Secret. Private
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
deployed probes. These source inputs alone do not prove CNPG extension-image
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
| OpenCode | `4bd4891e9e3d67460eb43dcfcdefb5172c1279ff` |
| OpenShell integration | `b849a25c79177e9306c52a93de82d9dfc879cb92` |

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
`mise run docs` on OpenShell b849 (zero errors, three warnings, 6.291 seconds),
and upstream chart render/lint/admission. Runtime checks separately proved Cilium
ingress/egress denial and allowance, CNPG extension/TLS access, private/public
scoped S3 byte round trips and cross-bucket denial. Successful render, image pull
or Pod Ready alone is not application acceptance.

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
not inference or valid media encoding. Host reboot/recovery and the final bounded
`deepinfra/XiaomiMiMo/MiMo-V2.6-Pro` scenario remain pending at this checkpoint.
Do not carry historical Docker or signed-provider evidence forward as proof of
these pending Kubernetes cases.

Evidence is retained outside Git: `/tmp/k3s-foundation-20261019/` on Linux,
`/tmp/k3s-native-factory-qualification-20261020/` on Linux and
`/tmp/k3s-live-admin-20261020/` on the operator Mac. These paths identify receipts,
not portable bootstrap inputs; private artifacts must not be copied into source.
The canonical execution plan owns subsequent acceptance and closeout status.
