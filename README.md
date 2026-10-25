# veoveo-template-installation

A Veoveo installation records approved identity, component revisions and environment
configuration. Lore owns source/revisions; native OpenShell executes Computers,
factories and managed MCP workloads through its Kubernetes driver on Linux.

For Linux, start with the [K3s foundation](kubernetes/README.md) and
[native workload bootstrap](kubernetes/WORKLOADS.md). Kubernetes owns PostgreSQL,
SeaweedFS, Lore, the OpenShell gateway and the tunnel. OpenShell's native APIs own
dynamic sandboxes; Helm does not inventory them. These installation inputs are
being qualified on Linux; see the execution record for measured results.
Mac remains ordinary source development and component/integration testing with
local fixtures. No Mac Kubernetes cluster is required.

The selected [Lore storage backend](openshell/LORE_STORAGE.md) uses a dedicated
schema and login in the existing `veoveo` PostgreSQL database and a dedicated
SeaweedFS bucket. Its source/deployed qualification precedes the live backend switch.

For user Computers, Entra authenticates people into Hub. Hub's existing issuer
uses explicit repository registrations and tenant/principal assignments to issue
Lore's native signed resource grants. Native Lore enforces its existing token
contract; native OpenShell renews/injects separate factory credentials. There is
no additional issuer service, signer, refresh daemon or custom Lore authorization
layer. See Hub's [Lore credentials](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#native-contributed-credential-lifecycle). Local Keycloak remains management/identity bootstrap. The existing pre-push hook
is not comprehensive branch isolation.

## Mac development bootstrap

The application PostgreSQL image includes pgvector and PostGIS. Follow the
[build, existing-database enablement and rollback-only service checks](postgres/README.md);
Keycloak's separate database and application schema ownership are unchanged.

Keep the ignored `.env`, installation CA and private operator state. Local Keycloak
and its dedicated PostgreSQL are permanent bootstrap, alongside Lore, PostgreSQL,
SeaweedFS and the Keycloak HTTPS ingress. Their external data volumes survive
Compose cleanup. The native OpenShell gateway runs as the macOS service.

Complete the one-time [CA trust setup](identity/README.md#macos-certificate-trust),
then follow [native OpenShell setup](openshell/README.md). Local management bootstrap uses
`docker compose --profile keycloak up -d`. Lore repository registrations and
explicit assignments use [Hub's live control plane](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#live-native-destinations-and-computer-defaults),
not directory client/resource mappers. The former OIDC egress proxy is retired;
workload egress follows native OpenShell policy and credential mediation.
A fresh installation must first provision identity credentials and external volumes.
No agent joins bootstrap networks, mounts operator state or accesses Docker.

`factory/` packages a pinned Rust toolchain as the base of user Computers, which mount
the Lore CLI and the Hub SDK client from their own images. A pushed Lore branch of a server
repository with `openshell/run` is served from its own factory sandbox, built from
that repository's `openshell/image/`; the installation supplies the
[base policy, provider profiles, log store, image builder and registry](kubernetes/README.md#factory-sandbox-policy-and-providers).
OpenCode and model credentials belong in user Computers. The
[qualification checklist](https://github.com/VeoveoAI/veoveo-docs/blob/main/QUALIFICATION_CHECKLIST.md) records current acceptance.

The [Entra reference](openshell/REFERENCE.md) retains identity contracts and
historical Docker deployment receipts. Its Docker/Compose placement commands do
not launch the replacement Linux installation; use the Kubernetes instructions
above. Linux uses Entra and does not require local Keycloak.

[Bioma's installation](https://github.com/BiomaAI/veoveo-installation) is the concrete
Linux reference implementation derived from this template. It records the selected
native policies, public routes, source revisions and current qualification status.
Not every template component is enabled there. Keep reusable fixes in this template
and merge reviewed updates into that installation; private runtime state stays out
of both repositories. Follow its acceptance record to distinguish deployed and
verified behavior from work still in progress.

## User Computers

[Computer image and native configuration](computer/README.md) extend the reviewed
factory toolchain with native OpenCode and user-owned Lore credentials. Persistent
HOME/XDG state belongs in the native owner workspace; factory sandboxes run no
agent and hold no model credentials. These inputs require separate source,
image and deployed qualification.

## Scope and recovery

Desktop acceptance was withdrawn. Its adapter, signing/manual-token helpers and
Desktop-only ingress are retired; `grpcs://localhost:7443` is no longer an active
endpoint. Private signing/session recovery state and durable Lore/identity data
are preserved.

Earlier local direct-issuer receipts remain historical. Hub-issued native Lore,
interactive user login and the reference Linux/Entra Computer goal are active in
the [plan](https://github.com/VeoveoAI/veoveo-docs/blob/main/PLAN.md).
Source/configuration preparation is separate from deployed and model acceptance.
GPU/CUDA and general environment composition remain separate qualifications.
