# Installation Design

Lore is source/revision authority. OpenShell owns author/check/service execution,
credential custody, policy, lifecycle and forwarding. Veoveo owns application
permissions, context composition/routing and Task authority. The accepted design is
in [veoveo-docs](../veoveo-docs/LORE_OPENSHELL_INTEGRATION.md).

The local Compose file starts trusted bootstrap/data services. The former Lore
Desktop adapter and ingress are retired. Move compatible Lore, database and storage services
onto native OpenShell workloads after their persistence and protocol checks. Local
Keycloak, its dedicated database and HTTPS endpoint stay in Compose to avoid an
OIDC cold-start dependency loop. All managed author and MCP workloads use OpenShell's native
Docker driver. There is no Kubernetes, Helm, Flux or legacy runtime profile.

Desktop acceptance is withdrawn as of 2026-10-17. Follow the
[direct Lore credential cutover](../veoveo-docs/LORE_DIRECT_OIDC_PLAN.md): the IdP
issues ordinary repo grants, OpenShell renews/injects them, and native Lore validates
them. The qualified direct path has replaced the Desktop adapter/ingress. No replacement
Veoveo signer, token endpoint or refresh worker is part of the design.

Durable bootstrap data volumes are external to Compose cleanup. Ignored operator
state includes identity credentials, TLS keys and the hard-cut recovery bundle.
Factory workloads receive native credential placeholders and cannot mount operator
state, bootstrap volumes or Docker access.

OpenShell's native macOS service runs with a pinned release and qualified supervisor
image. Its external issuer and native workspace permissions authenticate management
calls. Private service traffic uses authenticated native forwarding; application
requests retain Veoveo permission checks and narrowed tokens.

The first implementation slice is local only. Linux replacement, general builds,
environment composition and application migration are subsequent work through the
same selected runtime path; their completion requires recorded evidence.

Private user Computers use Hub browser admission and native OpenShell owner
workspaces, provider custody, ForwardTcp and persistent Docker workspaces. The
[installation inputs](computer/README.md) keep mutable HOME/XDG/source state under
`/sandbox` and trusted agent configuration under `/etc/computer`. Human Lore grants
remain in native Lore's own store; Hub user grants and the installation model key
are mediated by native workspace-local providers. No agent or model credential is
restored to deterministic factories. Image/template preparation does not claim
live user, persistence, revocation or model acceptance.
