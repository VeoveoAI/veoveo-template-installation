# Native OpenShell Setup

Homebrew manages the native macOS OpenShell service. The Media slice adopts local
CLI/gateway builds from reviewed revision `6589a918` in its existing binary paths;
the service wrapper, LaunchAgent and configuration ownership remain unchanged.
Its native Docker driver runs factories and managed MCP services. The pinned
supervisor supplies HTTP/2/gRPC credential mediation, native verified ingress and
disjoint REST/MCP path admission. No replacement launcher, token service, refresh
daemon or secret store is required. Homebrew upgrade/reinstall can overwrite the
local binaries; deliberately select and requalify the reviewed source afterward.

The direct Lore path is **IdP issuance → OpenShell renewal/injection → Lore native
authorization**. Veoveo supplies trusted configuration and branch assignments.
The build commands below select reviewed Lore source at
`8be7e6e69216c9c974f12027b35910ad1d3fd708`, merging upstream
`ec6bdc2195753b50f70696d367788b4bc8d08964` (0.10.2-nightly). Native OIDC
discovery, device login and partition exchange come from upstream. Human token
refresh remains unimplemented there; this source update does not qualify that flow.
Only the existing opaque-token factory integration and configured push hook remain
Veoveo additions. Native OpenShell
configuration selects runtime images; the [execution checklist](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md)
records current acceptance. Local image tags are not registry releases.

## Operator identity and trust

The gateway reads `~/.config/openshell/gateway.toml` and private `gateway.env`.
It uses verified TLS, generic OIDC, the native encrypted credential store and
telemetry disabled. Its database/encryption key stay outside workloads.

