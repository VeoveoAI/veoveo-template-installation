# Hub-issued native Lore credentials

Select this mode for the hosted Computer installation. Entra authenticates the
person into Hub. Hub's existing issuer owns explicit repository registrations and
assignments; unchanged Lore verifies its existing signed resource-grant contract.
There is no additional issuer process, custom Lore role parser, permission service,
Desktop adapter or refresh daemon. Local Keycloak remains identity/management bootstrap; it is not a parallel
repository-grant provisioning path.

These are synthetic configuration examples, not deployed identities or image
receipts. Keep the reviewed Lore refresh-only source and current image pins until
new artifacts are explicitly reviewed. No Lore source change is required here.

## Match the native contracts

Merge [hub.example.json](hub.example.json) into a **complete** admitted Hub control
plane. Replace records by their typed identity, never concatenate duplicate IDs or
replace unrelated arrays wholesale. Retain existing database/signing/registration,
identity-provider, service, Computer ownership and working-storage configuration.
The fragment assumes these existing installation identities:

- Hosted authorization server `hub`, issuer `https://veoveo.example.com/oauth`,
  signing-key/JWKS configuration and enterprise identity provider `entra`.
- Policy revision `installation`, tenant `example`, attribution Work Context
  `default`, and Computer MCP profile/resource `computer` /
  `https://veoveo.example.com/mcp/computer`.
- Existing broker secret reference `hub-entra-client-secret`. Replace the synthetic
  broker client ID and use the existing Entra broker registration/secret. Its
  callback is `/oauth/callback`; it brokers both the Computer MCP audience and
  the separate Lore audience. This confidential broker is not the native public
  client and does not own repository assignments.
- The Computer's existing public PKCE+refresh client `computer-browser` and exact
  callback `https://veoveo.example.com/computer/callback`, with the current MCP
  profile/Work Context eligibility. Preserve its registration and memberships.

Replace these names consistently when the installation uses different admitted
IDs. Keep `computers.gateway` and `nativeUpstreams` on the same dedicated native
control reference described in [NATIVE_CONTROL.md](NATIVE_CONTROL.md). That
management identity is independent of all Lore users and factory grants.

| Native input | Matching Hub data |
|---|---|
| `jwt_issuer = ["https://veoveo.example.com/oauth"]` | Hosted server `hub` issuer and normal discovery/JWKS |
| `jwt_audience = ["https://veoveo.example.com/resources/lore"]` | Deployment `audience`; each native client's one `allowedResources` entry |
| `resource_claim = "resources"`, `resource_id_claim = "resource_id"`, `permission_claim = "permission"` | Fixed native typed claim encoding; not arbitrary JWT templates |
| `resource_id_template = "urc-{id}"` | Deployment `resourceIdTemplate` |
| OIDC `client_id = "lore-computer"` | Dedicated secretless public registration with device, refresh and public exchange grants |
| OIDC `scopes = ["lore:login"]` | Finite deployment/client scopes `lore:login` plus native-added `offline_access` |
| OIDC `scope_template = "lore-repo-{id}"` | Deployment `scopeTemplate`; factory profile's exact target scope |
| Private `grpcs://host.openshell.internal:41337` | Deployment `endpoint`, native server certificate SAN, policy hostname and `VEOVEO_LORE_REMOTE` |

The trusted Compose Lore service publishes only `127.0.0.1:41337:41337`.
Native OpenShell policy DNS maps `host.openshell.internal` to the backend-provided
host-gateway address; the trusted Docker supervisor reaches the host loopback
publication. No host `/etc/hosts` edit, workload Docker network attachment or
separate DNS service is required. Keep the policy's exact hostname, port and
executable. Remove the old Lore-container `allowed_ips: [10.231.10.10]`: native
host-gateway pinning takes precedence over that field. Do not replace it with a
loopback CIDR; ordinary `allowed_ips` rejects loopback ranges. The native backend
pin admits only its own host address, not arbitrary loopback destinations.

The Lore TLS leaf must contain **DNS SAN `host.openshell.internal`**, signed by the
existing trusted installation CA. Extend the current trusted OpenSSL leaf issuance
inputs with that DNS SAN while preserving the existing SANs, CA and private key
custody. A certificate valid only for `vv-lore.internal` or `10.231.10.10` is
insufficient for this client URI. Preserve the mounted certificate/key paths
(`vv-lore.internal.crt`/`.key` may retain their filenames); no new CA or image trust
change is needed when the existing issuer CA is unchanged. The Compose-only alias
`vv-lore.internal` and private address `10.231.10.10` remain internal service data.

Keep the endpoint hostname, certificate DNS SAN, CLI remote and template input
aligned. Hub derives the signed native `root_domains` recipient from its live
`lore.deployments[].endpoint` hostname. Publish that endpoint through the existing
live control-plane API; do not replace unrelated catalog data. This changes neither
the OAuth issuer/audience nor repository grants. Native raw TCP preserves Lore's
gRPC/TLS bytes; this does not use OpenShell's HTTP service-exposure route.

