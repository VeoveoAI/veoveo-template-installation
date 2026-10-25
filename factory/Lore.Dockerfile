# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Lore 0.10.2-nightly from the reviewed source pin in openshell/README.md.
# Native OIDC comes from upstream. The reviewed fork adds native S3/PostgreSQL
# storage and minimal opaque-token factory integration; the server runs no hook.
# Build the CLI/server together from the same source revision.
# Every host builds from source, because Lore's arm64 release
# binaries carry a Neoverse tuning that Apple Silicon does not run.
FROM rust:1.99.0-trixie@sha256:6ff07edce8775d0f64be7aba9197229407301bddf2054d62c27b541a6238a181 AS build
RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/* /var/log/apt /var/log/dpkg.log
COPY --from=lore-source / /build
WORKDIR /build
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release --locked --bin lore --bin loreserver \
    && install -D -m 0755 target/release/lore target/release/loreserver -t /out

FROM debian:trixie-slim@sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* /var/log/apt /var/log/dpkg.log \
    && find / -xdev -type f -perm /6000 -exec chmod ug-s {} +
COPY --from=build /out/lore /out/loreserver /usr/local/bin/
# The server's shipped defaults, then the installation's settings in local.toml.
COPY --from=build /build/lore-server/config/default.toml /etc/lore/config/default.toml
# The trusted named context contains reviewed public CA certificates only, one per file.
# Private issuer/signing material never enters this image.
COPY --from=ca --chmod=0644 /*.crt /usr/local/share/ca-certificates/
RUN update-ca-certificates
ENV LORE_CONFIG_PATH=/etc/lore/config
EXPOSE 41337/tcp
ENTRYPOINT ["loreserver"]
