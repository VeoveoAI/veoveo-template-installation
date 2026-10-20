# Mac embedding development fixtures

This guide covers Mac development with a native launchd/MPS runtime and an OpenShell Docker MCP fixture. Linux deployment uses the [Kubernetes embedding guide](../kubernetes/EMBEDDING.md); the former Linux Docker workload path is retired. Components own their contracts and checks; the installation owns trusted placement and configuration. See the [architecture](https://github.com/VeoveoAI/veoveo-docs/blob/main/LORE_OPENSHELL_INTEGRATION.md#embedding-generation) and [execution plan](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md#embedding-generation--2026-10-19) for measured deployment evidence and current gates.

Historical text-only evidence does not qualify the current multimodal contract. The maintained runtime supports ordered text/image/audio/video parts, one vector per item and optional native prefixes and dimensions. The MCP resolves media through Artifact using the caller's Hub authority; the private runtime receives the signed transfer and decodes verified bytes. Follow the [multimodal execution gates](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md#multimodal-embedding-replacement--2026-10-19) for current status. Knowledge, vector storage/search and PostgreSQL extensions remain separate work.

## Inspect before changing retained state

Inspect the running Hub image/source, existing catalog/issuer, Work Context, grants, registration secret, signing keys, native bindings and owned volumes before preparation. Preserve the selected external or Hub-hosted authorization server; do not regenerate an older local catalog over it. Use the helper matching the reviewed running Hub contract. If its retained identity/authority differs, stop for review rather than replacing it. Do not alter existing Time/Artifact/Media selections, grants or callers.

`identity/configure-hub --include-embedding` adds only the approved private binding `http://embedding.internal:8803/embedding/mcp` → `vv-services/embedding:8803`. The existing local and Entra preparation modes support this selection. Approved reruns retain it, together with Time and any selected Artifact/Media. Foreign/duplicate/changed placements fail for review. Multimodal media inputs require the ordinary Artifact deployment and the caller's Artifact read permission. Embedding declares that use through native registration; it adds no user/client/role, billed risk or priced-provider claim. Text-only inputs make no Artifact call.

The helper prepares a bootstrap file or candidate; it does not publish or activate the running control plane. The Hub seeds an empty installation from `--config`, then loads its authoritative active revision from the database on later starts. Editing a mounted `hub.json` and restarting therefore does not update an already-published installation. Prepare a fresh installation with its complete ordinary arguments plus `--include-embedding`; do not choose a different identity mode just to add the binding. For an existing installation, use the authoritative publication procedure below rather than regenerating over its current state.

1. Serialize this update with other control-plane writers. Using an existing authorized administrative identity, read `GET /admin/{profile}/control-plane` and retain its complete response privately (directory mode0700/file0600), including non-null `revisionId`, `sha256` and `controlPlane`. Never substitute the initial bootstrap file for this active document.
2. Use the response's complete `controlPlane` as the candidate. Preserve profiles, credentials, grants, Work Contexts, Computer defaults and every existing binding; append only the reviewed embedding destination under `nativeUpstreams.bindings`. Validate with the matching current Hub executable: `veoveo-hub validate --config "$REVIEWED_CONTROL_PLANE"`. The file validator alone does not publish or prove runtime transport preparation.
3. Submit the full candidate object with `PUT /admin/{profile}/control-plane` using the existing authorized administrator. The current Hub prepares/adopts changed destinations and publishes an audited database revision without a process restart; invalid candidates retain the working revision. The API does not take an expected-export-revision condition, so preserve the exclusive writer window across read, edit, publication and readback. Gateway/authentication bootstrap remains fixed; adding this binding does not change it.
4. Read back the active revision and exact binding, verify unrelated fields and existing service/Computer calls, and record that the Hub process stayed unchanged. Other Hub replicas reconcile authoritative revisions after notifications/reconnect. Registration and destination publication are separate operations: the MCP still registers through the native SDK. Do not restart Hub to apply a bootstrap-file edit or regenerate its live catalog.

## Component source and checks

Use clean exact reviewed revisions for both components; an OCI revision label does not select source. Record source hashes and immutable image IDs. Builder commands are owned by the components:

```sh
docker build --target check --progress=plain ../veoveo-mcp-embedding
docker build --target check --progress=plain ../veoveo-runtime-embedding
docker build --target runtime --build-arg VERSION=0.1.0 --build-arg REVISION="$EMBEDDING_MCP_REVISION" -t veoveo/embedding:native ../veoveo-mcp-embedding
```

The deterministic runtime check uses synthetic fixtures without model packages/cache/GPU. It supports Mac Docker Linux/arm64 and Linux/amd64; neither proves inference. Its README owns exact uv/Python pins, canonical HF snapshot prefetch and the native launchd template. No request downloads a model. Record model revision, cache snapshot/file hashes, runtime source/package identity and actual forward device/dtype separately from semantic processing identity.

## Mac runtime and credential custody

Install the reviewed runtime in a persistent user-owned directory with its uv0.13.1 environment and prefetched canonical cache. Adapt its native LaunchAgent template to that user's actual interpreter/working directory/cache/private token paths and reviewed40-character source revision. No sudo or fixed root-owned path is required. Bind only loopback127.0.0.1:8804 initially, with explicit mps and fallback disabled. launchd owns lifecycle; no custom launcher or restart daemon is introduced.

Set `VEOVEO_EMBEDDING_STORAGE_ORIGIN` to the exact trusted origin used by Artifact's signed downloads, including an explicit port and excluding a path, query or trailing slash. Select the actual Artifact signing origin, not an independently chosen storage alias. It must resolve and be reachable from the native Mac process with ordinary TLS trust. `host.openshell.internal` is an OpenShell workload policy-DNS alias, not a promise of Mac host DNS resolution; do not copy it from a sandbox policy without proving the host can resolve the actual signed origin. Keep Artifact signing configuration and runtime origin aligned rather than rewriting signed URLs. The runtime requires it even for text-only calls, rejects transfers to other origins, preserves signed URL/headers, and checks declared size and SHA-256 before native decoding. It holds no Hub identity or object-storage signing key. Native TorchCodec uses installed FFmpeg shared libraries; a working `ffmpeg` shell command alone does not establish decoder loading. Record the actual linked library versions. Input media stays in request-scoped temporary files until the real worker finishes.

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

Qualify native `host.openshell.internal:8804` reachability and path-bound credential injection from an otherwise equivalent restricted probe before creating the MCP fixture. A supported host alias is not proof that a loopback listener is reachable. If it fails, report the actual native network evidence and decision needed; do not broaden the listener, open a public port or add a bridge/controller automatically. Verify unauthenticated and wrong-credential calls fail without GPU work and a non-admitted binary/path cannot obtain the bearer.

## MCP workload

`embedding.yaml` is the Mac host-provider policy. It admits native Hub registration/metadata/token paths, the typed `artifact__create_download` call at `/mcp/local`, three private runtime paths and Core PostgreSQL for `/usr/local/bin/veoveo-mcp-embedding`. The SDK selects `/mcp/{profile}` from the caller's admitted profile; this example permits only `local`. Verify that route, issuer and the Mac gateway port against retained configuration before use. MCP protocol inspection admits discovery/ping/tool listing plus that single Artifact tool. The MCP policy grants neither object-storage access nor model downloads. Artifact metadata and signed transfers travel privately through the existing Hub connection; media bytes travel from storage to the runtime.

Use matching reviewed SDK/Hub revisions with native nested delegation for a composed service → Embedding → Artifact call. Keep the original caller, actor history and inherited deadline through native token exchange. An originating Task retains its own owner and liveness checks; ordinary logout semantics remain unchanged. Do not substitute a service credential or a direct Artifact endpoint when delegation is refused. Artifact's application contract and ACL remain authoritative.

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

Set `EMBEDDING_HUB_ISSUER` to the existing service registration/discovery origin, not the human/Lore OAuth issuer path. Match policy metadata/token paths to it under review before running. Do not pass real secrets using --env. There is no public embedding exposure: Hub reaches the MCP over native ForwardTcp at its approved logical binding. Preserve the normal SDK registration and Core-owned login; no direct SQL into other domains, new DB extension or infrastructure service is needed.

## Acceptance boundaries

First run component checks, then native provider/network/registration/readiness probes, then real pinned-model service calls through Hub. Exercise no-prefix text, named/literal prefixes, all supported dimensions, each media modality, a heterogeneous batch and one joint item containing text/image/audio/video. Upload valid fixture media through native Artifact and record its identities, hashes and sizes. Include video soundtrack extraction, default preservation of audio longer than 30 seconds and explicitly requested native truncation. Reject the upstream-incompatible combination of soundtrack extraction and separate audio parts. Record actual sampled frame indices, decoded PTS, model timestamps, retained audio samples and effective preprocessing.

Qualify one and two joint 8,192-token items with native `batch_size=1`, plus the item/part/byte/pixel/duration/token ceilings. Record actual ordered vector shapes, finite unit norms, model/processing/runtime identities, token usage and measured timings. Verify Python/Rust RFC8785 processing hashes, caller/Artifact access denials, malformed media, expired/tampered transfers and all-items preflight before inference. Check busy without queuing, timeout/disconnect retaining the slot, and successful actual versus rejected zero/ambiguous estimated Core usage. The estimate is reserved only after authorized Artifact resolution. No automatic inference replay.

Keep the frozen three-concept image/speech/video matching corpus and its cosine matrices separate from structural correctness. It is a bounded semantic smoke test using generated valid media and real model vectors. Cross-device numerical parity uses identical bytes/options and its recorded thresholds; it does not establish general model quality.

Exercise existing service paths before/after live binding publication, plus runtime/MCP dependency restart without identity drift. Keep deterministic synthetic receipts, real GPU prerequisites, Mac acceptance, Linux Kubernetes CUDA acceptance and final selected OpenCode MiMo acceptance separate. This guide describes fixture inputs and required checks, not a new measured result; use the linked execution record for current qualification status. Make no OpenCode calls during iteration. Results do not establish vector storage/search or general GPU scheduling.
