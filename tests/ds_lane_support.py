"""Shared scaffolding for the ds-lane test modules.

Hermetic: every end-to-end case builds a throwaway git repo and a throwaway
DS_LANE_HOME in temp directories and points DS_LANE_REASONIX at a fake worker
script, so no test calls the real Reasonix, a model or the network.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

This module holds what the cases share: the fake worker, the throwaway repo and
home fixtures, and the helpers that drive `tools/ds-lane`, the entry-point shim,
as a subprocess. The cases live in test_ds_lane_unit.py (the package's own
units), test_ds_lane_lanes.py (the lane commands end to end),
test_ds_lane_size.py (the 1,000-line rule and the data-file exemption),
test_ds_lane_compaction.py (deduplicating and compressing a run's evidence),
test_ds_lane_finalize.py (the finalize commit: link exclusions and a git
failure), test_ds_lane_crash.py (run numbering and the record a crashed run
leaves), test_ds_lane_supervisor.py (slots, kills, the stall watchdog and the
entry point's re-entry into the detached supervisor) and test_ds_lane_confine.py
(the read boundary: every worker here runs under it, and the fixture's home is
the one it masks).
"""
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DS_LANE = ROOT / "tools" / "ds-lane"

sys.path.insert(0, str(ROOT / "tools"))  # the ds-lane shim does this for the package
import ds_lane as DS  # noqa: E402

# ---------------------------------------------------------------- fake worker

