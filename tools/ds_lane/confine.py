"""The worker's read boundary: bubblewrap, a masked home, an explicit allowlist.

Reasonix's own sandbox confines a worker's *writes* to its worktree, but nothing
confined its *reads*: every file under the owner's home - `~/.ssh/id_*`,
`~/.config/gh/hosts.yml`, another project's session - was readable by the
worker's bash tool, and what a worker reads reaches the model provider through
its trajectory. The environment is an allowlist since 2026-09-25
(`config.worker_env`); files were the remaining gap, and this module closes it.

The supervisor starts the worker through `bwrap` (`supervisor.run_worker` calls
`wrap`, the one place a worker command becomes an argv):

* the root filesystem is bound read-only, the worker's home is replaced by a
  tmpfs, and the private `/tmp` too, so nothing under them is in view;
* the allowlist is bound back in - read-only for the toolchain, the worker
  binary and the lane's inputs, writable for the three things a run owns: the
  lane worktree, the run directory and the lane's own Reasonix home;
* the network stays shared: the worker needs its provider API.

There is no way to turn confinement off, and `require_bwrap` refuses to launch
without it. What the boundary does and does not cover is written down in
`README.md`; the interesting, non-obvious parts are documented at each helper
below, with the observation they come from.
"""
import os
import shutil
import sys
from pathlib import Path

from .config import reasonix_bin

BWRAP = "bwrap"                  # on PATH; bubblewrap 0.12 also ships at /usr/bin/bwrap
REASONIX_DIR = ".reasonix"       # the worker binary's home directory name
LANE_HOME = "reasonix-home"      # per-lane Reasonix home, inside the lane's state dir
# What Reasonix needs from its home, and nothing else (2026-09-25, reasonix
# v1.38.12, measured with the real CLI in build/lane-evidence of lane
# h1-confine): `.env` is the credential store - a `config.toml` provider table
# names each key with `api_key_env` and does *not* fall back to the legacy
# store, so a lane home without `.env` fails at once with `missing_credential`
# - and it must exist even when there is nothing to copy, because Reasonix's
# own bash-tool sandbox binds `/dev/null` over it and bubblewrap cannot create
# that destination under a read-only tree. `config.toml` carries the settings
# the lanes depend on (`sandbox.bash = "enforce"` is what confines the worker's
# own bash tool, `agent.subagent_effort = "max"` is pinned there, and the
# provider table resolves `--model deepseek-flash`). `config.json` is the
# legacy config; where there is no `config.toml`, Reasonix imports its `apiKey`
# into `.env` by itself. Sessions, stats, global skills, MCP state and another
# project's credentials stay out of view.
SEED_FILES = ("config.json", "config.toml", ".env")


# ------------------------------------------------------------ bubblewrap

def bwrap_bin():
    """The bubblewrap to run workers under: `DS_LANE_BWRAP`, else `bwrap`.

    The seam exists to *point* at another bubblewrap (the tests use it to prove
    the refusal below); it can never turn confinement off, because a value that
    does not resolve is a refusal, not a passthrough.
    """
    return os.environ.get("DS_LANE_BWRAP") or BWRAP


def bwrap_path():
    """The resolved bubblewrap path, or None when there is none to run."""
    exe = bwrap_bin()
    if os.sep in exe:
        path = Path(exe)
        return str(path) if path.is_file() and os.access(path, os.X_OK) else None
    return shutil.which(exe)


def require_bwrap():
    """Refuse to launch a worker when bubblewrap is missing.

    Called by `lanes.prepare_run`, so `ds-lane start` and `ds-lane resume` fail
    with a message instead of starting a run that either dies in the detached
    supervisor or - worse - would have to run unconfined.
    """
    found = bwrap_path()
    if not found:
        sys.exit(f"ds-lane: {bwrap_bin()} not found (install bubblewrap, or set "
                 f"DS_LANE_BWRAP to it): a worker runs only under the read boundary, "
                 f"and ds-lane has no way to run it without one")
    return found


# ------------------------------------------------------------------ home

def confine_home(env=None):
    """The home directory the worker's view masks.

    `DS_LANE_CONFINE_HOME` is the test seam: a case points it at a throwaway
    home with its canary in it. Otherwise it is the worker's own `HOME`, so the
    path that is masked and the path `~` expands to are the same one.
    """
    override = os.environ.get("DS_LANE_CONFINE_HOME")
    if override:
        return Path(override).expanduser()
    env = os.environ if env is None else env
    return Path(env.get("HOME") or Path.home())


def lane_reasonix_home(lane):
    """The lane's own Reasonix home: `<state>/reasonix-home`, bound at `~/.reasonix`.

    One directory per lane, not per run: `resume` must find the session the
    previous run left, while another project's sessions and secrets stay out of
    view.
    """
    return Path(lane["state_dir"]) / LANE_HOME


