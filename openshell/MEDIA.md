# Native Media preparation

These are installation and qualification instructions; the
[Media execution plan](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md#media-native-integration-plan)
records actual deployed evidence and remaining gates. Preserve the qualified Time/Artifact paths, existing
PostgreSQL/SeaweedFS data and registration keys. Endpoint/provider/placement
selection stays in this installation and uses the existing local Work Context;
there is no Media registry or fixed dev/stage/release environment hierarchy.

## Source and identity prerequisites

Media's production and fixture images must come from the same reviewed component
revision. Its SDK and Artifact pins must refer to published, tested revisions.
Use the component's ordinary targets, recording each resulting immutable image ID:

```sh
(
set -e
MEDIA_REVISION=392c2c1000c2b96fb8ee823bbf0996444c547623
test "$(git -C ../veoveo-mcp-media rev-parse HEAD)" = "$MEDIA_REVISION"
test -z "$(git -C ../veoveo-mcp-media status --porcelain)"
docker build --target check ../veoveo-mcp-media
docker build --target runtime --build-arg VERSION=0.1.0 \
  --build-arg REVISION="$MEDIA_REVISION" -t veoveo/media:native ../veoveo-mcp-media
docker build --target fake-provider --build-arg VERSION=0.1.0 \
  --build-arg REVISION="$MEDIA_REVISION" -t veoveo/media-provider:native ../veoveo-mcp-media
)
```

Use a clean component checkout of `MEDIA_REVISION`; a revision label does not
select the build source. Set it to the reviewed commit before these commands. Inspect the
runtime labels and retain exact `MEDIA_RUNTIME_IMAGE_ID` and
`MEDIA_PROVIDER_IMAGE_ID` values for native creation. The fixture's `testing`
feature is enabled only in its separate build; production Media does not serve a
test build. Build qualification is separate from native deployment qualification.

REST registration/token exchange and MCP calls share the Hub origin through
nonoverlapping endpoint paths. This depends on the reviewed OpenShell source
`6589a918a7a84640969f9f77b852695e210d713f`, selected through the existing
[supervisor build workflow](README.md#build-and-provision). Verify actual policy
admission and the selected supervisor image before service creation. The gateway's
incremental policy-merge RPC also owns admission logic; changing its supervisor
image alone does not replace the stock gateway's merge implementation. Adopt and
verify the reviewed native gateway build as well before using incremental updates. Keep ambiguous
REST/MCP overlaps rejected and preserve MCP inspection, verified TLS and credential
bindings. A generic REST rule over `/mcp/local` is not an alternative.

After [Hub/Time preparation](HUB_TIME.md) and [private Artifact setup](ARTIFACT.md),
run the trusted identity/catalog helper:

```sh
./identity/configure-hub \
  --hub-origin https://vv-control--hub--http.openshell.localhost:17670 \
  --include-artifact --include-media
```

The helper adds only the reviewed private binding
`http://media.internal:8802/media/mcp` → `vv-services/media:8802`. Ordinary reruns
retain selected Time, Artifact and Media bindings; foreign placements fail for
review. Media requires selected Artifact. Existing keys, registration secret and
IdP client secrets/subjects persist. Existing Work Context output owner, policy and
membership constraints are retained; an ambiguous contributor membership fails.

Only selected/retained Media adds `vv-hub-billed`: its Hub role grants read/write
with the `billed` risk, without sensitive, destructive, physical or administer
risks. It uses the existing external IdP/client-credentials claims, local profile
and contributor membership alongside member/admin. The ordinary member receives
no billed grant. Dedicated clients receive no OpenShell realm authority; the
existing forwarder's service-workspace authority is unchanged. There is no new
server-permission registry: registered requirements and ordinary role evaluation
still decide the permitted calls.

Privately copy the generated `hub.json` into the existing `vv-hub-config` volume.
Hub captures native placements at startup, so activate it through native stop/start
and verify the same public key, external origin and Time/Artifact calls afterward.
Keep its management provider and other config/state volumes unchanged. Do not
replace the live file or restart anything as part of running this helper.

## Service credential custody and volumes

Inspect definitions before native creation; reuse matching owned state and refuse
foreign/conflicting resources. Import registration material from trusted Hub state
in a subshell, never a token argument:

```sh
(
  set -a
  . ./state/openshell/hub/hub.env
  set +a
  openshell profile import --workspace vv-services --file openshell/providers/media-registration.yaml
  openshell provider create --workspace vv-services --name vv-media-registration \
    --type vv-media-registration --credential VEOVEO_REGISTRATION_SECRET
)
(
  export MEDIA_PROVIDER_API_KEY=fake-generation-key
  openshell profile import --workspace vv-services --file openshell/providers/media-generation.yaml
  openshell provider create --workspace vv-services --name vv-media-generation \
    --type vv-media-generation --credential MEDIA_PROVIDER_API_KEY
)
```

The latter value is a public fake-provider fixture credential, not a real billed
provider key. Media receives only native opaque references. Its registration
reference is bound to POST `/oauth/register`; the generation reference is bound
only to the fixture's `/api/v3/**` bearer requests. Output GETs have a separate
unbound path. Media's own signing key signs SDK delegation assertions; it receives
no Artifact credential or S3 signing key.

Prepare the required fake webhook HMAC secret once in ignored
`state/openshell/media/provider.env`, directory 0700/file 0600. It contains only
`MEDIA_PROVIDER_WEBHOOK_SECRET`, generated privately by the trusted operator.
Preserve an existing value. Although this slice sends no webhooks, both binaries
require the configuration. For this fake-only phase, prepare a separate private
`state/openshell/media-provider/provider.env` containing the same fake HMAC value.
Copy the service file only into `vv-media-config`, and the fixture file only into
`vv-media-provider-config`, each directory 0700/file 0600 readable by UID 10001.
Never print either value, put it on command arguments or copy the whole operator
state directory. The fixture must never mount Media's service configuration volume.
When selecting a real provider, replace only the trusted Media service config with
its actual webhook signing secret; never copy that secret or real provider
credentials into the fixture's config volume. Preserve the fake-only fixture state
independently.

Use only native workspace-labelled named volumes:

```sh
docker volume create --label openshell.ai/sandbox-attachable=true \
  --label openshell.ai/sandbox-attachable-workspace=vv-services vv-media-state
docker volume create --label openshell.ai/sandbox-attachable=true \
  --label openshell.ai/sandbox-attachable-workspace=vv-services vv-media-config
docker volume create --label openshell.ai/sandbox-attachable=true \
  --label openshell.ai/sandbox-attachable-workspace=vv-services vv-media-provider-config
```

Inspect labels and contents first. Ordinary sandbox replacement preserves the
approved `keys/server-key.p8` in `vv-media-state`, mode 0600 and UID 10001 ownership.
For an authorized breaking-contract development cutover, reset disposable database
and Task state; do not reconcile legacy migration checksums or add compatibility
for old results. Keep trusted installation inputs, and follow the
[fresh reference preparation](REFERENCE.md#fresh-bootstrap-and-retired-reference-keys)
when retiring an installation. Writable service state survives routine sandbox
replacement.
Configuration is directory 0700/file 0600. [Media](media-mounts.json) mounts only
`vv-media-config` read-only, while [the fixture](media-provider-mounts.json) mounts
only its separate `vv-media-provider-config` read-only at the same component path.
Neither mounts operator directories, host binds or Docker sockets. These are
startup mount selections: do not alter a running Mac sandbox as part of preparing
the fresh reference; verify each new sandbox's actual mounts before activation.
The approved driver
configuration keeps resource admission enabled and bind mounts disabled;
`allow_driver_config` also admits validated tmpfs/CDI requests, but these specs use
neither.

## Native fixture and private service

Start the separate fixture first, with an explicit expected origin. These commands
use standard native lifecycle only; inspect existing same-name resources before
creation, and never replace unknown workloads:

```sh
openshell sandbox create --workspace vv-services --name media-provider \
  --from "$MEDIA_PROVIDER_IMAGE_ID" --policy openshell/media-provider.yaml \
  --no-auto-providers --driver-config-json "$(cat openshell/media-provider-mounts.json)" \
  --restart-policy on-failure --detach \
  --env MEDIA_FAKE_PROVIDER_LISTEN=127.0.0.1:8803 \
  --env MEDIA_FAKE_PROVIDER_API_BEARER=fake-generation-key \
  --env MEDIA_FAKE_PROVIDER_ADVERTISED_BASE=https://vv-services--media-provider--http.openshell.localhost:17670 -- \
  /bin/sh -c 'set -a; . /etc/veoveo/media/provider.env; set +a; exec /usr/local/bin/veoveo-mcp-media-fake-provider'
openshell service expose --workspace vv-services media-provider 8803 http \
  --authorization-mode bearer-passthrough
```

Require the returned exposure origin to equal the configured advertised origin
exactly. The gateway remains loopback-bound with HTTPS and normal hostname/CA
verification. This exposure is for the test fixture only, preserving provider
Bearer semantics; it is not public Media MCP ingress. The fake's HTTP status/output
links must use that same origin, never its bind address or rewritten Host.
It has no outgoing network policy and makes no callback in status-check mode.
The selected fixture image requires the public test bearer above on its
provider API, matching the native generation provider's injected value. This is
a fake credential, not a real provider key. Missing or incorrect API bearers are
rejected; output downloads and trusted test controls use their separate paths.

After PostgreSQL, SeaweedFS, Hub, Artifact and the fixture are ready:

```sh
openshell sandbox create --workspace vv-services --name media \
  --from "$MEDIA_RUNTIME_IMAGE_ID" --policy openshell/media.yaml \
  --provider vv-media-registration --provider vv-media-generation --no-auto-providers \
  --driver-config-json "$(cat openshell/media-mounts.json)" \
  --restart-policy on-failure --detach \
  --env VEOVEO_HUB_ISSUER=https://vv-control--hub--http.openshell.localhost:17670 \
  --env VEOVEO_PUBLIC_BASE_URL=http://media.internal:8802 \
  --env VEOVEO_PORT=8802 --env VEOVEO_KEY_DIRECTORY=/var/lib/veoveo/media/keys \
  --env MEDIA_PROVIDER_URL=https://vv-services--media-provider--http.openshell.localhost:17670 -- \
  /bin/sh -c 'set -a; . /etc/veoveo/media/provider.env; set +a; exec /usr/local/bin/veoveo-mcp-media'
```

Keep `VEOVEO_PUBLIC_ORIGIN` unset. No Media service exposure is created: Hub reaches
its logical endpoint over authenticated native ForwardTcp. [Media's policy](media.yaml)
separates registration, metadata/JWKS, `/oauth/token`, and `/mcp/local` Artifact
calls. The SDK upload path requires create/sign-parts/complete/abort tools,
resource reads and subscriptions for verification; it uses modern MCP inspection.
PostgreSQL and signed S3 bytes use TCP on ports 5432 and 9000, respectively.
Verify `host.openshell.internal:9000` from each byte client without changing signed
authority/headers; the [Mac resolution prerequisite](ARTIFACT.md#trusted-preparation)
still applies to the final host-client qualification.

## Trusted acceptance preparation

Use a separate trusted workspace such as `vv-media-qa`. Import
[the member/billed profile](providers/media-qualification.yaml) there and create
`vv-media-qualification --runtime-credentials` with the standard native provider
commands from [Hub/Time setup](HUB_TIME.md#native-providers-and-workspace-authority).
Configure two independent refresh entries before creating a runner:

| Credential key | Trusted refresh input | Actual client |
|---|---|---|
| `HUB_MEMBER_REFERENCE` | `member.refresh.env`, `VV_HUB_MEMBER_CLIENT_ID` / `VV_HUB_MEMBER_CLIENT_SECRET` | `vv-hub-member` |
| `HUB_ADMIN_REFERENCE` | `billed.refresh.env`, `VV_HUB_BILLED_CLIENT_ID` / `VV_HUB_BILLED_CLIENT_SECRET` | `vv-hub-billed` |

Use `--secret-material-env client_secret=...` for each secret and rotate each entry
once for provisioning. Keep refresh material in native custody; never mount those
files. `HUB_ADMIN_REFERENCE` is only the existing acceptance role selector here;
it must not receive `vv-hub-admin` credentials. One explicit provider binding
selects either distinct supplied opaque reference without substituting identities.
Do not attach this qualification profile to author factories or the Media service.

For the deployed `392c2c1` runtime/fixture pair, use qualification source
`20e2c6edb13d86cf981cf78e2a45fa41cbc94ff5`. Its later changes correct acceptance
comparisons and documentation and synchronize C31 compliance metadata; production
Rust and fixture behavior are unchanged. It compares canonical typed checksums,
the actual Task-required refusal, and completion-result versus persisted lifecycle
timestamps according to their respective contracts. Record the test revision
separately from the runtime image labels.
Compile the component-owned acceptance executable from that clean qualification checkout with
`--features testing --test acceptance --no-run` in its dev image. Transfer only the
reviewed executable to `/sandbox/bin/media-acceptance` in the trusted runner and
use [its narrow policy](media-qualification.yaml). OpenShell pins a network-calling
executable's identity on first use. When updating an existing runner's binary,
stop/start that runner through native lifecycle controls before running the new
executable, then verify its hash, policy and provider application. Do not hot-swap
it under the old supervisor or clear integrity state manually. Recheck opaque
handle identity before using a pre-restart credential-renewal baseline.
The runner holds no management
credential or restart authority. It admits `media__run` for both identities so the
ordinary member's request reaches Hub's billed-permission refusal. Successful
catalog/schema/run, Task get/cancel, resource/subscription and Artifact download
requests must produce actual application outcomes; native transport errors prove
no Hub refusal. The disjoint GET own-usage rule verifies fake-generation estimate
and actual records; it also supports the retained separate real-provider case,
without authorizing that case now.

```sh
export VEOVEO_ACCEPTANCE_HUB=https://vv-control--hub--http.openshell.localhost:17670
export VEOVEO_ACCEPTANCE_MEMBER_BEARER_ENV=HUB_MEMBER_REFERENCE
export VEOVEO_ACCEPTANCE_ADMIN_BEARER_ENV=HUB_ADMIN_REFERENCE
export VEOVEO_ACCEPTANCE_MEDIA_FIXTURE=https://vv-services--media-provider--http.openshell.localhost:17670
export VEOVEO_ACCEPTANCE_BILLED_PRINCIPAL='https://auth.localhost:8443/realms/veoveo#vv-hub-billed'
export VEOVEO_ACCEPTANCE_WORK_CONTEXT=local
export VEOVEO_ACCEPTANCE_LABELS='[]'
/sandbox/bin/media-acceptance --ignored --skip real_provider --skip restart --nocapture --test-threads=1
```

The expected principal, Work Context and labels must match the actual retained
Hub configuration; the values above describe this local installation. They are
assertions about ownership/provenance, not authority to change the output policy.

Run restart phases separately. The in-flight probe is
`a_running_generation_recovers_after_media_and_hub_restart`: it reports the held
Task/prediction, closes its client and waits for exactly `continue` followed by LF
on stdin. Keep native Exec attached with `--tty` and an open terminal session;
verify stdin with a harmless `read` command before creating the controlled Task.
The qualification's noninteractive Exec invocation closed stdin and failed before
the restart boundary, so it is not suitable for this phase.
Only the trusted operator restarts the actual Media and Hub sandboxes,
verifies their identities/readiness and persisted keys, and then supplies that
line. The fixture must keep running. Do not treat a prompt or process restart
inside the runner as a service restart.

For terminal persistence, run only `restart_persistence_prepare`, retain its
reported `VEOVEO_MEDIA_RESTART_TASK_ID`, restart Media and Hub with the native
operator controls, then run only `restart_persistence_verify` with that Task ID.
Use `--ignored --exact TEST_NAME --nocapture --test-threads=1` for each phase.
These phases prove different behavior; a retained terminal result does not prove
in-flight recovery.

The initial deployment passed the two existing fake-provider cases. Additional deployed lifecycle
proofs remain pending: independent request evidence for no resubmission, actual
cancellation, externally orchestrated in-flight recovery, retained bytes/metadata,
provenance, usage and renewal. The reviewed testing-only fixture controls expose
safe request counts and prediction IDs/status at `GET /testing/state`, quote
hold/release at `POST /testing/quotes`, automatic completion selection at
`POST /testing/delivery`, and explicit completion at
`POST /testing/predictions/{prediction}/complete`. The qualification policy admits
only those routes and methods for the trusted acceptance executable. Media's
production policy cannot access `/testing/**`. These controls require selected
revision `392c2c1`; the initial `0589cb6` image does not provide them.
A single stored output is not evidence of one submission. Keep the fixture running across a Media restart when testing
provider-operation recovery. Inspect actual provider refresh application and leave
measured margin before the next native credential fence; do not retry mutations or
hide interrupted calls. Record same persisted public keys separately from generated
registration client IDs, which may change at restart.

The controlled cases deliberately leave automatic fixture completion disabled.
After the final restart proof, release quote holds and restore automatic delivery
before an interactive client run, using the trusted operator on this Mac:

```sh
curl --fail --silent --show-error --cacert /opt/homebrew/var/openshell/tls/ca.crt \
  --json '{"hold":false}' \
  https://vv-services--media-provider--http.openshell.localhost:17670/testing/quotes
curl --fail --silent --show-error --cacert /opt/homebrew/var/openshell/tls/ca.crt \
  --json '{"automatic":true}' \
  https://vv-services--media-provider--http.openshell.localhost:17670/testing/delivery
```

OpenCode with explicit `deepinfra/XiaomiMiMo/MiMo-V2.6-Pro` is the last bounded
qualification only after deterministic/unit/integration/deployed probes pass.
Preflight its actual Task interoperation without model calls first. No real billed
provider, public webhook, generic environment composition or Media Lore/factory
qualification is claimed by these templates.
