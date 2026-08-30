# RQ-4 DRAGONFLY task runner.
# Two commands close a coding session: `just check` then `just types`.

default:
    @just --list

# check: format and lint, both languages

fmt-rust:
    cargo fmt --all

lint-rust:
    cargo clippy --workspace --all-targets -- -D warnings

fmt-ui:
    pnpm exec oxfmt .

lint-ui:
    pnpm exec oxlint

# formats in place, then lints. run before calling a coding session done.
check: fmt-rust fmt-ui lint-rust lint-ui

# types: type check, both languages

types-rust:
    cargo check --workspace --all-targets

types-ui:
    pnpm -F ui exec tsc --noEmit

types: types-rust types-ui

# test

# `just test` for everything, `just test -p engine-model throttle` for one.
test *args:
    cargo test --workspace {{args}}

# run

# vcan0 is the seam. swap to can0 and this talks to real hardware.
can:
    sudo modprobe vcan
    sudo ip link add dev vcan0 type vcan || true
    sudo ip link set up vcan0

sim:
    cargo run -p dragonfly-sim

core:
    cargo run -p dragonfly-core

ui:
    pnpm -F ui dev

build-ui:
    pnpm -F ui build

kiosk:
    chromium --app=http://127.0.0.1:8787 --start-fullscreen

# prove frames are actually on the wire. the D5 acceptance test.
candump:
    candump vcan0

# prove decoded telemetry reaches a websocket client. the D6 acceptance test.
# uses uv rather than websocat, which is not installed and would be one more
# thing to get onto the demo machine.
probe frames="20":
    uv run --quiet --with websockets --with msgpack python scripts/ws_probe.py {{frames}}

# regenerate the dronecan-ice golden vectors from the reference implementation.
# run this when the message set changes, never in CI. python stays offline.
golden:
    uv run --quiet --with dronecan python scripts/golden_vectors.py \
        > crates/dronecan-ice/tests/vectors/golden.rs
    cargo test -p dronecan-ice --test vectors

# the D4 physics gate. regenerates docs/model_validation.md.
validate:
    cargo run -p engine-model --example validation_sweep

clean:
    cargo clean
    rm -rf ui/dist node_modules ui/node_modules
