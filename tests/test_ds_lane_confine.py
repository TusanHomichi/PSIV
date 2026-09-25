"""ds-lane's read boundary: the allowlist, the argv, and the worker's own view.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

Two halves. The unit cases build the bubblewrap argument list and the allowlist
from synthetic paths, so they need no namespace. The end-to-end cases run the
real `ds-lane start`/`resume` with the fake worker *inside the same
confinement*, plant a canary credential in the home the lane masks, and have
the fake worker report what its own view held (`ds_lane_support.FAKE_REASONIX`'s
`probe` spec). They need bubblewrap to be able to create a user namespace
inside whatever sandbox runs this suite.
"""
import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import CANARY, DS, LaneFixture
except ImportError:  # `python -m unittest tests.test_ds_lane_confine`
    from ds_lane_support import CANARY, DS, LaneFixture


def plant(root, rel=None, text="x\n"):
    """Write a file (at `root/rel`, or at `root` when no relative part is given)."""
    path = Path(root) / rel if rel else Path(root)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return path


class AllowlistUnitCase(unittest.TestCase):
    """The generated argv and the paths it names: no worker, no namespace."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="ds-lane-confine-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.home = self.root / "home"
        plant(self.home, ".cargo/registry/marker.txt")
        plant(self.home, ".rustup/toolchains/marker.txt")
        self.bin_target = plant(self.root / "opt/thing/bin/thing")
        bindir = self.home / ".local/bin"
        bindir.mkdir(parents=True)
        (bindir / "thing").symlink_to(self.bin_target)
        (bindir / "ds-lane").symlink_to(plant(self.root / "repo/tools/ds-lane", "#!/bin/sh\n"))
        plant(self.root / "repo/.git/HEAD", "ref: refs/heads/main\n")
        self.link = plant(self.root / "repo/generated/data.txt", "linked\n")
        self.add_dir = self.root / "elsewhere"
        self.add_dir.mkdir()
        self.wt = self.root / "wt/t1"
        self.wt.mkdir(parents=True)
        self.state = self.root / "state/t1"
        self.run_dir = self.state / "run-1"
        self.run_dir.mkdir(parents=True)
        self.lane = {"repo": str(self.root / "repo"), "worktree": str(self.wt),
                     "state_dir": str(self.state), "link_sources": {"generated": str(self.link)},
                     "add_dirs": [str(self.add_dir)]}
        self.env = {"HOME": str(self.home)}
        patcher = mock.patch.object(DS.confine, "node_on_path", return_value=None)
        patcher.start()
        self.addCleanup(patcher.stop)

    def argv(self, cmd=("reasonix", "run")):
        reads = DS.read_paths(self.lane, self.home, self.env)
        return DS.bwrap_argv(cmd, home=self.home, worktree=self.wt, run_dir=self.run_dir,
                             reasonix_home=self.state / "reasonix-home", reads=reads)

    def binds(self, argv, flag):
        return [a for i, a in enumerate(argv) if i and argv[i - 1] == flag]

    def mounts(self, argv):
        """The `(flag, source, destination)` triples the generated line holds."""
        out, i = [], 0
        while i < len(argv):
            if argv[i] in ("--ro-bind", "--bind"):
                out.append((argv[i], argv[i + 1], argv[i + 2]))
                i += 3
            elif argv[i] in ("--dev", "--proc", "--tmpfs"):
                out.append((argv[i], argv[i + 1], argv[i + 1]))
                i += 2
            else:
                i += 1
        return out

    def test_every_allowlisted_path_is_bound_and_the_home_is_masked(self):
        argv = self.argv()
        reads = set(self.binds(argv, "--ro-bind"))
        for path in (self.home / ".cargo", self.home / ".rustup", self.home / ".local/bin",
                     self.bin_target, self.link, self.add_dir, self.root / "repo/.git"):
            self.assertIn(str(path), reads, f"{path} must be readable")
        # The home itself is never bound back: it is the tmpfs mask.
        self.assertNotIn(str(self.home), reads)
        self.assertNotIn(str(self.home / ".ssh"), reads)
        self.assertEqual(self.mounts(argv)[:2],
                         [("--ro-bind", "/", "/"), ("--dev", "/dev", "/dev")])
        self.assertIn(("--proc", "/proc", "/proc"), self.mounts(argv))
        self.assertEqual(self.binds(argv, "--tmpfs"), ["/tmp", str(self.home)])
        self.assertEqual(argv[-4:], ["--die-with-parent", "--", "reasonix", "run"])

    def test_only_three_paths_are_writable(self):
        argv = self.argv()
        typed = {flag: [m for m in self.mounts(argv) if m[0] == flag]
                 for flag in ("--ro-bind", "--bind")}
        self.assertEqual([m[1] for m in typed["--ro-bind"]],
                         [m[2] for m in typed["--ro-bind"]],
                         "every read bind is a path bound onto itself")
        self.assertEqual([m[1:3] for m in typed["--bind"]],
                         [(str(self.wt), str(self.wt)), (str(self.run_dir), str(self.run_dir)),
                          (str(self.state / "reasonix-home"), str(self.home / ".reasonix"))],
                         "the worktree, the run directory and the lane's Reasonix home")
        self.assertNotIn(str(self.home), [m[2] for m in typed["--bind"]])
        self.assertEqual(argv.count("--die-with-parent"), 1)
        self.assertEqual(argv[argv.index("--die-with-parent") + 1:],
                         ["--", "reasonix", "run"],
                         "nothing follows the worker's own command line")

    def test_reads_under_the_home_come_after_the_mask(self):
        """Order is the boundary: a bind inside the home must follow the tmpfs.

        A path *containing* the home (the tests' worker lives next to their
        throwaway home) is bound before it instead, because binding an ancestor
        after the mask would put the masked home back in view.
        """
        outside = plant(self.root / "outside/worker", "#!/bin/sh\n")
        reads = [self.home / ".cargo", outside]
        argv = DS.bwrap_argv(["w"], home=self.home, worktree=self.wt, run_dir=self.run_dir,
                             reasonix_home=self.state / "reasonix-home", reads=reads)
        mask = argv.index(str(self.home))
        self.assertLess(argv.index(str(outside)), mask)
        self.assertLess(mask, argv.index(str(self.home / ".cargo")))

    def test_worker_roots_resolve_the_node_install_behind_a_shim(self):
        """`reasonix` is an nvm shim, so its path alone names no Node install."""
        nvm = self.root / "nvm"
        for rel in ("nvm.sh", "alias/default", "versions/node/v22/bin/node",
                    "versions/node/v22/lib/node_modules/reasonix/bin/reasonix.js"):
            plant(nvm, rel)
        (nvm / "versions/node/v22/lib/node_modules/reasonix/bin/reasonix").symlink_to(
            nvm / "versions/node/v22/lib/node_modules/reasonix/bin/reasonix.js")
        shim = plant(self.home / ".local/bin/reasonix", "#!/usr/bin/env bash\n")
        env = {"HOME": str(self.home), "NVM_DIR": str(nvm)}
        roots = DS.worker_roots(binary=str(shim), env=env, home=self.home)
        self.assertIn(shim.parent, roots)
        self.assertIn(nvm / "nvm.sh", roots)
        self.assertIn(nvm / "versions", roots)
        self.assertNotIn(nvm, roots, "nvm's own checkout and cache stay out of view")
        with mock.patch.object(DS.confine, "node_on_path",
                               return_value=nvm / "versions/node/v22/bin/node"):
            roots = DS.worker_roots(binary=str(shim), env=env, home=self.home)
        self.assertIn(nvm / "versions/node/v22", roots, "the Node prefix, not just its bin")
        # A binary that is already a Node install's entry point names its prefix.
        entry = nvm / "versions/node/v22/lib/node_modules/reasonix/bin/reasonix.js"
        self.assertEqual(DS.node_prefix(entry), nvm / "versions/node/v22")
        self.assertIsNone(DS.node_prefix(self.root), "a plain directory is not an install")

    def test_read_paths_drop_the_missing_and_the_covered(self):
        reads = DS.read_paths({**self.lane, "link_sources": {"gone": str(self.root / "gone")}},
                              self.home, self.env)
        self.assertFalse([p for p in reads if not p.exists()], "a missing source would fail bwrap")
        self.assertNotIn(self.bin_target.parent, reads, "covered by ~/.local/bin")
        self.assertEqual(len(reads), len(set(reads)))
        self.assertNotIn(self.home, reads, "the masked home is never bound back")

    def test_seed_copies_the_config_and_always_creates_env(self):
        src = self.root / "real/.reasonix"
        plant(src, "config.json", '{"apiKey": "one"}\n')
        plant(src, "config.toml", "model = 'x'\n")
        plant(src, "sessions/other-project.jsonl", "someone else's session\n")
        plant(src, "stats/2026-09-25.jsonl", "usage\n")
        dest = self.root / "lane/reasonix-home"
        self.assertEqual(DS.seed_reasonix_home(src, dest), ["config.json", "config.toml"])
        self.assertEqual((dest / "config.json").read_text(), '{"apiKey": "one"}\n')
        self.assertTrue((dest / ".env").exists(), "the nested sandbox needs it to exist")
        self.assertFalse((dest / "sessions").exists(), "another project's sessions stay out")
        self.assertFalse((dest / "stats").exists(), "so does the usage history")
        # A rotated key reaches the next run; the lane's own state is kept.
        (dest / "sessions").mkdir()
        plant(dest, "sessions/lane.jsonl", "this lane's session\n")
        plant(src, "config.json", '{"apiKey": "two"}\n')
        DS.seed_reasonix_home(src, dest)
        self.assertEqual((dest / "config.json").read_text(), '{"apiKey": "two"}\n')
        self.assertTrue((dest / "sessions/lane.jsonl").exists())

    def test_bwrap_must_exist(self):
        with mock.patch.dict(os.environ, {"DS_LANE_BWRAP": str(self.root / "no-bwrap")}):
            self.assertIsNone(DS.bwrap_path())
            with self.assertRaises(SystemExit) as caught:
                DS.require_bwrap()
        self.assertIn("no-bwrap", str(caught.exception))
        self.assertIn("read boundary", str(caught.exception))
        self.assertTrue(DS.bwrap_path(), "the real bubblewrap resolves when nothing is set")


class BoundaryCase(LaneFixture):
    """The worker's own filesystem view, end to end under real bubblewrap."""

    def test_canary_is_unreadable_and_absent_from_the_workers_view(self):
        """The issue's core claim: a credential under the home is not in view."""
        canary = self.canary()
        hosts = self.canary(".config/gh/hosts.yml", "github-token\n")
        spec = {"files": {}, "probe": {
            "out": str(self.probe_out("t1")),
            "read": {"ssh": str(canary), "gh": str(hosts)},
            "write": {"in_home": [str(self.home / "planted.txt"), "planted\n"],
                      "in_ssh": [str(self.home / ".ssh/planted.txt"), "planted\n"]},
            "list": {"home": str(self.home)}}}
        self.start("Probe the home.\n", "t1", spec)
        self.assertEqual(self.run_json("t1")["exit_code"], 0)
        report = self.probe_report("t1")
        for name in ("ssh", "gh"):
            self.assertFalse(report["reads"][name]["exists"], report["reads"][name])
            self.assertIn("No such file", report["reads"][name]["error"])
        self.assertNotIn(CANARY.strip(), json.dumps(report), "its content reached the worker")
        self.assertNotIn("github-token", json.dumps(report))
        # The home's listing holds the allowlist's own parent chain and nothing
        # else: no `.ssh`, no `.config`, no `.cache` from the real home.
        self.assertEqual(sorted(report["lists"]["home"]), [".reasonix", "state", "wt"])
        # Writes aimed at the masked home either fail or land in its private
        # tmpfs; either way the owner's home is untouched.
        for name in ("in_home", "in_ssh"):
            self.assertTrue(report["writes"][name]["path"].startswith(str(self.home)))
        self.assertFalse((self.home / "planted.txt").exists())
        self.assertFalse((self.home / ".ssh/planted.txt").exists())
        self.assertEqual(canary.read_text(), CANARY)
        self.assertEqual(hosts.read_text(), "github-token\n")

    def test_allowlisted_inputs_and_writes_stay_reachable(self):
        """The boundary keeps what a lane needs: inputs read, work written."""
        plant(self.home, ".cargo/registry/marker.txt", "cargo cache\n")
        plant(self.home, ".local/bin/ds-lane", "#!/bin/sh\n")
        linked = self.repo / "generated/data.txt"
        spec = {"files": {"tools/new.txt": "worker output\n"},
                "probe": {"out": str(self.probe_out("t1")),
                          "read": {"worktree": str(self.lane_wt("t1", "tools/new.txt")),
                                   "cargo": str(self.home / ".cargo/registry/marker.txt"),
                                   "local_bin": str(self.home / ".local/bin/ds-lane"),
                                   "repo_git": str(self.repo / ".git/HEAD")},
                          "write": {"run_dir": [str(self.worker_path("t1", "scratch.txt")), "s\n"]}}}
        self.start("Write tools/new.txt.\n", "t1", spec, extra=["--link", "generated"])
        self.assertEqual(self.run_json("t1")["exit_code"], 0)
        report = self.probe_report("t1")
        self.assertEqual(report["reads"]["worktree"]["content"], "worker output\n")
        self.assertEqual(report["reads"]["cargo"], {"path": str(self.home / ".cargo/registry/marker.txt"),
                                                   "exists": True, "content": "cargo cache\n"}, report)
        self.assertTrue(report["reads"]["local_bin"]["exists"])
        self.assertTrue(report["reads"]["repo_git"]["content"].startswith("ref:"))
        self.assertTrue(report["writes"]["run_dir"]["ok"])
        self.assertEqual(self.worker_path("t1", "scratch.txt").read_text(), "s\n")
        # The linked input is a symlink in the worktree: it resolves inside too.
        self.assertTrue(self.lane_wt("t1", "generated/data.txt").exists())
        link_probe = self.lane_wt("t1", "generated/data.txt").resolve()
        self.assertEqual(link_probe, linked.resolve())

    def test_the_lane_reasonix_home_is_seeded_and_persists_across_runs(self):
        """One Reasonix home per lane: seeded config, and a resumed session."""
        plant(self.home, ".reasonix/config.json", '{"apiKey": "fixture-key"}\n')
        sessions = self.home / ".reasonix/sessions"
        spec = {"files": {}, "probe": {
            "out": str(self.probe_out("t1")),
            "read": {"config": str(self.home / ".reasonix/config.json")},
            "write": {"session": [str(sessions / "lane.jsonl"), "session one\n"]}}}
        self.start("Seed the lane home.\n", "t1", spec)
        self.assertEqual(self.run_json("t1")["exit_code"], 0)
        report = self.probe_report("t1")
        self.assertEqual(report["reads"]["config"]["content"], '{"apiKey": "fixture-key"}\n')
        self.assertNotIn("fixture-key", json.dumps(self.run_json("t1")), "not in the receipt")
        lane_home = self.lane_state("t1", "reasonix-home")
        self.assertTrue((lane_home / ".env").exists())
        self.assertTrue((lane_home / "config.json").exists())
        self.assertTrue((lane_home / "sessions/lane.jsonl").exists())
        # The copy of a creditor's file keeps the real home's privacy, and the
        # run's log says which files it was seeded with: `.env` is where the
        # credential has to come from once a config.toml provider table exists.
        self.assertEqual(stat.S_IMODE((lane_home / "config.json").stat().st_mode), 0o600)
        log = (self.lane_state("t1", "run-1", "supervisor.log")).read_text()
        self.assertIn("confined: masked", log)
        self.assertIn("seeded lane home: config.json", log)
        self.assertIn("writable: " + str(self.lane_wt("t1")), log)

        spec2 = {"files": {}, "probe": {
            "out": str(self.probe_out("t1", run=2)),
            "read": {"session": str(sessions / "lane.jsonl")}}}
        self.resume("t1", "Continue the work.\n", spec=spec2, check=0)
        self.assertEqual(self.probe_report("t1", run=2)["reads"]["session"]["content"],
                         "session one\n", "the resumed run found what run 1 wrote")

    def test_reasonix_sandbox_nests_inside_the_boundary(self):
        """Reasonix's own bash-tool sandbox still works inside ds-lane's.

        It re-runs bubblewrap (a user namespace inside a user namespace, which
        this kernel allows) and it binds `/dev/null` over `~/.reasonix/.env`;
        that only works when the file exists, because bubblewrap cannot create
        it under a read-only tree. The seeded Reasonix home provides it.
        """
        spec = {"files": {}, "probe": {"out": str(self.probe_out("t1")), "nested": True}}
        self.start("Nest the sandbox.\n", "t1", spec)
        report = self.probe_report("t1")
        nested = report["nested"]
        self.assertEqual(nested.get("rc"), 0, nested)
        self.assertIn("nested-ok", nested.get("stdout", ""))
        self.assertIn("--ro-bind /dev/null", " ".join(nested["argv"]))

    def test_start_and_resume_refuse_without_bwrap(self):
        """Confinement has no switch: no bubblewrap, no launch."""
        brief = self.write("nb1.md", "Touch only tools/keep.txt.\n")
        missing = str(self.work / "no-bwrap")
        r = self.cli("start", brief, "--repo", self.repo, "--id", "nb1",
                     spec={"files": {}}, env={"DS_LANE_BWRAP": missing}, check=1)
        self.assertIn("not found", r.stdout)
        self.assertIn("read boundary", r.stdout)
        self.assertFalse(self.lane_state("nb1", "run-1").exists(), "no run was created")

        self.start("Write tools/new.txt.\n", "t1", {"files": {"tools/new.txt": "x\n"}})
        self.assertEqual(self.run_json("t1")["exit_code"], 0)
        follow = self.write("follow.md", "# Repair\n\nTouch only tools/new.txt again.\n")
        r = self.cli("resume", "t1", follow, "--repo", self.repo, "--no-wait",
                     env={"DS_LANE_BWRAP": missing}, check=1)
        self.assertIn("not found", r.stdout)
        self.assertFalse(self.lane_state("t1", "run-2").exists())

    def test_canary_is_readable_without_the_wrapper(self):
        """Negative control: the canary is in view when the wrapper is absent.

        Same worker, same canary, same environment allowlist as the case below,
        with `confine.wrap`'s bubblewrap prefix taken off the command line: the
        worker reads the credential straight out of the home the lane masks. So
        what makes the confined case pass is the prefix and nothing else.
        """
        canary = self.canary()
        out = self.work / "unconfined-probe.json"
        spec = {"probe": {"out": str(out), "read": {"canary": str(canary)}}}
        env = DS.worker_env({"HOME": str(self.home), "PATH": os.environ["PATH"],
                             "DS_LANE_WORKER_ENV_PASS": "FAKE_REASONIX_SPEC",
                             "FAKE_REASONIX_SPEC": json.dumps(spec)})
        r = subprocess.run([str(self.fake), "run"], input="go", text=True, capture_output=True,
                           env=env, cwd=str(self.repo), timeout=60)
        self.assertEqual(r.returncode, 0, r.stderr)
        report = json.loads(out.read_text())
        self.assertTrue(report["reads"]["canary"]["exists"], report)
        self.assertEqual(report["reads"]["canary"]["content"], CANARY)
