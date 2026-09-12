# One image, two binaries, chosen by the entrypoint argument.
#
# `dragonfly-sim` and `dragonfly-core` share a network namespace on the host so
# they share one `vcan0`, and sharing an image as well means one build and one
# push rather than two that can drift a commit apart.

FROM node:24-bookworm-slim AS ui

ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
RUN corepack enable

WORKDIR /src
# Manifests before sources, so a change to a component does not re-resolve the
# dependency tree.
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY ui/package.json ui/
RUN pnpm install --frozen-lockfile

COPY ui/ ui/
# `pnpm -F ui build` is `tsgo -b --noEmit && vite build`, so a type error fails
# the image rather than shipping a bundle that never type checked.
RUN pnpm -F ui build


FROM rust:1-slim-bookworm AS rust

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
# Release, and not negotiable: in debug the UKF step eats most of the 50 ms
# frame budget and blocks the CAN read loop. The justfile comment on `sim` has
# the measurement.
RUN cargo build --release -p dragonfly-core -p dragonfly-sim


FROM debian:bookworm-slim AS runtime

# iproute2 creates vcan0 inside the namespace. curl is the healthcheck and the
# thing the reset watchdog polls.
RUN apt-get update \
    && apt-get install -y --no-install-recommends iproute2 curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=rust /src/target/release/dragonfly-core /usr/local/bin/
COPY --from=rust /src/target/release/dragonfly-sim /usr/local/bin/
COPY --from=ui /src/ui/dist /app/ui
COPY deploy/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["core"]
