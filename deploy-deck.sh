#!/bin/bash
# Package and push PSIV to a Steam Deck. Usage: ./deploy-deck.sh <deck-ip> [user]
# The Deck user is almost always `deck`. Requires key auth (same setup as
# MiraCraft's account). After the push: on the Deck, add
# ~/psiv/psiv.x86_64 as a non-Steam game (Desktop Mode > Steam > Add a Game),
# or run it straight from Desktop Mode.
set -e
DECK_IP="${1:?usage: ./deploy-deck.sh <deck-ip> [user]}"
DECK_USER="${2:-deck}"
HERE="$(cd "$(dirname "$0")" && pwd)"

# Always ship a current build: release lib, then export.
cargo build --release -p psiv-godot --manifest-path "$HERE/rust/Cargo.toml"
"$HOME/.local/bin/psiv-godot-4.7.1" --headless --path "$HERE/godot" \
    --export-release "Linux-SteamDeck" 2>&1 | tail -3

STAGE="$HERE/build/deck-stage/psiv"
rm -rf "$STAGE" && mkdir -p "$STAGE"
cp "$HERE/build/deck/psiv.x86_64" "$STAGE/"
cp "$HERE/build/deck/"*.so "$STAGE/" 2>/dev/null || true
cp -r "$HERE/runtime-pack" "$STAGE/runtime-pack"

echo "staged $(du -sh "$STAGE" | cut -f1); pushing to $DECK_USER@$DECK_IP:~/psiv"
rsync -az --delete --info=progress2 "$STAGE/" "$DECK_USER@$DECK_IP:psiv/"
ssh "$DECK_USER@$DECK_IP" 'chmod +x ~/psiv/psiv.x86_64 && echo "deployed: ~/psiv/psiv.x86_64"'