Complete the one-time [macOS CA trust setup](../identity/README.md#macos-certificate-trust).
Keep the registered gateway CA in
`~/.config/openshell/gateways/openshell/mtls/ca.crt` as well. No per-shell certificate
export, client certificate or insecure flag is required:

```sh
openshell gateway login openshell
openshell status
openshell sandbox list --all-workspaces
```

`identity/configure-openshell` provisions only the local management clients.
`identity/configure-lore` separately provisions the local factory clients, built-in
optional repository scopes/mappers and the public `lore-cli` metadata registration.
It preserves client secrets and service-account subjects. A Keycloak service-account
OAuth flow does not set Lore's protection-bypass `is_service_account` claim.
Neither helper is a runtime dependency for an external issuer.

Private operator configuration is under `state/openshell/lore-direct/`, directory
0700/files 0600. The IdP client secrets enter native refresh material with explicit
secret classification; they are not injectable workload credentials. The generated
`server.toml` contains trusted hook assignments and is mounted read-only in Lore.
Agents cannot edit this state. The tracked `lore/server.toml` is the public template.

## Build and provision

Run the following blocks in Bash from the installation root with the maintained
`../lore` and `../openshell` repositories available. Each block refuses an existing
source destination and stops on export or build failure. Existing source exports
are left untouched. Build the server from the reviewed hook-enabled candidate:

```bash
(
set -euo pipefail
mkdir -p state/openshell
mkdir state/openshell/lore-hook-source
git -C ../lore archive 8be7e6e69216c9c974f12027b35910ad1d3fd708 \
  | tar -x -C state/openshell/lore-hook-source
docker build -f factory/Lore.Dockerfile --target runtime \
  --build-context lore-source=./state/openshell/lore-hook-source \
  --build-context ca=./tls/ca -t vv-lore:local .
)
```

This adds only the public installation CA to the normal trust store. Lore discovers
JWKS from its single configured issuer. There is no adapter issuer/key override,
auth-service URL or custom catalog. Keep its private gRPC address, certificate IP
SAN and approved policy endpoint aligned (`grpcs://10.231.10.10:41337` locally).
Native denied-baseline catalog listing is empty; provide known assigned URLs. Native
repository lookup now lets a verified caller resolve an ID/name/metadata hash;
reading repository contents still requires its native grant. No new human
permission policy or issuer registration is introduced here.

The obsolete `server.auth.identity_claim = "sub"` setting is removed from the
public template. `configure-lore` also removes that exact default from generated
configuration on its next explicit run; it refuses a nondefault value for review.
Upstream ignores this removed key and natively selects `identity`, then its
namespaced alias, then `sub`. Existing factory claim mappers supply no identity
override, so their stable subject remains the identity. Resource claims, catalog
mode, native branch protection and trusted factory assignments are preserved.

Upgrade the host CLI, images and any existing local Lore background service
as a coordinated set. Upstream changed client/service IPC from JSON to bitcode
message protocol 2: client and local service must come from the same build, and
an older running service must stop before the replacement starts. Use the matching
currently installed CLI for `lore service status` and `lore service stop` before
replacing it; then use the new CLI for `lore service start` and status. This local IPC
change is separate from the repository gRPC protocol; qualify the deployed path
and existing store reopen before claiming upgrade acceptance. Do not use
`--trust-remote` merely to make a check pass: upstream now trusts local stored
payloads and verifies remote fetches by default. No data reset or migration is
performed by these source/build instructions.

Build the Lore CLI from that same candidate, then package it with OpenCode
and the Rust toolchain selected by `factory/Dockerfile`:

```bash
(
set -euo pipefail
mkdir -p state/openshell
mkdir state/openshell/lore-client-source
git -C ../lore archive 8be7e6e69216c9c974f12027b35910ad1d3fd708 \
  | tar -x -C state/openshell/lore-client-source
docker build -f factory/Lore.Dockerfile --target runtime \
  --build-context lore-source=./state/openshell/lore-client-source \
  --build-context ca=./tls/ca -t veoveo/lore-client:local .
docker build -f factory/Dockerfile \
  --build-context lore=docker-image://veoveo/lore-client:local \
  --build-context ca=./tls/ca -t veoveo/factory:local factory
)
```

Build the patched supervisor using OpenShell's native prebuilt workflow, then add
only the public installation CA. Install its native build prerequisites from
`CONTRIBUTING.md` before running the build. Use a fresh worktree; do not reuse
partial cross-build outputs:

```bash
(
set -euo pipefail
test ! -e state/openshell/supervisor-source-6589a918
git -C ../openshell worktree add --detach "$PWD/state/openshell/supervisor-source-6589a918" \
  6589a918a7a84640969f9f77b852695e210d713f
(
  cd state/openshell/supervisor-source-6589a918
  IMAGE_REGISTRY=veoveo IMAGE_TAG=native-ingress mise run build:docker:supervisor
)
docker build -f openshell/Supervisor.Dockerfile \
  --build-context ca=./tls/ca --build-arg BASE=veoveo/supervisor:native-ingress \
  -t veoveo/supervisor:local .
)
```

For Linux, also build and pin the separate trusted sandbox runtime image as
described in [the reference installation](REFERENCE.md#pin-all-native-runtime-images).
It is distinct from the application base and supervisor images.

Build the native Mac CLI and gateway from that same pinned worktree. This is the
reproducible source command, not a transcript of the initial local build:

```sh
(
  cd state/openshell/supervisor-source-6589a918
  mise exec -- cargo build -p openshell-cli --bin openshell \
    -p openshell-gateway --bin openshell-gateway
)
```

The initial adoption retained the Homebrew wrapper at
`/opt/homebrew/opt/openshell/libexec/openshell-gateway-homebrew-service` and replaced
only its `bin/openshell` and `bin/openshell-gateway` executables with the reviewed
`target/debug` outputs. Before replacing binaries, retain the original executables,
private gateway configuration and LaunchAgent in a private backup directory, then
install the reviewed outputs as executable files and restart the existing service.
The local backup is `state/openshell/gateway-cutover-6589a918/` (directory 0700,
files 0600); preserve it. The selected native outputs have SHA-256
`b438df2a46ef289683b12217f0c4515f96c3f1eaf04b382791cd4a2c3cf1c32b`
(CLI) and `2838f3c1a7710cf310d91f7308012a903bc67fa587fa045860e6a913c2c9c288`
(gateway). Do not restore stock executables while claiming the patched policy
behavior. This local adoption is not a published Homebrew formula.

Select the resulting immutable image ID with the native gateway's
`OPENSHELL_SUPERVISOR_IMAGE` setting. Set
`OPENSHELL_ENABLE_LOOPBACK_SERVICE_HTTP=false` so native service exposures advertise
HTTPS. Back up private gateway configuration before restarting its Homebrew service.
The Docker driver selects the supervisor image when the gateway starts and
reconciles supervisors for **all existing running sandboxes**, including unrelated
fixtures. Plan gateway restarts for that scope. Native sandbox stop/start also
recreates its supervisor while retaining sandbox identity, workload, named volumes
and exposure. Merely restarting the application process does not establish the new
supervisor. Verify the actual supervisor and workload image IDs before acceptance.
The selected revision includes native service DNS, scoped gateway TLS, credential
mediation and official MCP Tasks-extension inspection. The execution checklist
records which paths have been qualified. Run the focused
`first-path service-ingress --image sha256:PROBE_IMAGE_ID` qualification from
`veoveo-qualify-openshell` against fresh sandboxes. Its README describes the probe
image build and measured limits; Hub/Time application acceptance is separate.

The first factory has no inference provider and makes no model calls.

For disposable qualification only, export the offline seeder:

```sh
docker build -f factory/Lore.Dockerfile --target fixture \
  --build-context lore-source=./state/openshell/lore-hook-source \
  --build-context ca=./tls/ca \
  --output type=local,dest=./state/openshell/factory-hook-bin .
```

Stop the owning Lore server and take a consistent private volume backup before
opening its stores. Run `factory-branch-fixture /data` with that volume in an
isolated operator container. It creates fresh repo/branch IDs, valid empty heads
and protected `main`. Save the final JSON line privately as
`state/openshell/factory-hook-fixture.json`; it is not a server/agent bootstrap API.
Run `identity/configure-lore`, then start bootstrap with `--profile keycloak`.
Real installations replace the disposable manifest with approved trusted bindings.

## Native provider and qualification

The public [profile example](providers/lore.example.yaml) shows native
`oauth2_client_credentials` metadata. Render its issuer, repository scope and
profile ID from trusted installation configuration. Each independently revocable
factory has its own IdP client and provider. Refresh lead is 180 seconds, maximum
lifetime 300 seconds; preflight rejects a longer signed IdP lifetime. Workloads
receive an identity-bound `openshell:resolve:` reference, never client secrets or JWTs.

In `veoveo-qualify-openshell`, build `first-path` with
`cargo build --locked --features first-path --bin first-path`.
Provision the disposable smoke factory with the image you built:

```sh
target/debug/first-path --installation /absolute/path/to/veoveo-template-installation \
  factory --image veoveo/factory:local --replace
```

Other checks use the same `--installation` argument:

- `lore-preflight` verifies signatures, type/audience/subject, bounded grant shape,
  lifetime and omitted/unlinked scope denial without storing token bytes.
- `factory --image IMAGE --replace` provisions the native smoke provider and replaces
  the disposable smoke factory using the supplied image and read-only RPC policy.
  It is a setup action, not automatic renewal evidence. Ordinary renewal needs no
  manual token file.
- `lore --repository ID --forbidden ID` and `boundaries` check the smoke path,
  custody, inaccessible operator files and denied direct egress.
- `lore-direct --image IMAGE_ID` runs the disposable direct-issuer workflow and
  timed lifecycle gates. Build its std-only canonical test workload using
  `LoreDirect.Dockerfile` in the qualification repository. `--phase workflow` omits
  lifecycle measurement and cannot qualify renewal/removal. `--phase renewal`
  selects only the workflow and renewal gate; `--phase remaining`
  and `--phase recovery` select independent gates; they do not imply renewal passed.
- `lore-negative` checks a valid direct grant against a management bearer. The
  probe runs only on the trusted host and does not expose credentials to workloads.
- `service --image IMAGE --reuse` checks the existing native private MCP fixture.

The lifecycle workload captures its opaque reference once and checks a random
per-launch marker in fresh command replies while running real Lore commands across
at least two native renewals and original expiry plus 60-second verifier leeway.
Only one initial native rotate is allowed
per provider. It tests unavailable issuance by disabling only fixture A's IdP
client while B operates/renews; this local qualification action uses Keycloak admin
credentials only on the trusted host, never in a workload or runtime service.
Cleanup restores the prior enabled value only for a client changed by that run.
The recovery gate leaves A's state untouched. Its process marker comes from a fresh
answered command, allowing native workload restarts while preserving sandbox identity.
Earlier PID-only records do not establish uninterrupted process lifetime.

OpenShell intentionally rejects detach while a policy references the provider.
The supported sequence removes the explicit binding with native `UpdateConfig`,
awaits the active policy revision, then detaches and awaits applied `REVOKED`.
Results distinguish this combined transition from detach alone. Notification streams
carry actual branch events; report their observed behavior, then stop the sandbox
and await stopped state when execution must end. Disabling an IdP client alone
cannot revoke existing JWTs immediately, and removal alone does not promise to
cancel requests already forwarded. No Lore transport changes manufacture a cutoff.

Reports remain private under `state/openshell/lore-direct/`; the execution checklist
records current acceptance. Temporary resources have ownership-aware cleanup,
including partial setup. Existing workspaces are refused. Preserve unrelated
workspaces/providers and durable Lore/identity data.

The pre-push hook remains a normal-push workflow guard, not complete low-level
branch isolation. Protected branches and ordinary human permissions keep their
native meaning. See the [hook contract](https://github.com/VeoveoAI/lore/blob/native-openshell/docs/reference/factory-branch-hook.md).
The active Linux/Entra and signed webhook goal uses the existing native path.
Customer issuers, interactive human Lore login, GPU/CUDA and broader composition
remain separate qualifications in the [execution plan](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md).

## Hub and Time preparation

Use the [local identity, catalog and native runtime preparation](HUB_TIME.md) after
the qualified ingress fixture. Its trusted helper configures dedicated IdP clients
and component files without deploying workloads. Actual Hub/Time qualification
and the prepared Lore/factory build remain execution gates.

## Prepared Time factory

The [prepared Time workflow](FACTORY_TIME.md) adds separate standard policies and
profiles for a trusted source publisher and bounded factory. The original fixture
policy and qualification flow above remain separate. Do not publish source or run
a model until the execution checklist has accepted deployed Hub/Time.

## Private Artifact follow-up

After Hub/Time, use the [native Artifact preparation](ARTIFACT.md) for its explicit
private Hub binding, persistent registration key, dedicated bucket-scoped signing
credential and direct signed S3 transfers. Public sharing and Media remain separate.

## Media preparation

Use [native Media preparation](MEDIA.md) for the private service, separate fake
provider and member/billed qualification templates. These require the reviewed
disjoint REST/MCP admission build above; source preparation is not deployed Media
acceptance. Preserve existing Time/Artifact data and configuration.

## Reference Linux and Entra

Use [reference preparation](REFERENCE.md) for the active Linux/Entra and signed
Media webhook goal. It preserves these native runtime contracts with separate
operator-owned reference configuration; Mac and factory receipts remain historical.
GPU/CUDA and generalized source-to-deployment automation remain separate.
