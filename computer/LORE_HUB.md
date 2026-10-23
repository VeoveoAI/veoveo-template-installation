# Hub-issued native Lore credentials

Select this mode for the hosted Computer installation. Entra authenticates the
person into Hub. Hub's existing issuer owns explicit repository registrations and
assignments; unchanged Lore verifies its existing signed resource-grant contract.
There is no additional issuer process, custom Lore role parser, permission service,
Desktop adapter or refresh daemon. Local Keycloak remains identity/management bootstrap; it is not a parallel
repository-grant provisioning path.

These are synthetic configuration examples, not deployed identities or image
receipts. Keep the current image pins until new artifacts are explicitly reviewed.
Computer and factory Lore access needs a Lore command line that reads
`LORE_ACCESS_TOKEN` and `LORE_IDENTITY` when its `--access-token` and `--identity`
flags are absent.

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
  profile/Work Context eligibility. Merge the example client by its typed ID and
  preserve existing memberships/identity settings; do not append a duplicate. Its
  allowed resources/scopes explicitly include the selected native Lore audience
  and login scope set for personal workspace creation.

Replace these names consistently when the installation uses different admitted
IDs. Keep `computers.gateway` and `nativeUpstreams` on the same dedicated native
control reference described in [NATIVE_CONTROL.md](NATIVE_CONTROL.md). That
management identity is independent of all Lore users and factory grants.

| Native input | Matching Hub data |
|---|---|
| `jwt_issuer = ["https://veoveo.example.com/oauth"]` | Hosted server `hub` issuer and normal discovery/JWKS |
| `jwt_audience = ["https://veoveo.example.com/resources/lore"]` | Deployment `audience`; native CLI resource and initiating Computer/OpenCode client allowlist |
| `resource_claim = "resources"`, `resource_id_claim = "resource_id"`, `permission_claim = "permission"` | Fixed native typed claim encoding; not arbitrary JWT templates |
| `resource_id_template = "urc-{id}"` | Deployment `resourceIdTemplate` |
| OIDC `client_id = "lore-computer"` | Dedicated secretless public registration with device, refresh and public exchange grants |
| OIDC `scopes = ["lore:login"]` | Finite deployment/client scopes `lore:login` plus native-added `offline_access` |
| OIDC `scope_template = "lore-repo-{id}"` | Deployment `scopeTemplate`; the repository exchange selector |
| Private `grpcs://host.openshell.internal:41337` | Deployment `endpoint`, native server certificate SAN, policy hostname and `VEOVEO_LORE_REMOTE` |

For Linux, the selected private native endpoint is
`grpcs://lore.veoveo-system.svc.cluster.local:41337`, managed by Kubernetes with
end-to-end TLS. Select that Service DNS, certificate SAN/root and the exact
`/usr/local/bin/veoveo-hub`/Lore binary policy entries together. Hub's native E0
client trusts the public Lore root through its existing CA-only mount.

**Mac Docker fixture only:** the trusted Compose Lore service publishes only `127.0.0.1:41337:41337`.
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

The sample grants Alice/Bob the shared `1111…1111` repository and only Bob the
`2222…2222` repository.
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

## Computer Lore access

A Computer reaches Lore as its owner with no login of its own: the owner's Hub
sign-in is the only one. The installation configures no Computer client or secret.
Each custody handoff gives the Computer's own OAuth client, whose identifier is the
Computer's native name, a new secret, and Hub keeps only its digest. Its
client-credentials grant issues the owner's Lore token, with one native `resources`
entry for each repository the owner is assigned on the deployment and none without
an assignment; the client requests no scope and holds no grant of its own. The
gateway holds the secret in refresh custody of the workspace-local `computer-lore`
provider, created from the global profile [`vv-lore`](../openshell/providers/lore.yaml)
that Hub's `computers.loreProvider` names, and Hub rotates it before the sandbox
starts. The sandbox receives only the provider's placeholder in `LORE_ACCESS_TOKEN`
and the owner's principal in `LORE_IDENTITY`. The native Lore CLI reads both, and
the proxy substitutes the token in Lore's gRPC headers on the profile's inspected
endpoint. The [Kubernetes Computer policy](../kubernetes/policies/computer-v1.yaml)
therefore has no Lore rule of its own: a TCP passthrough rule to that endpoint
conflicts with the profile's, and no device login remains. The Mac Docker fixture's
[policy.yaml](policy.yaml) keeps its passthrough and device-login rules and does not
provide this access.