The `urc-{id}` native resource-ID format and `lore-repo-{id}` exchange selector
are separate fields. Both are installation data; exactly one canonical lowercase
32-hex repository ID is substituted. Do not register every dynamic repository
scope as an unrestricted OAuth scope. The finite login scopes bind the client;
the typed contribution parses the target and checks current exact assignments.

Select [lore-oidc.example.toml](lore-oidc.example.toml) in the trusted Lore server's
existing configuration, preserving TLS, S3/PostgreSQL and hooks. Native
`environment.endpoint.auth_url` must be absent for this resource-claim mode;
combining that external-auth-service selection with `resource_claim` is a native
configuration error. Do not replace it with GlobalGrants, allow-all or a new auth
service. Native `jwt_typ = ["at+jwt"]` matches Hub's access JWT header. The
server discovers Hub's verifier keys; no Hub signing key goes to Lore or users.

The public client has no secret. Its Login token carries stable native `identity`
and signed `root_domains` for the private recipient, with no repository grants.
Public RFC 8693 exchange returns the exact native `resources` entry allowed by the
current deployment/repository/tenant/principal assignment. Repository tokens are
not unrestricted login/exchange authority. Empty `actions: []` means ordinary
native repository access **including unprotected writes**, not read-only access.
Only native `owner`, `admin`, `obliterate`, `migrate` privileges may be explicitly
assigned. Neither Hub administration nor Work Context membership implies any
Lore repository access or `is_service_account` bypass.

The sample grants Alice/Bob the shared `1111…1111` repository, only Bob the
`2222…2222` repository, and a separate factory principal the shared repository.
Replace all synthetic IDs with reviewed native IDs and actual canonical Hub
principal IDs. Changing a user's directory app roles is not how this Hub-owned
repository data is managed. A Hub administrator can publish explicit assignments
through the existing administrative authority; being an administrator alone does
not add an assignment.

Hub registration/publication does not create native repository data or learn a
new ID from native Create. Use reviewed native repository commands and verify the
actual backend IDs/URLs. Native creator, catalog and branch semantics remain
unchanged: no automatic creator grant, new creation gate or branch ACL is added.
The selected native baseline catalog is non-disclosing; this is not a new
assignment-filtered listing. Operate on known registered repository URLs. Existing
native protection and any separately configured factory hook retain their own
scope and limitations; this fragment adds no hook assignment or service bypass.

## Native Computer network and login

[policy.yaml](policy.yaml) admits only the native Lore binary to these unbound
REST routes at the Hub authority:

- `GET /oauth/.well-known/openid-configuration`
- `POST /oauth/device_authorization`
- `POST /oauth/token` for native polling, repository exchange and refresh
- `GET /oauth/jwks.json`

The native client's stored Work Context is platform attribution only; it does
not add a repository-membership requirement or grant source access.

They are disjoint from OpenCode's managed `/mcp/computer` endpoint. No transparent
TCP rule, credential injection, host alias, broad `/oauth/**` rule or additional
network client is used. Native Lore uses its ordinary HTTPS proxy and system/native
CA support. Browser verification `/oauth/device` and Entra sign-in occur in the
human browser; they are not extra CLI egress rules. Private Lore gRPC remains
end-to-end direct TLS to the exact private address. Replace all advertised origins,
certificate trust and policy selectors together; native policy ambiguity/admission
and actual Linux proxy behavior still require qualification.

On first use the user runs the existing native command:

```sh
lore auth login "$VEOVEO_LORE_REMOTE" --no-browser
```

Follow the native verification URL/code and approve the correct client. Hub uses
its existing Entra broker for identity proof. Never copy an access/refresh token
into chat, shell arguments or another user's Computer. Native Lore keeps its own
grant under persistent HOME/XDG and renews it with the retained native refresh fix.
Its login is separate from the Computer browser/MCP grant. Grant withdrawal blocks
new repository issuance after live catalog adoption; cached native JWTs and
in-flight operations retain their ordinary expiry behavior. This sample selects
300-second tokens; it does not promise immediate native token revocation.

## Explicit factory identity and native refresh

Select [lore-time.example.yaml](../openshell/providers/lore-time.example.yaml),
using native profile/provider name `vv-lore-time`. Do not import over an unrelated
existing profile/provider. The existing deterministic
[factory RPC policy](../openshell/factory-time.yaml) already binds that provider.
Retain its limited read RPCs and exact dependency access. Native token permissions
are ordinary access; the workload policy separately bounds the factory's calls.

