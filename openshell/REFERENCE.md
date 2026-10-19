# Native reference installation with Entra

This prepares the reference Linux goal in the
[execution plan](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md#reference-linux-deployment-and-signed-wavespeed-webhook-goal).
It is configuration, not a qualification receipt or deployment controller. Public
origins below are examples; use the operator's reviewed DNS/TLS configuration in
private state. The authorized fresh Linux reference discards old application
containers, clusters and data volumes, with no backups requested. Preserve source,
the Mac installation and the active new OpenShell/Entra control plane. Scope the
operator's deletion inventory to old reference resources; this is not permission
to clear unrelated Docker or native state. GPU/CUDA is separate.

Use the existing Docker-driver gateway, providers, policies, volumes and lifecycle.
Do not run the local Keycloak profile or identity HTTPS edge on this Entra reference.
The Mac's existing setup remains unchanged. Hub requires Tasks and clients must
implement the supported MCP specification; add no taskless fallback.

## Pin all native runtime images

Build the Linux/amd64 gateway, supervisor and sandbox runtime from the same reviewed
OpenShell revision `6589a918a7a84640969f9f77b852695e210d713f`, using its native
build prerequisites and [the existing build workflow](README.md). In that clean
source checkout, also build the separate trusted sandbox runtime image:

```sh
MISE_AUTO_INSTALL=0 DOCKER_PLATFORM=linux/amd64 \
  IMAGE_REGISTRY=veoveo IMAGE_TAG=reference-6589a918 mise run build:docker:sandbox
docker image inspect --format '{{.Id}}' veoveo/sandbox:reference-6589a918
```

Select the inspected immutable image ID in the existing gateway environment as
`OPENSHELL_SANDBOX_RUNTIME_IMAGE`. The Docker driver extracts the trusted
`openshell-sandbox` executable from this image for every sandbox; it is distinct
from the application's `default_image` and from `OPENSHELL_SUPERVISOR_IMAGE`.
Pin all three inputs independently rather than allowing a mutable development
runtime image to be pulled during gateway startup. The reference build produced
sandbox runtime `sha256:d87ca9c45c4cd8bc7876f5b1cb86768f4dd42b3babc2398dc00bff206d993a75`
and supervisor `sha256:22368e9fd71ebbb887402a91ca4ba7e5c754146179a4f53f7495f41d15644f51`;
these are Linux reference receipts, not changes to the Mac's selected images.

## Entra and the two token relationships

OpenShell authenticates Entra access tokens directly. The gateway's issuer,
audience, roles and scopes must match independently verified tokens for its own
resource. Start from [the native gateway example](entra-gateway.example.toml).
Use `roles_claim = "roles"` and `scopes_claim = "roles"` when application roles
carry the required native permissions. Do not disable role/scope enforcement
because an app-only token has no delegated `scp` claim.

The operator needs `openshell-admin` and `openshell:all`. The separate forwarder
needs only `openshell-user` and `sandbox:write`; add its actual validated token
`sub` as a **user** in `vv-services` only. Do not substitute Entra `oid` for this
native membership key, share the operator identity, or assign the forwarder an
admin role. App roles and their user/application assignments are Entra operator
configuration; possessing a client secret does not prove those grants. Configure
the selected resource's v2 access-token version and exact audience, then verify
issuer, audience, roles, expiry and native admission before deployment.

Register the native CLI with its exact gateway URL, tenant-specific issuer,
client ID and selected resource scopes using `openshell gateway add ...
--oidc-issuer ... --oidc-client-id ... --oidc-scopes ...`. The reference qualified
native device authorization with a separate Entra public CLI registration
(`isFallbackPublicClient = true`): request the resource's `access_as_user` scope
plus `openid profile offline_access`, then run:

```sh
OPENSHELL_NO_BROWSER=1 openshell gateway login <name>
```

The browser authorization-code flow uses PKCE, state and an HTTP loopback callback;
it requires an exact provider-supported redirect registration. A random-port
`127.0.0.1` callback was not qualified by this reference device-flow receipt.
Trusted client-credentials login instead reads `OPENSHELL_OIDC_CLIENT_SECRET` from the
operator environment; never put it on argv. Select `<resource-App-ID-URI>/.default`
for app-only tokens. This is distinct from the JWT's `aud` value. No client mTLS
identity is required for these OIDC users; ordinary server TLS verification and
native supervisor/control certificates remain required.

Hub application clients use its existing OIDC code broker. Entra authenticates
the user; Hub issues the normal application access/refresh grant, including its
Work Context authority. A plain Entra access token is not substituted for Hub's
RFC 9068 profile bearer. The Entra confidential Web registration must include
`https://veoveo.example.com/oauth/callback`; the secret is held only by trusted
Hub, not by the agent. Hub checks signature, issuer, application audience and
nonce, maps `oid` to the stable subject and maps only the selected `tid` to the
installation tenant. Sign-in alone grants no Work Context or permission.

## Prepare trusted Hub configuration

Privately supply `VEOVEO_IDP_OIDC_TENANT_ID`, `VEOVEO_IDP_OIDC_CLIENT_ID`,
`VEOVEO_IDP_OIDC_ISSUER`, `VEOVEO_IDP_OIDC_CLIENT_SECRET` and the existing trusted
`VEOVEO_DATABASE_PASSWORD`. Save the verified issuer's standard OIDC discovery
JSON privately; preparation does not fetch it or mutate Entra. Review its issuer,
JWKS and authorization/token endpoints. Public example UUIDs in
[entra.example.json](../identity/entra.example.json) are deliberately unenrolled.

From the separate reference installation checkout, run the existing helper:

```sh
./identity/configure-hub --identity-provider entra \
  --hub-origin https://veoveo.example.com \
  --oidc-discovery-file /operator/private/entra-discovery.json \
  --native-gateway https://host.openshell.internal:17670 \
  --tenant reference --work-context reference-work --policy-version reference-policy \
  --owner-object-id '<administrator-object-id>' \
  --admin-object-id '<administrator-object-id>' \
  --member-object-id '<ordinary-member-object-id>' \
  --billed-object-id '<billed-test-user-object-id>' \
  --application-client-id opencode \
  --client-redirect-uri http://127.0.0.1:3210/callback \
  --include-artifact --include-media
```

Use the actual application's supported exact callback; the loopback example is not
an assertion that every client uses that port/path. Member/admin/billed selectors
are explicit stable Entra object IDs, never display names. Their installation
roles select declared access/risk; billed grants only `billed`, not administrative
or destructive risk. The selected administrator owns output policy. Work Context
membership lists principals, not the shared public application client, so another
tenant user cannot gain membership merely by logging in. A single identity given
admin authority cannot also prove the ordinary-member denial case.

The helper performs no identity-administration, database, provider or deployment
operation. It writes only standard owner-only component files under ignored
`state/openshell/hub/`: `hub.json`, `hub.env` and the public
`authorization-server.jwks.json`. Hub's own application signing key, forwarded
identity key, audit/refresh keys and registration secret persist across reruns;
no caller signing keys are generated. The Entra secret is copied only into trusted
Hub's private environment. Validate the generated catalog using the component CLI:

```sh
veoveo-hub validate --config state/openshell/hub/hub.json
```

Before first startup, populate the existing `vv-hub-config` named volume with
**only** `hub.env`, `hub.json` and `authorization-server.jwks.json`, mounted read-only
at `/etc/veoveo/hub` through [the existing mount configuration](hub-mounts.json).
The Entra catalog references that public JWK file at this exact path; omitting it
breaks application-token verification. Make the directory mode 0700 and files 0600,
owned/readable by the runtime's UID 10001. Do not copy operator state or caller
refresh material. The Mac's `installation-ca.crt` is not needed for publicly
trusted Entra endpoints. The separate native control connection still uses the
OpenShell-provisioned workload CA described in [Hub/Time setup](HUB_TIME.md).

### Fresh bootstrap and retired reference keys

After the reviewed old-resource cleanup, initialize empty external volumes
`vv-local_vv-postgres` and `vv-local_vv-seaweedfs`, which the existing Compose file
requires. Privately select new `VEOVEO_POSTGRES_PASSWORD` (bootstrap superuser),
`VEOVEO_DATABASE_PASSWORD` (installation owner), `VEOVEO_STORAGE_ACCESS_KEY`,
`VEOVEO_STORAGE_SECRET_KEY` and `VEOVEO_SEAWEEDFS_ADMIN_PASSWORD`; keep them out of
argv/output. Do not copy old application credentials or restore an old dump.
[PostgreSQL's existing init file](../postgres/initdb/owner.sql) creates
`veoveo_owner LOGIN CREATEROLE` and database `veoveo` owned by that role on an empty
data directory. It does not migrate a pre-existing database. The fresh helper's
default owner/server URLs already select this new `veoveo`; no override is needed
for that bootstrap.

The helper makes no database connection. It has **no database-URL override flag**
and does not read URL overrides from the invoking environment. If another fresh
database name is deliberately selected, create it through the trusted PostgreSQL
bootstrap procedure, first prepare the standard files above, then privately edit
only these standard fields in `state/openshell/hub/hub.env` before mounting or
starting Hub:

```sh
VEOVEO_DATABASE_URL=postgres://veoveo_owner@host.openshell.internal:5432/veoveo_reference
VEOVEO_SERVER_DATABASE_URL=postgres://host.openshell.internal:5432/veoveo_reference
```

`veoveo_reference` is an example of the selected new database, not a command to
create it. Supply its reviewed owner password separately through the existing
`VEOVEO_DATABASE_PASSWORD` field; never put it in argv or published URLs. Preserve
all generated keys and other environment fields. The two URLs reach the same
selected database, with Hub's owner login and the server-login template
respectively. A normal helper rerun reads and preserves the existing `hub.env`,
including these URL values; it refuses a changed supplied password rather than
rotating it. Review the private values before deployment; catalog validation does
not establish database readiness or migration compatibility.

This reference does not adopt historical database migration ledgers, rows or
objects. Start current migrations on the newly initialized database; do not forge
old checksums or rename the legacy `gateway` schema into `hub`. No history
migration feature or backup/restore step is required for this authorized fresh
setup.

The old Linux Time, Artifact and Media `server-key.p8` identities were exposed and
are retired: **never deploy or adopt those keys**, including staged copies. This
overrides only the linked Mac key-adoption steps for this reference. Start with
fresh Hub private output and empty UID 10001 service-state volumes. First Hub
preparation generates new forwarded/application signing keys, audit/refresh keys
and initial registration secret; do not seed it from old `hub.env`. Each service's
existing `KeyDirectory::load_or_generate` creates its new Ed25519 `server-key.p8`
atomically, mode 0600, on first startup in its writable key directory. The fresh
database has no namespace claims and pins those new public identities at normal
registration. Preserve the new state and keys on subsequent reruns/restarts. The
active OpenShell operator/forwarder identities are separate and are not reset by
this application cleanup.

Reruns retain selected private Time/Artifact/Media bindings and policy definitions.
Changed issuer, clients, principal grants, tenant/context, owner, keys or native
control endpoint require separate reviewed migration/recovery; the helper refuses
to reconcile them silently. Never point the reference helper at Mac private state.
Public application origin and private native control endpoint are distinct inputs.
Hub still captures placement at startup; preserve the existing native service
coordinates and mount/lifecycle procedures from [Hub/Time](HUB_TIME.md),
[Artifact](ARTIFACT.md) and [Media](MEDIA.md), rather than adding a launcher.

## Native custody and exact policy

Adapt [Hub's Entra policy](hub-entra.example.yaml) to the verified discovery
endpoint paths. It admits only Hub's executable, public JWKS GET and token POST,
plus its existing private ForwardTcp and database endpoints. It needs no Keycloak
administration or JWKS alias. Compare actual native exposure and public routing
before using the public origin; these templates create neither route.

Import [the forwarder profile](providers/entra-hub-forwarder.example.yaml) only
after selecting its exact tenant/token URL, resource scope and dedicated client.
Use existing `provider refresh configure --strategy oauth2-client-credentials`
with public `client_id` material and `--secret-material-env` for `client_secret`.
Attach only its opaque management reference to Hub and only the ForwardTcp policy.
Native custody owns refresh; no token worker is required.

After the agent's ordinary Hub PKCE login, its Hub refresh grant can use the
[native application profile](providers/entra-hub-application.example.yaml):
`oauth2_refresh_token`, the selected public client ID and protected `refresh_token`
material. Use `--secret-material-env`, never argv values. Bind the provider to the
exact Hub MCP endpoint in the existing client policy. For qualification, keep
member and billed sessions distinct; use their intended opaque references and
separate refresh entries. An Entra client secret is not a Hub client-credentials
grant. Fully unattended hosted Hub clients retain their existing `private_key_jwt`
contract and need a separately reviewed caller-custody decision.

After an authorized development database reset, Hub's old application refresh
families no longer exist. Obtain fresh, separate administrator/member PKCE grants,
then reauthorize their **existing** native custodian. For the reference QA provider,
configure each corresponding key using its new refresh token through a private
child environment variable:

```sh
openshell provider refresh configure vv-hub-qualification --workspace vv-hub-qa \
  --credential-key HUB_ADMIN_REFERENCE --strategy oauth2-refresh-token \
  --material client_id=opencode \
  --secret-material-env refresh_token=NEW_ADMIN_REFRESH_TOKEN
```

Repeat with `HUB_MEMBER_REFERENCE` and that member's distinct refresh grant.
Do not use `provider update --credential` for these access-token fields: native
OpenShell rejects direct writes to refresh-owned outputs, including entries parked
as `reauthorization_required`. Inspect `provider refresh status`; if the worker has
not already minted the new access token, request one `provider refresh rotate` for
that key. The controller stores the minted token and any rotated refresh material.
Do not provide the PKCE access JWT or an explicit old-credential expiry to refresh
configuration, create a second provider, or share QA's grant with OpenCode.

Explicit refresh configuration begins a new authorization epoch, invalidating old
workload handles. Recreate/restart affected QA processes after both entries have
refreshed; subsequent ordinary renewals preserve the new epochs. For expiry proof,
record the first successful mint and initial workload identities/handle hashes.
The reference profile's 300-second native lease is shorter than Hub's 900-second
JWT lifetime. If the controller-held JWT was not decoded, use a conservative bound
of initial mint completion plus the configured 900 seconds, identify it as a
source/configuration-derived bound, and verify fresh calls after it without manual
rotation. A call after only the native lease is not post-JWT-expiry proof.

### Canonical Hub authority and private service placement

Service registration uses the existing per-service initial-access providers.
For **Time, Artifact and Media**, set:

```sh
VEOVEO_HUB_ISSUER=https://veoveo.example.com
```

This is Hub's configured canonical public issuer, not its native exposure origin.
SDK registration discovery requires the returned `issuer` to equal this value
exactly. Adapt [Time](time.yaml), [Artifact](artifact.yaml) and [Media](media.yaml)
service policies to `veoveo.example.com:443` for the existing
`/.well-known/oauth-authorization-server`, `/jwks.json`, `/oauth/register` and
registration-management paths. Keep only each service's required `/oauth/token`
and `/mcp/local` calls, existing executable restrictions and exact REST/MCP rules.
The application authorization server at `/oauth` has separate metadata and
`/oauth/jwks.json`; do not substitute it for service registration discovery.
Adapt application/qualification policies and refresh token URLs to the same
canonical public authority. Do not broaden paths to accommodate the change.

Cloudflare's Hub route instead connects to `https://127.0.0.1:17670` with verified
TLS `originServerName=vv-control--hub--http.openshell.localhost` and
`httpHostHeader=vv-control--hub--http.openshell.localhost:17670`. Native ingress selects that service
and rewrites the upstream Host to `127.0.0.1:8080`; retain Hub's existing
`serve --allow-loopback-hosts` command and loopback-only listener. Hub builds issuer,
OAuth redirect and audience values from its configured catalog/deployment, not
forwarded headers. The native service hostname is a routing/TLS authority, not an
alternate application issuer.

Keep the startup-approved ForwardTcp bindings and logical service bases unchanged:
Time `http://time.internal:8800`, Artifact `http://artifact.internal:8801`, and Media
`http://media.internal:8802`. Time and Artifact need no public exposure or public
origin. Their private calls retain logical Host and audience through ForwardTcp.

### Media Host admission for native HTTP exposure

Whenever Media is selected for native HTTP service exposure, including an early
fake-provider ingress probe, add the following nonsecret literal to the existing
trusted `provider.env` mounted at `/etc/veoveo/media/provider.env`:

```sh
VEOVEO_ALLOW_LOOPBACK_HOSTS=true
```

Retain the file's provider/webhook secret and other reviewed settings, private
permissions and UID 10001 readability. The existing startup command sources this
file before executing Media; update that mounted file through trusted preparation,
then use native stop/start of the existing `vv-services/media` sandbox to load it.
Do not replace the file with only the flag or introduce another startup wrapper.

This is independent of public origin and completion mode. Keep
`VEOVEO_PUBLIC_ORIGIN` unset for the fake/status-check baseline; setting the Host
allowance does not enable Events. Media binds `0.0.0.0:8802` inside its isolated
sandbox network. Native HTTP ingress forwards with `Host: 127.0.0.1:8802`;
the flag adds only `localhost`, `127.0.0.1` and `::1` to the SDK's accepted hosts.
It does not widen native policy, expose the host network or bypass normal TLS,
application authentication or webhook signature verification. Private ForwardTcp
continues to retain the logical `media.internal:8802` authority.

### Media event completion and callback ingress

For the signed callback gate, replace the fake-only configuration with the
reviewed real provider origin, API-only credential binding, permitted output hosts
and actual webhook signing secret in the existing trusted service configuration.
Keep API credentials off output requests. Set Media's non-secret settings:

```sh
VEOVEO_PUBLIC_ORIGIN=https://veoveo.example.com
VEOVEO_ALLOW_LOOPBACK_HOSTS=true
```

Keep `VEOVEO_PUBLIC_BASE_URL=http://media.internal:8802`. Public origin selects
Events completion; absence selects status checks, with no automatic fallback.
Expose Media through its existing native service ingress:

```sh
openshell service expose --workspace vv-services media 8802 http \
  --authorization-mode bearer-passthrough
```

Configure the existing tunnel to route only
`/media/operations/{operation_id}/{token}` to this service; the service accepts
POST on that route. Use verified native TLS
`originServerName=vv-services--media--http.openshell.localhost` and
`httpHostHeader=vv-services--media--http.openshell.localhost:17670`; the relay
sends `Host: 127.0.0.1:8802`, hence the explicit loopback allowance. Do not publicly
route Media MCP, fixture controls or arbitrary service paths. Preserve the callback
path and signature headers/body; never record its bearer token in logs. The adapter
currently sends legacy `webhook`, paired with its existing v3 signature verifier;
do not switch only the parameter to `standard_webhook`.

Verify accepted signed callback receipt and generation operation completion
`events` with zero generation status checks. Billing's separate checks remain
expected. A completed Task or successful route alone cannot prove webhook delivery.

## Native SeaweedFS HTTPS and signed authority

SeaweedFS remains trusted durable bootstrap. Its pinned 4.48 binary natively serves
S3 HTTPS: `s3.cert.file` and `s3.key.file` with `s3.port.https=0` serve TLS on the
selected `s3.port` itself. The existing bootstrap defaults stay HTTP for the Mac.
For the reference, privately set:

```sh
VEOVEO_STORAGE_TLS_DIRECTORY=/operator/private/object-tls
VEOVEO_STORAGE_TLS_CERT_FILE=/etc/seaweedfs/tls/tls.crt
VEOVEO_STORAGE_TLS_KEY_FILE=/etc/seaweedfs/tls/tls.key
```

Use the standard [read-only TLS override](../compose.storage-tls.yaml) with the
existing Compose bootstrap. Inspect the source certificate/key and volume state
first; the bind refuses to create a missing source. Validate the combined files
before starting only the PostgreSQL and SeaweedFS services on the fresh volumes,
using `--no-deps` to avoid reconciling other bootstrap services. No certificate/key
is copied into an image, and no additional proxy is introduced. Keep bootstrap
administration private; provision a new dedicated bucket-scoped Artifact identity,
then apply the reviewed [native Artifact storage policy](ARTIFACT.md#trusted-preparation)
with `s3.policy`; the built-in `readwrite` preset alone lacks required startup and
multipart actions. Never mount the bootstrap S3 credential in Artifact. Preserve
all newly created data and credentials thereafter.

The existing tunnel's object-host route must use verified TLS to that native S3
listener, with a certificate valid for the selected origin server name. Preserve
the public S3 Host, method, path/query and headers; never rewrite a returned signed
URL. Configure Artifact's `VEOVEO_STORAGE_SIGNING_ENDPOINT` to
`https://objects.veoveo.example.com`. For initial qualification the same reachable
HTTPS authority may be used for `VEOVEO_STORAGE_ENDPOINT`; a distinct internal
endpoint requires its own verified route/trust. Artifact uses path-style S3, so
no wildcard bucket hostname or S3 adapter is needed. In the reference copies of
Artifact, Media and acceptance-client policies, admit exactly
`objects.veoveo.example.com`, port `443`, protocol `tcp`, for their intended
executables. This keeps signed requests outside HTTP interception and preserves
SigV4 method, Host, path/query and headers unchanged. Replace the Mac's
`host.openshell.internal:9000` byte-route selection; do not grant a broad HTTPS
endpoint or attach an application/provider credential to signed storage requests.
The object tunnel route preserves the **public S3 Host**, unlike native Hub/Media
service ingress; its TLS origin-server name must match the S3 listener certificate.

Qualify an actual signed upload/download from both the Linux workload and the Mac,
unchanged method/URL/headers, exact bytes/hash, ranges where required and refused
bucket/IAM access. TLS configuration, successful rendering and a public DNS record
are not that acceptance. The public object route must not expose management
listeners; app registration and MCP stay behind Hub.
