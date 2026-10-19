# Standalone embedding installation

Implementation candidate; component checks and deployed Mac acceptance remain gates. The exact pinned EmbeddingGemma 2 prerequisite probes do not prove this installed service. Follow the existing architecture/execution plans for evidence and rollout order: qualify Mac, merge/push reviewed main, then deploy/qualify Linux. No Knowledge, vector database, pgvector or Compose application profile is part of this slice.

## Inspect before changing retained state

Inspect the running Hub image/source, existing catalog/issuer, Work Context, grants, registration secret, signing keys, native bindings and owned volumes before preparation. Concurrent Computers work may have introduced a hosted issuer; do not regenerate the older local catalog over it. Use the helper matching the reviewed running Hub contract. If its retained identity/authority differs, stop for review rather than replacing it. Do not alter existing Time/Artifact/Media selections, grants or callers.

`identity/configure-hub --include-embedding` adds only the approved private binding `http://embedding.internal:8803/embedding/mcp` → `vv-services/embedding:8803`. The existing local and Entra preparation modes support this selection. Approved reruns retain it, together with Time and any selected Artifact/Media. Foreign/duplicate/changed placements fail for review. Embedding is independent of Artifact/Media and adds no user/client/role, billed risk or priced-provider claim. Native registered requirements still own read/write authorization.

Prepare with the existing installation's complete ordinary arguments plus `--include-embedding`; do not choose a different identity mode just to add the binding. Validate the prepared config and compare all retained fields before activation. Only the binding should change for an already-configured installation. Native upstream bindings are captured at Hub startup, so a controlled native Hub restart is required. Preserve its actual current image, public/application issuer, provider attachments, config/state mounts and exposure. Verify the same keys/issuer/grants and Time/Artifact/Media paths afterward. The helper does not restart or deploy anything.

## Component source and checks

Use clean exact reviewed revisions for both new components; an OCI revision label does not select source. Record source hashes and immutable image IDs. Builder commands are owned by the components:

```sh
docker build --target check --progress=plain ../veoveo-mcp-embedding
docker build --target check --progress=plain ../veoveo-runtime-embedding
docker build --target runtime --build-arg VERSION=0.1.0 --build-arg REVISION="$EMBEDDING_MCP_REVISION" -t veoveo/embedding:native ../veoveo-mcp-embedding
```

The deterministic runtime check uses synthetic fixtures without model packages/cache/GPU. It supports Mac Docker Linux/arm64 and Linux/amd64; neither proves inference. Its README owns exact uv/Python pins, canonical HF snapshot prefetch and the native launchd template. No request downloads a model. Record model revision, cache snapshot/file hashes, runtime source/package identity and actual forward device/dtype separately from semantic processing identity.

## Mac runtime and credential custody

Install the reviewed runtime in a persistent user-owned directory with its uv0.13.1 environment and prefetched canonical cache. Adapt its native LaunchAgent template to that user's actual interpreter/working directory/cache/private token paths and reviewed40-character source revision. No sudo or fixed root-owned path is required. Bind only loopback127.0.0.1:8804 initially, with explicit mps and fallback disabled. launchd owns lifecycle; no custom launcher or restart daemon is introduced.

Prepare one random runtime bearer privately, file mode0600/directory0700, readable by the runtime user. The runtime reads the file; the native provider stores the same secret. Neither component receives the other's Hub/database signing material. Import only the designated values in a private subshell, with command tracing disabled:

```sh
(
  set -a
  . ./state/openshell/hub/hub.env
  set +a
  openshell profile import --workspace vv-services --file openshell/providers/embedding-registration.yaml
  openshell provider create --workspace vv-services --name vv-embedding-registration --type vv-embedding-registration --credential VEOVEO_REGISTRATION_SECRET
)
(
  VEOVEO_EMBEDDING_RUNTIME_TOKEN="$(cat "$EMBEDDING_RUNTIME_TOKEN_FILE")"
  export VEOVEO_EMBEDDING_RUNTIME_TOKEN
  openshell profile import --workspace vv-services --file openshell/providers/embedding-inference.yaml
  openshell provider create --workspace vv-services --name vv-embedding-inference --type vv-embedding-inference --credential VEOVEO_EMBEDDING_RUNTIME_TOKEN
)
```

Inspect matching existing profiles/providers first; do not replace foreign resources or rotate an existing bearer incidentally. Real credentials stay in private operator files/provider custody, never argv, tracked source, transcripts or logs. The MCP sees native opaque references. Its registration reference is bound to POST /oauth/register; the inference reference is bound only to GET /capabilities, GET /readyz and POST /embeddings by the admitted binary.

Qualify native `host.openshell.internal:8804` reachability and path-bound credential injection from an otherwise equivalent restricted probe before creating the production MCP. A supported host alias is not proof that a loopback listener is reachable. If it fails, report the actual native network evidence and decision needed; do not broaden the listener, open a public port or add a bridge/controller automatically. Verify unauthenticated and wrong-credential calls fail without GPU work and a non-admitted binary/path cannot obtain the bearer.

## MCP workload

