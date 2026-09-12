#!/bin/sh
# Start one of the two binaries. `core` owns the network namespace and creates
# the bus; `sim` joins that namespace and has to wait for it.
set -eu

IFACE="${DRAGONFLY_IFACE:-vcan0}"

wait_for_bus() {
    # The compose `service:` network mode gives the simulator the core's
    # namespace as soon as the namespace exists, which is before the core's
    # entrypoint has created the interface in it. Without this wait the
    # simulator exits on "opening CAN interface vcan0" and only recovers on the
    # restart policy, which fills the log with a failure that is not one.
    i=0
    while [ "$i" -lt 60 ]; do
        if ip link show "$IFACE" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.5
        i=$((i + 1))
    done
    echo "entrypoint: $IFACE never appeared, is NET_ADMIN granted to the core?" >&2
    exit 1
}

case "${1:-core}" in
core)
    shift || true
    # Created here rather than on the host: the container has its own network
    # namespace, so the host only has to have the vcan module loadable. The
    # module itself cannot be loaded from in here.
    if ! ip link show "$IFACE" >/dev/null 2>&1; then
        ip link add dev "$IFACE" type vcan
    fi
    ip link set up "$IFACE"
    exec dragonfly-core \
        --iface "$IFACE" \
        --bind 0.0.0.0:8787 \
        --ui-dir /app/ui \
        --record-dir /data/missions \
        --no-record \
        "$@"
    ;;
sim)
    shift || true
    wait_for_bus
    exec dragonfly-sim --iface "$IFACE" "$@"
    ;;
*)
    exec "$@"
    ;;
esac
