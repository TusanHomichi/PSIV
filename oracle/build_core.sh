#!/usr/bin/env bash
# Fetch and build the Genesis Plus GX libretro core the oracle runs on.
#
# The core is third-party source and a 12MB binary, so neither is committed;
# this script reproduces oracle/core/genesis_plus_gx_libretro.so from scratch.
# The commit is pinned because the oracle's numbers are only reproducible
# against a fixed emulation core.
set -euo pipefail
cd "$(dirname "$(realpath "$0")")"

COMMIT=2d7131c5efa606f649d36e1685a8ca47c24f31b3

# A plain source tree with no .git (how the checkout is shipped after a build)
# cannot be re-pinned, so replace it rather than trying to fetch into it.
if [ -d gpgx-src ] && [ ! -d gpgx-src/.git ]; then
	rm -rf gpgx-src
fi
if [ ! -d gpgx-src ]; then
	git init -q gpgx-src
	git -C gpgx-src remote add origin https://github.com/libretro/Genesis-Plus-GX.git
fi
git -C gpgx-src fetch --depth 1 origin "$COMMIT"
git -C gpgx-src checkout --detach FETCH_HEAD
make -C gpgx-src -f Makefile.libretro platform=unix -j"$(nproc)"

mkdir -p core
cp gpgx-src/genesis_plus_gx_libretro.so core/
echo "built core/genesis_plus_gx_libretro.so from Genesis Plus GX $COMMIT"
