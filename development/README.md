# Reusable source runtime

This image is the source-activation runtime input for the accepted environment slice.
It derives from a reviewed immutable Factory image through a named Docker build
context; it does not change existing Factory jobs or Computers. Supply the actual
published Factory digest, not a mutable tag. Build the helper once from the same
checked Hub source revision using its `environment-artifact` target, and supply
that immutable image as the `environment` context:

```sh
docker build \
  --build-context factory=docker-image://REGISTRY/veoveo-factory@sha256:DIGEST \
  --build-context environment=docker-image://REGISTRY/veoveo-environment@sha256:DIGEST \
  --file development/Dockerfile --tag veoveo-development:reviewed .
```

The pinned Debian `s6=2.13.1.0-1` provides `/usr/bin/s6-svscan`, the fixed native
sandbox main process. It supervises service directories under `/sandbox/services`.
Supply `/usr/bin/s6-svscan /sandbox/services` explicitly as the command in native
sandbox creation (`-- /usr/bin/s6-svscan /sandbox/services` with the CLI). Omitting
the command selects the CLI's interactive shell default; it does not run this
image's entrypoint.
Native scoped Exec prepares builds and controls s6; child worker lifetime belongs
to s6, independently of the Exec caller. The packaged `veoveo-environment` helper
materializes verified snapshots, executes foreground repository build/check recipes,
and prepares/registers/starts revision workers. Hub owns desired/serving state and
selection; this image adds no daemon or scheduler. Native lifecycle, policy,
credential mediation and ForwardTcp placement remain OpenShell responsibilities.
The helper receives private transfer/admission input through native Exec stdin,
not template environment or command-line credentials. Initial helper packaging and
the full source-activation path still require their own deployed qualification.

The runtime runs as UID/GID 10001. Native Kubernetes automatic workspace-PVC seeding
retains `/sandbox/services`, `/sandbox/sources`, `/sandbox/builds`, and writable
`/sandbox/cache` across native lifecycle. Rustup uses the image's installed Rust
1.99.0 explicitly, while Cargo home and incremental targets use the private PVC.
Toolchain changes require a separately admitted runtime image/template change.

Builds may share the incremental target cache, but a worker must execute a copied,
hash-recorded binary at a fresh `/sandbox/builds/<BuildId>/service`, never the mutable
Cargo target path. Interpreted workers likewise retain separate source directories.
The activation implementation must keep an old worker's source/output unchanged
while its in-flight work drains; one reusable sandbox need not mean one process.

The Linux native-runtime probe on 2026-10-20 verified offline Rust/Cargo compilation
as UID/GID 10001, independently routed HTTP workers, replacement of one worker
without changing its sibling, s6, Pod UIDs or container IDs, and recovery from the
automatic workspace PVC after native Stop/Start. Its disposable sandbox was removed.
The probe used synthetic HTTP markers and independent build directories; it did
not exercise the typed Hub activation contract, application credentials or MCP
environment routing. Full source-activation acceptance remains separate.

## Initial native runtime admission

The [Time runtime policy](../kubernetes/policies/development-time-v1.yaml) is an
installation-specific Linux example. It admits the exact helper for SDK registration,
metadata/resume and signed Artifact bytes, the unchanged Lore CLI for private native
TLS, explicitly listed Cargo/Git dependency authorities and build-specific service
executables for Time's metadata/database access. It injects no global registration
secret or shared factory Lore credential. The application's own TLS clients still
verify the server roots in the image. Review the dependency origins/binaries for
another recipe; this example is not a universal interpreted-worker policy.

Create the native workspace/template through the ordinary operator CLI, using the
inspected immutable initial image (no per-edit image publication):

```sh
openshell workspace create --name vv-environments
openshell sandbox template create development-time-v1 --workspace vv-environments \
  --image "$DEVELOPMENT_IMAGE" --cpu 2 --memory 4Gi \
  --env HOME=/sandbox/home --env RUSTUP_HOME=/usr/local/rustup \
  --env RUSTUP_TOOLCHAIN=1.99.0 --env CARGO_HOME=/sandbox/cache/cargo \
  --env CARGO_TARGET_DIR=/sandbox/cache/target \
  --driver-config-json "$(cat kubernetes/development-time-driver-config.json)"
```

These nonsecret native template variables are required for scoped Exec as well as
the canonical main. Native Exec reconstructs its environment from native
template/spec settings; image `ENV` alone does not supply the build helper's
toolchain/cache configuration. The helper supplies each worker's deployment ID,
private BuildId key directory, port, public base URL and Hub issuer as fixed argv.
It prepares and saves the scoped registration before s6 starts the worker; the
worker resumes that registration and obtains its database login through the SDK.
Do not put a namespace secret, database password or manually chosen worker ID in
the recipe or template environment.

