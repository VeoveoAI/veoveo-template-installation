# Private user Computers

This is trusted installation input for Hub's owner-scoped `/computer` entrypoint.
It is not a deployment controller or a qualification receipt. Hub authenticates
with its existing OAuth client and current membership, selects one native OpenShell
workspace/Computer per tenant and principal, and forwards only to that owner's
loopback server. The browser is redirected to `/computer/c/<opaque-id>/`. Native
OpenShell owns execution, credential mediation, persistent storage and forwarding.
The image has no independent browser login, issuer, factory identity or Docker access.

## Reviewed source inputs

Build for one explicit target platform; all named contexts must match it. The
component pins below identify verified source revisions in the maintained VeoveoAI
forks, not upstream release versions. Source selection alone is not build or
deployment acceptance.

- Factory toolchain: this revision's [factory Dockerfile](../factory/Dockerfile),
  UID/GID 10001. Select its inspected immutable image digest.
- Native Lore: source commit `4b7b7e236419414b4a0c67ecd66ca2409f19d35c`, retaining
  native OIDC refresh and S3/PostgreSQL storage, ending a lagging subscriber's
  notification stream and reading `LORE_ACCESS_TOKEN` and `LORE_IDENTITY` when their
  flags are absent, which Computers and factory sandboxes need. Build with the existing
  [Lore Dockerfile](../factory/Lore.Dockerfile)
  and the selected source/CA named contexts. It is no build context of the Computer
  image: the Computer template mounts the Lore image read-only, as Hub and every
  factory sandbox do, and the installation names it once in
  [component-lock.json](../component-lock.json).
- Hub SDK CLI: the `cli` target of the SDK's Dockerfile, an image holding only
  `/usr/local/bin/veoveo-hub-sdk-cli`, which the Computer template mounts read-only
  beside Lore and the lock names the same way.