The `lore-computer` public client stays: Hub issues each person's Lore token as the
token that client would obtain through the exchange, and Lore's OIDC configuration
names it. When a published revision adds, removes or changes an owner's assignment,
Hub at once asks the gateway to renew that owner's Computer credential, so the next
Lore request carries a token with the current assignments; this happens once per
installation, whether the revision was published through the admin route, imported
before startup or adopted by a replica, and a Computer that cannot renew is logged
without failing the publication. A token already sent or cached keeps its native
expiry; this does not promise immediate native token revocation. Never copy a token or the placeholder
into chat, shell arguments or another user's Computer.

## Factory sandbox Lore access

The installation configures no factory client, secret or repository assignment.
Hub registers one OAuth client per factory instance, whose identifier is the
instance's deployment ID, and keeps only the digest of its secret. Its
client-credentials grant issues a repository-scoped Lore token for the instance's
latest pusher, admitted against that person's current assignment on every issue;
the client requests no scope and holds no grant of its own. The gateway holds the
secret in refresh custody and refreshes the token through the global profile
[`vv-lore`](../openshell/providers/lore.yaml), which Hub's `factory.loreProfile`
names, as Computers do. The sandbox receives only the provider's placeholder in
`LORE_ACCESS_TOKEN` and the instance's deployment ID in `LORE_IDENTITY`, which
Lore's command line reads.
Install the profile as described in
[Factory sandbox policy and providers](../kubernetes/README.md#factory-sandbox-policy-and-providers).
Attach no human refresh grant, administrator/native-control provider, model
credential or agent to a factory sandbox.

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
the installation's admitted administrative profile/client. These routes need an
explicit Hub policy rule in `policies[].rules` whose `profiles` names the route's
`{profile}` (`local` in the reference installation) and whose `roles` names the Entra
`administrator` app role, which Hub carries in the token's `roles` claim:

```json
{"id": "installation-administration", "effect": "allow",
 "actions": ["admin_read", "admin_write"], "profiles": ["local"],
 "roles": ["administrator"], "tenantIds": ["<tenant>"]}
```

Making someone an administrator is assigning them that Entra app role; Hub's `admin`
installation role, with its risks, governs only server tool permissions.

A trusted operator's private native curl config may hold its existing Authorization
header:

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
validation on it composed with the `vv-lore` profile's endpoint, and lint the chosen
profile with native `openshell profile lint`.
Check exact scope/recipient/audience/client mappings above before image/runtime
promotion. No Dockerfile, binary pin or copied Computer instruction changes are
made in this configuration slice. Full native CLI/Entra exchange, refresh,
per-user isolation, grant withdrawal/expiry and Linux proxy behavior remain later
acceptance, not conclusions from JSON/TOML parsing.

## Personal workspace first use

`lore.personalWorkspace.deployment` is an optional explicit first-use deployment
ID. It never selects the first deployment by name or replaces an existing owner's
association. The Computer fragments include the matching `computer-browser` public
client. For an existing `opencode` OAuth client used to request personal workspace
selection, merge the same native audience into its `allowedResources` and all
selected deployment `loginScopes` into `allowedScopes`, preserving its existing
MCP audience, redirect URIs, Work Context and grants. Do this only when merging the
matching registered Lore contribution; adding an unknown audience to the standalone
identity example would be invalid. Broker `oidcClients.allowedResources` is separate
and cannot substitute for initiating OAuth-client admission.

A direct authenticated user can POST `{"deployment":"primary"}` (or the selected
registered ID) to `/personal-workspace/{profile}` before first-use binding. To reuse
an already authorized repository, also provide `repository` with its exact native
ID. Reuse checks the existing native resource grant and Get; it adds no Owner
privilege. A conflicting existing personal association is not replaced.

After native creation, Hub verifies the actual repository/default-branch IDs and
creator before publishing only that actual repository and caller Owner assignment
through the existing control revision CAS. A reserved publication ID prevents a
crash retry from regranting an assignment after its committed publication was later
revoked. No Computer/session deletion cascades into this owner association.

Computer access remains usable while optional onboarding is preparing, auth-needed,
denied or unavailable. `GET /computer/onboarding` is a read-only browser-session
status; admitted entry schedules a bounded preparation retry. `ready` means the
optional repository default and public descriptor are prepared, not that native
login or checkout occurred. The user or agent chooses ordinary native Lore login,
repository, branch and destination when wanted. Credentials remain in the user's native persistent store,
separate from the Computer's platform refresh family and factory custody.

This source path requires component/deployed qualification; configuration alone
is not a successful native creation, auth, checkout or isolation receipt.
