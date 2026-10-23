# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Required named contexts: reviewed factory and Lore images,
# and the matching-platform OpenCode package directory (containing bin/opencode).
FROM factory
USER root
COPY --from=lore --chmod=0755 /usr/local/bin/lore /usr/local/bin/lore
COPY --from=opencode --chmod=0755 /bin/opencode /usr/local/bin/opencode
COPY --chmod=0644 computer/opencode.json /etc/computer/
# /sandbox is the person's home and the platform writes nothing into it: the
# factory base leaves only the mount point, owned by the sandbox user, and
# OpenCode and Lore create their own directories under HOME. Connection
# configuration stays read-only in /etc/computer.
ENV HOME=/sandbox \
    SSL_CERT_DIR=/etc/ssl/certs \
    OPENCODE_CONFIG=/etc/computer/opencode.json \
    OPENCODE_DISABLE_MODELS_FETCH=1
RUN test "$(id -u sandbox)" = 10001 \
    && test "$(id -g sandbox)" = 10001 \
    && chmod 0755 /etc/computer
USER sandbox
WORKDIR /sandbox
# Hub also supplies this fixed command as native Create governance. Native
# templates do not contain command/policy, and callers cannot override it.
CMD ["/bin/sh", "-ec", "umask 077; exec /usr/local/bin/opencode serve --trusted-local-proxy --hostname 127.0.0.1 --port 4096 --base-path \"${PUBLIC_BASE_PATH:?trusted PUBLIC_BASE_PATH is required}\""]
