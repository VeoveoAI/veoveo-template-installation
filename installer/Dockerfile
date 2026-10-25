# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# The installer's development environment, its one check and its binary. The context is
# the repository, whose declaration and files the checks read:
#   docker build -f installer/Dockerfile --target check .
#   docker build -f installer/Dockerfile --target binary --output type=local,dest=out .

FROM rust:1.99.0-trixie@sha256:6ff07edce8775d0f64be7aba9197229407301bddf2054d62c27b541a6238a181 AS dev
# OpenShell's crates compile their protocol with protoc; AWS-LC builds with cmake and clang.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates clang cmake git protobuf-compiler libprotobuf-dev pkg-config \
    && rm -rf /var/lib/apt/lists/* /var/log/apt /var/log/dpkg.log \
    && rustup component add clippy rustfmt
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo install --locked cargo-deny@0.20.2
RUN useradd --create-home --uid 1000 --shell /bin/bash sandbox \
    && chown -R sandbox:sandbox /usr/local/rustup /usr/local/cargo
USER sandbox

FROM dev AS source
WORKDIR /home/sandbox/src
COPY --chown=sandbox:sandbox . .
WORKDIR /home/sandbox/src/installer

FROM source AS check
# Cargo's registry, its Git checkouts and the target stay in BuildKit cache mounts of the
# build host, so consecutive checks reuse compiled crates.
RUN --mount=type=cache,id=veoveo-installer-cargo-registry,target=/usr/local/cargo/registry,uid=1000,sharing=locked \
    --mount=type=cache,id=veoveo-installer-cargo-git,target=/usr/local/cargo/git,uid=1000,sharing=locked \
    --mount=type=cache,id=veoveo-installer-target,target=/home/sandbox/src/installer/target,uid=1000,sharing=locked \
    cargo fmt --check \
    && cargo clippy --locked --all-targets -- -D warnings \
    && cargo deny --locked check \
    && cargo test --locked

FROM source AS build
# target/ is a cache mount, so the binary leaves it in the same step.
RUN --mount=type=cache,id=veoveo-installer-cargo-registry,target=/usr/local/cargo/registry,uid=1000,sharing=locked \
    --mount=type=cache,id=veoveo-installer-cargo-git,target=/usr/local/cargo/git,uid=1000,sharing=locked \
    --mount=type=cache,id=veoveo-installer-target,target=/home/sandbox/src/installer/target,uid=1000,sharing=locked \
    cargo build --locked --release \
    && cp target/release/veoveo-installer /home/sandbox/veoveo-installer

FROM scratch AS binary
COPY --from=build /home/sandbox/veoveo-installer /veoveo-installer
