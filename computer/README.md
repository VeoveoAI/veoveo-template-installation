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

- Factory toolchain: UID/GID 10001 installation commit
  `983a9f915f480996c974d98e1519c6d1089cb6f4` or a reviewed descendant retaining
  the deterministic factory boundary. Select its inspected immutable image digest.
- Native Lore: source commit `6b7a0e598b8fbbaed82c777109135ddb25873bbf`, retaining
  native OIDC refresh and S3/PostgreSQL storage. Build with the existing
  [Lore Dockerfile](../factory/Lore.Dockerfile)
  and the selected source/CA named contexts. Use the resulting immutable image ID.
- OpenCode: source commit `ed62aad52c80ecdf4297310ce84305aff5717b7d`, including
  runtime base paths, trusted-local-proxy admission, mount-scoped browser caches and
  online-only UI, mounted-router navigation and route-relative titlebar history.
  The Linux reference image is
  `registry.cloudflare.com/cd9522254303bfb3d2322a0d9b82764f/veoveo-computer@sha256:12c2ba83bd6f3dea715d1f8ab25ee20dd81dfb8145868f9a193f73cb4ce695d4`.
  Both fresh owner homepages render. Direct mounted Settings, Back to app,
  New session and browser Back/Forward retain the Computer prefix. The final
  bounded exact-model acceptance passed; see the dated
  [qualification record](../kubernetes/README.md#reference-qualification-checkpoint--2026-10-20). Use the final reviewed native
  platform package directory, with
  `bin/opencode`; an earlier prefix-only package does not contain all these changes.
- Environment helper: build Hub's `environment-artifact` target from the same
  checked Hub source revision used by activation. This exports only the
  `/usr/local/bin/veoveo-environment` artifact; it is an initial image input, not
  a per-edit image publication requirement. Select the inspected immutable digest.
- Native OpenShell gateway/driver: `53e3665811a72dd1555edb18aeb4fedef4158dbd`,
  retaining Kubernetes workspace persistence, connected-target forwarding readiness,
  destination-CA staging, admitted GPU allocation propagation and native recovery
  of a terminated admitted supervisor under `always`/`on-failure`. Temporarily
  Pending Pods and stale controller status preserve running intent; fresh runtime
  generations replace terminal Pods while retaining supported persistent state.
  The selected supervisor is
  `d7a990083214e8a618f05b1b471aeeac6d9aa82a`, retaining mediated HTTP/2 stream
  turnover and trusted gateway Service-DNS resolution, and renewing an expired
  native Kubernetes launch credential pair before policy discovery. Fresh bundles
  skip rebootstrap to avoid the initial runtime-binding persistence race. The
  sandbox runtime remains
  `846de5fa01234a22827915b3f4e5b9092367078b`. Select each component's reviewed
  immutable image through native configuration. Both Kubernetes Pods deliberately
  use `Never`; host recovery uses fresh generations rather than restarting an old
  Pod with its expired bootstrap. The third Linux host reboot restored the owned
  `always` workloads without manual Starts; mounted browser navigation and the
  final bounded exact-model acceptance also pass in the recorded scope.

Source commits and local builds do not establish live deployment or user/model
acceptance. The remote Lore endpoint and its owning deployment must be verified
before rollout; this example does not change an existing Lore service or storage.

With the reviewed IDs and package directory supplied in the operator environment:

```sh
docker buildx build --platform "$COMPUTER_PLATFORM" --load \
  --build-context "factory=docker-image://$COMPUTER_FACTORY_IMAGE" \
  --build-context "lore=docker-image://$COMPUTER_LORE_IMAGE" \
  --build-context "environment=docker-image://$COMPUTER_ENVIRONMENT_IMAGE" \
  --build-context "opencode=$COMPUTER_OPENCODE_PACKAGE" \
  -f computer/Dockerfile -t veoveo/computer:reviewed .
docker image inspect --format '{{.Id}}' veoveo/computer:reviewed
```

The Dockerfile has no fallback base image, package download or runtime self-update.
Promote the inspected immutable image identity, never the example tag, into the
trusted native workload template. The inherited factory trust must cover both the
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

`WORKDIR=/sandbox` is the native persistent-workspace target. `/sandbox/home`
contains HOME, XDG config/data/state/cache, OpenCode databases and native Lore's
credential/config/data directories. `/sandbox/projects` holds working trees. The
image seeds private UID/GID 10001 paths, matching the selected native runtime.
Docker preserves them through its configured persistent workspace volume and native
copy-up. Kubernetes uses OpenShell's
automatic per-sandbox workspace PVC and native image-workdir seeding with sandbox
UID/fsGroup ownership; do not add a shared explicit `/sandbox` claim or another
copy-up Job. `/etc/computer` is immutable connection configuration outside the
persistent mount; public managed instructions live in the private home.
No secret is copied into the image.

The operator imports the global, nonsecret native provider profiles and creates the
trusted workload template using the existing authenticated native CLI. The example
below selects the **Mac Docker development fixture**; Linux Kubernetes uses the
[automatic-PVC template command](../kubernetes/WORKLOADS.md#hub-and-automatic-private-computers)
without the Docker driver-config flag:

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
active/512 pending per replica. Set these in trusted gateway TOML and restart the
gateway when changing them; SSH limits remain unchanged. Warm HTTP requests open
their native tunnel immediately and fail at an actual configured limit, with no
fixed retry delay. Current browser-owner/session admission remains mandatory.

The supported Linux path uses the private Kubernetes Lore Service at
`grpcs://lore.veoveo-system.svc.cluster.local:41337`. Select that same endpoint in
the versioned Computer template, Hub deployment data and
[Kubernetes Computer policy](../kubernetes/policies/computer.yaml). The leaf must
cover that Service DNS name and the image must trust its installation public CA;
see [the Kubernetes trust inputs](../kubernetes/WORKLOADS.md#private-service-names-and-trust).
No Compose-published port or host alias is needed for this Linux path.

The Mac Docker development fixture above instead uses `host.openshell.internal`
with its native backend host-gateway pin and a trusted Lore fixture published only
on host loopback port 41337. Its leaf must cover that hostname, with the same
end-to-end TLS verification; no host DNS edit is needed. Replace an endpoint only
as a consistent installation input. The Computer's exact Lore TCP rule uses `tls: skip`
to pass through the immediate gRPC TLS handshake; Lore still verifies the server
certificate and hostname. `SSL_CERT_DIR` adds the image's installed public CA
roots alongside OpenShell's injected `SSL_CERT_FILE`; keep both so native Lore
trusts the private service and the supervised OAuth route. Set this template input
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
recreate the missing runtime with fresh native runtime identity. Stop/Start and
provisioning failures retain files. Explicit Delete removes the exactly owned
workspace volume. A crash during deletion can retain an orphan requiring native
operator reconciliation; absence of a container is not evidence that files are
disposable. User files remain scratch until pushed/uploaded, despite this routine
lifecycle durability. Factory jobs remain separate deterministic executions.

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
disabled. The built-in Git/Mercurial providers and turn snapshots are disabled;
the agent uses native Lore for source history and undo. The native policy admits its exact chat-completion endpoint and the
configured Hub MCP path; unsupported egress fails closed. MCP methods still face
Hub's current route authorization. Installing configuration is not a model call
or evidence of real inference. Run paid acceptance only after deterministic,
component and deployed probes pass, with this exact model selected.

Artifact object transfers use Hub-issued upload/download grants and native
`/usr/bin/curl` from the agent's Bash tool. The `artifact_bytes` policy admits
only that binary to `objects.veoveo.example.com:443` as unbound TCP, preserving
end-to-end HTTPS and SigV4 method, authority, path/query and headers without
HTTP interception or credential injection. Replace this exact authority together
with Artifact's signing endpoint and verified TLS route; do not add arbitrary
HTTPS egress or OpenCode itself to this object-byte rule. Hub/model credentials
must never accompany signed object requests. Keep signed URLs/headers out of
reports and tracing, use the returned method/URL/required headers unchanged,
and complete the Hub contract's finalize step where required. Qualify actual
upload/download byte counts and hashes separately from metadata/MCP success.
See the [native signed-authority contract](../openshell/REFERENCE.md#native-seaweedfs-https-and-signed-authority).

Lore uses the user's native persistent login, separate from the Computer browser
and MCP grant. Select the [Hub-issued native Lore setup](LORE_HUB.md): Entra proves
identity through Hub's existing broker, Hub manages explicit live repository
assignments and signs native claims, and unchanged Lore enforces its own contract.
The native `lore auth login "$VEOVEO_LORE_REMOTE" --no-browser` command uses the
server-advertised public client/device/exchange settings; later expiry uses native
refresh. No human token is copied to a factory or another Computer.

For an operator's native exec qualification, explicitly use the image workload's
paths rather than assuming the exec session inherits its environment:

```sh
openshell --workspace "$COMPUTER_WORKSPACE" sandbox exec --no-tty --no-login-shell \
  "$COMPUTER_SANDBOX" -- env \
  HOME=/sandbox/home \
  XDG_CONFIG_HOME=/sandbox/home/.config \
  XDG_DATA_HOME=/sandbox/home/.local/share \
  XDG_STATE_HOME=/sandbox/home/.local/state \
  XDG_CACHE_HOME=/sandbox/home/.cache \
  LORE_GLOBAL_PATH=/sandbox/home/.local/share/lore \
  LORE_AUTH_PATH=/sandbox/home/.config/lore/auth \
  /usr/local/bin/lore auth login "$VEOVEO_LORE_REMOTE" --no-browser
```

Use the same explicit paths for subsequent native exec Lore checks. The measured
exec defaults were `HOME=/sandbox` with no XDG or Lore path overrides, so login
there wrote `/sandbox/.config/lore/tokenstore.toml`; OpenCode shell tools read
`/sandbox/home/.config/lore/auth/tokenstore.toml`. A successful exec login in the
first store does not authenticate the workload's separate store. Ordinary Lore
commands inside the Computer's OpenCode shell already inherit the image paths.

Optional personal-workspace onboarding is selected by the Hub contribution's
`personalWorkspace.deployment`, an explicit registered native deployment ID.
Computer entry starts preparation outside browser custody; `GET /computer/onboarding`
reads the last typed result without starting work. Lore unavailability or missing
user auth leaves Computer access usable. The owner's repository association survives
Computer replacement, and changing the installation default does not replace it.

Hub delivers public repository coordinates to
`/sandbox/home/.config/computer/workspace.env` through native owner-scoped Exec.
Onboarding `ready` means that optional repository default and descriptor are
prepared; it does not assert native authentication or a checkout. Computer entry
does not log in or clone. The user or agent chooses whether to use the default or
another authorized repository, and chooses the branch and destination through
ordinary native Lore commands. Existing checkouts are not changed. The descriptor
contains no tokens; native Lore owns authentication, refresh and checkout behavior.
The automatically loaded [Computer handbook](instructions.md) explains private
workspace layout, explicit Lore/Artifact publication and environment apply/selection.
Without explicit environment selection, MCP uses the Hub profile's configured
default environment. A checkout alone never selects or applies an environment.

The image seeds `/sandbox/home/.config/computer/instructions.md` before OpenCode's
first instruction discovery, and its config loads only that path. Installation
supplies the shared Markdown as plain `computers.handbook` text in the trusted Hub
control document; [hub.example.json](hub.example.json) includes it. This is not a
template language. Guidance references the Computer's current native inputs and
optional public descriptor; it does not copy credentials, grants or project
environment selections into a global prompt.

Each admitted `/computer` entry or completed login schedules best-effort delivery
from the **current** live catalog through the existing owner-scoped native Exec.
Hub atomically replaces the file; OpenCode's existing watcher refreshes subsequent
model steps. Delivery is independent of personal repository preparation and never
blocks entry or invokes login/clone. Omitting `handbook` skips replacement;
`"handbook":""` explicitly clears the managed text. A live config update reaches
the Computer on its next entry, not through immediate fleet-wide push.

To update guidance, read the fresh full control response using the existing admin
API, then prepare the ordinary publication candidate with the selected Markdown:

```sh
python3 - "$PRIVATE/control-read.json" computer/instructions.md "$PRIVATE/control-candidate.json" <<'PY'
import json
from pathlib import Path
import sys
control = json.loads(Path(sys.argv[1]).read_text())["controlPlane"]
control["computers"]["handbook"] = Path(sys.argv[2]).read_text()
output = Path(sys.argv[3])
output.write_text(json.dumps(control, indent=2) + "\n")
output.chmod(0o600)
PY
```

Publish that full candidate through the same live admin control-plane API, preserving
unrelated settings. For initial trusted import, include the same Markdown field in
the complete seed before `veoveo-hub validate` / `import-control-plane`. Install the
Hub binary that understands this field before publishing it. Existing snapshots do
not need rewriting because guidance is optional. Initial Computer packaging must
select this managed instruction path; later text updates need no image rebuild.

The [native OIDC fragment](lore-oidc.example.toml), Hub contribution/client records
and exact OAuth REST paths in policy must be selected together. Native private
gRPC remains direct TLS. The OAuth paths share Hub's authority but are disjoint
from managed MCP; there is no transparent TCP overlap, issuer host alias or broad
network fallback. These examples are not deployed credentials or real-login
receipts. Existing source/image pins remain unchanged until coordinated review.

## Validation boundary

Changing the image's initial handbook seed or configured instruction path requires
a Computer image rebuild. Once that path is installed, publish text changes through
`computers.handbook` and enter the Computer again; no image or OpenCode restart is
needed. Policy changes alone require no binary rebuild.

Before promotion, validate the JSON with this fork's native OpenCode Config.Info
schema after substituting synthetic opaque references; parse the policy through
native `openshell_policy::parse_sandbox_policy_file` and lint both native profiles.
Also admit the selected Hub control policy/profile using the exact RPC/binding
boundary in [NATIVE_CONTROL.md](NATIVE_CONTROL.md).
Build the image from the reviewed local contexts without inference. Under UID/GID 10001,
check exact binary versions/digests, seeded managed-instruction ownership, writable
HOME/XDG paths, non-writable `/etc/computer`, and absence of a factory token. A native test-owned
Computer must then prove persisted OpenCode/Lore state across Stop/Start, runtime
loss and gateway restart, followed by exactly owned Delete cleanup. Only later
qualify real user login, Hub revocation/owner isolation and explicit MiMo tool use
on the coordinated Linux deployment. The component retention E2E receipt does not
prove this image, Hub browser ingress, real Lore login or model inference.

## Explicit environment source apply

The packaged `veoveo-environment apply --definition FILE --local COMPONENT=DIRECTORY`
uses the existing `COMPUTER_HUB_URL` and opaque `COMPUTER_HUB_REFERENCE` supplied by
native custody. `watch` accepts the same arguments and submits only changed selected
content/definitions; neither command synchronizes the Computer home. The definition
is Hub's one typed environment definition, not a second source inventory. Do not
copy browser tokens or service-client credentials into the project.

The Computer native policies admit the helper's platform MCP operations and exact
owner environment REST paths through the existing `computer-hub` provider. Signed
Artifact byte transfers use the configured object authority without HTTP inspection,
credential injection or URL rewriting. Source transfer uses the configured platform
profile, independently of the application environment being prepared. Ordinary
application MCP connections select their environment separately. These source and
packaging changes do not extend the historical qualification above; the complete
apply/watch and retained-worker path needs deployed qualification.
