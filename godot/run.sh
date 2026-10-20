#!/bin/bash
# Build and launch PSIV.
set -e
cd "$(dirname "$0")/.."
if ! command -v flock >/dev/null 2>&1; then
    echo "Cannot launch PSIV: required tool 'flock' is missing." >&2
    exit 1
fi
# Serialize launch/build attempts. Godot inherits the lock for its lifetime.
exec 9> "/tmp/psiv-launch-${UID}.lock"
if ! flock -n 9; then
    echo "PSIV is running or starting. Close it before launching again."
    exit 0
fi
if [ -f /tmp/psiv.pid ] && kill -0 "$(cat /tmp/psiv.pid)" 2>/dev/null; then
    echo "already running (pid $(cat /tmp/psiv.pid)); kill it first to pick up a new build"
    exit 0
fi
# The Godot to run: PSIV_GODOT, else `godot` on PATH, at the pinned version.
python3 tools/godot.py check || exit 1
cargo build -p psiv-godot --manifest-path rust/Cargo.toml
# The game runs from its own run root (tools/godot.py run): a prepared project
# copy beside a snapshot of the library just built, so a later build never
# touches the library the game has loaded; it is removed when the game exits.
# The managed checkout is read-only outside the workspace, so Godot's default
# user:// timestamped log can abort a detached launch. Keep launcher logs in
# /tmp where the comparison loop can write them reliably.
nohup python3 tools/godot.py run -- \
    --log-file /tmp/psiv-godot.log > /tmp/psiv-run.log 2>&1 &
pid=$!
echo "$pid" > /tmp/psiv.pid
# The run root is made before Godot starts and its path is the first line of
# the log: report success only once it exists, as the old foreground launch did.
launched=
for _ in $(seq 50); do
    if grep -q ': run root ' /tmp/psiv-run.log 2>/dev/null; then
        launched=1
        break
    fi
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.2
done
if [ -z "$launched" ]; then
    echo "PSIV failed to start:" >&2
    cat /tmp/psiv-run.log >&2
    exit 1
fi
echo "launched (pid $pid)"
