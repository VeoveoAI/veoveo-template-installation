# Computer-enabled Hub native authority

Select this mode explicitly when enabling Computers. The existing
[`openshell/hub.yaml`](../openshell/hub.yaml) and
[`hub-entra.example.yaml`](../openshell/hub-entra.example.yaml) remain the
forwarder-only mode. Their `vv-hub-forwarder` identity is intentionally insufficient
for provisioning. Do not add administrative grants to that existing client.

The Computer-enabled Hub uses **one new dedicated service identity** and native
provider, `vv-hub-native-control`, for both registered upstream forwarding and
Computer provisioning. Both clients select the same injected
`HUB_NATIVE_CONTROL_REFERENCE`; the example has one gateway origin and workload
CA. A single endpoint has one `credential_binding.provider`. Attaching a second
provider to dispatch the same ForwardTcp path is not supported by these policies:
native policy admission rejects conflicting endpoint metadata. Do not work around
this with host aliases, path aliases or disabled mediation.

## Native grants and custody

An IdP operator must provision a separate confidential client for this service.
Use the native gateway's actual issuer/audience and its configured admin role
(`openshell-admin` in the examples), plus exactly these seven native permissions:

- `config:read` (selected native backend discovery)
- `workspace:read`, `workspace:write`
- `sandbox:read`, `sandbox:write`
- `provider:read`, `provider:write`

The platform-admin role is real platform-wide trust: native CreateWorkspace and
reading the preinstalled global provider profiles require it. It also satisfies
workspace role checks; it does not bypass enabled scope checks. Do not use
`openshell:all`, an operator certificate, the operator's personal client, a user
Computer's Hub grant or an existing application/factory identity. The workload's
exact RPC egress further limits this service, but does not turn its native role
into workspace-only authority. Neither its reference nor its refresh material is
attached to user Computers.

For local Keycloak, assign only the configured admin role and exact seven scopes to
the new service account; remove unrelated inherited/default grants. The existing
`identity/configure-hub` helper prepares the forwarder-only identity, not this
client. Use the selected IdP's ordinary administration interface for the new
client and independently verify its native token claims before enabling Hub.

For Entra, assign the configured admin application role and the seven permission
application roles on the native OpenShell resource. The resource must emit its
verified audience and v2 issuer. With `roles_claim = "roles"` and
`scopes_claim = "roles"`, native authorization reads app-only permissions from
`roles`, not an absent delegated `scp`. The refresh profile requests the resource's
`/.default`; it does not request delegated scopes. Possessing a client secret or
successfully minting a token does not prove these assignments.

Choose one endpointless native credential profile:

| Installation | Profile | Hub policy |
|---|---|---|
| Local Keycloak | [hub-native-control.yaml](../openshell/providers/hub-native-control.yaml) | [hub-computer.yaml](../openshell/hub-computer.yaml) |
| Entra | [entra-hub-native-control.example.yaml](../openshell/providers/entra-hub-native-control.example.yaml) | [hub-computer-entra.example.yaml](../openshell/hub-computer-entra.example.yaml) |

Replace the profile's token URL/resource and the policy's exact origins together.
The native gateway owns client-credentials renewal. Hub captures only its opaque
reference at startup; calling the SDK again does not renew a copied access JWT.

## Trusted preparation and attachment

Run these native commands only with the separately authenticated trusted operator
CLI. The existing `vv-control` workspace must exist. Check for pre-existing profile,
provider and Hub sandbox names first; stop on conflicts rather than replacing
foreign state. Select exactly one profile file above, then supply the new client's
ID and secret privately in the operator environment. The ID is nonsecret. Never
write the secret into Hub configuration, a command argument, a user workspace or
an image.

```sh
(
  : "${HUB_CONTROL_PROFILE:?select one reviewed profile file}"
  : "${VV_HUB_NATIVE_CONTROL_CLIENT_ID:?new dedicated service client ID}"
  : "${VV_HUB_NATIVE_CONTROL_CLIENT_SECRET:?new dedicated service client secret}"
  export VV_HUB_NATIVE_CONTROL_CLIENT_SECRET
  openshell profile lint --workspace vv-control --file "$HUB_CONTROL_PROFILE"
  openshell profile import --workspace vv-control --file "$HUB_CONTROL_PROFILE"
  openshell provider create --workspace vv-control --name vv-hub-native-control \
    --type vv-hub-native-control --runtime-credentials
  openshell provider refresh configure --workspace vv-control vv-hub-native-control \
    --credential-key HUB_NATIVE_CONTROL_REFERENCE --strategy oauth2-client-credentials \
    --material "client_id=$VV_HUB_NATIVE_CONTROL_CLIENT_ID" \
    --secret-material-env client_secret=VV_HUB_NATIVE_CONTROL_CLIENT_SECRET
  openshell provider refresh rotate --workspace vv-control vv-hub-native-control \
    --credential-key HUB_NATIVE_CONTROL_REFERENCE
  openshell provider refresh status --workspace vv-control vv-hub-native-control \
    --credential-key HUB_NATIVE_CONTROL_REFERENCE
)
```