def seed_reasonix_home(source, dest):
    """Copy the config Reasonix needs into the lane's home; returns what it wrote.

    `source` is the real `<home>/.reasonix`, read by the supervisor (outside the
    sandbox) and not visible to the worker. A file is (re)copied when it exists
    as a regular file, so a rotated key reaches the next run; anything else the
    lane's home already holds - its sessions, its stats - is left alone. The
    copies are mode 0600: `.env` and `config.json` carry the provider
    credential, and the real home has both private already. An empty `.env` is
    created when there is nothing to copy (see SEED_FILES).
    """
    dest.mkdir(parents=True, exist_ok=True)
    seeded = []
    for name in SEED_FILES:
        src = Path(source) / name
        if src.is_file():  # not a directory, and not the /dev/null mask a bash tool sees
            shutil.copyfile(src, dest / name)
            (dest / name).chmod(0o600)
            seeded.append(name)
    (dest / ".env").touch(exist_ok=True)
    return seeded


def seed_names(source):
    """The files a seed would copy from `source`, for the run's log.

    Worth recording per run, because it says whether the worker got a
    credential at all: with a `config.toml` provider table in place, Reasonix
    authenticates from `<Reasonix home>/.env` and does *not* fall back to the
    legacy `config.json` key (observed with the real CLI, 2026-09-25: a lane
    home without `.env` fails at once with `missing_credential`).
    """
    return [name for name in SEED_FILES if (Path(source) / name).is_file()]


# -------------------------------------------------------------- allowlist

def node_prefix(path):
    """The Node install root containing `path`, if any.

    A Node prefix is a directory holding both `bin/node` and `lib/node_modules`;
    `nvm` installs one per version. Reasonix's own entry point resolves into
    one of them (`<prefix>/bin/reasonix` -> `../lib/node_modules/reasonix/...`),
    and that whole prefix must be readable for it to run. The filesystem root
    is never one: `/bin/node` and `/lib/node_modules` exist on a system that
    packages Node, and treating `/` as an install would hide the real one.
    """
    for parent in [Path(path), *Path(path).resolve().parents]:
        if parent == Path(parent.anchor):
            continue
        if (parent / "lib/node_modules").is_dir() and (parent / "bin/node").exists():
            return parent
    return None


def nvm_paths(env=None, home=None):
    """What the nvm shim needs from `$NVM_DIR`: `nvm.sh`, the aliases, the versions.

    Deliberately not the whole `$NVM_DIR`: it also holds nvm's own git checkout
    and a 55 MB tarball cache - 2.8 GB here - and `reasonix` reads none of it.
    The shim sources `$NVM_DIR/nvm.sh`, `nvm use default` resolves
    `$NVM_DIR/alias/default`, and Node runs out of `$NVM_DIR/versions/`.
    """
    env = os.environ if env is None else env
    home = Path(home) if home else confine_home(env)
    nvm = Path(env.get("NVM_DIR") or home / ".nvm")
    return [p for p in (nvm / "nvm.sh", nvm / "alias", nvm / "versions") if p.exists()]


def node_on_path():
    """The `node` that `reasonix` would find, if any."""
    node = shutil.which("node")
    return Path(node).resolve() if node else None


def worker_roots(binary=None, env=None, home=None):
    """Directories that must be readable for the worker binary to start at all.

    `reasonix` on PATH is normally an nvm shim (`~/.local/bin/reasonix`) that
    sources `$NVM_DIR/nvm.sh`, runs `nvm use default` and execs
    `$NVM_DIR/versions/node/<v>/bin/reasonix`; the shim's own path says nothing
    about the Node install, so nvm's pieces and the prefix of `node` on PATH are
    allowlisted too. A binary that is already a Node install's own entry point
    resolves to its prefix directly. The directory holding the binary is always
    readable - in the tests the worker is a script next to the throwaway home -
    and a script that is not a Node entry point may dispatch to one, which is
    the case the nvm and `node` entries cover.
    """
    env = os.environ if env is None else env
    home = Path(home) if home else confine_home(env)
    exe = shutil.which(binary or reasonix_bin()) or (binary or reasonix_bin())
    path = Path(exe)
    roots = [path.resolve().parent]
    prefix = node_prefix(path)
    if prefix:
        roots.append(prefix)
    else:
        roots += nvm_paths(env, home)
        node = node_on_path()
        if node:
            roots += [node.parent, node_prefix(node)]
    return [p for p in roots if p is not None]


def local_bin_targets(bindir):
    """The resolved targets of the symlinks in `~/.local/bin`.

    `~/.local/bin` is a PATH entry full of symlinks (`ds-lane`, `claude`, a
    Godot build); the directory is allowlisted, and so is what each link points
    at, or the link would dangle inside the sandbox.
    """
    if not Path(bindir).is_dir():
        return []
    targets = []
    for entry in Path(bindir).iterdir():
        if entry.is_symlink():
            resolved = entry.resolve()
            if resolved.exists():
                targets.append(resolved)
    return targets


