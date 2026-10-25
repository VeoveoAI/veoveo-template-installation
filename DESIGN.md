# Installation Design

Linux uses K3s for foundation services and OpenShell's native Kubernetes driver
for Hub/MCP, Computer and factory workloads. This document owns how an installation
is assembled and deployed: placement, images, versions, configuration ownership,
identity wiring, policies and providers. The platform's components and trust
boundaries are in the
[architecture](https://github.com/VeoveoAI/veoveo-docs/blob/main/ARCHITECTURE.md), and
qualification status in the
[checklist](https://github.com/VeoveoAI/veoveo-docs/blob/main/QUALIFICATION_CHECKLIST.md).

## Placement

The ordinary [Kubernetes inputs](kubernetes/README.md) bootstrap PostgreSQL as a plain
StatefulSet, SeaweedFS, single-replica Lore, OpenShell gateway and the existing public
tunnel. K3s has an independent embedded datastore. Gateway and application PostgreSQL
roles/databases are distinct and share one PostgreSQL instance. This is a
single-node recovery target, not qualified multi-host HA.

Cilium enforces Kubernetes ingress and egress policy; K3s's own Flannel, network
policy, Traefik and ServiceLB are disabled. The Agent Sandbox controller and CRDs are
a prerequisite of OpenShell's Kubernetes driver. Workloads live in OpenShell
workspaces: `installation` holds the Computer template, `vv-control` holds Hub,
`vv-services` holds the platform MCP servers (console among them), `vv-factory`
holds factory instances, and Hub creates one workspace for each Computer owner.

These runtime names are the installation's. Foundation services run in the
`veoveo-system` namespace and the gateway in `openshell`; OpenShell creates a
namespace for each workspace. A platform sandbox is named after what it runs: `hub`,
`console`, `artifact`, `media`, `media-provider`, `embedding` and `embedding-runtime`.
Hub names a factory instance's sandbox after its deployment ID and a Computer's after
its Computer ID. The Computer template is `computer-v<n>`, and provider profiles are
`vv-<role>`.

Mac remains normal native source development and component/integration testing,
including Docker fixtures. Local Keycloak and its database may remain external.
No Mac Lima/K3s deployment is required. Compose recipes serve the Mac test and
identity fixtures; Linux deploys only on Kubernetes.

## Configuration Ownership

Kubernetes/Helm owns foundation resources only. OpenShell owns dynamic workspace
namespaces, sandbox resources, policies, providers, templates, lifecycle and
persistent workspace PVCs. Existing APIs create/remove workloads; Helm values
must not inventory runtime sandboxes or force unrelated rollouts. Hub catalogs
are database-authoritative: initial files seed only an empty installation. Live
MCP request destinations, Computer defaults and explicit repository grants adopt
through the admitted control-plane path, preserving active state on rejection.
New defaults make a new Computer version, which new sandboxes start at; an existing
Computer keeps its recorded version until its owner restarts it.

A value that would differ in a second installation belongs here; a value that changes
with a server's code belongs to that server's repository. The installation owns the
base policies, provider profiles, the Computer image and template, the factory
template, public origins and identity-provider clients. The factory template,
`factory-v1` in workspace `installation`, holds every factory sandbox's size and its
mounts, and Hub's `factory.template` names it. The soft caps and the base branch are
installation values too, but they live in Hub's control plane (`factory.caps`,
`execution.baseBranch`), seeded from [hub.example.json](kubernetes/hub.example.json)
and changed through Hub's live control-plane route; no other file carries them.

## Images

Veoveo's own images are built from source and served only by the installation's
registry; third-party images stay at their upstream registries. Every image is pinned
by digest.

Platform tools reach the workloads that use them from their own image. The Lore image
holds the only Lore CLI: the Lore server runs it, and Hub, Computers and factory
instances mount it read-only at `/opt/veoveo/lore` as a Kubernetes image volume pinned
by digest, through OpenShell's driver configuration. Computers mount the SDK CLI image
the same way at `/opt/veoveo/hub-sdk-cli`. No other image copies either tool, and Hub
holds no image reference for them: a Computer takes its mounts from the Computer
template, and a factory instance from the factory template, whose size and mounts Hub
applies with the branch's own image. Policies name the mounted binaries' paths. A
mounted binary uses the workload image's C library, as a copied one did. Image volumes
are stable in the installation's Kubernetes 1.36. OpenShell's resource admission,
which approves every attachment a sandbox requests, admits an image volume only when
it is pinned by digest and its repository is one the installation lists; upgrading a
tool changes its digest, never that list.

The installation names each image once, in `component-lock.json`, and fills every
manifest, template and driver configuration that uses it from there. Upgrading Lore
builds one image and changes one value. Each workload picks it up at its next start:
Hub when it restarts, a Computer as a new Computer version, and a factory instance
when its next push recreates it.

## Identity

Lore owns source/revision semantics and enforces its unchanged native signed-token
contract. Entra authenticates humans; Hub's existing issuer and revisioned control
plane own explicit repository assignments/native token issuance. Creating a
repository through Hub makes its creator the owner; no other grant is automatic,
and no administrator role grants repositories. There is no Desktop adapter,
standalone signer, custom Lore permission layer or token worker. Factory renewal/injection belongs to native
OpenShell. Human native Lore grants remain in each private Computer.

The installation's identity provider (Entra in the reference, local Keycloak for Mac
development, or a customer's own OIDC provider) authenticates people for Hub through
Hub's authorization-code broker. Hub's authorization server, at `{origin}/oauth`,
issues the application tokens and the native Lore tokens. The OpenShell gateway
validates the same provider for its own management API under a separate audience;
Hub's control identity is a client-credentials application of that provider, renewed
in OpenShell custody. A local Keycloak stays outside OpenShell: the gateway
initializes its OIDC issuer before it recovers workloads, so an issuer hosted inside it
could never start. An installation that uses Entra or another external issuer runs no
Keycloak.

Installation administration is the identity provider's `administrator` app role,
which Hub reads from the token's `roles` claim and which the control-plane policy rule
`installation-administration` names. Hub's `admin` installation role governs only
server tool permissions.

Hub's seed catalog is [identity/entra.example.json](identity/entra.example.json) merged
with [kubernetes/hub.example.json](kubernetes/hub.example.json) by typed keys. Its names
are one set: identity provider and broker client `entra`, tenant `reference`, Work
Context `reference-work`, policy revision `reference-policy`, profiles `local` and
`computer`, and authorization server `hub`. The OpenShell gateway accepts the same
provider for its management API, with its role names in the `roles` claim.

Lore's server settings ([lore.toml](kubernetes/lore.toml)) trust only Hub's
`{origin}/oauth` issuer, with audience `{origin}/resources/lore`, `at+jwt` tokens,
resource IDs `urc-{id}` and `baseline_access = "denied"`; Lore's public client is
`lore-computer`, with the `lore:login` scope and the scope template `lore-repo-{id}`.
QUIC is off, so every Lore operation is gRPC that OpenShell mediates. The repository
catalog is `baseline`: Lore lists no repositories per person. Lore runs no hook: what a
workload may do in Lore is its token's grants and its provider profile's endpoint rules.

## Storage

PostgreSQL is one StatefulSet with one volume, the `veoveo-postgres-rw` Service, TLS
from the installation's own PostgreSQL CA and no operator. Its image is the pinned
PostgreSQL server plus pinned pgvector and PostGIS packages; the extensions live in
the bootstrap-owned `_extensions` schema, usable by every login and writable by none.
The first start creates only `veoveo_owner` (`LOGIN CREATEROLE CREATEDB`, a member of
`pg_signal_backend`) with the `veoveo` database, the gateway's and Lore's logins, the
`openshell_gateway` database, the `lore` schema and `max_connections = 512`. Everything
after is plain SQL: Hub creates server roles and data-environment databases, and
settings change with `ALTER SYSTEM`. An operator's declared roles would fight the
memberships PostgreSQL gives Hub's role in every role it creates. `pg_hba.conf` admits
TLS with a password to every database, so databases created at run time are reachable.
The gateway, Lore and Hub name the CA through `sslrootcert`; servers in sandboxes
verify against the platform roots, so the server database URL Hub hands out keeps
`sslmode=verify-full` without `sslrootcert`. There is no connection pooler: Core-backed
processes hold session locks and listeners.

SeaweedFS runs from its official chart with TLS on every component. Each consumer has
its own bucket and bucket-scoped identity: `artifacts` for Artifact, which alone holds
`Admin` on its bucket for its startup configuration, and `lore` for Lore. Sharing the
store combines no credentials. Lore keeps its immutable and mutable stores in the
`s3_postgres` plugin of the maintained Lore fork, over the `lore` schema and bucket,
with a local lock store; one Lore process owns those locks, so Lore runs one replica.
Storage credentials stay in trusted private installation state and never enter
images or factories.

## Policies And Providers

Trusted server configuration lives in operator-populated, admitted, workspace-local
PVCs mounted read-only outside `/sandbox`. No caller Secret/ConfigMap/hostPath or
protected `/run` bypass is introduced. Computer `/sandbox` uses native automatic
per-sandbox persistence, never a shared explicit claim. Actual credentials,
private TLS/signing material and operator state are never tracked or included in
agent-controlled images. Only reviewed public CAs enter application build contexts.

Services/DNS provide private PostgreSQL, S3 and Lore addressing. Every workload's
policy is in OpenShell's own schema under [policies](kubernetes/policies). An endpoint
that carries a credential comes from the attached provider's profile, so Lore is
reached only through the credential-bound endpoint of `vv-lore` or `vv-factory-lore` and
no policy has a passthrough Lore rule. A profile's endpoint rules bound what its
credential may do, whatever the token allows. The public-only `platform-destination-roots-v1` bundle holds the
gateway CA, the Lore and SeaweedFS destination roots and the PostgreSQL CA; the
supervisor injects it into every workload's trust, and changing it means recreating
the affected runtimes.

Real credentials stay in the gateway's encrypted custody; a workload sees only the
placeholder its provider's environment names, which OpenShell replaces on the request.
A credential that must sign stays with its signer, such as Artifact's bucket key. The
provider profiles are global: `vv-lore` (a Computer's Lore access as its owner, every
method), `vv-factory-lore` (a factory instance's Lore access as its latest pusher,
reads only), both refreshed by the gateway with each one's own Hub client,
`vv-factory-registration` (an instance's registration token), the Computer's
`vv-computer-hub` and `vv-computer-model`, each platform server's
`vv-<server>-registration`, and Hub's own `vv-hub-native-control` and
`vv-hub-forwarder`, which never attach to a Computer.

## Public Ingress

One public host serves the installation. The remotely managed Cloudflare tunnel's
routes are, in order: `/artifacts/` goes to SeaweedFS's S3 Service with the public Host,
path, query and headers unchanged, because SigV4 binds them; when Media completes its
runs by provider webhooks, `/media/operations/{operation_id}/{token}` goes to Media
through OpenShell's native service exposure, and no other Media path is public;
everything else goes to Hub through OpenShell's native service exposure. An
installation whose Media checks run status instead publishes no Media route. The
tunnel verifies TLS to every Service against its public CA. Public origins stay
installation data.

## Factory

Factory sandboxes run in `vv-factory`. Each gets the base policy
[factory-base.yaml](kubernetes/policies/factory-base.yaml): its checkout in
`/sandbox/src`, Hub's metadata and token endpoints, PostgreSQL, and open internet
access stated as one explicit rule, every name of three or more labels plus the
two-label names in use on ports 80 and 443, so it can be tightened later. Inspected
rules for exact hosts still apply and private addresses stay closed. A repository's
`openshell/policy.yaml` adds rules only for something else. The base policy withholds
the log store, so no branch reads another instance's output. Each sandbox also gets the
factory template's size and its
[driver configuration](kubernetes/factory-driver-config.json), which mounts the Lore
image read-only, with the branch's own image.

Three installation services serve every repository and branch: Vector on every node
ships all container output to single-node VictoriaLogs, where it outlives its pod; one
BuildKit daemon in its user-namespace variant, which only Hub reaches, builds
environment images; and one plain Distribution registry, at a Service with a fixed
ClusterIP that the node's container runtime mirrors, stores them. Vector discovers
files every second and keeps its checkpoints on a volume, so short-lived pods are
captured and a restart re-reads nothing. Registry cleanup deletes unused tags under
`factory/`. No installation-wide factory client or secret exists: each instance's
Lore access comes from its own Hub client.

Factory builds share no writable cache. Branch code is untrusted, and Lore permissions
can differ between branches of one repository, so a cache that one build writes and
another reads would let a branch plant compiled code in a build it has no right to
change. Each instance compiles on its own sandbox's disk, and a new branch or a
recreated sandbox compiles from nothing.

The image builder keeps that rule for `openshell/image/`. Hub builds every branch with
BuildKit's cache-mount namespace (`BUILDKIT_CACHE_MOUNT_NS`, a build option the client
sets and a `Dockerfile` cannot) set to the SHA-256 of its repository ID and branch name,
so one branch's `RUN --mount=type=cache` directory is never another's, and prunes
BuildKit's cache mounts before and after every build, so one lives for one build only.
BuildKit keys a cached step by its instruction and inputs and not by what a cache mount
held, so a cache mount's content must not outlive its build. Layers stay cached: a push
that changes nothing in `openshell/image/` gets the same image back. The builder runs
no frontend but `docker/dockerfile`'s, whatever a `# syntax=` line names.

Build steps share the BuildKit daemon's network, so Kubernetes network policy, which
Cilium enforces, bounds what a build reaches: the daemon reaches the public internet,
the registry and cluster DNS and no other private or cluster address, and only Hub's
supervisor reaches the daemon. The log store admits only Vector, which writes to it,
and the supervisors of `vv-services`, where console reads it; no factory sandbox or
build reaches it. The registry has no authentication yet, so a build step can still
push an image to it. Every workload names its image by digest, so a pushed image
redirects nothing, but the registry's authentication stays open.

A factory instance reads Lore and never writes to it. Its Lore token is the latest
pusher's, with that person's grants, and the branch's code runs beside it: with write
access, code an earlier push planted would act with a later pusher's rights. So Hub's
`factory.loreProfile` names `vv-factory-lore`, whose endpoint admits only the read
methods the instance's start command uses, `lore clone` and `lore sync --reset`:
OpenShell refuses every other Lore method before the token is attached. Computers keep
`vv-lore`, with every method, since their owner pushes from them.

## Computers

The Computer template `computer-v1`, in workspace `installation`, its
[policy](kubernetes/policies/computer-v1.yaml) and Hub's `computers` settings make the
Computer version. Whoever publishes a template writes its `veoveo.ai/release-note`
annotation. The template may appear before or after Hub starts; Hub reads it when a
Computer is entered.

`/sandbox` is the person's home: the image sets `HOME=/sandbox`, ships only the empty
mount point and sets no XDG or Lore paths, and the platform never writes there.
Everything outside `/sandbox` and `/tmp` comes with the Computer version and is
read-only; `/etc/computer` holds the immutable connection configuration. The handbook's
one source is [computer/instructions.md](computer/instructions.md), published into
`computers.handbook` through the live control plane.

OpenCode's configuration ([opencode.json](computer/opencode.json)) selects
`deepinfra/XiaomiMiMo/MiMo-V2.6-Pro` by default, turns off its Git and Mercurial
plugins and its snapshots, since Lore owns source history and undo, and connects one
MCP server, `hub`, with the Computer's own Hub credential. The mounted Hub SDK CLI acts
as the owner the same way: the template sets `VEOVEO_CLI_HUB` to the Hub's public
origin, `VEOVEO_CLI_PROFILE=computer` and `VEOVEO_CLI_BEARER_ENV=COMPUTER_HUB_REFERENCE`,
and the policy's `hub` rule admits `/mcp/computer`, with the `computer-hub` credential
binding, for both OpenCode and the CLI's binary, so `veoveo-hub-sdk-cli tools` and `call`
need no flags. The model endpoint stays OpenCode's alone; the CLI's `login`, `logout`
and `ask` say that they do not apply in a Computer. `computers.collectAfterSeconds`
sets idle collection (7200 in the reference); the forwarding capacity is Hub's
`computers.forwardingConnectionLimit`, beside the gateway's per-sandbox and per-replica
TCP limits, which are separate from SSH's.

## Recovery

The tunnel preserves public signed S3 bytes and uses verified TLS to native Services.
Native controllers, readiness/restarts and application retry/reconnect handle
recovery; there is no custom startup DAG or proxy. Lore reads Hub's OIDC discovery at
startup and restarts until Hub serves its issuer; no init Job orders them.

## Applying An Installation

This repository's installer, `veoveo-installer`, makes a cluster match this
repository. It is an operator's command,
not a service: it keeps no state of its own, runs only when an operator runs it, and
exits, so it is neither an orchestrator nor a second launcher. It installs and
upgrades only the substrate the platform cannot deploy itself: the Kubernetes
foundation (Cilium, the Agent Sandbox controller, PostgreSQL, SeaweedFS, Vector and
VictoriaLogs, the registry and BuildKit), OpenShell, the Lore server, Hub, the
platform servers while they run as installation sandboxes, the provider profiles and
the Computer and factory templates. K3s comes first, from K3s's own installer.
Everything built on the platform is deployed by the factory, never by the installer.

Its inputs are this repository (the lock, manifests, Helm values, policies, provider
profiles, templates and Hub's seed) and the operator's private directory of
installation values and secrets, which never enters Git. Image digests come only from
the lock: `veoveo-installer build` builds a component at its locked revision and
records the digest there, and nothing else writes one.

[installation.toml](installation.toml) declares the substrate as an ordered list of
steps, each changing through one owner: Kubernetes manifests and ConfigMaps built from
repository files, a Helm release and its values files, OpenShell workspaces, provider
profiles, templates and sandboxes, and last Hub's control plane with the files of Hub's
trusted claim that come from this repository. The order is the dependency order apply
takes. A file names an image as `@image:<name>@`, which the installer fills from the
lock, or its digest alone as `@digest:<name>@`, as a chart's values do, beside a
registry and repository that plan requires to be the lock entry's repository; a file
still holding a `REPLACE_WITH_` marker is not ready and is not compared.
The private directory, mode `0700`, holds `private.toml`: the Kubernetes context and
OpenShell CLI sign-in the operator uses, where the Hub sign-in is kept (the Hub SDK
CLI's grant directory unless it names another), the private values files layered over
a release's own, a sandbox's private environment values (those its declaration lists
as `private_environment`, such as a server's deployment ID) and the identity
provider's registration (`[identity]`), from which the installer prepares Hub's
trusted files. Its paths are relative to the directory, and the installer writes
nothing there.

`veoveo-installer plan` compares each piece's declared state with what runs and
prints the difference without changing anything. It reads each piece from its owner:
a Kubernetes object as a server-side dry-run apply would leave it, against the object
as it is, with the fields an earlier `kubectl apply` set that the declaration no longer
has; a release's chart, status and supplied values; profiles and policies in
OpenShell's canonical form, parsed by OpenShell's own crates, without the rules
providers add to a sandbox's policy; a template's or sandbox's image, environment,
resources, driver configuration, providers, command and restart policy; Hub's control
plane through `GET /admin/{profile}/control-plane`, against the seed merged by typed
keys and the handbook, leaving out the members people change (grants, Work Context
memberships, output owners and initial grants, repositories and assignments), which a
publication carries from the active revision, so the seed's example people, such as
its example output owner, are never published; and Hub's trusted files, read in Hub's
sandbox through native exec. It lists, in step order, what differs and what it could
not read, and succeeds only when everything was read and matches. A seed Job runs
once and is deleted, so plan does not read the claim it fills.

`veoveo-installer apply` makes only those changes, in dependency order, checks each one
(readiness, registration with Hub, the standard probes) and stops at the first failure
with its evidence; run again, it changes nothing. First installation and every upgrade
take the same path. It changes nothing unless the plan read every piece and has a
change for each difference. A Kubernetes object is server-side applied as
`veoveo-installer`, which takes over the fields an earlier `kubectl apply` set and
removes the members it no longer declares, and a workload that reads a changed object
only when it starts is restarted; a release is upgraded with exactly its declared
values; a profile is linted, then imported or updated at the version read. OpenShell
has no template or sandbox update, so a template or a platform sandbox whose declared
state differs is deleted and created again: a template keeps its release note, and a
sandbox is stopped first and keeps its workspace, claims and providers. Each change is
checked (a rollout complete, a sandbox ready, Hub serving its issuer and the Computer
version, a server's deployment healthy in Hub's server health), and its step is then
planned again and must match; workloads that become ready only once Hub serves, such
as Lore, are checked after the last step.

Hub's trusted files that come from this repository, the Computer and factory base
policies, are named by their content: `<name>-<the first 12 hexadecimal digits of
their SHA-256>.yaml`. The seed names them as `@claim:<name>@`; the installer writes a
missing one into Hub's claim with the claim's seed Job, which copies every key of a
temporary Secret, and the control plane names the one Hub uses. A changed policy is a
new file, so no revision reads a file it did not name. The claim's policy files that
the new revision does not name are removed only after it is active.

It changes each piece through its owner's interface and adds no logic the owner
already has: Kubernetes through its API, Helm releases with `helm`, OpenShell through
its Rust SDK as the operator's OpenShell identity, and Hub's live control plane through
Hub's administrative route. Hub alone decides what a control plane is: plan sends the
document it would publish to Hub's validation
(`POST /admin/{profile}/control-plane/validate`), which answers it as Hub would keep it,
with its SHA-256, compares that with the active revision's, and shows the members where
Hub's two forms differ. While the claim step has still to write a file the document
names, which Hub reads to validate it, apply has Hub validate it once the file is
written. It publishes only what differs from the active revision and never resets the
control plane, so what people created since the seed, such as their repositories,
survives every apply: a publication takes the members people change from the revision
active at that moment and names that revision in `If-Match`, so Hub refuses it (412)
when another became active first, and apply reads, validates and publishes again,
three times at most. Hub reads its `factory` section only when it starts, so a
publication that changes it, as Hub's validation shows, restarts Hub. To read or
publish it, the operator signs in to Hub in the browser as themselves, an installation
administrator, with `veoveo-installer login` (the Hub SDK's sign-in, as its
`veoveo-hub-sdk-cli` client), so Hub's audit names the person and no standing
administrator credential exists; Hub refuses a document it does not admit, with the
reason, and keeps its active revision. A first installation needs no sign-in: Hub
seeds an empty control plane from this repository.


`veoveo-installer identity hub` prepares Hub's trusted files in the private
directory from the identity provider's registration: `hub.env` with Hub's own keys
(the forwarded-token and access-token Ed25519 keys, the audit and refresh-delivery keys
and the registration secret), the secrets `[identity.secrets]` supplies and the
settings `[identity.environment]` holds; the authorization server's public key set,
whose key ID is its RFC 7638 thumbprint; and the seed `hub.json`, the repository's seed
with that key ID and the people of the earlier seed or of `[identity] people`, never
the examples; a seed holding the same document however laid out is kept. Keys are
made once: over existing files it recovers them, checks that they agree with what the
private directory supplies, and otherwise refuses without writing, since a rotation is
a reviewed act of its own. The claim step writes these files into Hub's claim when the
claim lacks them, as on a first installation, and a changed seed; a changed key is
refused. The Mac fixtures are
`veoveo-installer fixture keycloak` (the persistent local Keycloak, its TLS leaf and
realm), `fixture openshell` (the native gateway's OIDC clients), `fixture hub` (the
fixture Hub's catalog, keys and service clients) and `fixture public-client` (a local
human PKCE client), each changing Keycloak only through its administration API.

The installer is Rust source in this repository, `installer/`, with its own check, and it
reads the lock with core's installation contract crate. Each installation owns its copy
and changes it freely: its steps are the first thing an installation customizes. An
installation brings in the template's later improvements by comparing its installer with
the latest template's and taking what applies, as it does with the rest of its files.
