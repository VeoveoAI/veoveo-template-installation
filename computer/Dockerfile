# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Required named contexts: reviewed factory, Lore and environment-helper images,
# and the matching-platform OpenCode package directory (containing bin/opencode).
FROM factory
USER root
# Same checked Hub helper artifact, built once for this target platform.
COPY --from=environment --chmod=0755 /usr/local/bin/veoveo-environment /usr/local/bin/veoveo-environment
COPY --from=lore --chmod=0755 /usr/local/bin/lore /usr/local/bin/lore
COPY --from=opencode --chmod=0755 /bin/opencode /usr/local/bin/opencode
COPY --chmod=0644 computer/opencode.json /etc/computer/
ENV HOME=/sandbox/home \
    SSL_CERT_DIR=/etc/ssl/certs \
    XDG_CONFIG_HOME=/sandbox/home/.config \
    XDG_DATA_HOME=/sandbox/home/.local/share \
    XDG_STATE_HOME=/sandbox/home/.local/state \
    XDG_CACHE_HOME=/sandbox/home/.cache \
    LORE_GLOBAL_PATH=/sandbox/home/.local/share/lore \
    LORE_AUTH_PATH=/sandbox/home/.config/lore/auth \
    OPENCODE_CONFIG=/etc/computer/opencode.json \
    OPENCODE_DISABLE_MODELS_FETCH=1
# Native Docker copy-up or Kubernetes automatic workspace-PVC seeding preserves
# these image defaults and private UID/GID-10001 paths; no extra copy-up Job is needed.
# Native connection configuration stays read-only; public managed guidance is
# seeded in the persistent home before OpenCode discovers its instruction source.
RUN test "$(id -u sandbox)" = 10001 \
    && test "$(id -g sandbox)" = 10001 \
    && install -d -o 10001 -g 10001 -m 0700 /sandbox /sandbox/home /sandbox/projects \
       /sandbox/home/.config /sandbox/home/.config/computer /sandbox/home/.local /sandbox/home/.local/share \
       /sandbox/home/.local/state /sandbox/home/.cache \
       /sandbox/home/.local/share/lore /sandbox/home/.config/lore /sandbox/home/.config/lore/auth \
    && chmod 0755 /etc/computer
COPY --chown=10001:10001 --chmod=0600 computer/instructions.md /sandbox/home/.config/computer/instructions.md
USER sandbox
WORKDIR /sandbox
# Hub also supplies this fixed command as native Create governance. Native
# templates do not contain command/policy, and callers cannot override it.
CMD ["/bin/sh", "-ec", "umask 077; exec /usr/local/bin/opencode serve --trusted-local-proxy --hostname 127.0.0.1 --port 4096 --base-path \"${PUBLIC_BASE_PATH:?trusted PUBLIC_BASE_PATH is required}\""]
