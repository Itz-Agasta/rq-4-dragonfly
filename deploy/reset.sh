#!/bin/bash
# Give every visitor a virgin engine.
#
# Clearing the fault over the bus is not enough. `FaultKind::Clear` restores the
# simulator, but `prognostics::Trends` accumulates inside the core's pump loop
# for the life of the process, so a fault someone injected and cleared leaves a
# fitted decline behind it and the next visitor is shown a remaining life for a
# fault that no longer exists. Only a restart rebuilds `Twin` and `Trends`,
# because the pump builds them per CAN connection.
#
# Restarting is therefore gated on the stack being dirty, not merely idle. An
# untouched core is left flying: it stays locked and warm, and a judge who opens
# the link gets an estimate immediately rather than waiting through a lock.

set -euo pipefail

COMPOSE_DIR="${COMPOSE_DIR:-/opt/dragonfly/deploy}"
HEALTH_URL="${HEALTH_URL:-http://127.0.0.1:8787/api/health}"
STATE="${STATE:-/run/dragonfly-empty-ticks}"

# Consecutive empty polls before a restart is allowed. At the timer's 30 s
# cadence this is 90 s, which is long enough that a judge reloading the page
# does not read as a departure.
EMPTY_TICKS=3

# Restart an idle stack this old even when nothing was injected. The mission
# clock is wall time from process start, and a demonstration link reading
# T+312:47:19 invites a question about the engine that has nothing to do with
# the engine.
MAX_UPTIME_S=$((12 * 3600))

compose() { docker compose -f "$COMPOSE_DIR/docker-compose.yml" "$@"; }

ticks=$(cat "$STATE" 2>/dev/null || echo 0)

health=$(curl -fsS --max-time 3 "$HEALTH_URL" 2>/dev/null || true)
if [[ -z $health ]]; then
    # The core is down or still starting. Compose's restart policy owns that
    # case; piling a restart on top of one in progress only lengthens it.
    echo 0 >"$STATE"
    exit 0
fi

clients=$(sed -n 's/.*"clients":[[:space:]]*\([0-9]*\).*/\1/p' <<<"$health")
if [[ -z $clients ]]; then
    echo "reset: no clients field in $health" >&2
    exit 1
fi

if ((clients > 0)); then
    echo 0 >"$STATE"
    exit 0
fi

ticks=$((ticks + 1))
if ((ticks < EMPTY_TICKS)); then
    echo "$ticks" >"$STATE"
    exit 0
fi

id=$(compose ps -q core)
if [[ -z $id ]]; then
    echo 0 >"$STATE"
    exit 0
fi
started=$(docker inspect -f '{{.State.StartedAt}}' "$id")
started_epoch=$(date -d "$started" +%s)
uptime_s=$(($(date +%s) - started_epoch))

# The log line `send_command` emits after a successful write to the bus. Reading
# the log rather than tracking state in a file means the marker and the restart
# cannot disagree: both are anchored to the same container start.
dirty=0
if docker logs --since "$started_epoch" "$id" 2>&1 | grep -q "fault command published"; then
    dirty=1
fi

if ((dirty == 0 && uptime_s < MAX_UPTIME_S)); then
    # Idle and clean. Leave it flying and keep the counter latched, so the next
    # tick re-checks without waiting out the grace period again.
    echo "$EMPTY_TICKS" >"$STATE"
    exit 0
fi

reason=$( ((dirty == 1)) && echo "a fault was injected" || echo "uptime ${uptime_s}s")
echo "reset: restarting core and sim, no clients and $reason"

# Cleared before the restart, not after: a restart that fails part way leaves the
# counter latched at the threshold otherwise, and every subsequent tick retries
# immediately instead of waiting out the grace period.
echo 0 >"$STATE"

# **Order matters and `compose restart core sim` gets it wrong.** The simulator
# joins the core's network namespace, so restarting them together races: the
# daemon refuses with `cannot join network namespace of a non running container`
# and the simulator stays down. The core then serves a healthy looking API with
# `link_ok: false` forever, which is the one failure mode this whole file exists
# to avoid. Stop the simulator first, bring the core back, then start it again.
compose stop sim
compose restart core

# `compose restart` returns once the container is started, but "started" is the
# daemon's word and the namespace has to be joinable. Cheap to confirm.
for _ in {1..30}; do
    [[ $(docker inspect -f '{{.State.Running}}' "$(compose ps -q core)" 2>/dev/null) == true ]] && break
    sleep 1
done

compose start sim
