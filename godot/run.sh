#!/bin/bash
# Launch PSIV. Builds first — this launcher cannot start a stale binary.
set -e
cd "$(dirname "$0")/.."
cargo build -p psiv-godot --manifest-path rust/Cargo.toml
if [ -f /tmp/psiv.pid ] && kill -0 "$(cat /tmp/psiv.pid)" 2>/dev/null; then
    echo "already running (pid $(cat /tmp/psiv.pid)); kill it first to pick up a new build"
    exit 0
fi
# The managed checkout is read-only outside the workspace, so Godot's default
# user:// timestamped log can abort a detached launch. Keep launcher logs in
# /tmp where the comparison loop can write them reliably.
nohup "$HOME/.local/bin/psiv-godot-4.7.1" --path godot \
    --log-file /tmp/psiv-godot.log > /tmp/psiv-run.log 2>&1 &
echo $! > /tmp/psiv.pid
echo "launched (pid $!)"