FAKE_REASONIX = '''#!/usr/bin/env python3
"""Fake `reasonix` for the ds-lane test suite: no model, no network, no sandbox.

Reads the task from stdin, the way `reasonix run` does when argv carries none,
and refuses a prompt handed to it as an argument: that refusal is what turns a
regression to the old `cmd + [prompt]` shape into a failed run.

Reads a JSON behaviour spec from FAKE_REASONIX_SPEC (inline JSON or a path):
files to write, trajectory events to append, seconds to sleep, seconds to burn
CPU for, exit code. `resume_spec` overrides any of those keys for a run that
carries `--resume`, so a resumed run can behave differently from its first.
`opts_out` records the flags it was given, its whole argv (program name
included) and the prompt it read, so a case can check what the harness put
where.

`probe` is the confinement cases' eyes inside the worker's own view: it reads
and writes the paths it is given, lists directories, and can re-run the sandbox
Reasonix puts around its own bash tool inside this one; everything it found is
recorded as JSON in `probe["out"]`.

`burn` spins flat out for N seconds; `burn_low` spins in short bursts forever
(a worker that is alive and doing a trickle of work, like a process idling on
a 1 s timer). Both are silent: the trajectory does not grow while they run.
"""
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

VALUE_FLAGS = {"--dir", "--trajectory", "--metrics", "--model", "--effort",
               "--permission-mode", "--output-format", "--max-steps", "--add-dir",
               "--resume"}


def load_spec():
    raw = os.environ.get("FAKE_REASONIX_SPEC", "{}")
    path = Path(raw)
    return json.loads(path.read_text()) if path.exists() else json.loads(raw)


def nested_sandbox():
    """Reasonix's own bash-tool sandbox, run inside ds-lane's boundary.

    The argv is the one observed on this host for a real worker (2026-09-25):
    a read-only root, a private /tmp taken from the session's own tmp
    directory, the worktree bound writable, and /dev/null bound over the
    Reasonix home's .env. The last one can only work when that file exists -
    bubblewrap cannot create it inside a read-only tree - which is why the
    lane's seeded Reasonix home carries an empty .env.
    """
    home = Path(os.environ.get("HOME") or "/")
    tmp = home / ".claude" / "tmp" / "nested-probe"
    cwd = str(Path.cwd())
    argv = [shutil.which("bwrap") or "/usr/bin/bwrap",
            "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc",
            "--bind", str(tmp), "/tmp", "--bind", cwd, cwd,
            "--ro-bind", "/dev/null", str(home / ".reasonix" / ".env"),
            "--", "/bin/bash", "-c", "echo nested-ok; touch /tmp/nested.txt"]
    try:
        tmp.mkdir(parents=True, exist_ok=True)
        done = subprocess.run(argv, text=True, capture_output=True, timeout=60)
        return {"argv": argv, "rc": done.returncode, "stdout": done.stdout.strip(),
                "stderr": done.stderr.strip()}
    except (OSError, subprocess.SubprocessError) as e:  # no bwrap, no nesting
        return {"argv": argv, "error": repr(e)}


def run_probe(spec):
    """Record what this worker's own filesystem view holds, into probe["out"]."""
    probe = spec.get("probe")
    if not probe:
        return
    report = {"cwd": str(Path.cwd()), "home": os.environ.get("HOME"),
              "reads": {}, "writes": {}, "lists": {}}
    for name, path in sorted(probe.get("read", {}).items()):
        entry = {"path": str(path), "exists": Path(path).exists()}
        try:
            entry["content"] = Path(path).read_text()
        except OSError as e:
            entry["error"] = "%s: %s" % (type(e).__name__, e.strerror)
        report["reads"][name] = entry
    for name, path in sorted(probe.get("list", {}).items()):
        try:
            report["lists"][name] = sorted(os.listdir(path))
        except OSError as e:
            report["lists"][name] = "%s: %s" % (type(e).__name__, e.strerror)
    for name, pair in sorted(probe.get("write", {}).items()):
        path, text = pair
        try:
            Path(path).parent.mkdir(parents=True, exist_ok=True)
            Path(path).write_text(text)
            report["writes"][name] = {"ok": True, "path": str(path)}
        except OSError as e:
            report["writes"][name] = {"ok": False, "path": str(path),
                                      "error": "%s: %s" % (type(e).__name__, e.strerror)}
    if probe.get("nested"):
        report["nested"] = nested_sandbox()
    Path(probe["out"]).write_text(json.dumps(report, indent=2))


def main(argv):
    if not argv or argv[0] == "--version":
        print("reasonix 0.0.0-fake")
        return 0
    if argv[0] != "run":
        print("fake-reasonix: unsupported command " + argv[0], file=sys.stderr)
        return 2
    opts, positional, i = {}, [], 1
    while i < len(argv):
        arg = argv[i]
        if arg in VALUE_FLAGS:
            opts[arg] = argv[i + 1]
            i += 2
        elif arg.startswith("--"):
            i += 1
        else:
            positional.append(arg)  # the task belongs on stdin, never in argv
            i += 1
    if positional:
        print("fake-reasonix: unexpected argument(s), the prompt arrives on stdin: "
              + " ".join(positional), file=sys.stderr)
        return 2
    prompt = sys.stdin.read()
    spec = load_spec()
    if opts.get("--resume") and isinstance(spec.get("resume_spec"), dict):
        spec = {**spec, **spec["resume_spec"]}  # a resumed run may behave differently
    cwd = Path.cwd()
    for rel, content in sorted(spec.get("files", {}).items()):
        dest = cwd / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(content)
    run_probe(spec)
    if spec.get("pid_file"):
        Path(spec["pid_file"]).write_text(str(os.getpid()))
    child = spec.get("spawn_sleep")
    if child:  # a stand-in for `cargo test`: a group member SIGTERM alone does not stop
        proc = subprocess.Popen([sys.executable, "-c",
                                 "import signal, time; "
                                 "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                                 "time.sleep(%d)" % int(child.get("seconds", 60))])
        if child.get("pid_file"):
            Path(child["pid_file"]).write_text(str(proc.pid))
    if spec.get("burn"):  # a silent but busy worker: CPU time, no trajectory growth
        deadline, spin = time.monotonic() + float(spec["burn"]), 0
        while time.monotonic() < deadline:
            spin += 1
    if spec.get("opts_out"):
        Path(spec["opts_out"]).write_text(
            json.dumps({"opts": opts, "argv": sys.argv, "prompt": prompt,
                        "env": sorted(os.environ)}, indent=2))
    trajectory = opts.get("--trajectory")
    if trajectory:
        with open(trajectory, "a") as handle:
            for event in spec.get("events", []):
                handle.write(json.dumps({"event": event}) + "\\n")
    if spec.get("sleep"):
        time.sleep(spec["sleep"])
    trickle = spec.get("burn_low")
    if trickle:  # alive and busy at a rate far below one percent of a core
        period = float(trickle.get("period", 1.0))
        busy = float(trickle.get("busy", 0.004))
        while True:
            end = time.monotonic() + busy
            spin = 0
            while time.monotonic() < end:
                spin += 1
            time.sleep(max(0.0, period - busy))
    print(json.dumps({
        "type": "result", "subtype": "success",
        "is_error": bool(spec.get("is_error", False)),
        "num_turns": spec.get("num_turns", 1),
        "result": spec.get("result", "fake worker finished\\n"),
        "total_cost_usd": spec.get("cost_usd", 0.001),
        "usage": {"promptTokens": 10, "completionTokens": 5,
                  "cacheHitTokens": 0, "cacheMissTokens": 10, "totalTokens": 15},
    }))
    return int(spec.get("exit_code", 0))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
'''


