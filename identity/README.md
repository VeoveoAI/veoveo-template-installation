# Installation identity

Local Keycloak, its Compose services and the helpers `configure-openshell` and
`provision` are **Mac development fixtures**; `configure-hub` prepares Hub's trusted
files from local Keycloak or an existing Entra registration. Linux uses Entra and
needs no Keycloak: its Hub seed is [entra.example.json](entra.example.json)
merged with the [K3s Hub fragment](../kubernetes/hub.example.json), as the
[workload bootstrap](../kubernetes/WORKLOADS.md#hub-and-automatic-private-computers)
describes.

Local development uses persistent Keycloak. The remote Linux reference uses
Microsoft Entra ID. Installers may keep the bundled Keycloak service or configure
another OIDC provider; Hub authorization, roles and resource tokens remain
Veoveo's responsibility. The qualified local Lore path uses its maintained auth
patches and native pre-push hook.

## Persistent local Keycloak

After creating the installation CA and preparing the ignored `.env`, run from
the installation root:

```sh
./identity/provision
```

Provisioning enables `COMPOSE_PROFILES=keycloak` in the ignored `.env`. Keycloak
26.8.1 runs in production mode with a dedicated PostgreSQL 18.6 database, not the
in-memory test fixture. All three containers restart with Docker. The operator-owned external volume
`vv-local_vv-keycloak-db` survives container replacement and `compose down -v`.
Provisioning creates it explicitly and refuses to replace missing credentials when
this database already exists. Do not delete it when
resetting a component's development state.

| Purpose | Address |
|---|---|
| OIDC issuer | `https://auth.localhost:8443/realms/veoveo` |
| Account and password management | `https://auth.localhost:8443/realms/veoveo/account/` |
| Keycloak administration | `https://auth.localhost:8443/admin/` |

The local browser resolves `auth.localhost` to the loopback HTTPS listener. On
Linux hosts without automatic `.localhost` resolution, add
`127.0.0.1 auth.localhost` to `/etc/hosts`. Bootstrap containers resolve the issuer
through the identity edge's Docker alias.

This bootstrap exposes only Keycloak. Native OpenShell service ingress has been
qualified with sandbox DNS, matching request authority and verified TLS. Actual
Hub and Time application qualification remains pending; follow the
[native setup](../openshell/HUB_TIME.md) and execution checklist. This
Compose/Envoy stack provides no Hub route or application alias. The former Docker
application restart path is removed; manage Hub through its native workload.

### macOS certificate trust

After generating the installation CA, install its public certificate once in the
current macOS user's login keychain with SSL trust. Run from the installation root:

```sh
security add-trusted-cert -r trustRoot -p ssl \
  -k "$HOME/Library/Keychains/login.keychain-db" tls/ca/ca.crt
```

macOS may request authorization for this one-time trust change. It uses the user
trust domain and does not require `sudo`. Only the public CA certificate enters the
keychain; its private signing key stays in the installation's protected state.

OpenShell's OIDC discovery, login and token refresh use native certificate roots.
Earlier installations trusted only the `auth.localhost` leaf certificate for the
browser, which left the CLI reporting `UnknownIssuer`. Trusting the installation
CA supplies the issuer chain the CLI needs. Existing leaf entries can remain.
The CA trust persists across terminals and reboots; ordinary OpenShell commands
need no `SSL_CERT_FILE` or `SSL_CERT_DIR` export. Remove any Veoveo-specific shell
override added for the earlier smoke setup.

To inspect existing entries, compare these SHA-256 fingerprints:

```sh
openssl x509 -in tls/ca/ca.crt -noout -fingerprint -sha256
security find-certificate -a -c "Veoveo local installation CA" -Z \
  "$HOME/Library/Keychains/login.keychain-db"
security dump-trust-settings
```

Preserve this CA across bootstrap reruns. Renew endpoint certificates under the
same CA before expiry and reload the affected TLS endpoints; that requires no new
root trust installation. A new CA requires installing its public certificate on
each client before switching endpoints. Linux clients use their platform's trust
store. An externally hosted issuer with a publicly trusted chain needs no local
Veoveo CA trust for its login endpoint.

### Online public keys from native sandboxes

Hub can fetch the realm's live JWKS through the existing identity listener at
`https://host.openshell.internal:8443/realms/veoveo/protocol/openid-connect/certs`.
The native host alias provides routing. Its URL, TLS SNI and HTTP authority remain
`host.openshell.internal:8443`; do not substitute a Host header or disable TLS
verification. Admit only the Hub executable, that exact host/port/path and GET in
its native network policy. The selected supervisor image must contain the public
installation CA for upstream verification; clients use provisioned workload roots.

This alias serves only GET on that exact public-key path. Other alias paths and
methods are refused. The issuer, token endpoint, browser and administrator URLs
remain under `auth.localhost:8443`. Canonical forwarding headers still identify
that issuer. This is a public JWKS route on the retained identity listener, not a
Hub application route or a new proxy. Actual sandbox fetching and key-rotation
acceptance require deployed qualification.

Fresh identity leaves include both DNS names and the existing loopback IP.
Ordinary provisioning preserves an existing leaf unchanged. To add the alias to
an existing installation, explicitly renew only the identity certificate:

```sh
./identity/provision --renew-identity-certificate
```

This action requires the existing complete leaf pair and installation CA. It
reuses the leaf private key, validates the new chain and names, privately backs up
both current leaf files under `state/identity/tls-backups/`, then atomically replaces
the certificate. It changes neither CA files nor identity/database credentials and
does not start or restart services. Incomplete material fails without replacement.
Preserve the backup directory with the other private operator state.

After reviewing the new certificate, reload only the existing HTTPS listener:

```sh
docker compose up -d --no-deps --force-recreate vv-identity-edge
```

No new root trust installation is needed. Keep existing CA trust and all Keycloak,
Lore, application and native OpenShell identities intact.

### Local credentials

Initial user and administrator details live in `state/identity/credentials.json`,
mode 0600 inside a mode 0700 directory. The ordinary username is the host's login
name; the separate administrator appends `-admin`. Change their passwords in the
account/admin UI and keep them in your password manager. The admin's initial
password must be changed on first login. This file is an initial handoff, not a
password synchronization mechanism. Re-running provisioning preserves database
users and passwords. The temporary bootstrap service account and its container
environment credential are removed after setup.

Provisioning creates the permanent identity realm, users and account permissions.
It does not create a Hub OIDC client or choose an application callback origin.
Existing private credentials and IdP data are preserved; the former application
client is not a native Hub setup instruction. Prepare application client/claims
and catalog configuration with the Hub integration after its native origin is
qualified. Human sign-in alone must not grant application membership.

### Local OpenCode human sign-in

For normal interactive use, register a public Keycloak client and explicit Hub
membership rather than launching OpenCode with a service-account bearer token.
The existing `configure-hub` helper has an opt-in mode that leaves the service
clients, users, their roles and all unrelated catalog sections unchanged:

```sh
identity/configure-hub --hub-origin "$HUB_ORIGIN" --local-public-client \
  --installation-root "$INSTALLATION" \
  --control-plane-file "$FRESH_CATALOG" --output-control-plane "$CANDIDATE" \
  --application-client-id opencode-local --member-object-id "$KEYCLOAK_USER_ID" \
  --work-context "$WORK_CONTEXT" --protected-resource "$HUB_ORIGIN/mcp/local" \
  --client-redirect-uri http://127.0.0.1:3210/callback
```

Run this only from trusted operator tooling. `$FRESH_CATALOG` must be a fresh
authoritative Hub control-plane snapshot; validate and publish `$CANDIDATE`
separately with existing native Hub administration. The selected Work Context
supplies its tenant. The helper adds only a member grant and contributor
membership for the explicitly selected, enabled Keycloak human user IDs. It
does not grant every user of the public client access. Repeating preparation is
idempotent; a conflicting existing registration requires operator review.

Keycloak owns the public client's authorization-code/S256 and native refresh
flow. The built-in User Property mapper copies each actual user's immutable
`id` into Hub's `principal` and `initiator` claims; the native `sub` remains
unchanged. The principal is this opaque user ID, not a service-account name.
The exact MCP audience, direct invocation, tenant and Work Context claims are
configured on this dedicated client. No client secret, user-attribute extension,
password grant or application refresh worker is involved. Optional `mcp` and
`offline_access` scopes use Keycloak's existing scope configuration; the human
must remain eligible for native offline access under the realm's existing roles
and session policy. This mode does not configure a Hub-hosted OIDC broker.

Configure OpenCode's remote server with `oauth.client_id: "opencode-local"`,
`oauth.scope: "mcp"`, `oauth.redirect_uri: "http://127.0.0.1:3210/callback"` and
`oauth.callback_port: 3210`. Remove static Authorization headers and `oauth: false`.
Use ordinary `opencode mcp auth <server-name>` for real browser sign-in. OpenCode
stores the resulting grant natively and refreshes on an authenticated request
when required; it does not need the old token-minting launcher.

Start the OpenCode process with standard `NODE_EXTRA_CA_CERTS` pointing to a
public PEM bundle containing both the native gateway CA and Keycloak's
installation CA. `SSL_CERT_FILE` may name the same bundle. Preserve certificate
verification; there is no per-server CA option in the reviewed MCP configuration.
Already-running servers retain their startup environment, so qualify a separate
native process before replacing one that owns other sessions.

Envoy binds the browser listener to `127.0.0.1:8443`. PostgreSQL, Keycloak HTTP and
its management port are unpublished. Database and Keycloak run as non-root, with
all capabilities dropped and no-new-privileges. The database owner has no
superuser/create-role/create-database privileges. An interactive Hub OIDC client's
credentials belong only to the approved trusted Hub. Native application and
management client secrets stay in OpenShell credential custody; Hub's external
client admission metadata does not require those secrets.
Factory workloads receive no identity administration credentials, bootstrap
mounts or route to the identity network. Environment files,
private keys and provisioning state remain ignored and outside build contexts.
Envoy removes caller-supplied forwarding headers before adding the configured
HTTPS origin. Preserve that mutation order; removing those headers afterwards
causes Keycloak to reject the browser's token exchange with `Invalid origin`.

Back up the identity database independently of component databases. For example,
run `docker compose exec -T vv-keycloak-db pg_dump -U postgres -Fc keycloak` into an
owner-only file under `state/identity/`, then encrypt it before off-host storage.
It contains password hashes, signing keys and client credentials. Preserve the
installation CA and private provisioning credentials through the operator's
normal encrypted backup process too. Test restoration before relying on a backup.

## Installer choice and the Linux reference

Bundled Keycloak is a supported provider choice, not a required Veoveo dependency.
For a network installation, give Keycloak its own HTTPS hostname, adjust
`KC_HOSTNAME`, Envoy's hosts/certificate/forwarded headers, the Hub's issuer,
client callback and identity configuration together. Keep the database and
management interface private. Use the provider's supported clustering and backup
configuration when the installation needs high availability. A local loopback
configuration is not a public production ingress configuration.

For Entra, omit the `keycloak` Compose profile and its three services. Follow
[reference preparation](../openshell/REFERENCE.md) and the existing helper's
`--identity-provider entra` path. [entra.example.json](entra.example.json) shows
the full standard catalog shape, with deliberately unenrolled example UUIDs and
a placeholder signing-key thumbprint; do not deploy it unchanged. Point the relevant profile and authorization
server's `identityProvider` to `entra`, enable `oidc_authorization_code_pkce`, and
configure the public Hub authorization/token endpoints and client registration.
Grant roles explicitly to issuer + stable object ID principals; sign-in alone must
not confer installation membership.

Register a single-tenant confidential **Web** application in Entra with the exact
`https://veoveo.example.com/oauth/callback` redirect. Supply its credential only to
the trusted Hub through its configured secret source. Use the tenant-specific
v2 issuer; do not use `common` or `organizations`. Permit the configured Entra
JWKS/token host through the approved native egress policy and qualify the Hub's
human OAuth routes through the selected authenticated HTTPS ingress. Local
bootstrap does not provision that application ingress. Follow Microsoft's
[OIDC metadata](https://learn.microsoft.com/en-us/entra/identity-platform/v2-protocols-oidc)
and [authorization code flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow)
documentation. Entra requires the reference tenant and app registration; this
local setup does not create those or modify the remote deployment.

## Lore identity and repository ownership

Entra (or the selected upstream IdP) authenticates humans into Hub. The existing
Hub broker application remains confidential and its credential stays trusted.
Native Lore's secretless public client and distinct factory client are registered
with Hub, not provisioned as directory repository-role/resource mappers. Hub's
revisioned control plane owns exact native repository registrations and explicit
tenant/principal/action assignments. Hub's [Lore credentials](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#native-contributed-credential-lifecycle)
describe the public client, issuer, factory custody and live publication.

The retired `configure-lore` direct-issuer helper and its helper-only tests have
been removed. Do not regenerate its local client/grant bookkeeping. Retain any
trusted existing storage/TLS/identity state; removal of tooling does not authorize
deleting operator data. Directory administration is not required to change a Hub
repository assignment. Native Lore behavior remains unchanged.

The former `veoveo-lore` Desktop registration remains disabled. Its adapter/ingress
and manual signing helpers stay retired. Preserve durable identity data and the
private recovery checkpoint; do not restore Desktop bootstrap for factory renewal.
