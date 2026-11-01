#!/bin/bash
# Package and push PSIV to a Steam Deck. Usage, from the repository root:
# ./tools/package/deploy-deck.sh <deck-ip> [user]
# The Deck user is almost always `deck`. Requires SSH key authentication to
# that user. After the push: on the Deck, add
# ~/psiv/psiv.x86_64 as a non-Steam game (Desktop Mode > Steam > Add a Game),
# or run it straight from Desktop Mode.
# pipefail: a failed export must stop the script, not vanish into `| tail`.
set -euo pipefail
DECK_IP="${1:?usage: ./tools/package/deploy-deck.sh <deck-ip> [user]}"
DECK_USER="${2:-deck}"
# tools/package/deploy-deck.sh: the repository root is two levels up.
HERE="$(cd "$(dirname "$0")/../.." && pwd)"

# The Godot that exports: PSIV_GODOT, else `godot` on PATH, at the pinned version.
python3 "$HERE/tools/godot.py" check

# Always ship a current build: release lib, then export. The Deck runs
# SteamOS glibc 2.41 while this laptop tracks Fedora's newest; a plain
# cargo build stamps the host's glibc symbol versions and the extension
# refuses to load on the Deck (GLIBC_2.43 not found). cargo-zigbuild links
# against the pinned version instead (zig + cargo-zigbuild in ~/.local).
DECK_GLIBC="2.41"
PATH="$HOME/.local/bin:$PATH" cargo zigbuild --release -p psiv-godot \
    --manifest-path "$HERE/rust/Cargo.toml" \
    --target "x86_64-unknown-linux-gnu.$DECK_GLIBC"
# Export from a run root (tools/godot.py run), kept for the staging below: a
# prepared project copy beside a snapshot of the library zigbuild just linked,
# as the release library its psiv.gdextension names. The export ships exactly
# the library run-root.json names by sha256, and the checkout's
# rust/target/release/ is never overwritten. Godot refuses to export into a
# directory that does not exist.
DECK_ROOT="$HERE/build/deck-root"
mkdir -p "$HERE/build/deck"
python3 "$HERE/tools/godot.py" run --dir "$DECK_ROOT" --replace --keep --profile release \
    --library "$HERE/rust/target/x86_64-unknown-linux-gnu/release/libpsiv_godot.so" -- \
    --headless --export-release "Linux" "$HERE/build/deck/psiv.x86_64" \
    2>&1 | tee "$HERE/build/deck-export.log" | tail -3

STAGE="$HERE/build/deck-stage/psiv"
rm -rf "$STAGE" && mkdir -p "$STAGE"
cp "$HERE/build/deck/psiv.x86_64" "$STAGE/"
cp "$HERE/build/deck/"*.so "$STAGE/" 2>/dev/null || true

# An exported build reads its pack and keeps its saves in its user data
# folder (rust/psiv-godot/src/install.rs), ~/.local/share/PSIV on the Deck.
# Saves a build before that kept in ~/psiv/saves move there first, since the
# rsync below deletes what the stage lacks; if both exist, the old ones are
# kept aside, stamped.
ssh "$DECK_USER@$DECK_IP" 'set -e; mkdir -p ~/.local/share/PSIV
    if [ -d ~/psiv/saves ]; then
        if [ -e ~/.local/share/PSIV/saves ]; then
            mv ~/psiv/saves ~/psiv-saves-$(date -u +%Y%m%dT%H%M%SZ)
        else
            mv ~/psiv/saves ~/.local/share/PSIV/saves
        fi
    fi'
echo "staged $(du -sh "$STAGE" | cut -f1); pushing to $DECK_USER@$DECK_IP:~/psiv"
rsync -az --delete --info=progress2 "$STAGE/" "$DECK_USER@$DECK_IP:psiv/"
# The pack the run root resolved once, so the Deck gets one version.
rsync -az --delete --info=progress2 "$DECK_ROOT/runtime-pack/" \
    "$DECK_USER@$DECK_IP:.local/share/PSIV/runtime-pack/"
ssh "$DECK_USER@$DECK_IP" 'chmod +x ~/psiv/psiv.x86_64 && echo "deployed: ~/psiv/psiv.x86_64"'