`embedding.yaml` is the Mac host-provider policy. It admits only native Hub registration/metadata/token paths, three private runtime paths and Core PostgreSQL for `/usr/local/bin/veoveo-mcp-embedding`. Verify its metadata paths/origin against the currently retained Hub configuration before use, especially after concurrent hosted-issuer changes; do not conceal mismatches with a blanket REST rule. It has no S3/Artifact/network model download or cache mount.

Create/reuse only the owned `vv-embedding-state` volume with workspace labels, holding the normal native registration key directory, UID10001 and private key mode0600. No text/vector state is stored:

```sh
docker volume create --label openshell.ai/sandbox-attachable=true --label openshell.ai/sandbox-attachable-workspace=vv-services vv-embedding-state
openshell sandbox create --workspace vv-services --name embedding --from "$EMBEDDING_MCP_IMAGE_ID" \
  --policy openshell/embedding.yaml --provider vv-embedding-registration --provider vv-embedding-inference --no-auto-providers \
  --driver-config-json "$(cat openshell/embedding-mounts.json)" --restart-policy on-failure --detach \
  --env VEOVEO_HUB_ISSUER="$EMBEDDING_HUB_ISSUER" \
  --env VEOVEO_PUBLIC_BASE_URL=http://embedding.internal:8803 --env VEOVEO_PORT=8803 \
  --env VEOVEO_KEY_DIRECTORY=/var/lib/veoveo/embedding/keys \
  --env VEOVEO_EMBEDDING_RUNTIME_URL=http://host.openshell.internal:8804 \
  --env VEOVEO_EMBEDDING_DEVICE=mps -- /usr/local/bin/veoveo-mcp-embedding
```

Set `EMBEDDING_HUB_ISSUER` to the existing verified issuer, not a newly invented origin. Match policy metadata/token paths to it under review before running. Do not pass real secrets using --env. There is no public embedding exposure: Hub reaches the MCP over native ForwardTcp at its approved logical binding. Preserve the normal SDK registration and Core-owned login; no direct SQL into other domains, new DB extension or infrastructure service is needed.

## Linux native CUDA placement, after main merge

Use the published reviewed runtime source and its pinned Linux/amd64 production target only after Mac acceptance and main merge/push. The private runtime uses port8804, uid10001 and official cu130 wheels, with no CPU fallback. Inspect the actual gateway/driver capabilities, resource admission settings, Docker CDI inventory, approved device selector, model memory and driver compatibility. A CLI version string is not capability proof.

Native OpenShell accepts `--gpu 1` and driver config `{docker:{cdi_devices:["nvidia.com/gpu=0"],mounts:[...]}}`; the exact example is `embedding-runtime-mounts.json`. The selected device must exist in the actual deployment inventory. Gateway resource admission must allow GPU and validated driver config while bind mounts remain disabled. Native resource claims supply approved GPU filesystem paths. There is no GPU broker, driver patch or raw Docker socket in the workload.

Create the workspace-labelled named volumes `vv-embedding-cache` and `vv-embedding-runtime-config` only after inspecting existing ownership. Populate the native HF cache outside the serving workload. Mount it read-only at /var/cache/huggingface, traversable/readable by uid10001; put only the bearer file in the config volume, owned by uid10001 and mode0600. The runtime policy admits no outgoing network and cannot download a missing snapshot.

```sh
openshell sandbox create --workspace vv-services --name embedding-runtime --from "$EMBEDDING_RUNTIME_IMAGE_ID" \
  --gpu 1 --policy openshell/embedding-runtime.yaml --no-auto-providers \
  --driver-config-json "$(cat openshell/embedding-runtime-mounts.json)" --restart-policy on-failure --detach \
  --env VEOVEO_EMBEDDING_TOKEN_FILE=/etc/veoveo/embedding/runtime.token -- \
  /opt/embedding/.venv/bin/python -m veoveo_runtime_embedding
openshell service expose --workspace vv-services embedding-runtime 8804 http --authorization-mode bearer-passthrough
```

Require the returned private exposure to match `https://vv-services--embedding-runtime--http.openshell.localhost:17670` and retain ordinary CA/hostname checks. Use `embedding-linux.yaml` for the MCP and set its runtime URL to that private exposure and device to cuda. Attach the same path-bound inference provider. This is private native ingress, not public DNS. Only Hub is the public platform entry point. Record actual CDI device and lower model-forward GPU placement/FP32, not merely final output tensor placement.

## Acceptance boundaries

First run component checks, then native provider/network/registration/readiness probes, then real pinned-model service calls through Hub. Use at least text retrieval, code retrieval, mixed purposes, optional titles and real tokenizer limits. Include 2x8192 and 1x8192+7short scenarios; record actual item order, finite768d/unit norms, model/processing/runtime identities, token usage and measured timings. Check wrong credentials/permissions, token overflow with no GPU dispatch, busy without queuing, timeout/disconnect retaining the slot, and successful actual versus rejected zero/ambiguous estimated Core usage. No automatic inference replay.

Exercise existing Time/Artifact/Media paths before/after the controlled Hub restart, plus runtime/MCP dependency restart without identity drift. Keep deterministic synthetic receipts, real GPU prerequisites, deployed Mac acceptance, later Linux CUDA acceptance and final selected OpenCode MiMo acceptance separate. Make no OpenCode calls during iteration. Results do not establish vector storage/search or general GPU scheduling.
