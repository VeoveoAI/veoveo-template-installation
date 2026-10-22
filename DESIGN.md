# Installation Design

Linux uses K3s for foundation services and OpenShell's native Kubernetes driver
for Hub/MCP, Computer and factory workloads. The accepted architecture and
qualification status live in [veoveo-docs](../veoveo-docs/LORE_OPENSHELL_INTEGRATION.md).
The ordinary [Kubernetes inputs](kubernetes/README.md) bootstrap PostgreSQL as a plain
StatefulSet, SeaweedFS, single-replica Lore, OpenShell gateway and the existing public
tunnel. K3s has an independent embedded datastore. Gateway and application PostgreSQL
roles/databases are distinct and share one PostgreSQL instance. This is a
single-node recovery target, not qualified multi-host HA.

Mac remains normal native source development and component/integration testing,
including Docker fixtures. Local Keycloak and its database may remain external.
No Mac Lima/K3s deployment is required. Existing Compose recipes are historical
Linux inputs or local test/identity fixtures; replacement Linux placement must
qualify before obsolete Linux launch paths are removed. The undeployed native
Docker infrastructure candidate is superseded and must not be executed.

Lore owns source/revision semantics and enforces its unchanged native signed-token
contract. Entra authenticates humans; Hub's existing issuer and revisioned control
plane own explicit repository assignments/native token issuance. There is no
Desktop adapter, standalone signer, automatic creator/admin grant, custom Lore
permission layer or token worker. Factory renewal/injection belongs to native
OpenShell. Human native Lore grants remain in each private Computer.

Kubernetes/Helm owns foundation resources only. OpenShell owns dynamic workspace
namespaces, sandbox resources, policies, providers, templates, lifecycle and
persistent workspace PVCs. Existing APIs create/remove workloads; Helm values
must not inventory runtime sandboxes or force unrelated rollouts. Hub catalogs
are database-authoritative: initial files seed only an empty installation. Live
MCP request destinations, Computer defaults and explicit repository grants adopt
through the admitted control-plane path, preserving active state on rejection.
New versioned defaults affect future or explicitly replaced instances; existing
Computers retain their admitted provisioning snapshot.

Trusted server configuration lives in operator-populated, admitted, workspace-local
PVCs mounted read-only outside `/sandbox`. No caller Secret/ConfigMap/hostPath or
protected `/run` bypass is introduced. Computer `/sandbox` uses native automatic
per-sandbox persistence, never a shared explicit claim. Actual credentials,
private TLS/signing material and operator state are never tracked or included in
agent-controlled images. Only reviewed public CAs enter application build contexts.

Services/DNS provide private PostgreSQL, S3 and Lore addressing. Public origins
stay installation data. The tunnel preserves public signed S3 bytes and uses
verified TLS to native Services. Native controllers, readiness/restarts and
application retry/reconnect handle recovery; there is no custom startup DAG or
proxy. Dependency interruption and dynamic lifecycle behavior require deployed
qualification, separate from component tests and final exact MiMo acceptance.
