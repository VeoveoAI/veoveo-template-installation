# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
ARG BASE=rust:1.99.0-trixie@sha256:6ff07edce8775d0f64be7aba9197229407301bddf2054d62c27b541a6238a181
FROM ${BASE}
USER root
# The base of Computers: the component toolchain, as the components' own factory images
# have it (clang for bindgen, cmake, pkg-config, and the clippy and rustfmt their
# rust-toolchain.toml names, which rustup cannot add where the toolchain is read-only),
# and ripgrep, which OpenCode searches with. Lore and the Hub SDK CLI are not in it:
# Computers mount their images read-only (kubernetes/computer-driver-config.json).
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates clang cmake git pkg-config protobuf-compiler libprotobuf-dev ripgrep tzdata \
    && rm -rf /var/lib/apt/lists/* /var/log/apt /var/log/dpkg.log \
    && rustup component add clippy rustfmt
# The trusted named context contains reviewed public CA certificates only, one per file.
COPY --from=ca --chmod=0644 /*.crt /usr/local/share/ca-certificates/
# Match the native Kubernetes runtime identity, including image-workspace seeding.
RUN groupadd --gid 10001 sandbox \
    && useradd --create-home --uid 10001 --gid 10001 --shell /bin/bash sandbox \
    && test "$(id -u sandbox)" = 10001 && test "$(id -g sandbox)" = 10001 \
    && find / -xdev -type f -perm /6000 -exec chmod ug-s {} + \
    && update-ca-certificates >/dev/null \
    && mkdir -p /sandbox && chown sandbox:sandbox /sandbox
USER sandbox
WORKDIR /sandbox
CMD ["sleep", "infinity"]