Configure the Hub `lore-factory` registration with its private Env secret reference
and exact repository assignment. Its service identity is
`https://veoveo.example.com/oauth#lore-factory`, distinct from every human and native
management identity. Supply the same reviewed client secret privately to native
OpenShell's existing refresh custody and to the trusted Hub secret locator; neither
belongs in the profile, image, factory environment/argv or Computer. The gateway's
own trusted outbound access must reach the configured token URL. The factory does
not fetch tokens or receive the client secret.

For a new, operator-owned `vv-time-build` workspace (inspect existing objects before
creating anything):

```sh
(
  : "${VEOVEO_LORE_FACTORY_CLIENT_SECRET:?trusted dedicated client secret}"
  export VEOVEO_LORE_FACTORY_CLIENT_SECRET
  openshell profile lint --workspace vv-time-build \
    --file openshell/providers/lore-time.example.yaml
  openshell profile import --workspace vv-time-build \
    --file openshell/providers/lore-time.example.yaml
  openshell provider create --workspace vv-time-build --name vv-lore-time \
    --type vv-lore-time --runtime-credentials
  openshell provider refresh configure --workspace vv-time-build vv-lore-time \
    --credential-key LORE_AUTH_TOKEN --strategy oauth2-client-credentials \
    --material client_id=lore-factory \
    --secret-material-env client_secret=VEOVEO_LORE_FACTORY_CLIENT_SECRET
  openshell provider refresh rotate --workspace vv-time-build vv-lore-time \
    --credential-key LORE_AUTH_TOKEN
  openshell provider refresh status --workspace vv-time-build vv-lore-time \
    --credential-key LORE_AUTH_TOKEN
)
```

Initial rotation proves setup only, not renewal beyond expiry. The native factory
receives its opaque `LORE_AUTH_TOKEN` reference and uses the existing native
`lore --access-token` path under its admitted policy. Preserve the deterministic
pinned revision/build/test/output workflow in [FACTORY_TIME.md](../openshell/FACTORY_TIME.md);
the retired local direct-issuer helper is not part of this setup. Attach
no human refresh grant, administrator/native-control provider, model credential or
agent to the factory. Keep any existing member SDK preflight identity separate.

## Live publication and first cutover

The active PostgreSQL control-plane revision is authoritative. The configuration
file seeds only an empty installation; changing that file and restarting does not
overwrite live repositories or grants. For an intentional first cutover/import,
stop the hosted process through the coordinated native lifecycle, validate the
complete trusted file, then run the existing Hub operator command with its normal
private database environment (do not put passwords in argv):

```sh
veoveo-hub validate --config "$HUB_COMPLETE_CONFIG"
VEOVEO_CONFIG="$HUB_COMPLETE_CONFIG" veoveo-hub import-control-plane
```

`VEOVEO_DATABASE_URL` and private `VEOVEO_DATABASE_PASSWORD` must already be supplied
to that trusted operator invocation. Import uses the normal revision/audit
transaction. This is not a running-process reload command or another launcher.
Start the reviewed Hub normally after import. The unreleased resource-binding
migration intentionally discards disposable OAuth/device-browser continuity as
specified in the execution plan; preserve actual Computer working storage,
Tasks/Artifacts, configuration, native service credentials and unrelated services.

Routine repository/grant changes need no restart: read the current complete
control plane through existing `GET /admin/{profile}/control-plane`, edit its typed
records and publish through existing authenticated `PUT` on the same route. Use
the installation's admitted administrative profile/client. For example, a trusted
operator's private native curl config may hold its existing Authorization header:

```sh
curl --config "$PRIVATE_HUB_ADMIN_CURL_CONFIG" --request PUT \
  --header 'Content-Type: application/json' \
  --data-binary "@$HUB_COMPLETE_CONFIG" \
  https://veoveo.example.com/admin/admin/control-plane
```

No admin credential is supplied by this example. Preserve the current configuration
when constructing a complete replacement. Publication/audit commit atomically and
the publishing process adopts it under one ordered guard. Other replicas follow
native active-revision notifications and reconnect reconciliation. Propagation is
asynchronous; this is not synchronous global revocation. Startup-only gateway or
Computer placement changes still require deliberate stopped-process import and
restart. Do not reset the application database or source/object storage to change
a repository assignment.

## Validation handoff

Parse the TOML with native Lore's existing `AuthSettings`/settings validation and
select `ResourceGrantsAuthorizer`, not a custom grant mode. Validate the complete
merged Hub file with the matching new `veoveo-hub validate --config` binary; an old
binary cannot admit this contribution. Parse Computer policy through native
`openshell_policy::parse_sandbox_policy_file`, call existing endpoint-ambiguity
validation and lint the chosen profile with native `openshell profile lint`.
Check exact scope/recipient/audience/client mappings above before image/runtime
promotion. No Dockerfile, binary pin or copied Computer instruction changes are
made in this configuration slice. Full native CLI/Entra exchange, refresh,
per-user isolation, grant withdrawal/expiry and Linux proxy behavior remain later
acceptance, not conclusions from JSON/TOML parsing.
