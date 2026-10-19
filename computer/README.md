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
- Native Lore: source commit `821da217` (native OIDC refresh over storage base
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
trusted native workload template. The inherited factory CA must trust both the
private Lore service and the selected public/native endpoints.

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
  --env VEOVEO_LORE_REMOTE=grpcs://10.231.10.10:41337
```

Replace the private Lore address with the verified installation endpoint. The
template supplies image/environment/resources/driver settings only. Hub clones
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
reference is a locator only. Provisioning authority is a separate trusted native
OpenShell identity with workspace/provider/sandbox creation rights; the ordinary
sandbox-only forwarder is insufficient. Never inject that authority into Computers.

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

Lore is different: each user explicitly runs native
`lore auth login "$VEOVEO_LORE_REMOTE" --no-browser` on first use and completes the
IdP prompt. `VEOVEO_LORE_REMOTE` is installation instruction input, not a Lore-native
endpoint setting. Native Lore gets OIDC issuer/client/scopes from its server and
uses its own persistent token store; later expiry uses native refresh. Existing
old grants without refresh-client binding require one new login. Neither factory
tokens nor shared human credentials are injected. [The native OIDC fragment](lore-oidc.example.toml)
shows server metadata configuration; merge it with the actual server's existing
claims/audiences/TLS/storage and registered public client. Review native Lore's
IdP discovery/device/token host egress in `policy.yaml` too. The example public
issuer and private URI are not receipts that those endpoints exist or are live.

## Validation boundary

Before promotion, validate the JSON with this fork's native OpenCode Config.Info
schema after substituting synthetic opaque references; parse the policy through
native `openshell_policy::parse_sandbox_policy_file` and lint both native profiles.
Build the image from the reviewed local contexts without inference. Under UID1000,
check exact binary versions/digests, seeded ownership, writable HOME/XDG paths,
non-writable `/etc/computer`, and absence of a factory token. A native test-owned
Computer must then prove persisted OpenCode/Lore state across Stop/Start, runtime
loss and gateway restart, followed by exactly owned Delete cleanup. Only later
qualify real user login, Hub revocation/owner isolation and explicit MiMo tool use
on the coordinated Linux deployment. The component retention E2E receipt does not
prove this image, Hub browser ingress, real Lore login or model inference.
