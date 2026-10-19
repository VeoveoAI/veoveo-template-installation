# Agent Instructions

- This repository holds installation choices and trusted bootstrap configuration.
  Planning belongs in `veoveo-docs`. Components own their build and check logic.
- All coding factories and managed MCP workloads use OpenShell's native Docker
  driver. Use its existing APIs, lifecycle, credential store, policy and diagnostics.
  Do not add Compose author/application profiles, a token worker or a second launcher.
- Use permanent Keycloak locally and provider-agnostic OIDC configuration elsewhere.
  Generic installation must not require Keycloak administration credentials.
- Real provider tokens, signing keys, operator state, bootstrap volumes and Docker
  access stay outside author and agent-controlled service workloads. Use native
  OpenShell credential mediation. Unsupported enforcement fails closed.
- Prefer upstream configuration and extension points. Maintained, revision-pinned
  VeoveoAI security/transport patches are allowed by the accepted architecture.
  Preserve ordinary Lore operations and upstream security boundaries. The factory
  branch guard belongs in Lore's existing pre-push hook and trusted configuration;
  it is not a comprehensive branch authorization or transport-expiry system.
  Credential lifecycle qualification stays separate in native OpenShell controls.
- The user withdrew Desktop acceptance on 2026-10-17. The current accepted
  [Hub-issued native Lore architecture](../veoveo-docs/LORE_OPENSHELL_INTEGRATION.md#hub-issued-native-lore-credentials)
  uses Entra for human identity, Hub's existing issuer and revisioned control plane
  for explicit repository assignments/native tokens, and unchanged native Lore
  enforcement. Native OpenShell owns factory renewal/injection; user Lore refresh
  remains in each private Computer. The retired Desktop adapter/standalone signer
  and ingress remain retired. Do not add another issuer service, refresh daemon,
  custom Lore authorization rules, automatic admin/member repository rights or
  global grants to compensate for an installation mismatch. Local Keycloak
  remains identity/management bootstrap; repository assignments belong to Hub.
  Do not restore the retired direct-issuer provisioning helper or its grant state.
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