RECEIPT_RESULT = ("Wrote the owned file.\n\n"
                  "## Receipt\n- Status: done\n- Changed files: `tools/new.txt`\n"
                  "- Commands run: `true` -> exit 0\n- Acceptance: met\n- Open issues: none\n")

# A stand-in for a credential the worker must not find. The confinement cases
# plant it in the home the lane masks and have the fake worker look for it.
CANARY = "canary-credential\n"


class LaneFixture(unittest.TestCase):
    """A throwaway repo, home and fake worker, plus the helpers that drive the CLI."""

    # -- scaffolding

    def setUp(self):
        self.tmpdir = tempfile.TemporaryDirectory(prefix="ds-lane-test-")
        self.addCleanup(self.tmpdir.cleanup)
        self.root = Path(self.tmpdir.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.work = self.root / "work"  # scratch: briefs, specs, pid files
        self.work.mkdir()
        self.fake = self.root / "fake-reasonix"
        self.fake.write_text(FAKE_REASONIX)
        self.fake.chmod(0o755)
        self.init_repo()
        self.env = {"DS_LANE_HOME": str(self.home), "DS_LANE_REASONIX": str(self.fake),
                    "DS_LANE_WORKER_ENV_PASS": "FAKE_REASONIX_SPEC",
                    # The worker's HOME is the throwaway home, so the lane masks
                    # that one (and only that one): a case never touches the
                    # owner's real home, and the seeded Reasonix home is built
                    # from the fixture's own config. DS_LANE_CONFINE_HOME names
                    # the same path explicitly (confine.py).
                    "HOME": str(self.home), "DS_LANE_CONFINE_HOME": str(self.home)}
        self.base_sha = self.git("rev-parse", "HEAD")
        self.addCleanup(self.reap_workers)

    def init_repo(self):
        subprocess.run(["git", "init", "-q", "-b", "main", str(self.repo)], check=True)
        self.git("config", "user.email", "lane@example.invalid")
        self.git("config", "user.name", "ds-lane test")
        (self.repo / "README.md").write_text("# throwaway repo\n")
        (self.repo / ".gitignore").write_text("generated/\nbuild/\nruntime-pack/\n")
        (self.repo / "generated").mkdir()
        (self.repo / "generated" / "data.txt").write_text("generated input\n")
        pack = self.repo / "build" / "snap" / "runtime-pack"
        pack.mkdir(parents=True)
        (pack / "pack.bin").write_text("pack\n")
        (self.repo / "tools").mkdir()
        (self.repo / "tools" / "keep.txt").write_text("keep\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "init")

    def git(self, *args, cwd=None):
        r = subprocess.run(["git", "-C", str(cwd or self.repo), *args], text=True,
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        return r.stdout.strip()

    def reap_workers(self):
        """Kill any supervisor or fake worker a failed test left behind."""
        for pidfile in self.work.glob("*.pid"):
            self.kill_pid(pidfile)
        for pidfile in self.home.glob("state/*/*/run-*/*.pid"):  # supervisor, worker, child
            self.kill_pid(pidfile)

    def kill_pid(self, pidfile):
        try:
            pid = int(pidfile.read_text().strip())
        except (OSError, ValueError):
            return
        if pid == os.getpid():
            return  # a case that drives exec_run in process writes its own pid there
        try:
            os.kill(pid, 9)
        except OSError:
            pass

    def pid_exists(self, pid):
        try:
            os.kill(pid, 0)
            return True
        except ProcessLookupError:
            return False

    def cli(self, *args, spec=None, env=None, timeout=120, check=None, cwd=None, tool=DS_LANE):
        environ = dict(os.environ)
        environ.update(self.env)
        if spec is not None:
            environ["FAKE_REASONIX_SPEC"] = json.dumps(spec) if isinstance(spec, dict) else str(spec)
        environ.update(env or {})
        r = subprocess.run([sys.executable, str(tool)] + [str(a) for a in args], text=True,
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=environ,
                           cwd=str(cwd or self.repo), timeout=timeout)
        if check is not None:
            self.assertEqual(r.returncode, check, r.stdout)
        return r

    def write(self, name, text):
        path = self.work / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def lane_state(self, lane_id, *more):
        path = self.home / "state" / self.repo.name / lane_id
        return path.joinpath(*more) if more else path

    def lane_wt(self, lane_id, *more):
        path = self.home / "wt" / self.repo.name / lane_id
        return path.joinpath(*more) if more else path

    # -- confinement helpers

    def worker_path(self, lane_id, name, run=1):
        """A file the worker writes, inside the one directory it may write.

        The supervisor binds exactly three writable paths - the worktree, the
        run directory and the lane's own Reasonix home - so a pid file, an
        options dump or a probe report has to live in one of them, or the
        worker cannot leave it behind at all. The run directory is the right
        one: it is the harness's own per-run scratch, and it never reaches the
        lane's commit.
        """
        return self.lane_state(lane_id, f"run-{run}", name)

    def canary(self, rel=".ssh/id_canary", text=CANARY):
        """Plant a credential-shaped file in the home the lane masks.

        `~/.ssh/id_*` is the file the issue names first; the point is that it
        is unreachable from the worker and untouched afterwards.
        """
        path = self.home / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def probe_out(self, lane_id, run=1):
        """Where a case tells the fake worker to record what it could see."""
        return self.worker_path(lane_id, "probe.json", run)

    def probe_report(self, lane_id, run=1, timeout=60):
        return self.read_json(self.probe_out(lane_id, run), timeout=timeout)

    def wait_for(self, predicate, what, timeout=60, interval=0.1):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            value = predicate()
            if value:
                return value
            time.sleep(interval)
        self.fail(f"timed out after {timeout}s waiting for {what}")

    def read_json(self, path, timeout=60):
        def attempt():
            try:
                return json.loads(Path(path).read_text())
            except (OSError, json.JSONDecodeError):
                return None
        return self.wait_for(attempt, str(path), timeout=timeout)

    def run_json(self, lane_id, run=1, timeout=60):
        return self.read_json(self.lane_state(lane_id, f"run-{run}", "run.json"), timeout=timeout)

    def lane_json(self, lane_id):
        return self.read_json(self.lane_state(lane_id, "lane.json"))

    def summary(self, lane_id, run=1):
        path = self.lane_state(lane_id, f"run-{run}", "summary.txt")
        self.wait_for(path.exists, str(path))
        return path.read_text()

    def start(self, brief_text, lane_id="t1", spec=None, extra=(), env=None, wait=False):
        brief = brief_text if isinstance(brief_text, Path) else self.write(f"{lane_id}.md", brief_text)
        r = self.cli("start", brief, "--repo", self.repo, "--id", lane_id,
                     *(["--no-wait"] if not wait else []), *extra, spec=spec, env=env, check=0)
        if wait:
            return r, self.run_json(lane_id)
        return r

    def resume(self, lane_id, followup_text, spec=None, extra=(), check=None):
        followup = followup_text if isinstance(followup_text, Path) else \
            self.write(f"{lane_id}-followup.md", followup_text)
        return self.cli("resume", lane_id, followup, "--repo", self.repo, "--no-wait",
                        *extra, spec=spec, check=check)

