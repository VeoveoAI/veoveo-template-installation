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
`seaweedfs-s3-cert`. The chart mounts its native `tls.crt`, `tls.key`/`ca.crt`
paths. Issue the component/service SANs required by the selected chart from the
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

S3 clients use HTTPS on `seaweedfs-s3.veoveo-system.svc.cluster.local:9000`.
Internal master/volume/filer data and metadata use their explicit local-path PVCs;
there is no hostPath. The chart's native network policies admit its peers and the
S3 port from foundation Pods/native supervisors; actual egress still requires
the workload's native policy. Preserve public SigV4 Host/method/path/query/headers
through the tunnel; never rewrite the public object hostname to Service DNS.

## Native OpenShell gateway

Use the maintained chart source at OpenShell
`846de5fa01234a22827915b3f4e5b9092367078b`, together with gateway, supervisor
and sandbox-runtime Linux images built from that same revision. Supply all three
actual registry/repository/digest overrides; do not select stock images with the
same tag. Create the standard `registry-pull` dockerconfigjson Secret in the
`openshell` namespace from the private pull-only registry config. Native managed
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
only approved private config from a temporary native Secret, sets the actual
sandbox UID/GID ownership, and exits. Mount that admitted PVC read-only outside
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
