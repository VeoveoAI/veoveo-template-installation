# Local native Hub and Time preparation

These files prepare the existing native Hub → Time contract. They do not deploy
services or establish application, renewal, database or restart acceptance. Use the
[execution checklist](https://github.com/VeoveoAI/veoveo-docs/blob/main/OPENSHELL_HARDCUT_PLAN.md#subsequent-adoption)
for those gates. Ordinary reruns preserve application state. During the current
development phase, an authorized breaking-contract cutover resets disposable
application data and qualifies the matched stack from fresh state; it does not
require legacy migrations or backups. See the [fresh reference preparation](REFERENCE.md#fresh-bootstrap-and-retired-reference-keys).
Hub creates its own `hub` schema; this preparation helper performs no database
migration, reset or rename.

## Trusted identity preparation

After the restricted [online JWKS alias](../identity/README.md#online-public-keys-from-native-sandboxes)
is active, run this helper only from the trusted installation directory. Export the
existing bootstrap `VEOVEO_DATABASE_PASSWORD` privately for its first run, without
putting its value in an argument or copying the rest of operator state into a sandbox:

```sh
./identity/configure-hub \
  --hub-origin https://vv-control--hub--http.openshell.localhost:17670
```

The origin is a required input, fixed to workspace `vv-control`, sandbox `hub`,
service `http`, gateway port17670. On deployment compare the native exposure's
returned URL with this exact origin before starting Time or accepting application
results. The helper does not create the exposure or change gateway configuration.

It configures only `vv-hub-member`, `vv-hub-admin` and `vv-hub-forwarder` in the local
Keycloak realm. Existing client secrets and service-account subjects are retained;
a conflicting saved secret fails for recovery rather than rotating it. Scope links
are explicit, default realm grants/groups are removed from these dedicated service
accounts, and the forwarder gets only `openshell-user` with `sandbox:write`.
No application client receives an OpenShell role. Application access tokens use
RFC9068 `at+jwt`, audience `<Hub origin>/mcp/local`, scope `mcp`, subject/client ID,
service principal, tenant/context `local` and automated invocation claims.

The helper writes owner-only files beneath ignored `state/openshell/hub/`:

| File | Owner and use |
|---|---|
| `hub.json` | Existing Hub catalog schema: external IdP AS, remote alias JWKS, member/admin principal grants and context memberships, registration and startup native binding. No caller authentication secret/JWKS or local application-token issuer. |
| `hub.env` | Trusted Hub only: persistent forwarded-token PKCS#8 key, separate audit seed and refresh-delivery key, registration secret and existing database-owner password; also fixed runtime settings. Ordinary reruns preserve it. No caller signing keys are generated. |
| `installation-ca.crt` | Public installation trust anchor for IdP verification, readable by the trusted Hub runtime. |
| `member.secret`, `admin.secret`, `forwarder.secret` | Local recovery copies of the existing dedicated IdP client secrets; never mount these in a workload. |
| `*.refresh.env` | Standard private shell environment for importing each client's native refresh material. Never mount these in a workload. |

The native management client trusts `/run/openshell-supervisor-ca/material/ca.crt`,
provisioned read-only by OpenShell. The supervisor intercepts that HTTPS connection
and presents a certificate signed by its workload CA. Separately, the supervisor
verifies the upstream gateway using its driver-owned gateway CA at the pinned
gateway socket. Do not copy that upstream CA into Hub or replace workload trust
with it; both TLS connections retain hostname verification.

The helper prints the forwarder's native service-account subject for the separate
workspace membership step. It runs no Docker, database, native provider, lifecycle
or deployment operation. Do not use the hosted-auth local generator here, because
that fixture generates caller signing keys and a Hub application issuer.

## Native providers and workspace authority

Create the two native workspaces through the trusted operator identity. Grant the
printed forwarder subject **user** membership only in `vv-services`:

```sh
openshell workspace create vv-control
openshell workspace create vv-services
openshell workspace member add --workspace vv-services \
  --subject '<printed-forwarder-subject>' --role user
```

It must have neither platform-admin authority nor control/factory workspace
membership. Workload policy further admits only the ForwardTcp RPC, so its opaque
reference cannot be used to call other management methods.

Import [the forwarder profile](providers/hub-forwarder.yaml) in `vv-control`, then
configure its refresh material using the standard CLI. Run this block in a trusted
subshell so the client secret leaves the shell environment afterward:

```sh
(
  set -a
  . ./state/openshell/hub/forwarder.refresh.env
  set +a
  openshell profile import --workspace vv-control --file openshell/providers/hub-forwarder.yaml
  openshell provider create --workspace vv-control --name vv-hub-forwarder \
    --type vv-hub-forwarder --runtime-credentials
  openshell provider refresh configure --workspace vv-control vv-hub-forwarder \
    --credential-key HUB_MANAGEMENT_REFERENCE --strategy oauth2-client-credentials \
    --material "client_id=$VV_HUB_FORWARDER_CLIENT_ID" \
    --secret-material-env client_secret=VV_HUB_FORWARDER_CLIENT_SECRET
  openshell provider refresh rotate --workspace vv-control vv-hub-forwarder \
    --credential-key HUB_MANAGEMENT_REFERENCE
)
```

This one initial rotation is provisioning, not renewal evidence. Check existing
profile/provider definitions before rerunning creation; do not replace foreign or
conflicting state. Native custody stores the secret refresh material. Hub receives
only `HUB_MANAGEMENT_REFERENCE`, captured once by its native transport.

Import [the registration profile](providers/time-registration.yaml) in
`vv-services` and store only the initial access token as a static provider credential:

```sh
(
  set -a
  . ./state/openshell/hub/hub.env
  set +a
  openshell profile import --workspace vv-services --file openshell/providers/time-registration.yaml
  openshell provider create --workspace vv-services --name vv-time-registration \
    --type vv-time-registration --credential VEOVEO_REGISTRATION_SECRET
)
```

Time receives only its injected reference. [Time's policy](time.yaml) binds it to
POST `/oauth/register`; metadata, JWKS, delegated token exchange and later SDK
registration management use separate unbound path endpoints. Time keeps its own
registration key and SDK-issued management credentials.

[Member](providers/hub-member.yaml) and [admin](providers/hub-admin.yaml) profiles
use the same native refresh flow with their corresponding `*.refresh.env` and
`HUB_MEMBER_REFERENCE` / `HUB_ADMIN_REFERENCE` credential keys. Install them only in
the separately approved client workspaces. The author factory receives only the
member provider. [The SDK client policy](hub-client.yaml) is a member-only access
preflight template using MCP inspection and explicit `2026-07-28` methods/tool
names for Time calls, Tasks and subscriptions/reconnect. It is not the full trusted
two-identity acceptance policy or the factory's Lore/dependency policy. Its SDK
executable must exist at the admitted path. The static bearer path uses MCP
Discover directly; it needs no REST metadata rule on the application host/port.
Keep registration, issuer metadata and JWKS as narrow REST endpoints in their
service policies. Native policy rejects mixing MCP and REST on the same host/port.
The [trusted acceptance policy](hub-qualification.yaml) admits the fixed executable
`/sandbox/bin/time-acceptance`, the two admin-inspection calls and deliberate
`time__undeclared_tool` probe. Its [qualification-only provider profile](providers/hub-qualification.yaml)
declares two independent credentials. Import it and create `vv-hub-qualification`
only in the trusted runner's approved workspace, then configure refresh twice:
credential key `HUB_MEMBER_REFERENCE` uses the member refresh material, and
`HUB_ADMIN_REFERENCE` uses the admin material. Each has its own client ID, secret
and native opaque handle; one endpoint binding permits exactly those two keys.
Native resolution selects the supplied handle, never substitutes one identity for
the other. Do not attach this provider to the author factory. Production member
policy stays narrow. Separate conflicting bindings on the same endpoint are not a
supported alternative.

Set `VEOVEO_ACCEPTANCE_MEMBER_BEARER_ENV=HUB_MEMBER_REFERENCE` and
`VEOVEO_ACCEPTANCE_ADMIN_BEARER_ENV=HUB_ADMIN_REFERENCE` in that runner. The
[Time acceptance source](https://github.com/VeoveoAI/veoveo-mcp-time/blob/main/README.md#acceptance) now sends a valid
modern MCP unauthenticated request requiring Hub HTTP401/Bearer challenge, and
checks actual schedule Task cancellation with a pushed and retained Cancelled
outcome. Native transport refusal must not count as Hub permission refusal. Run
the seven ordinary live cases using the prepared test executable:

```sh
/sandbox/bin/time-acceptance --ignored --skip restart_persistence_ \
  --skip a_running_schedule_task_recovers_after_restart --nocapture --test-threads=1
```

Native automatic credential renewal preserves each opaque reference but fences
connections captured under the previous policy generation. This can interrupt an
in-flight request or subscription even when authored policy is unchanged. Preserve
the failure and explicitly reconnect; do not replay mutating calls automatically.
Qualify renewal separately with the same captured reference, unchanged workload
identities and a fresh successful call after the original token expiry. For an
isolated restart proof, observe successful and applied provider updates first and
allow sufficient time before the next scheduled refresh. Record a refresh-confounded
run as such rather than treating it as Task recovery evidence or changing policy.

Run `restart_persistence_prepare` and `restart_persistence_verify` separately around
trusted operator Time and Hub restarts, carrying only the printed standard event
URI/version and Task ID/expected terminal state. Run
`a_running_schedule_task_recovers_after_restart` alone with streamed native Exec
stdin/stdout: after its flushed Working Task readiness line, the operator proves
the old Time runtime stopped and the new runtime is ready, then sends exactly
`continue` followed by LF. The same Task must still be Working after that boundary,
then open Working on a fresh subscription and push Completed with the expected
typed schedule and retained result. Early completion fails as inconclusive.
Follow the [domain-owned instructions](https://github.com/VeoveoAI/veoveo-mcp-time/blob/main/README.md#in-flight-task-restart)
for exact invocations. The runner receives no management authority, files or
restart manifest. Source checks do not establish these deployed outcomes; record
actual runtime and registration identity evidence in the execution checklist.

The qualification policy intentionally permits these requests to reach Hub:

| Probe | Expected application result |
|---|---|
| Valid member discovery, resolve/convert, event calls and schedule Task | Admitted under member permissions. |
| `tasks/get`, `tasks/cancel` and `subscriptions/listen`, including a fresh connection | Admitted protocol methods; `tasks/get` carries the modern Task result. Durable ownership and delivery assertions belong to Time tests. |
| `time__list_sources` / `time__list_active_authorities` with member reference | Hub MCP/tool permission refusal, not an HTTP transport error. |
| The same inspection calls with admin reference | Admitted. |
| `time__undeclared_tool` | Hub's undeclared-tool MCP refusal. |
| Valid modern `tools/list` without Authorization | Hub401 and Bearer challenge. |

No rule admits every `tools/call`, and no unrestricted REST application POST is
used to bypass MCP inspection.

## Runtime state and placement

Select exact reviewed Hub and Time runtime image IDs explicitly. Both images run
as UID10001. The factory uses Time's `dev` image with the shared Lore layer and
UID1000; it receives no service state, signing keys, admin reference or Docker access.

The native Docker mount path requires operator approval and installed-daemon
validation before enabling `allow_driver_config`. Keep `resource_admission` enabled
and `enable_bind_mounts=false`. This native switch also admits validated tmpfs/CDI
fields; it is not a volume-only switch. These templates request only named volumes:

```sh
docker volume create --label openshell.ai/sandbox-attachable=true \
  --label openshell.ai/sandbox-attachable-workspace=vv-control vv-hub-config
docker volume create --label openshell.ai/sandbox-attachable=true \
  --label openshell.ai/sandbox-attachable-workspace=vv-services vv-time-state
```

Inspect existing volume labels and contents before adoption. Trusted operator
preparation must populate `vv-hub-config` with **only** `hub.env`, `hub.json` and the
public `installation-ca.crt`, readable by10001, directory0700/files0600. Never copy the entire
state directory. Prepare `vv-time-state` owned by10001 with `keys`, `acquisitions`
and `releases` directories. Preserve both volumes across sandbox replacement.
No bind mounts, image mounts, Docker sockets or operator directories are used.

[Hub mount JSON](hub-mounts.json) mounts its config read-only at `/etc/veoveo/hub`;
[Time mount JSON](time-mounts.json) mounts only its persistent state read-write.
These are standard `--driver-config-json` inputs, not a deployment manifest.
[Hub policy](hub.yaml) admits only its executable's exact native forwarding RPC,
public IdP JWKS GET and direct PostgreSQL connection. No token/admin IdP alias
route is admitted. Hub's trusted main command loads its private environment file:

```sh
/bin/sh -c 'set -a; . /etc/veoveo/hub/hub.env; set +a; exec /usr/local/bin/veoveo-hub serve --allow-loopback-hosts'
```

The existing `--allow-loopback-hosts` flag is required for this loopback-bound
native reverse proxy. OpenShell verifies the external service authority, then its
relay intentionally sends `Host: 127.0.0.1:8080` to Hub. The flag admits that Host
beside the configured public authority; it also admits `localhost` and `::1` with
any port. Keep Hub bound only to `127.0.0.1:8080`, retain native external
hostname/port checks and verified TLS, and do not trust forwarded headers.
`VEOVEO_ISSUER`, advertised URLs and token audiences stay fixed to the exact
external HTTPS origin; this flag changes neither application authentication nor
admission. Do not use it to expose a public listener or change Time's private
ForwardTcp contract.

Use that as the native sandbox command with Hub's selected image/policy/mount JSON
and `vv-hub-forwarder` provider. Do not pass signing keys or database passwords with
native `--env`, which is for non-secret settings. Create the named `http` exposure
on loopback8080 with `--authorization-mode bearer-passthrough` and verify its URL.
This helper does not execute any of those deployment steps. After validation and
operator approval, the corresponding native commands are:

```sh
openshell sandbox create --workspace vv-control --name hub --from "$HUB_RUNTIME_IMAGE_ID" \
  --policy openshell/hub.yaml --provider vv-hub-forwarder --no-auto-providers \
  --driver-config-json "$(cat openshell/hub-mounts.json)" --restart-policy on-failure --detach -- \
  /bin/sh -c 'set -a; . /etc/veoveo/hub/hub.env; set +a; exec /usr/local/bin/veoveo-hub serve --allow-loopback-hosts'
openshell service expose --workspace vv-control hub 8080 http \
  --authorization-mode bearer-passthrough
```

`HUB_RUNTIME_IMAGE_ID` must name the exact reviewed immutable image. Refuse
pre-existing conflicting sandboxes; these commands do not reconcile or delete them.

Time's non-secret runtime settings are:

```text
VEOVEO_HUB_ISSUER=https://vv-control--hub--http.openshell.localhost:17670
VEOVEO_PUBLIC_BASE_URL=http://time.internal:8800
VEOVEO_PORT=8800
```

After verifying Hub's exposure and registration metadata, use Time's exact reviewed
runtime image, [policy](time.yaml), state mount JSON and registration provider:

```sh
openshell sandbox create --workspace vv-services --name time --from "$TIME_RUNTIME_IMAGE_ID" \
  --policy openshell/time.yaml --provider vv-time-registration --no-auto-providers \
  --driver-config-json "$(cat openshell/time-mounts.json)" --restart-policy on-failure --detach \
  --env VEOVEO_HUB_ISSUER=https://vv-control--hub--http.openshell.localhost:17670 \
  --env VEOVEO_PUBLIC_BASE_URL=http://time.internal:8800 --env VEOVEO_PORT=8800 -- \
  /usr/local/bin/veoveo-mcp-time
```
 Hub's initial
`nativeUpstreams` binding preserves `http://time.internal:8800/time/mcp` while
ForwardTcp reaches that sandbox's loopback8800. Time needs no public exposure.
The binding is fixed at Hub startup; changes require a Hub restart.

Start PostgreSQL, Hub, then Time. Hub uses the existing owner at
`host.openshell.internal:5432/veoveo`; its `VEOVEO_SERVER_DATABASE_URL` supplies the
same reachable database address for registration grants. Time receives its own
login and migrates only its schema. Policies retain direct PostgreSQL sessions for
instance locks and LISTEN/NOTIFY; no HTTP credential proxy handles database traffic.

Hub readiness is its native origin `/readyz`; registration discovery is the same
origin `/.well-known/oauth-authorization-server`, with `/oauth/register`,
`/jwks.json` and `/oauth/token`. Time readiness is private `/readyz` through native
forwarding. Verify readiness, registration, IdP admission, permissions, Tasks,
subscriptions, credential renewal and restart from the actual sandboxes before
preparing the identical Git/Lore tree or running the no-model factory preflight.