Before using the template, follow the existing
[workspace-local public CA seed procedure](../kubernetes/WORKLOADS.md#postgresql-client-trust-shared-within-each-workspace)
for the actual native `vv-environments` namespace. Render the existing CA PVC/Job
documents for that workspace, preserving its attachable-workspace label and the
approved public certificates. Native workspace/template creation creates logical
records; the first sandbox Create prepares the managed Kubernetes namespace. For a
new workspace, run the initial package probe there with the reviewed image, policy,
explicit environment and s6 command, without the additional CA PVC mount. That
probe checks the packaged tools and files, not application/database access. Seed
the public CA claim in the resulting native namespace before starting a runtime
from the final CA-mounted template, then remove the probe through native lifecycle.
Do not create a replacement namespace or dummy provider.
The [CA-only driver input](../kubernetes/development-time-driver-config.json)
mounts that workspace's `postgres-client-ca-v1` read-only at `/etc/veoveo/tls`,
matching the explicit `sslrootcert=/etc/veoveo/tls/postgres-ca.crt` in Hub's issued
database grant. It does not mount the base Time service's state PVC. The private
automatic `/sandbox` PVC remains this runtime's source/build/cache state.

The selected initial image must also contain Time's existing bootstrap inputs:
`/usr/sbin/zic`, `/usr/share/zoneinfo/tzdata.zi`, and
`/usr/share/zoneinfo/leap-seconds.list`. Check their presence in the selected image
before admission; the reusable runtime does not implicitly inherit the Time
component image's packages. A recipe using Time's defaults reads those public
files and places its writable acquisition/release directories under
`/sandbox/cache/time`.

Native automatic workspace-PVC seeding supplies the private `/sandbox` directories.
Hub's trusted native-control identity needs existing `sandbox:write` and the exact
Create/Get/Exec RPC admissions in this workspace; add no new execution scope.
Install the reviewed policy as the public file named by the admission, using the
existing trusted Hub config claim/seed procedure. The native template itself does
not contain governance policy or provider attachments.

[hub.example.json](hub.example.json) is a fragment of the existing live control-plane
contract, not a second runtime inventory. Merge it into the current full revision
through the ordinary trusted publication/CAS path. Preserve unrelated fields.
Replace the illustrative EnvironmentId with the actual owned environment: the user
can first create an empty environment using the owner API, then an operator grants
this precise environment/component eligibility. The owner subsequently applies
source/recipe changes without AdminWrite. `providers: []` is explicit for this
Time example; provider-using components require reviewed native provider names
here, never credentials supplied in source. `shared_logical_schema` deliberately
uses Time's current schema and ACLs; it does not clone or isolate application data.
For local snapshot materialization, the Hub policy must separately admit its own
exact configured profile MCP resource for `server/discover` and
`artifact__create_download`, as in the
[Hub policy example](../kubernetes/policies/hub-computer-entra.example.yaml).
That call carries the currently admitted user's bearer; no provider credential
injection is configured on this self-call. Replace the example `/mcp/computer`
with each actual profile resource used for source apply. This platform/default
Artifact connection is independent of the candidate environment's selection.

Use separate immutable source and BuildId directories. Time recipes can place their
writable acquisition/release caches under `/sandbox/cache/time` with Time's native
configuration flags. A changed native image/template/policy/provider set requires
an explicit new versioned runtime admission; reusing its name must not silently
change an already retained runtime. `/readyz` plus the exact live registration gates
selection; ordinary selected MCP sessions still have to qualify discovery and tools.

## Owner apply and native definition following

After an owner creates their environment and the operator admits its exact
component/template/storage binding, ordinary apply uses that Computer's existing
`COMPUTER_HUB_URL` and opaque `COMPUTER_HUB_REFERENCE`. Keep those native values in
the image workload environment when invoking the client through native Exec; Exec
session defaults are not automatically the image's environment. Use the one
[Hub definition and owner API](../../veoveo-hub/README.md#environment-source-activation):

```sh
veoveo-environment apply --definition environment.json --local time=./time --exclude target
veoveo-environment watch --definition environment.json --local time=./time --exclude target
```

Only the selected source directory and explicit exclusions enter a snapshot. The
client reserves desired identity before uploading to private Artifact; completion
uses the same BuildId. Watch is opt-in, retries transient transport errors without
reserving obsolete intent, and does not synchronize the Computer home. Inspect the
owner-bound apply status and diagnostics for bounded check/compiler failure output.
A failed candidate leaves the previous serving worker selected. A native outage
can recover; a deterministic failed check/build needs explicit retry or new source.

The same `EnvironmentDefinition` JSON may be published as an ordinary file in an
authorized native Lore repository. Select it explicitly with:

```sh
veoveo-environment follow-definition --environment "$ENVIRONMENT_ID" \
  --deployment "$LORE_DEPLOYMENT_ID" --repository "$REPOSITORY_ID" \
  --branch "$BRANCH_ID" --path environments/work.json
```

Native subscriptions are hints; Hub reads exact current revisions on subscription,
reconnect and periodically. Local definition apply switches off this definition
follower; restoring it is explicit. Each component's source selector remains its
own local/pin/follow choice. Published branch rewinds are valid new transitions.
Current native grants are re-admitted; an expired caller JWT is not retained.

The initial Hub runtime build needs the reviewed native CLI artifact in addition
to the helper artifact used above:

```sh
docker build --build-context "lore=docker-image://$LORE_IMAGE" \
  --file ../veoveo-hub/Dockerfile --tag veoveo-hub:reviewed ../veoveo-hub
```

All named contexts must match the target platform. The Hub policy admits only the
exact native Lore executable to the configured private Lore endpoint. Short-lived
native credentials enter the fixed helper over private stdin; unchanged Lore's
existing `--access-token` is child argv inside its owner runtime, not logged top-level
Exec input or persisted worker state. Definition reads use Hub-owned bare temporary
repositories and native single-file reconstruction, so they do not require a
running Computer or a prepared component runtime.

This is candidate setup, not an acceptance receipt. Full component gates and real
local/Lore activation, retained Tasks, failure/restart and independent selections
must pass before marking the integrated workflow qualified.
