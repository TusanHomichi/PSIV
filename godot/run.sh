#!/bin/bash
# Launch PSIV. Build only while its extension is not in use.
set -e
cd "$(dirname "$0")/.."
for psiv_dependency in rg flock; do
    if ! command -v "$psiv_dependency" >/dev/null 2>&1; then
        echo "Cannot safely rebuild PSIV: required tool '$psiv_dependency' is missing." >&2
        exit 1
    fi
done
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
# Also covers the editor or a direct Godot launch, which has no launcher PID.
# Replacing a mapped GDExtension can crash the game already using it.
# Inspect mappings directly: fuser misses mapped libraries on this host.
psiv_library_users=$(rg -l -F -- "$PWD/rust/target/debug/libpsiv_godot.so" /proc/[0-9]*/maps 2>/dev/null || true)
if [ -n "$psiv_library_users" ]; then
    echo "PSIV's game library is in use. Close that Godot session before rebuilding."
    exit 0
fi
cargo build -p psiv-godot --manifest-path rust/Cargo.toml
# The managed checkout is read-only outside the workspace, so Godot's default
# user:// timestamped log can abort a detached launch. Keep launcher logs in
# /tmp where the comparison loop can write them reliably.
nohup "$HOME/.local/bin/psiv-godot-4.7.1" --path godot \
    --log-file /tmp/psiv-godot.log > /tmp/psiv-run.log 2>&1 &
echo $! > /tmp/psiv.pid
echo "launched (pid $!)"
