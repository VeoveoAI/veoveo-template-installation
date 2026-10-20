# Private user Computers

This is trusted installation input for Hub's owner-scoped `/computer` entrypoint.
It is not a deployment controller or a qualification receipt. Hub authenticates
with its existing OAuth client and current membership, selects one native OpenShell
workspace/Computer per tenant and principal, and forwards only to that owner's
loopback server. The browser is redirected to `/computer/c/<opaque-id>/`. Native
OpenShell owns execution, credential mediation, persistent storage and forwarding.
The image has no independent browser login, issuer, factory identity or Docker access.

## Reviewed source inputs

Build for one explicit target platform; all three contexts must match it.

- Factory toolchain: installation commit `71e2c0b` or a reviewed descendant retaining
  the deterministic factory boundary. Select its inspected immutable image ID.
- Native Lore: source commit `821da21710a9085797dc8a9d98955c1f3af1c389` (native OIDC refresh over storage base
  `7785408e`). Build with the existing [Lore Dockerfile](../factory/Lore.Dockerfile)
  and the selected source/CA named contexts. Use the resulting immutable image ID.
- OpenCode: source commit `376324a9fe6870e6e9825e97bcd96d4948d84d5a`, including
  runtime base paths, trusted-local-proxy admission, mount-scoped browser caches and
  online-only UI. Use the final reviewed native platform package directory, with
  `bin/opencode`; an earlier prefix-only package does not contain all these changes.
- Native OpenShell gateway/driver: `ea9b680dc780c1f24fd0cbf703cbfaf9b04f2d40`,
  including persistent working storage and connected-target forwarding readiness.
  Select matching reviewed runtime/supervisor images through native configuration.

Source commits and local builds do not establish live deployment or user/model
acceptance. The remote Lore endpoint and its owning deployment must be verified
before rollout; this example does not change an existing Lore service or storage.

With the reviewed IDs and package directory supplied in the operator environment:

```sh
docker buildx build --platform "$COMPUTER_PLATFORM" --load \
  --build-context "factory=docker-image://$COMPUTER_FACTORY_IMAGE" \
  --build-context "lore=docker-image://$COMPUTER_LORE_IMAGE" \
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

`WORKDIR=/sandbox` is the native Docker persistent-volume target. `/sandbox/home`
contains HOME, XDG config/data/state/cache, OpenCode databases and native Lore's
credential/config/data directories. `/sandbox/projects` holds working trees. The
image seeds private UID/GID 1000 paths; native Docker volume copy-up must remain
enabled to preserve that ownership. `/etc/computer` is immutable installation
configuration outside the persistent mount. No secret is copied into the image.

The operator imports the global, nonsecret native provider profiles and creates the
trusted workload template using the existing authenticated native CLI:

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

The trusted Compose Lore service publishes only host loopback port 41337; native
OpenShell's backend host-gateway pin supplies this hostname without a host DNS
edit. Require the matching Lore leaf DNS SAN and existing CA trust before use;
see [the endpoint requirements](LORE_HUB.md). Replace the endpoint only as a
consistent installation input. The Computer's exact Lore TCP rule uses `tls: skip`
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
disabled. The native policy admits its exact chat-completion endpoint and the
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

The [native OIDC fragment](lore-oidc.example.toml), Hub contribution/client records
and exact OAuth REST paths in policy must be selected together. Native private
gRPC remains direct TLS. The OAuth paths share Hub's authority but are disjoint
from managed MCP; there is no transparent TCP overlap, issuer host alias or broad
network fallback. These examples are not deployed credentials or real-login
receipts. Existing source/image pins remain unchanged until coordinated review.

## Validation boundary

Changes to `instructions.md` require rebuilding the Computer image because the
Dockerfile copies it into `/etc/computer`; changing policy alone requires no
binary rebuild. The signed-transfer instructions in this slice need a new image.

Before promotion, validate the JSON with this fork's native OpenCode Config.Info
schema after substituting synthetic opaque references; parse the policy through
native `openshell_policy::parse_sandbox_policy_file` and lint both native profiles.
Also admit the selected Hub control policy/profile using the exact RPC/binding
boundary in [NATIVE_CONTROL.md](NATIVE_CONTROL.md).
Build the image from the reviewed local contexts without inference. Under UID1000,
check exact binary versions/digests, seeded ownership, writable HOME/XDG paths,
non-writable `/etc/computer`, and absence of a factory token. A native test-owned
Computer must then prove persisted OpenCode/Lore state across Stop/Start, runtime
loss and gateway restart, followed by exactly owned Delete cleanup. Only later
qualify real user login, Hub revocation/owner isolation and explicit MiMo tool use
on the coordinated Linux deployment. The component retention E2E receipt does not
prove this image, Hub browser ingress, real Lore login or model inference.
