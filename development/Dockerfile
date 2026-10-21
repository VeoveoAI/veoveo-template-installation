# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Supply the reviewed immutable factory image as the named `factory` build context.
FROM factory
USER root
# Same checked Hub helper artifact, built once for this target platform.
COPY --from=environment --chmod=0755 /usr/local/bin/veoveo-environment /usr/local/bin/veoveo-environment
RUN apt-get update \
    && apt-get install -y --no-install-recommends s6=2.13.1.0-1 \
    && rm -rf /var/lib/apt/lists/* /var/log/apt /var/log/dpkg.log \
    && mkdir -p /sandbox/services /sandbox/builds /sandbox/sources \
        /sandbox/cache/cargo /sandbox/cache/target /sandbox/home \
    && chown -R 10001:10001 /sandbox \
    && chmod 0700 /sandbox /sandbox/services /sandbox/builds /sandbox/sources /sandbox/cache /sandbox/home \
    && test "$(id -u sandbox)" = 10001 && test "$(id -g sandbox)" = 10001
# Rustup's installed toolchain is immutable image content. An explicit toolchain avoids
# consulting a different HOME's missing default; Cargo's writable caches live on the PVC.
ENV HOME=/sandbox/home \
    RUSTUP_HOME=/usr/local/rustup \
    RUSTUP_TOOLCHAIN=1.99.0 \
    CARGO_HOME=/sandbox/cache/cargo \
    CARGO_TARGET_DIR=/sandbox/cache/target
USER 10001:10001
WORKDIR /sandbox
# Native OpenShell owns this canonical main process. s6 owns per-build child workers;
# disconnecting an Exec build/control caller must not own their process lifetime.
ENTRYPOINT ["/usr/bin/s6-svscan", "/sandbox/services"]
CMD []