- OpenCode: source commit `c5fb62662c9c6fcb5064ef191956ca0da14e1af9`, including
  runtime base paths, trusted-local-proxy admission, mount-scoped browser caches and
  online-only UI, mounted-router navigation and route-relative titlebar history.
  The earlier Linux reference image digest was
  `sha256:12c2ba83bd6f3dea715d1f8ab25ee20dd81dfb8145868f9a193f73cb4ce695d4`.
  Both fresh owner homepages render. Direct mounted Settings, Back to app,
  New session and browser Back/Forward retain the Computer prefix. The final
  bounded exact-model acceptance passed; see the dated
  [qualification record](../kubernetes/README.md#reference-qualification-checkpoint--2026-10-20). Use the final reviewed native
  platform package directory, with
  `bin/opencode`; an earlier prefix-only package does not contain all these changes.
- Native OpenShell gateway/driver, supervisor and sandbox runtime:
  `ebd6a1c3d5ae0bb440f680a10f4457a9317f2f65` for the gateway and `ab37a698cfd03cdfb2559af7fd4263075233a48a` for the supervisor and sandbox, as the
  [K3s gateway](../kubernetes/README.md#native-openshell-gateway) describes. It mounts
  the Lore and Hub SDK CLI images that the Computer template names, records the
  network activity Computer collection reads and keeps a Computer's workspace across
  replacement. Publish the three images to the local registry and select their
  digests through native configuration.

Source commits and local builds do not establish live deployment or user/model
acceptance. The remote Lore endpoint and its owning deployment must be verified
before rollout; this example does not change an existing Lore service or storage.

With the reviewed IDs and package directory supplied in the operator environment:

```sh
docker buildx build --platform "$COMPUTER_PLATFORM" --load \
  --build-context "factory=docker-image://$COMPUTER_FACTORY_IMAGE" \
  --build-context "opencode=$COMPUTER_OPENCODE_PACKAGE" \
  -f computer/Dockerfile -t veoveo/computer:reviewed .
docker image inspect --format '{{.Id}}' veoveo/computer:reviewed
```

The Dockerfile has no fallback base image, package download or runtime self-update.
Publish the image to the installation's
[local registry](../kubernetes/README.md#logs-image-builder-and-registry) as
`veoveo/computer` and put its digest there, never the example tag, in the lock as
`computer`, from which the installer's `templates` step fills the trusted native
workload template. The inherited factory trust must cover both the
private Lore service and the selected public/native endpoints. The Factory and
Lore `ca` named contexts contain only reviewed public `*.crt` files, exactly one
certificate per file. For the Linux private Lore endpoint, include its new
installation-specific Lore CA and the existing public CA that verifies native
storage TLS. Private CA/server keys stay on the trusted host, outside all build
contexts. The Dockerfiles install these public certificates through the standard
system trust update; Computer inherits Factory trust. A context containing only
`ca.crt` remains valid. Rebuild Lore, Factory and Computer when this trust input
changes; do not replace the storage server certificate or export a native gateway
private key. Supervisor's separate build recipe is unchanged by this input seam.

## State and native governance

`/sandbox` is the person's home: `HOME=/sandbox` and `WORKDIR=/sandbox`, the
native persistent-workspace target. The platform never writes into it. The image
has only the empty mount point, owned by UID/GID 10001 like the selected native
runtime, and sets no XDG or Lore path variables: OpenCode and native Lore create
their own configuration, data, state and cache directories under `HOME`. Docker
keeps `/sandbox` in its configured persistent workspace volume. Kubernetes uses
OpenShell's automatic per-sandbox workspace PVC with sandbox UID/fsGroup ownership;
its native seeding finds nothing to copy and writes only its own marker. Do not add
a shared explicit `/sandbox` claim or a copy-up Job. Everything outside `/sandbox`
and `/tmp` comes with the Computer version and is read-only; `/etc/computer` holds
the immutable connection configuration. No secret is copied into the image.

On Linux Kubernetes the installer's `profiles` step imports the global, nonsecret
native provider profiles and its `templates` step creates the trusted workload
template, the [automatic-PVC template](../kubernetes/WORKLOADS.md#hub-and-automatic-private-computers)
with its [Kubernetes driver configuration](../kubernetes/computer-driver-config.json),
which mounts Lore at `/opt/veoveo/lore` and the Hub SDK CLI at
`/opt/veoveo/hub-sdk-cli`. The **Mac Docker development fixture** has no installer:
the operator imports the profiles and creates its template with the existing
authenticated native CLI. OpenShell's Docker driver has no image volumes, so the
Mac fixture's Computer has neither CLI:

```sh
openshell profile lint --global --from computer/providers
openshell profile import --global --from computer/providers
openshell sandbox template create computer --workspace installation \
  --image "$COMPUTER_IMAGE_ID" --cpu 2 --memory 4Gi \
  --driver-config-json '{"docker":{"persistent_workspace":true}}' \
  --env SSL_CERT_DIR=/etc/ssl/certs \
  --env VEOVEO_LORE_REMOTE=grpcs://host.openshell.internal:41337
```

`computers.forwardingConnectionLimit` is the Hub's finite global forwarding bound
(default and reference: 512). The native gateway examples explicitly configure
independent TCP limits of 128 active/pending connections per sandbox and 1024
active/512 pending per replica. On Linux they are `openshell.gateway.tcp_forwarding`
in [openshell.values.yaml](../kubernetes/openshell.values.yaml), which the
installer's `openshell` step applies, and the chart restarts the gateway when its
configuration changes; the Mac fixture sets them in trusted gateway TOML and
restarts the gateway when changing them. SSH limits remain unchanged. Warm HTTP
requests open their native tunnel immediately and fail at an actual configured
limit, with no fixed retry delay. Current browser-owner/session admission remains
mandatory.

`computers.collectAfterSeconds` (reference: 7200) is how long a Computer may stay idle,
with no browser request through Hub and no network traffic from the Computer, before
Hub deletes its sandbox. `~` is kept and the next entry creates the sandbox at the
current version, so collection loses only running programs and `/tmp`, as a restart
does. Every working agent calls its model through OpenShell's proxy, so an agent at
work keeps its Computer; a long job with no network traffic does not.

The supported Linux path uses the private Kubernetes Lore Service at
`grpcs://lore.veoveo-system.svc.cluster.local:41337`. Select that same endpoint in
the versioned Computer template, Hub deployment data and the global
[`vv-lore` profile](../openshell/providers/lore.yaml), whose inspected endpoint
Computers share with factory sandboxes; the
[Kubernetes Computer policy](../kubernetes/policies/computer-v1.yaml) has no Lore
rule of its own. The leaf must cover that Service DNS name; the image and the
gateway's proxy must trust its installation public CA;
see [the Kubernetes trust inputs](../kubernetes/WORKLOADS.md#private-service-names-and-trust).
No Compose-published port or host alias is needed for this Linux path.

The Mac Docker development fixture above instead uses `host.openshell.internal`
with its native backend host-gateway pin and a trusted Lore fixture published only
on host loopback port 41337. Its leaf must cover that hostname, with the same
end-to-end TLS verification; no host DNS edit is needed. Replace an endpoint only
as a consistent installation input. The fixture's exact Lore TCP rule in
[policy.yaml](policy.yaml) uses `tls: skip` to pass through the immediate gRPC TLS
handshake. The shared `vv-lore` profile's endpoint is the Kubernetes Service, so the
fixture's Lore calls carry the provider's placeholder unsubstituted: this fixture
does not provide the owner's Lore access. On Linux the proxy terminates Lore's TLS on
the profile's inspected endpoint, and Lore verifies the proxy through OpenShell's
injected `SSL_CERT_FILE`. `SSL_CERT_DIR` adds the image's installed public CA roots
alongside it; keep both. Set this template input
for existing reviewed images; future Computer builds also include it by default.
The template
supplies image/environment/resources/driver settings only. Hub clones
it into the owner's workspace, supplies nonsecret `PUBLIC_BASE_PATH`, and creates
with `openshell.nvidia.com/retention=persistent`, the fixed command from
[the Hub fragment](hub.example.json), native restart policy `Always`, native
[policy](policy.yaml), and the centrally assigned provider instances. Template
annotations do not substitute for the explicit Create annotation. Do not expose a
network service for the app; Hub uses authenticated native ForwardTcp to literal
`127.0.0.1:4096` after owner admission. The app's explicit trusted-local-proxy mode
requires foreground loopback and refuses configured passwords/pairing/publication.
The outer gateway must authenticate every navigation and stream. Online-only UI
serving and private/no-store proxy responses require fresh navigation to reach
Hub admission. A mount path or browser-cache namespace is not an origin security
boundary; live two-user and account-switching acceptance remains required.

Native retention preserves the stable sandbox ID/spec on runtime loss; Start can
recreate the missing runtime with fresh native runtime identity. `/sandbox` is kept
across restarts: Stop/Start, provisioning failures and a native Delete that keeps
the workspace retain it, and the next sandbox of the same name in the same
workspace adopts the kept volume unchanged. The sandbox name comes from the
Computer ID, so the Computer's URL stays too. Hub never stops or starts a Computer:
its first creation, a restart and an update all delete the sandbox keeping the
workspace and create one at the version they start, which Hub records once it is
ready. An ordinary Delete removes the exactly owned workspace volume; that is how a Computer is removed for good. A crash during
deletion can retain an orphan requiring native operator reconciliation; absence of
a container is not evidence that files are disposable. User files remain unpublished
until pushed/uploaded, despite this lifecycle durability. Factory sandboxes are
separate: one per served branch.

## Credentials, endpoints and user login

Review public origin replacements together in `opencode.json`, `policy.yaml`,
`providers/hub.yaml` and Hub configuration. `hub.example.json` is a fragment merged
into a valid native Hub control plane, not a second configuration schema: declare
its public PKCE+refresh client, callback, profile/resource, Work Context and current
user membership through the existing Hub contracts. Its environment secret
reference is a locator only. Select the [Computer-enabled native control installation](NATIVE_CONTROL.md): one
new dedicated OpenShell service identity/provider supplies both upstream forwarding
and Computer provisioning through the same native reference. The ordinary
sandbox-only forwarder remains unchanged and is insufficient. Native workspace
creation/global-profile access require the configured platform-admin role plus
the six scoped permissions; that authority never enters user Computers.

Hub creates workspace-local `computer-hub` from global `vv-computer-hub` and
`computer-model` from `vv-computer-model`. The user's existing Hub OAuth refresh
grant is handed to native ConfigureProviderRefresh, followed by native
RotateProviderCredential before setup is ready. Refresh scopes stay omitted.
OpenShell is the sole custodian; OpenCode MCP OAuth is disabled and receives only
its native opaque `COMPUTER_HUB_REFERENCE`. Reauthorization is fail-closed while
custody is pending. Browser sessions bind to the native Hub family and current
owner/membership, including revocation of open streams, independently of refresh
rotation. No operator-only credential export is used.

Hub resolves the trusted Env `ProviderApiKey` reference to seed the workspace-local
model provider; OpenCode sees only its native `DEEPINFRA_API_KEY` placeholder.
The explicit model is `deepinfra/XiaomiMiMo/MiMo-V2.6-Pro`, present in the selected
OpenCode bundled catalog. Catalog fetch, warming, self-update and sharing are
disabled. Computer configuration sets `snapshots: false` and disables the built-in
`opencode.vcs.git` and `opencode.vcs.hg` plugins. Native Lore supplies source history
and undo. It also disables the `opencode.provider.ollama`, `opencode.provider.lmstudio`
and `opencode.provider.vllm` plugins, which otherwise probe loopback model servers
(`127.0.0.1:11434`, `:1234` and `:8000`) every 30 s; a Computer runs none. The native policy admits its exact chat-completion endpoint and the
configured Hub MCP path; unsupported egress fails closed. MCP methods still face
Hub's current route authorization. Installing configuration is not a model call
or evidence of real inference. Run paid acceptance only after deterministic,
component and deployed probes pass, with this exact model selected.

Artifact object transfers use Hub-issued upload/download grants and native
`/usr/bin/curl` from the agent's Bash tool. Signed object URLs are `/artifacts/`
paths on the public host. The `artifact_bytes` policy admits only that binary, and
only PUT and GET on `/artifacts/**`, at an inspected endpoint with no credential
binding: the proxy forwards the signed method, Host, path, query, headers and body
unchanged, and curl trusts the proxy through OpenShell's `CURL_CA_BUNDLE`. It is
disjoint from the Hub MCP path, which OpenCode alone reaches. Replace the host
together with Artifact's signing endpoint and the tunnel's `/artifacts/` route; do
not add arbitrary HTTPS egress or OpenCode itself to this object-byte rule.
Hub/model credentials must never accompany signed object requests. Keep signed
URLs/headers out of reports and tracing, use the returned method/URL/required
headers unchanged, and complete the Hub contract's finalize step where required.
Qualify actual upload/download byte counts and hashes separately from metadata/MCP
success.
See the [native signed-authority contract](../openshell/REFERENCE.md#native-seaweedfs-https-and-signed-authority).

Lore acts as the Computer's owner after the owner's one Hub sign-in; there is no
Lore login. Hub's [Lore credentials](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#native-contributed-credential-lifecycle) apply: Entra proves
identity through Hub's existing broker, Hub manages explicit live repository
assignments and signs native claims, and unchanged Lore enforces its own contract.
Each custody handoff gives the Computer's own Hub client a new secret in the
gateway's `oauth2_client_credentials` refresh custody of the workspace-local
`computer-lore` provider, created from global `vv-lore`, and rotates it; a running
sandbox's proxy uses the new token. For each Lore request that names a repository, the
sandbox's supervisor asks the gateway for the owner's token of that repository, which
the gateway mints with the same client, and holds it until shortly before it expires;
requests that name none carry the owner's token without a repository. The sandbox receives only the provider's
placeholder in `LORE_ACCESS_TOKEN` and the owner's principal in `LORE_IDENTITY`,
which the native Lore CLI reads; the proxy substitutes the token on the profile's
inspected endpoint. No human token is copied to a factory or another Computer.

An operator's native exec session need not inherit the workload's environment.
Check Lore from the Computer's OpenCode shell, which inherits `HOME` and both
variables.

The [Computer handbook](instructions.md) explains the home, explicit
Lore/Artifact publication, and served branches: a pushed branch of a server
repository with `openshell/run` is served, and Hub's tools select which branch of
each server the account uses. A checkout alone serves or selects nothing.
`computer/instructions.md` is the handbook's one source. Installation supplies its
Markdown as plain `computers.handbook` text in the trusted Hub control document,
and Hub serves it as the MCP server `instructions` of its Computer endpoint
(`computers.profile`) to Computer clients (`computers.oauthClient`), so every MCP
client learns it when it connects; OpenCode puts MCP server instructions in its
system prompt. Hub writes nothing into the Computer, and a handbook edit needs no
image rebuild, restart or new version: clients see it on their next connection.
An omitted or empty `handbook` serves no handbook. This is not a template language.
Guidance references the Computer's current native inputs; it does not copy
credentials, grants or project environment selections into a global prompt.

On Linux the installer fills the field from this file: the Hub step of
`installation.toml` names it as the seed's handbook, `veoveo-installer identity hub`
writes it into the complete seed, and the `hub-control-plane` step publishes a changed
handbook through the live admin control-plane API, preserving unrelated settings.

For the Mac fixture's initial trusted import, set the field in the complete seed
before `veoveo-hub validate` / `import-control-plane`:

```sh
umask 077
jq --rawfile handbook computer/instructions.md '.computers.handbook = $handbook' \
  "$PRIVATE/hub.json" > "$PRIVATE/hub.candidate.json"
```

To update the fixture's guidance later, read the fresh full control response using
the existing admin API, then prepare the ordinary publication candidate from the same
file:

```sh
umask 077
jq --rawfile handbook computer/instructions.md \
  '.controlPlane | .computers.handbook = $handbook' \
  "$PRIVATE/control-read.json" > "$PRIVATE/control-candidate.json"
```

Publish that full candidate through the same live admin control-plane API, preserving
unrelated settings. [hub.example.json](hub.example.json) omits the field so that the
handbook has no second copy.

The [native OIDC fragment](lore-oidc.example.toml), Hub contribution/client records
and exact OAuth REST paths in policy must be selected together. Native private
gRPC remains direct TLS. The OAuth paths share Hub's authority but are disjoint
from managed MCP; there is no transparent TCP overlap, issuer host alias or broad
network fallback. These examples are not deployed credentials or real-login
receipts. Existing source/image pins remain unchanged until coordinated review.

## Validation boundary

Handbook changes are published through `computers.handbook` and reach clients on
their next MCP connection; no image rebuild or restart is needed. Template, policy and
`computers` setting changes make a new Computer version, which each Computer takes at
its owner's next restart; policy changes alone require no binary rebuild.

Before promotion, validate the JSON with this fork's native OpenCode Config.Info
schema after substituting synthetic opaque references; parse the policy through
native `openshell_policy::parse_sandbox_policy_file` and lint both native profiles.
Also admit the selected Hub control policy/profile using the exact RPC/binding
boundary in [NATIVE_CONTROL.md](NATIVE_CONTROL.md).
Build the image from the reviewed local contexts without inference. Under UID/GID 10001,
check exact binary versions/digests, a writable `HOME`, non-writable `/etc/computer`,
and absence of a factory token. A native test-owned Computer must then prove
persisted OpenCode/Lore state across Stop/Start, runtime loss, gateway restart and a
delete that keeps the workspace followed by a sandbox of the same name, followed by
exactly owned Delete cleanup. Only later
qualify real user login, Hub revocation/owner isolation and explicit MiMo tool use
on the coordinated Linux deployment. The component retention E2E receipt does not
prove this image, Hub browser ingress, real Lore login or model inference.
