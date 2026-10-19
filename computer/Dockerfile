# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Required named contexts: reviewed factory image, native Lore image, and the
# matching-platform OpenCode package directory (containing bin/opencode).
FROM factory
USER root
COPY --from=lore --chmod=0755 /usr/local/bin/lore /usr/local/bin/lore
COPY --from=opencode --chmod=0755 /bin/opencode /usr/local/bin/opencode
COPY --chmod=0644 computer/opencode.json computer/instructions.md /etc/computer/
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
# Native Docker volume copy-up preserves these seeded, private UID-1000 paths.
# Trusted configuration stays outside the persistent workdir and is not writable.
RUN test "$(id -u sandbox)" = 1000 \
    && install -d -o 1000 -g 1000 -m 0700 /sandbox /sandbox/home /sandbox/projects \
       /sandbox/home/.config /sandbox/home/.local /sandbox/home/.local/share \
       /sandbox/home/.local/state /sandbox/home/.cache \
       /sandbox/home/.local/share/lore /sandbox/home/.config/lore /sandbox/home/.config/lore/auth \
    && chmod 0755 /etc/computer
USER sandbox
WORKDIR /sandbox
# Hub also supplies this fixed command as native Create governance. Native
# templates do not contain command/policy, and callers cannot override it.
CMD ["/bin/sh", "-ec", "umask 077; exec /usr/local/bin/opencode serve --trusted-local-proxy --hostname 127.0.0.1 --port 4096 --base-path \"${PUBLIC_BASE_PATH:?trusted PUBLIC_BASE_PATH is required}\""]