def _dedupe(paths, masked):
    """The existing paths that are not the masked home or something under a kept one.

    Deduplication runs per side of the mask (read_paths): a bind that contains
    the masked home - in the tests the worker's own directory sits next to the
    throwaway home - is applied *before* the mask, so it does not cover what is
    under it; dropping `~/.cargo` as "covered" by it would leave the toolchain
    invisible the moment the mask went on.
    """
    keep = []
    for path in sorted(set(paths), key=lambda p: (len(p.parts), str(p))):
        if not path.exists() or path == masked or path == Path(path.anchor) or path in keep:
            continue  # the masked home itself is never bound back, nor is `/`
        if any(parent in keep for parent in path.parents):
            continue  # already covered by a shallower bind
        keep.append(path)
    return keep


def read_paths(lane, home=None, env=None):
    """Everything the worker may read, deduplicated, mask-side first.

    The allowlist is the toolchain (`~/.cargo`, `~/.rustup`, `~/.local/bin` and
    its link targets), the Node install and the worker binary (worker_roots),
    the main repository's `.git` (a linked worktree keeps its objects there),
    and the resolved sources of every `--link` and `--add-dir`. Nothing else
    under the home is readable, and the whole tree is read-only: a worker cannot
    write its toolchain, and no path outside `wrap`'s three writable ones can be
    written at all.
    """
    env = os.environ if env is None else env
    home = Path(home) if home else confine_home(env)
    candidates = worker_roots(env=env, home=home)
    candidates += [home / ".cargo", home / ".rustup", home / ".local/bin"]
    candidates += local_bin_targets(home / ".local/bin")
    candidates.append(Path(lane["repo"]) / ".git")
    candidates += [Path(p) for p in (lane.get("link_sources") or {}).values()]
    candidates += [Path(p) for p in lane.get("add_dirs") or []]
    outside = [p for p in candidates if home not in p.parents]
    under = [p for p in candidates if home in p.parents]
    return _dedupe(outside, home) + _dedupe(under, home)


# ------------------------------------------------------------- the argv

def bwrap_argv(cmd, *, home, worktree, run_dir, reasonix_home, reads, bwrap=None):
    """The worker's command line: masks first, then the allowlist, then the writes.

    Order is the whole trick. `--ro-bind / /` makes the tree read-only, the two
    tmpfs masks hide `/tmp` and the home, and only then is the allowlist bound
    back: paths under the masked home *after* the mask, or the mask would cover
    them, and a path that contains the home (a worker binary next to it, in the
    tests) *before* it, because a bind of an ancestor hides everything below it.
    The three writable binds come last, so nothing masks them. Then the worker.
    """
    bwrap = bwrap or require_bwrap()
    home, reads = Path(home), list(reads)
    under_home = [p for p in reads if home in p.parents]
    outside = [p for p in reads if home not in p.parents]
    argv = [bwrap, "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp"]
    for path in outside:
        argv += ["--ro-bind", str(path), str(path)]
    argv += ["--tmpfs", str(home)]
    for path in under_home:
        argv += ["--ro-bind", str(path), str(path)]
    for src, dst in ((worktree, worktree), (run_dir, run_dir),
                     (reasonix_home, home / REASONIX_DIR)):
        argv += ["--bind", str(src), str(dst)]
    argv += ["--die-with-parent", "--"]
    return argv + [str(a) for a in cmd]


def wrap(cmd, lane, run_dir, env=None):
    """`cmd` as the supervisor runs it: under `lane`'s read boundary.

    Seeds the lane's Reasonix home and computes the allowlist, so every caller
    gets the same boundary and none of them has to know it.
    """
    env = os.environ if env is None else env
    home = confine_home(env)
    reasonix_home = lane_reasonix_home(lane)
    seed_reasonix_home(home / REASONIX_DIR, reasonix_home)
    return bwrap_argv(cmd, home=home, worktree=Path(lane["worktree"]), run_dir=Path(run_dir),
                      reasonix_home=reasonix_home, reads=read_paths(lane, home, env))


def summary(argv, seeded=None):
    """One line describing the boundary, for the run's `supervisor.log`.

    The receipts say what a run could read without anyone having to reconstruct
    the argv: the masked home, the allowlisted paths, the writable ones, and
    which config files the lane's Reasonix home was seeded with.
    """
    def paths(flag):
        return [a for i, a in enumerate(argv) if i and argv[i - 1] == flag]
    masked = paths("--tmpfs")
    reads = paths("--ro-bind")[1:]  # the first one is the root bind
    writes = paths("--bind")
    line = ("confined: masked " + ", ".join(masked)
            + f"; readable {len(reads)}: " + ", ".join(reads)
            + "; writable: " + ", ".join(writes))
    if seeded is not None:
        line += "; seeded lane home: " + (", ".join(seeded) or "(nothing)")
    return line