The initial rotation/status is preparation, not evidence of later renewal. Verify
native admission with that separate identity. No GetProviderCredentials export is
part of this workflow. Import the two global user data-plane profiles and create
the trusted Computer template using [the Computer instructions](README.md#state-and-native-governance)
before starting the Computer-enabled Hub.

Merge [hub.example.json](hub.example.json) into the installation's native Hub
configuration. Keep both gateway blocks on the same actual endpoint, workload CA
and `HUB_NATIVE_CONTROL_REFERENCE`. The fragment shows the reference service
bindings; retain the installation's reviewed `nativeUpstreams.bindings` rather than
blindly replacing them. Merge secrets by ID, retaining signing, registration and
IdP references. Configure the public PKCE client/callback, profile, Work Context
and memberships through the existing native Hub contracts. The public token URL
must agree with the browser origin, user Hub refresh profile and exact policy.

Install the reviewed JSON, policy at `/etc/veoveo/hub/computer-policy.yaml`, and
existing runtime settings into the read-only Hub configuration volume. The
trusted model-seed Env reference must be supplied separately as reviewed
installation input; it is not the native management identity. Do not put
`HUB_NATIVE_CONTROL_REFERENCE` or an access JWT into `hub.env`: native attachment
supplies that reference. Preserve existing Hub database/signing/registration
state and other services. The separately authorized unreleased Hub resource-binding
migration discards disposable OAuth continuity, not native service custody or
Computer working storage; see [issuer cutover](LORE_HUB.md#live-publication-and-first-cutover).

For a fresh Hub sandbox, select the matching Computer-enabled policy and reviewed
immutable Hub image. The existing native mount configuration supplies the
read-only configuration volume:

```sh
openshell sandbox create --workspace vv-control --name hub --from "$HUB_RUNTIME_IMAGE_ID" \
  --policy "$HUB_COMPUTER_POLICY" --provider vv-hub-native-control --no-auto-providers \
  --driver-config-json "$(cat openshell/hub-mounts.json)" --restart-policy on-failure --detach -- \
  /bin/sh -c 'set -a; . /etc/veoveo/hub/hub.env; set +a; exec /usr/local/bin/veoveo-hub serve --allow-loopback-hosts'
```

Set `HUB_COMPUTER_POLICY` to the reviewed local or Entra variant above. This is a
fresh-sandbox command, not a reconciliation script. For an existing Hub, coordinate
its native lifecycle cutover and preserve its durable application/configuration
volumes; do not delete it merely to run this example. Attach only the new control
provider in this mode, retiring the old forwarder attachment without changing its
IdP grants or other consumers. Keep the existing authenticated Hub exposure and
public-origin/TLS configuration.

## Exact network boundary and qualification

Both policies allow only the Hub binary to POST these native RPC paths beneath
`/openshell.v1.OpenShell/`, all bound to `vv-hub-native-control`:

| Purpose | RPCs |
|---|---|
| Existing upstreams and Computer browser bytes | ForwardTcp |
| Preinstalled global profiles and trusted template | GetProviderProfile, GetSandboxTemplate |
| Backend discovery | GetGatewayInfo (`config:read`, platform admin) |
| Per-owner workspace/template | GetWorkspace, CreateWorkspace, CreateSandboxTemplate |
| Workspace-local user/model custody | GetProvider, CreateProvider, ConfigureProviderRefresh, RotateProviderCredential |
| Native Computer lifecycle | GetSandbox, CreateSandbox, StartSandbox, StopSandbox |

One enforced REST endpoint selects `/openshell.v1.OpenShell/**` at the native
gateway authority, with the same provider binding and exactly the fifteen POST
rules above. The service-root selector chooses the HTTP route; it does not grant
additional RPCs. There is no wildcard allow rule, Exec, Delete, UpdateProvider,
profile import or operator-only credential export. Profiles are installed separately
by the trusted operator.

Keep these RPC rules in one endpoint configuration. The native
[HTTP/2 eligibility predicate](https://github.com/VeoveoAI/OpenShell/blob/9724fb1834dec157b4482a1da97338fd8ca1e50c/crates/openshell-supervisor-network/src/proxy/relay.rs#L45)
requires one supported REST inspection configuration before advertising `h2`.
Splitting the same authority into fifteen endpoints forces HTTP/1.1-only ALPN,
which tonic rejects with `NoApplicationProtocol`. Native
[per-stream admission](https://github.com/VeoveoAI/OpenShell/blob/9724fb1834dec157b4482a1da97338fd8ca1e50c/crates/openshell-supervisor-network/src/l7/http2.rs#L290)
still evaluates each actual method/path and resolves the bound credential; this
configuration change preserves the exact RPC authority and TLS verification.
The unbound public
POST `/oauth/token` admits the ordinary browser PKCE code exchange without injecting
a management credential. The local variant also permits its exact IdP code-token
POST; the Entra variant retains its existing token/JWKS rules. Existing database
and issuer-key routes remain. Own-issuer JWT verification reads Hub's signing keys
locally; no extra public JWKS egress is required for that path.

Before rollout, use native policy parsing/ambiguity admission and profile linting.
Confirm native policy construction produces one REST endpoint configuration, with
exactly the fifteen POST allow rules and unchanged provider/binary bindings;
there must be no wildcard allow rule. These structural checks and the reviewed
per-stream evaluator do not establish successful live RPCs or an observed denial.

Deployment acceptance must verify TLS ALPN `h2`, actual issuer/audience/roles/scopes
and the required RPCs with test-owned resources, including denial of an unlisted
RPC such as `/openshell.v1.OpenShell/DeleteSandbox`. Also prove ordinary user
data-plane identity separation, self PKCE exchange and native control renewal.
These examples do not claim that an IdP client, provider, endpoint or deployed
Computer exists, or that an unavailable CLI evaluator/prover was executed.
