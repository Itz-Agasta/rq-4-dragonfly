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
    pnpm -F ui exec tsgo -b --noEmit

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

# release, not debug. in debug the UKF step takes most of the 50 ms frame budget,
# blocks the CAN read loop, and the socket buffer overflows: measured 105 of 400
# frames carrying no twin, against 0 of 400 on release. it looks like a stale-gate
# problem and is not one.
sim *args:
    cargo run --release -p dragonfly-sim -- {{args}}

core *args:
    cargo run --release -p dragonfly-core -- {{args}}

ui:
    pnpm -F ui dev

build-ui:
    pnpm -F ui build

# `just ui` is the dev server on :5173; this is the bundle a judge sees, and the
# two are not the same artefact. `ui/dist` is gitignored, so on a fresh clone the
# core serves nothing until build-ui has run, and the failure mode is a blank
# screen rather than an error: hence the dependency, not an ordering note in a
# document nobody opens at demo time.

# the demo path: build the frontend, then full-screen it against the core
kiosk: build-ui
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

# Regenerate docs/fault_signatures.md, the diagnosis layer's evidence.
signatures:
    cargo run --release -p twin-core --example signature_matrix

# generate a recorded mission offline, far faster than real time.
#
# the whole pipeline runs on mission time rather than wall time, so a slow
# degradation the twin can actually extrapolate fits inside a coffee break.
# `--speed` on the simulator does NOT do this: the daemon stamps frames from the
# wall clock, so a compressed run hands the filter a violent transient.
#
#   just mission --hours 2 --fault-cylinder 3 --fault-onset 300 \
#       --fault-ramp 60000 --fault-scale 0.55 --out data/missions/coking.parquet
mission *args:
    cargo run --release -p mission-gen -- {{args}}

clean:
    cargo clean
    rm -rf ui/dist node_modules ui/node_modules
