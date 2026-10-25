# veoveo-template-installation

A Veoveo installation records approved identity, component revisions and environment
configuration. Lore owns source and revisions; native OpenShell executes Computers,
factories and the platform's MCP workloads. The [design](DESIGN.md) says how an
installation is assembled, and [component-lock.json](component-lock.json) pins each
component's revision and, once built, each image's digest.

## Linux: K3s

Linux deploys only on K3s, with OpenShell's Kubernetes driver.
[veoveo-installer](installer/README.md) applies [installation.toml](installation.toml)'s
steps; the guides keep the reference and the manual steps around it. Start with the
[K3s foundation](kubernetes/README.md), then the
[native workload bootstrap](kubernetes/WORKLOADS.md) and, for Embedding, the
[Kubernetes embedding guide](kubernetes/EMBEDDING.md). Kubernetes owns PostgreSQL,
SeaweedFS, Lore, the OpenShell gateway, the log store, the image builder, the
registry and the tunnel; OpenShell's native APIs own every sandbox, and Helm does not
inventory them.

Entra authenticates people into Hub. Hub's issuer issues Lore's native tokens from
explicit repository assignments, and whoever creates a repository through Hub owns
it; native Lore enforces its token contract and OpenShell renews and injects each
workload's credentials. There is no other issuer, signer, refresh daemon or Lore
authorization layer. See Hub's
[Lore credentials](https://github.com/VeoveoAI/veoveo-hub/blob/main/crates/veoveo-hub/DESIGN.md#native-contributed-credential-lifecycle).
Factory instances read Lore only: their provider profile admits no Lore write.

`factory/` packages a pinned Rust toolchain as the base of user Computers, which
mount the Lore CLI and the Hub SDK CLI from their own images. A pushed Lore branch of
a server repository with `openshell/run` is served from its own factory sandbox,
built from that repository's `openshell/image/`; the installation supplies the
[base policy, provider profiles, log store, image builder and registry](kubernetes/README.md#factory-sandbox-policy-and-providers).
[User Computers](computer/README.md) extend the factory toolchain with OpenCode; their
K3s template is in the [workload bootstrap](kubernetes/WORKLOADS.md#hub-and-automatic-private-computers).
OpenCode and model credentials belong in Computers only.

## Mac development fixtures

Mac is ordinary source development and component and integration testing; it runs no
cluster. These fixtures are for that alone and never launch a Linux installation:

- Compose recipes ([compose.yaml](compose.yaml)) run local Keycloak with its own
  PostgreSQL, the Keycloak HTTPS ingress, the application PostgreSQL with pgvector and
  PostGIS ([postgres](postgres/README.md)), SeaweedFS and Lore. Their external data
  volumes survive Compose cleanup. Keep the ignored `.env`, the installation CA and
  private operator state. A fresh fixture first provisions identity credentials and
  the external volumes: complete the one-time
  [CA trust setup](identity/README.md#macos-certificate-trust), then
  `docker compose --profile keycloak up -d`.
- The native OpenShell gateway runs as a macOS service with its Docker driver
  ([native OpenShell setup](openshell/README.md)). The `openshell/` documents and
  policies, `computer/policy.yaml` and `computer/hub.example.json` are that fixture's
  inputs. OpenShell's Docker driver has no image volumes, so the fixture's Computer
  has neither mounted CLI.
- The [Entra reference](openshell/REFERENCE.md) keeps identity contracts from the
  earlier Docker deployment; its placement commands launch nothing on Linux.

No agent joins bootstrap networks, mounts operator state or accesses Docker.

## Bioma's installation

[Bioma's installation](https://github.com/BiomaAI/veoveo-installation) is this
template's output with Bioma's values, laid out as the template produces it. It lists
its selections and every difference from the template. Keep reusable fixes in this
template and bring them into that installation; private runtime state stays out of
both repositories. The
[qualification checklist](https://github.com/VeoveoAI/veoveo-docs/blob/main/QUALIFICATION_CHECKLIST.md)
records what is accepted.
