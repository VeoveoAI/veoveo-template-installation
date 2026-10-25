# Agent Instructions

- This repository holds installation choices, trusted bootstrap configuration and the
  installer that applies them (`installer/`, Rust). An installation made from this
  template owns all of it; to update one, compare it with the latest template and bring
  over what applies.
  Planning belongs in `veoveo-docs`. Components own their build and check logic.
- The installation's design is in [DESIGN.md](DESIGN.md) and the platform's in the
  [architecture](https://github.com/VeoveoAI/veoveo-docs/blob/main/ARCHITECTURE.md). Change the design in the same change as the
  configuration it describes.
- Linux deployment uses K3s foundation controllers and OpenShell's native Kubernetes
  driver for Hub/MCP, Computer and factory workloads. Mac remains ordinary source
  development/testing with local fixtures; do not provision a Mac cluster. Use native
  APIs, lifecycle, credential store, policy and diagnostics. Do not add an orchestrator,
  Helm sandbox inventory, token worker or second launcher. The undeployed native-Docker
  infrastructure candidate is superseded; preserve Mac test/identity bootstrap tooling.
- Use permanent Keycloak locally and provider-agnostic OIDC configuration elsewhere.
  Generic installation must not require Keycloak administration credentials.
- Real provider tokens, signing keys, operator state, bootstrap volumes and Docker
  access stay outside author and agent-controlled service workloads. Use native
  OpenShell credential mediation. Unsupported enforcement fails closed.
- Prefer upstream configuration and extension points. Maintained, revision-pinned
  VeoveoAI security/transport patches are allowed by the accepted architecture.
  Preserve ordinary Lore operations and upstream security boundaries. Lore runs no
  hook. A factory instance's Lore access is read-only through its `vv-factory-lore`
  provider profile, which admits only the read methods of `lore clone` and
  `lore sync --reset`; Lore's own permissions decide the rest. Credential lifecycle
  qualification stays separate in native OpenShell controls.
- The user withdrew Desktop acceptance on 2026-10-17. The current accepted
  [Hub-issued native Lore architecture](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#native-contributed-credential-lifecycle)
  uses Entra for human identity, Hub's existing issuer and revisioned control plane
  for explicit repository assignments/native tokens, and unchanged native Lore
  enforcement. Native OpenShell owns factory renewal/injection; user Lore refresh
  remains in each private Computer. The retired Desktop adapter/standalone signer
  and ingress remain retired. Creating a repository through Hub makes its creator
  the owner; no other repository grant is automatic, and no administrator role
  grants repositories. Do not add another issuer service, refresh daemon, custom
  Lore authorization rules or global grants to compensate for an installation
  mismatch. Local Keycloak
  remains identity/management bootstrap; repository assignments belong to Hub.
  Do not restore the retired direct-issuer provisioning helper or its grant state.
- The platform never writes into a Computer's `/sandbox`; it is the person's home.
- Never commit credentials. `.env`, TLS/private state and local backups remain
  ignored and excluded from build contexts.
- During rapid development, application data is disposable for authorized resets
  on breaking contracts; do not add migrations, backward compatibility or backups
  merely to retain development data. Retain source and trusted installation/identity
  configuration as rebuild inputs. Preserve normal durable ownership across routine
  sandbox lifecycle; revisit retention when production data enters scope.
- Use upstream bootstrap images pinned by digest. Verify releases when changing pins.
  Committed public origins use `https://veoveo.example.com`.
- Validate changed bootstrap configuration and the focused new path on macOS.
  The reference Linux deployment, Entra and signed Media webhook are now in scope
  under the execution plan. GPU/CUDA qualification is separate.
- Commit coherent changes without AI attribution.
