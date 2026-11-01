# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Required named contexts: the reviewed factory image, the matching-platform OpenCode
# package directory (containing bin/opencode) and the n8n image, the `computer` target of
# veoveo-mcp-n8n's Dockerfile. Lore and the Hub SDK CLI come from their own images, which
# the Computer template mounts read-only under /opt/veoveo.
FROM factory
USER root
COPY --from=opencode --chmod=0755 /bin/opencode /usr/local/bin/opencode
# n8n in the person's Computer: Node.js and n8n as the n8n fork's image lays them out,
# veoveo-n8n-computer, which runs them, and the Veoveo nodes. The factory base is Debian
# trixie as the fork's image is, and installs the one library n8n's native modules add.
COPY --from=n8n /usr/local/bin/node /usr/local/bin/node
COPY --from=n8n /usr/local/lib/node_modules/n8n /usr/local/lib/node_modules/n8n
COPY --from=n8n /usr/local/bin/veoveo-n8n-computer /usr/local/bin/veoveo-n8n-computer
COPY --from=n8n /opt/veoveo/n8n /opt/veoveo/n8n
COPY --chmod=0644 computer/opencode.json /etc/computer/
# /sandbox is the person's home and the platform writes nothing into it: the
# factory base leaves only the mount point, owned by the sandbox user, and
# OpenCode and Lore create their own directories under HOME. So does cargo, as by
# default: the toolchain's own CARGO_HOME is read-only. Connection
# configuration stays read-only in /etc/computer.
ENV HOME=/sandbox \
    CARGO_HOME=/sandbox/.cargo \
    PATH=/opt/veoveo/lore/usr/local/bin:/opt/veoveo/hub-sdk-cli/usr/local/bin:${PATH} \
    SSL_CERT_DIR=/etc/ssl/certs \
    OPENCODE_CONFIG=/etc/computer/opencode.json \
    OPENCODE_DISABLE_MODELS_FETCH=1
RUN test "$(id -u sandbox)" = 10001 \
    && test "$(id -g sandbox)" = 10001 \
    && chmod 0755 /etc/computer \
    && ln -s /usr/local/lib/node_modules/n8n/bin/n8n /usr/local/bin/n8n \
    && node --version && n8n --version \
    && veoveo-n8n-computer --help >/dev/null
USER sandbox
WORKDIR /sandbox
# Hub also supplies this fixed command as native Create governance. Native
# templates do not contain command/policy, and callers cannot override it. It starts
# veoveo-n8n-computer in the background, again ten seconds after every exit, and replaces
# itself with OpenCode, so n8n never delays or stops OpenCode; both write to the
# Computer's own output.
CMD ["/bin/sh", "-ec", "umask 077; : \"${PUBLIC_BASE_PATH:?trusted PUBLIC_BASE_PATH is required}\"; (set +e; while :; do /usr/local/bin/veoveo-n8n-computer; echo \"veoveo-n8n-computer exited with $?; starting it again in 10 s\" >&2; sleep 10; done) </dev/null & exec /usr/local/bin/opencode serve --trusted-local-proxy --hostname 127.0.0.1 --port 4096 --base-path \"$PUBLIC_BASE_PATH\""]
