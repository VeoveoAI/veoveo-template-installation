# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
ARG BASE=rust:1.99.0-trixie@sha256:6ff07edce8775d0f64be7aba9197229407301bddf2054d62c27b541a6238a181
FROM ${BASE}
USER root
# Deterministic jobs use the component toolchain and native Lore CLI.
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates protobuf-compiler libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/* /var/log/apt /var/log/dpkg.log
# Component-owned conformance CLI; SDK has no runtime image. Keep its reviewed pin.
RUN cargo install --locked --git https://github.com/VeoveoAI/veoveo-hub-sdk \
    --rev a1d9d653ae3d723821cdeeab92f4a67acdca39ce --root /usr/local veoveo-hub-sdk-cli
COPY --from=lore --chmod=0755 /usr/local/bin/lore /usr/local/bin/lore
COPY --from=ca --chmod=0644 ca.crt /usr/local/share/ca-certificates/veoveo-installation.crt
RUN (id sandbox >/dev/null 2>&1 || useradd --create-home --uid 1000 --shell /bin/bash sandbox) \
    && find / -xdev -type f -perm /6000 -exec chmod ug-s {} + \
    && update-ca-certificates >/dev/null \
    && mkdir -p /sandbox && chown sandbox:sandbox /sandbox
USER sandbox
WORKDIR /sandbox
CMD ["sleep", "infinity"]
