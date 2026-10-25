# Native OpenShell Setup

**Mac development fixture.** This guide runs on the Mac's native OpenShell gateway
with its Docker driver. The Linux path is K3s only: [K3s foundation](../kubernetes/README.md) and [native workload bootstrap](../kubernetes/WORKLOADS.md).

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
`7785408e70ed302296a24383ae302c3a606e78c3`, based on upstream
`ec6bdc2195753b50f70696d367788b4bc8d08964` (0.10.2-nightly). Native OIDC
discovery, device login and partition exchange come from upstream. Human token
refresh remains unimplemented there; this source update does not qualify that flow.
The existing opaque-token factory integration, configured push hook and native
[S3 + PostgreSQL storage plugin](LORE_STORAGE.md) are Veoveo additions. The backend
passes native component conformance against real PostgreSQL and SeaweedFS;
deployed server, dependency restart and factory qualification remain pending. Native OpenShell
configuration selects runtime images; the [qualification checklist](https://github.com/VeoveoAI/veoveo-docs/blob/main/QUALIFICATION_CHECKLIST.md)
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

`identity/configure-openshell` provisions only local native management clients.
Lore uses [Hub-issued native resource grants](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#native-contributed-credential-lifecycle): the public
client is secretless; a separate factory client's secret enters native refresh
custody. Human identity comes from Hub's existing enterprise broker. IdP clients,
roles or optional scopes do not own Hub repository assignments. The old
`identity/configure-lore` helper and its helper-only tests are retired.

Trusted server/storage configuration remains under the existing private
`state/openshell/lore-direct/` directory (0700/files 0600). Prepare its selected
native `server.toml` and storage environment using the existing storage/TLS recipe;
no grant bookkeeping file/helper replaces the retired tool. The tracked
`lore/server.toml` supplies public Hub-native auth defaults and unchanged trusted
storage/TLS/hook sections. Preserve other native server settings and exact hook
assignments. Agents cannot edit that state.

## Build and provision

Run the following blocks in Bash from the installation root with the maintained
`../lore` and `../openshell` repositories available. The source export refuses an
existing destination and stops on failure. All build targets reuse that one fresh export;
existing source exports are left untouched. Build the server from the reviewed
Lore candidate:

```bash
(
set -euo pipefail
mkdir -p state/openshell
mkdir state/openshell/lore-s3-postgres-7785408e70ed302296a24383ae302c3a606e78c3
git -C ../lore archive 7785408e70ed302296a24383ae302c3a606e78c3 \
  | tar -x -C state/openshell/lore-s3-postgres-7785408e70ed302296a24383ae302c3a606e78c3
docker build -f factory/Lore.Dockerfile --target runtime \
  --build-context lore-source=./state/openshell/lore-s3-postgres-7785408e70ed302296a24383ae302c3a606e78c3 \
  --build-context ca=./tls/ca -t vv-lore:local .
)
```

Factory and Lore builds install the reviewed public `*.crt` files from the trusted
`ca` named context through the normal system trust update, one certificate per file.
A single `ca.crt` input remains valid. The context must contain public CA certificates
only, never private keys or operator state. Linux Lore can trust its own new private
service CA alongside the existing public root that verifies storage TLS without
replacing storage certificates. Computer inherits Factory trust. These trust changes
require new images; existing image receipts retain their original scope. The
Supervisor recipe below retains its separately selected single `ca.crt` input.
This adds only public installation trust to the normal trust store. Lore discovers
JWKS from its single configured issuer. There is no adapter issuer/key override,
auth-service URL or custom catalog. Keep its private gRPC hostname, certificate DNS
SAN and approved policy endpoint aligned (`grpcs://host.openshell.internal:41337` locally).
Compose publishes Lore only on host loopback `127.0.0.1:41337`; native OpenShell's
host-gateway pin supplies the hostname without an `/etc/hosts` edit or Docker
network attachment. Keep exact client hostname/port/binary rules and remove the
old Lore-container `allowed_ips`; do not substitute a loopback CIDR. The TLS leaf
must add DNS SAN `host.openshell.internal` under the existing trusted CA. Preserve
its mounted key/certificate paths and internal Compose alias. Keep the endpoint hostname, certificate SAN, CLI remote and Hub's
`lore.deployments[].endpoint` aligned.
Native denied-baseline catalog listing is empty; provide known assigned URLs. Native
repository lookup now lets a verified caller resolve an ID/name/metadata hash;
reading repository contents still requires its native grant. No new human
permission policy or issuer registration is introduced here.

The obsolete `server.auth.identity_claim = "sub"` setting is removed from the
public template. Upstream ignores this removed key and natively selects `identity`,
then its namespaced alias, then `sub`. During the authorized cutover, remove that
one obsolete line from existing private configuration and verify that all other
parsed TOML values are unchanged. No automatic configuration migration is needed.
Existing factory claim mappers supply no identity override, so their stable subject
remains the identity. Resource claims, catalog mode, native branch protection and
trusted factory assignments are preserved.

Upgrade the host CLI, images and any existing local Lore background service
as a coordinated set. Upstream changed client/service IPC from JSON to bitcode
message protocol 2: client and local service must come from the same build, and
an older running service must stop before the replacement starts. Use the matching
currently installed CLI for `lore service status` and `lore service stop` before
replacing it; then use the new CLI for `lore service start` and status. This local IPC
change is separate from the repository gRPC protocol; qualify the deployed path
and selected-backend fresh-process reopen before claiming upgrade acceptance. Do not use
`--trust-remote` merely to make a check pass: upstream now trusts local stored
payloads and verifies remote fetches by default. No data reset or migration is
performed by these source/build instructions.

Build the Lore CLI image from the same shared source export created above, and the
Rust toolchain selected by `factory/Dockerfile`. The factory image no longer carries
Lore or the Hub SDK client: on Kubernetes, workloads mount their images read-only, and
OpenShell's Docker driver has no image volumes, so this superseded native-Docker path
has no Lore in its factory or Computer images:

```bash
(
set -euo pipefail
test -f state/openshell/lore-s3-postgres-7785408e70ed302296a24383ae302c3a606e78c3/Cargo.toml
docker build -f factory/Lore.Dockerfile --target runtime \
  --build-context lore-source=./state/openshell/lore-s3-postgres-7785408e70ed302296a24383ae302c3a606e78c3 \
  --build-context ca=./tls/ca -t veoveo/lore-client:local .
docker build -f factory/Dockerfile \
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

Factories run explicit build commands against published Lore revisions. They have
no OpenCode installation or inference provider; agent sessions run in user Computers.

For disposable qualification only, export the offline seeder:

```sh
docker build -f factory/Lore.Dockerfile --target fixture \
  --build-context lore-source=./state/openshell/lore-s3-postgres-7785408e70ed302296a24383ae302c3a606e78c3 \
  --build-context ca=./tls/ca \
  --output type=local,dest=./state/openshell/factory-hook-bin .
```

For the fresh disposable backend cutover, follow [native Lore storage](LORE_STORAGE.md).
Stop the owning Lore server before opening its stores. The plugin-capable fixture
takes a native configuration directory containing `local.toml`, not `/data`, and
uses the configured backend with trusted storage credentials. Run it in an
isolated operator container. It creates fresh repo/branch IDs, valid empty heads
and protected `main`. Save the final JSON line privately as
`state/openshell/factory-hook-fixture.json`; it is not a server/agent bootstrap API.
Keep this fixture output as test evidence only. Select native server settings in
trusted TOML; Lore runs no hook. Publish actual repository/grant data
through Hub's existing control plane. Start only the intended bootstrap services
through the coordinated native setup; no IdP repository-mapping helper is needed.

## Native provider and qualification

Select [the Hub-native issuer and factory profile](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#native-contributed-credential-lifecycle) with
its exact repository scope, separate factory principal and native
`oauth2_client_credentials` custody. Hub admits the current explicit assignment
and returns Lore's native signed resource entry; native OpenShell renews it. The
factory receives only its opaque `LORE_ACCESS_TOKEN` reference. No model/agent,
human refresh grant, management identity, IdP admin credential or raw JWT is
attached to a deterministic factory.

Keep local component tests, deployed probes and model acceptance distinct.
Qualify unchanged native Lore CLI device login/exchange/refresh, actual issuer
signature/audience/recipient/identity, exact granted and ungranted repositories,
factory renewal beyond expiry and current grant withdrawal. Already-issued JWTs
retain their native expiry boundary. Preserve existing owner/provider data and
clean up only test-owned resources. Actual Linux ingress/native proxy and browser
owner isolation remain separate gates in the execution plan.

Historical `first-path lore-direct`/Keycloak lifecycle receipts remain historical;
their removed provisioning helper is not an active installation command. No new
qualification launcher replaces it. Use component-owned native test harnesses and
the coordinated Linux acceptance lane. Lore runs no hook; native protection and
ordinary permissions keep their existing meaning.

## Hub and Time preparation

Use the [local identity, catalog and native runtime preparation](HUB_TIME.md) after
the qualified ingress fixture. Its trusted helper configures dedicated IdP clients
and component files without deploying workloads. Actual Hub/Time qualification
and the prepared Lore/factory build remain execution gates.

## Prepared Time factory

Removed. The prepared Time workflow, its publisher and factory policies and the
`vv-lore-time` profile used an installation-wide `lore-factory` Hub client. Factory
sandboxes replace it: Hub serves each pushed branch from its own sandbox with a
per-instance Lore client ([Kubernetes inputs](../kubernetes/README.md#factory-sandbox-policy-and-providers)).
Historical fixture receipts remain in the execution checklist.

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
