# Reusable source runtime

This image is the source-activation runtime input for the accepted environment slice.
It derives from a reviewed immutable Factory image through a named Docker build
context; it does not change existing Factory jobs or Computers. Supply the actual
published Factory digest, not a mutable tag:

```sh
docker build \
  --build-context factory=docker-image://REGISTRY/veoveo-factory@sha256:DIGEST \
  --file development/Dockerfile --tag veoveo-development:reviewed .
```

The pinned Debian `s6=2.13.1.0-1` provides `/usr/bin/s6-svscan`, the fixed native
sandbox main process. It supervises service directories under `/sandbox/services`.
Supply `/usr/bin/s6-svscan /sandbox/services` explicitly as the command in native
sandbox creation (`-- /usr/bin/s6-svscan /sandbox/services` with the CLI). Omitting
the command selects the CLI's interactive shell default; it does not run this
image's entrypoint.
Native scoped Exec prepares builds and controls s6; child worker lifetime belongs
to s6, independently of the Exec caller. No environment/source manifest parser or
activation reconciler is installed by this image yet. Native lifecycle, policy,
credential mediation and named-service admission remain OpenShell responsibilities.

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
