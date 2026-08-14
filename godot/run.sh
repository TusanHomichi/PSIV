#!/bin/bash
# Launch PSIV, tracking the pid so tooling never greps command lines.
cd "$(dirname "$0")/.."
if [ -f /tmp/psiv.pid ] && kill -0 "$(cat /tmp/psiv.pid)" 2>/dev/null; then
    echo "already running (pid $(cat /tmp/psiv.pid))"
    exit 0
fi
nohup "$HOME/.local/bin/psiv-godot-4.7.1" --path godot > /tmp/psiv-run.log 2>&1 &
echo $! > /tmp/psiv.pid
echo "launched (pid $!)"
