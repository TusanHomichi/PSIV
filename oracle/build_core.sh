#!/usr/bin/env bash
# Fetch and build the Genesis Plus GX libretro core the oracle runs on.
#
# The core is third-party source and a 12MB binary, so neither is committed;
# this script reproduces oracle/core/genesis_plus_gx_libretro.so from scratch.
# The commit is pinned because the oracle's numbers are only reproducible
# against a fixed emulation core.
#
# Our own changes live in oracle/patches and are applied to the checkout after
# the pin is in place. Applying them again is a no-op, so re-running this
# script on a tree that already carries them is safe.
set -euo pipefail
cd "$(dirname "$(realpath "$0")")"

COMMIT=2d7131c5efa606f649d36e1685a8ca47c24f31b3
PATCH_DIR=$PWD/patches

if [ ! -d gpgx-src ]; then
	git init -q gpgx-src
	git -C gpgx-src remote add origin https://github.com/libretro/Genesis-Plus-GX.git
fi
if [ -d gpgx-src/.git ]; then
	git -C gpgx-src fetch --depth 1 origin "$COMMIT"
	git -C gpgx-src checkout --detach FETCH_HEAD
else
	# A plain source tree with no .git is how the checkout is shipped once it
	# has been built, and a tree with no history cannot be re-pinned. It is
	# used as it stands so the build works offline; the patches below are what
	# proves the two files this project depends on are the pinned ones (they
	# fail loudly on any other revision).
	echo "note: gpgx-src has no .git; the pinned commit $COMMIT cannot be" >&2
	echo "note: verified for this tree, only that oracle/patches applies to it" >&2
fi

command -v patch >/dev/null || {
	echo "oracle/build_core.sh: GNU patch is required to apply oracle/patches" >&2
	exit 1
}
shopt -s nullglob
for p in "$PATCH_DIR"/*.patch; do
	if patch -p1 -d gpgx-src --forward --dry-run --silent <"$p" >/dev/null 2>&1; then
		patch -p1 -d gpgx-src --forward <"$p"
	elif patch -p1 -d gpgx-src --reverse --dry-run --silent <"$p" >/dev/null 2>&1; then
		echo "already applied: $(basename "$p")"
	else
		echo "oracle/build_core.sh: $(basename "$p") does not apply to gpgx-src" >&2
		echo "oracle/build_core.sh: the tree is not the pinned revision $COMMIT" >&2
		exit 1
	fi
done

make -C gpgx-src -f Makefile.libretro platform=unix -j"$(nproc)"

mkdir -p core
cp gpgx-src/genesis_plus_gx_libretro.so core/
if command -v nm >/dev/null; then
	nm -D --defined-only core/genesis_plus_gx_libretro.so |
		grep -q psiv_hv_trace_enable ||
		{
			echo "oracle/build_core.sh: the built core exports no HV trace hooks," >&2
			echo "oracle/build_core.sh: so oracle/patches did not reach the build" >&2
			exit 1
		}
fi
echo "built core/genesis_plus_gx_libretro.so from Genesis Plus GX $COMMIT"
