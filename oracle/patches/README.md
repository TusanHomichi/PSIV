# Patches to the pinned emulation core

Every `*.patch` here is **ours**: a change to the Genesis Plus GX checkout
`oracle/build_core.sh` fetches at its pinned commit, not part of upstream. The
upstream source and its licence are in `oracle/gpgx-src/` once built.

`build_core.sh` applies the files in file-name order, right after the checkout,
and applying one twice is a no-op, so a tree that already carries them can be
rebuilt. A patch that does not apply fails the build: a checkout that is not
the pinned revision must never quietly produce a core the oracle's numbers were
not measured against. Without `gpgx-src/.git` (how a built checkout is shipped)
the pinned commit cannot be re-verified at all, so the patches are the only
thing holding the tree to a known state - keep them tight.

What a patch here has to obey:

- **Off means identical.** A hook must do nothing until the host turns it on.
  The oracle's checks run against the patched core, so a patch that changes
  emulation when its feature is idle is a bug, not a rounding error.
- **No game-specific addresses.** The core records what the hardware did - an
  access, its value, the PC that made it - and the host decides what that means
  for this cartridge. Addresses like `RNG_Seed` belong in `oracle/ram_map.json`
  and `oracle/host/`, never in `gpgx-src`.
- **Exported means declared.** The core links with
  `libretro/link.T` (`global: retro_*; local: *;`), so a host-visible function
  needs its prefix added there. `psiv_*` is ours; the `retro_*` namespace is
  the libretro ABI and stays untouched.
- **Match the tree's bytes.** The upstream sources this patch touches are
  CRLF, so the patch's context and added lines are too; that is not stray
  whitespace, and rewriting the endings would stop it applying.
- **Measured, not assumed.** Say in the patch comment what the change records
  and why the reader can trust it when it is off, and put the observation that
  justifies it in `oracle/README.md` next to the numbers it explains.

## 0001-rng-hv-trace.patch

Records every value `vdp_hvc_r()` returns, with the 68000 program counter of
the access, into a fixed buffer, and exports
`psiv_hv_trace_enable/reset/count/dropped/get`. It exists because PSIV's battle
rolls mix the VDP HV counter into the RNG seed (`UpdateRNGSeed2`,
`ps4.asm:86097`): that beam timing is not reproducible in a port, so the
cartridge's own rolls have to be captured and replayed. `oracle/host/rng_trace.c`
drains the buffer once per frame and writes them out; `oracle/README.md`
("RNG trace") has the columns and what the check proves.

The buffer holds 65,536 records, far more than one frame of HV reads, and the
host drains it every frame and refuses a capture that dropped any.
