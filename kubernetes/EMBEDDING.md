# Embedding on native OpenShell Kubernetes

These are Linux/amd64 installation inputs for the full multimodal embedding
replacement, not a sandbox inventory. Kubernetes owns the GPU device plugin and
trusted bootstrap PVCs/Job. OpenShell's existing Kubernetes driver owns the
runtime and MCP sandboxes, their workspace PVCs, supervisors and service exposure.
Mac development/MPS and the disposable Docker CUDA probe remain separate evidence.
Follow [WORKLOADS.md](WORKLOADS.md) for current foundation, native custody and live
Hub publication; the older Docker embedding guide is not this deployment path.
Integrate these additions with the current foundation, not the earlier shared
values from this worktree's base. Preserve its native destination-CA staging
(`additional_ca_bundle`, gateway fix `f39b1fee` and subsequent checked fixes),
both existing CA mounts, and the native-service alias DNS supervisor fix
`b849a25c`. Verify the selected gateway/supervisor image receipts and new workload
generation before relying on those paths; do not replace shared values or restart
unrelated workloads to install embedding.

## GPU prerequisite

Use the official NVIDIA device-plugin chart **0.20.1**. Its archive SHA256 is
`b705142f03ac17906bfcface9319e2f7b9a570d2af7f04f23252723ce2c0bb8e`.
[Official release](https://github.com/NVIDIA/k8s-device-plugin/releases/tag/v0.20.1),
[chart values](https://github.com/NVIDIA/k8s-device-plugin/blob/v0.20.1/deployments/helm/nvidia-device-plugin/values.yaml)
and [K3s native NVIDIA runtime](https://docs.k3s.io/advanced#nvidia-container-runtime)
define the selected foundation integration. No OpenShell driver patch is needed.

`nvidia-device-plugin.values.yaml` pins the publisher's Linux/amd64 image manifest
`sha256:d279ecdb47b252c4cd96f4224650e9808c36012f6979570c9611ddbc5b0749a7`.
The publisher multiarch index is
`sha256:27c1b2553a690ca3d29889dcd61df97525d46f58272c0be8dc63c2b18f120075`;
do not substitute the ARM64 manifest. The chart has no `image.digest` field; its
native `image.tag` supports the rendered `v0.20.1@sha256:...` reference.

Retain the official GPU node affinity. After verifying the actual NVIDIA node,
the trusted operator may label **that node only** `nvidia.com/gpu.present=true`.
This is the chart's manual-label branch; NFD/GFD are disabled. RuntimeClass
`nvidia`, the node driver/toolkit and nonzero `nvidia.com/gpu` capacity/allocatable
must be measured before sandbox admission. A RuntimeClass alone proves no device.
The application requests one GPU through native `--gpu 1`; its driver config
selects `pod.runtime_class_name: nvidia`. MCP requests no GPU. No gateway-wide
default RuntimeClass change is required.

The official chart uses host device/kubelet mounts as a trusted foundation
component. Application configs add no hostPath. Chart 0.20.1 also renders an
MPS-control DaemonSet unconditionally with the plugin; it requires the additional
`nvidia.com/mps.capable=true` node label. No MPS/time-sharing config or that label
is selected here. Verify zero MPS-control Pods and no sharing resource before
acceptance. Do not add an ignored `mps.enabled` field or filter upstream manifests.

## Workspace and trusted bootstrap

Prepare the existing `vv-services` native workspace and its required registration
and inference provider custody first. Verify its OpenShell-created namespace;
these templates use `openshell-openshell-vv-services`, matching gateway ID
`openshell`. Render namespaces to the actual admitted gateway ID. Do not create a
competing namespace, copy the old Docker gateway selector or add a dummy provider.

`embedding-storage.yaml` adds only three operator-approved, workspace-local claims:

| Claim | Runtime attachment | Purpose |
| --- | --- | --- |
| `embedding-state-v1` | MCP read/write | Native registration signing keys/state |
| `embedding-runtime-config-v1` | GPU runtime read-only | Existing private `runtime.token` |
| `embedding-model-cache-v1` | GPU runtime read-only | Canonical prefetched model cache |

The 1 GiB state/config and 32 GiB cache requests are installation allocations,
not Google model limits. Keep the existing approved `postgres-client-ca-v1` claim
for MCP; do not redefine shared storage inputs. The selected native service
UID/GID is **10001:10001**, from the effective gateway/workspace configuration.
Current foundation values explicitly select the same sandbox UID/GID 10001,
matching its corrected Factory/Computer seed ownership. The gateway Pod's own
user does not select workload identity. Confirm the effective native settings
and selected image owner, then render bootstrap files to that
actual selected service identity; keep `/opt/embedding` root-owned and read-only,
and let the driver own `/sandbox`. Never mount a caller-selected Secret/ConfigMap or protected `/run`
path through driver config. Workspace operators with approved PVC attachment
authority remain trusted; these labels are not per-service secret isolation.

The unchanged runtime image owns its empty `/sandbox` as UID 10001 mode 0700,
matching the selected native service UID. No image permission change is needed.
Verify actual workspace seeding without granting root/capabilities or adding a
shared `/sandbox` mount.

The runtime token file and existing `vv-embedding-inference` provider must contain
the same already prepared private credential. Use native provider custody; real
values never enter sandbox argv or reports. Initial token copy uses only:

```sh
kubectl apply -f "$PRIVATE/embedding-storage.yaml"
kubectl -n "$SERVICES_NAMESPACE" create secret generic embedding-runtime-config-bootstrap \
  --from-file=runtime.token="$PRIVATE/runtime.token"
kubectl apply -f "$PRIVATE/seed-embedding-config.yaml"
kubectl -n "$SERVICES_NAMESPACE" wait job/seed-embedding-runtime-config-v1 \
  --for=condition=Complete --timeout=120s
kubectl -n "$SERVICES_NAMESPACE" delete job seed-embedding-runtime-config-v1
kubectl -n "$SERVICES_NAMESPACE" delete secret embedding-runtime-config-bootstrap
```

Both seed Jobs pin public official Python 3.14.6 Linux/amd64 utility image
`docker.io/library/python@sha256:b921fe7e7522f828d45197a47656ec465a9b15689b27fa8e1fba2864fca5b967`.
This is the amd64 child of the runtime's reviewed Python base index
`sha256:7bec7ddcddeff7975d6ba9b4be7dd6f6b2f55e7491539145e2978f7f97ce9144`;
the verified utility supplies `/bin/sh` and GNU tar 1.35. It needs no private
registry credential. The token Job copies one nonempty file as UID/GID 10001,
mode 0600, refuses an existing
target, has no retry and emits no value. It does not reconcile or rotate custody.
If a partial Job fails, inspect its exact state before any explicit new attempt.

Populate the model claim separately from the existing trusted, hash-verified
**canonical** cache (approximately 1.52 GB for this snapshot). Copy only the exact
snapshot and its complete referenced relative-link chains and terminal blobs under
`HF_HOME/hub/`. The reviewed Linux cache includes two-hop links for
`model.safetensors` and `tokenizer.json`: snapshot link → model-repository blob link
→ selected shared `hub/blobs/` file. Copying only the model-repository tree would
leave those links broken. Preserve the canonical layout and every relative link;
do not flatten or rewrite them. Make files readable by 10001:10001; exclude
unreferenced blobs, unrelated caches and operator token files. Exact revision:
`914f7f89142e33e77833254d9c9b90c3cef7303b`. Compare the copied source file hashes
and resolve that revision with native `huggingface_hub.snapshot_download` using
`local_files_only=True` before admission. Mount the populated claim at
`/var/cache/huggingface` as HF_HOME, read-only, with HF offline. The trusted
one-time PVC transfer is an operator bootstrap boundary to finalize with the
foundation owner/builder. `seed-embedding-cache.yaml` supplies only that
one-time standard tar receiver; no hostPath, downloader, new HF credential flow,
model-weight image or runtime controller is introduced.

Prepare a private standard tar archive from an operator-reviewed member list.
Every member must be beneath this exact model's `snapshots/<revision>/` or belong
to a complete referenced chain terminating inside `HF_HOME/hub/`, including only
the selected shared `hub/blobs/` targets. Include every snapshot file/link,
intermediate relative link and terminal file. The reviewed Linux inventory has
30 members: two snapshot directories, 13 snapshot links, two intermediate links
and 13 terminal files. Its receipt is
`state/embedding-multimodal-20261019/k8s-cache-bootstrap-dryrun-20261020/linux-cache-inventory-final.stdout.json`
(SHA256 `2c8695298f9c6824b94e8cae27ccfef0673015dc81ad66ecff980e7bf78f345d`).
Preserve relative symlinks, and include no absolute/traversing member names,
unrelated revision/blob, `HF_HOME/token` or operator files. Verify all link chains
resolve within the copied canonical hub tree. The file list and tar SHA256 are
operator evidence, not a second model format. With a reviewed NUL-delimited
native tar member list relative to the canonical HF_HOME:

```sh
tar --create --file="$CACHE_ARCHIVE" --directory="$HF_CACHE_SOURCE" \
  --no-recursion --null --verbatim-files-from --files-from="$REVIEWED_CACHE_MEMBERS_NUL"
kubectl apply -f "$PRIVATE/seed-embedding-cache.yaml"
kubectl -n "$SERVICES_NAMESPACE" get pods \
  -l batch.kubernetes.io/job-name=seed-embedding-model-cache-v1 -o wide
```

Select the exact single Pod owned by that Job as `CACHE_POD`; verify its owner,
image and UID/GID before attaching. Native Pod readiness is enough for this input
phase: tar is waiting on stdin. No marker or sleeping server is involved.

```sh
kubectl -n "$SERVICES_NAMESPACE" wait "pod/$CACHE_POD" \
  --for=condition=Ready --timeout=120s
kubectl -n "$SERVICES_NAMESPACE" attach -i "pod/$CACHE_POD" -c cache < "$CACHE_ARCHIVE"
kubectl -n "$SERVICES_NAMESPACE" wait job/seed-embedding-model-cache-v1 \
  --for=condition=Complete --timeout=900s
```

The container uses native `stdin: true`, `stdinOnce: true`, `tty: false`. Its input
waits for the first attach and closes on that attachment's disconnect, per the
[native Container API](https://github.com/kubernetes/api/blob/v0.35.0/core/v1/types.go#L2918).
The operator uses [native kubectl attach](https://kubernetes.io/docs/reference/kubectl/generated/kubectl_attach/).
GNU tar
exits on archive completion; malformed or incomplete member data may fail, but a
zero exit alone does not prove the intended archive was fully received. Hold
exclusive bootstrap ownership of the claim until verification; do not admit the
runtime or another writer meanwhile. The Job refuses a nonempty claim, has
`backoffLimit: 0` and a 900-second overall deadline. Confirm
actual container exit zero and Job Complete, then compare copied bytes/hashes,
relative links, UID10001 readability and native `local_files_only=True` cache
resolution using the reviewed runtime image before admission. An attach exit
alone is not completion or byte-integrity proof. Preserve failed/partial state
for explicit operator review; do not automatically reattach, replay or delete a
partially populated claim. Remove the completed Job after checks. No serving
process may populate or repair the cache.

## Exact policy, identity and routing

Render only installation authorities/profile selectors in private policy copies:

| Connection | Required native boundary |
| --- | --- |
| MCP → Hub | Exact issuer/registration/token paths and `artifact__create_download`; native delegated JWT on the MCP group |
| MCP → runtime | Exact native exposure `/capabilities`, `/readyz`, `/embeddings`, bound to `vv-embedding-inference` |
| MCP → Core DB | Private PostgreSQL Service; native TLS passthrough plus granted URI `verify-full` and mounted public CA |
| Runtime → media bytes | Exact Artifact **signed object origin**, TCP/443 passthrough, ordinary public TLS trust |

The sample native runtime exposure uses port 8080, matching the current foundation
gateway convention. Verify the actual returned exposure authority and render the
runtime URL and policy together. Select `bearer-passthrough`; stripping the native
inference Authorization header breaks authentication. Runtime holds no Hub
identity, provider API key or storage credential. Native CPU decoding is
preprocessing; inference remains full-model CUDA FP32 without CPU fallback.

Set `VEOVEO_EMBEDDING_STORAGE_ORIGIN` to the exact signed URL origin **with an
explicit port**, for example `https://objects.veoveo.example.com:443`. This is not
the private S3 Service endpoint. Preserve signed URL host/path/query; never rewrite
it. `policies/embedding-runtime.yaml` passes TLS through to that exact host and
the verified Python ELF path. Runtime 2a3 uses HTTPX `trust_env=False`, so
`SSL_CERT_FILE`/proxy environment is not a custom-CA solution. This selected path
requires an ordinary publicly trusted certificate. A different trust/mediation
design needs review before rollout; no verification bypass is provided.

Retain native `vv-embedding-registration` and `vv-embedding-inference` profiles
and approved provider custody. Registration and inference use separate exact
credential bindings. Do not attach a member/admin provider to the MCP delegation
group, broaden declared uses, or deploy the older Hub over current live-binding
work. The current qualified Hub and Artifact must retain the nested SDK chain.

After foundation readiness and review, use immutable runtime/MCP image digests,
the two new native driver configs and privately rendered policies:

Select the maintained native CLI, its existing authenticated configuration home
and the verified current gateway from trusted installation inputs. Do not use the
stock CLI or retired default gateway. These required selectors contain no token;
reuse existing native credential custody without exporting credentials or
creating a new login/provider. Refuse inherited endpoint or insecure-TLS overrides
that could redirect the selected context. Set all three inputs before launch:

```sh
: "${NATIVE_OPENSHELL_CLI:?set the maintained native CLI path}"
: "${NATIVE_CLI_CONFIG_HOME:?set the existing native CLI configuration home}"
: "${NATIVE_GATEWAY:?set the verified current native gateway name}"
test -x "$NATIVE_OPENSHELL_CLI" || exit 1
test -d "$NATIVE_CLI_CONFIG_HOME" || exit 1
test -z "${OPENSHELL_GATEWAY_ENDPOINT:-}" || exit 1
test -z "${OPENSHELL_GATEWAY_INSECURE:-}" || exit 1
export XDG_CONFIG_HOME="$NATIVE_CLI_CONFIG_HOME"

"$NATIVE_OPENSHELL_CLI" --gateway "$NATIVE_GATEWAY" sandbox create --workspace vv-services --name embedding-runtime \
  --from "$RUNTIME_IMAGE_AT_DIGEST" --gpu 1 --cpu "$RUNTIME_CPU" --memory "$RUNTIME_MEMORY" \
  --policy "$PRIVATE/embedding-runtime.yaml" --no-auto-providers \
  --driver-config-json "$(cat kubernetes/embedding-runtime-driver-config.json)" \
  --restart-policy always --detach \
  --env VEOVEO_EMBEDDING_DEVICE=cuda \
  --env VEOVEO_EMBEDDING_TOKEN_FILE=/etc/veoveo/embedding/runtime.token \
  --env VEOVEO_EMBEDDING_STORAGE_ORIGIN="$SIGNED_OBJECT_ORIGIN_WITH_PORT" -- \
  /opt/embedding/.venv/bin/python -m veoveo_runtime_embedding
"$NATIVE_OPENSHELL_CLI" --gateway "$NATIVE_GATEWAY" service expose --workspace vv-services embedding-runtime 8804 http \
  --authorization-mode bearer-passthrough
"$NATIVE_OPENSHELL_CLI" --gateway "$NATIVE_GATEWAY" sandbox create --workspace vv-services --name embedding \
  --from "$MCP_IMAGE_AT_DIGEST" --policy "$PRIVATE/embedding.yaml" \
  --provider vv-embedding-registration --provider vv-embedding-inference --no-auto-providers \
  --driver-config-json "$(cat kubernetes/embedding-driver-config.json)" \
  --restart-policy always --detach \
  --env VEOVEO_HUB_ISSUER="$HUB_ORIGIN" \
  --env VEOVEO_PUBLIC_BASE_URL=http://embedding.internal:8803 --env VEOVEO_PORT=8803 \
  --env VEOVEO_EMBEDDING_RUNTIME_URL="$NATIVE_RUNTIME_URL" \
  --env VEOVEO_EMBEDDING_DEVICE=cuda -- \
  /usr/local/bin/veoveo-mcp-embedding
```

`HUB_ORIGIN` is the actual public Hub origin, for example
`https://veoveo.example.com`, whose registered-server discovery advertises
`/oauth/register` and `/jwks.json`. Do not append `/oauth` to this service setting.
The separate browser/native Lore authorization-server issuer retains its `/oauth`
suffix. Match the current foundation's exact issuer, registration policy and
native provider custody; verify discovery issuer equality before MCP registration.

Select explicit CPU/RAM allocations after the measured full-model CUDA probe;
defaults do not establish sufficient capacity. The runtime image embeds its exact
source revision; the existing canonical command keeps that identity. Native MCP
registration uses its own persistent key directory and narrowed Core DB grant.
Publish only the logical endpoint `http://embedding.internal:8803/embedding/mcp`
to `vv-services/embedding:8803` in the **fresh complete** current Hub control plane
through its qualified live API and revision comparison. Preserve other fields and
workload identities. No Hub restart, old seed regeneration, public MCP ingress or
Helm sandbox record is required for that live binding.

## Builder checks and remaining gates

Offline source preparation is not API-server or deployed acceptance. Using pinned
Helm 3.22.1, download the official chart and compare the archive hash above:

```sh
helm pull nvidia-device-plugin --repo https://nvidia.github.io/k8s-device-plugin \
  --version 0.20.1 --destination "$EVIDENCE"
helm lint "$EVIDENCE/nvidia-device-plugin-0.20.1.tgz" --namespace kube-system \
  -f kubernetes/nvidia-device-plugin.values.yaml
helm template nvidia-device-plugin "$EVIDENCE/nvidia-device-plugin-0.20.1.tgz" \
  --namespace kube-system -f kubernetes/nvidia-device-plugin.values.yaml \
  > "$EVIDENCE/nvidia-device-plugin.rendered.yaml"
kubectl apply --dry-run=server -f "$EVIDENCE/nvidia-device-plugin.rendered.yaml"
kubectl apply --dry-run=server -f "$PRIVATE/embedding-storage.yaml"
kubectl apply --dry-run=server -f "$PRIVATE/seed-embedding-config.yaml"
kubectl apply --dry-run=server -f "$PRIVATE/seed-embedding-cache.yaml"
```

Require exact amd64 image references, official GPU affinity, no NFD/GFD, and a
dormant MPS-control DaemonSet without eligible nodes. Validate both driver JSON
files and both policies through the current native gateway's typed admission
before launch; Kubernetes dry-run does not validate OpenShell policy/driver JSON.
Do not invent a standalone `openshell policy validate` command. No shared chart,
service-storage or existing application file is changed by these inputs.

Then separately prove native GPU allocation/actual CUDA forwards, read-only cache
and token files, registration/discovery, Artifact byte identity, ordered multimodal
vectors/coverage/tokens, usage, real ACL refusal, composition and service restart
recovery. Reuse frozen media/options/8192 recipes and MPS baselines. The checks
follow the coordinator's
[Linux CUDA qualification decision](../../veoveo-docs/OPENSHELL_HARDCUT_PLAN.md#multimodal-embedding-replacement--2026-10-19).
Identical-feature numerical comparison retains maximum component difference
0.0001 and cosine 0.9999. Native cross-platform cases retain exact source/model/
settings, coverage, tokens, cardinality/dimensions, finite normalized vectors and
actual full-model GPU placement, with cosine at least 0.9999; still report their
maximum component difference and original comparison verdict. Deployed Linux
calls additionally meet both original numerical thresholds against retained
native Linux baselines. The original native failure remains recorded; the
controlled pass proves one joint full-tower input only. This explicit acceptance
design correction follows observed different native video inputs and changes no
serving transform, precision or threshold number. Preserve failed attempts
and never replay ambiguous inference automatically. Component checks, disposable
CUDA qualification, native K3s deployment and final client acceptance are distinct.
