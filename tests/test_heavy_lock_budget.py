"""tools/heavy_lock.py: the memory budget and its shared classes.

    PYTHONPATH=. python3 -m unittest tests.test_heavy_lock_budget -v

Real `heavy_lock.py run` holders and waiters on a temp lock file, on
tests/test_heavy_lock.py's `LockProcesses` fixture, plus in-process grants:
build, godot and python holders share the budget while their units fit and
exclusive runs exclude them all; the units held never exceed the budget (the
negative control overfills it with a grant that is not all-or-none); a waiting
head is never passed by a smaller arrival; a failed or refused take gives back
every handle; a killed holder's units come back; nesting across shared classes
and a budget mismatch are refused. Each has a negative control.
"""

import fcntl
import io
import math
import os
import signal
import subprocess
import sys
import time
import unittest
from unittest import mock

from tests.test_heavy_lock import NESTED_REQUEST, TIMEOUT, TOOLS, LockProcesses
from tools import heavy_lock as heavy_lock_module
from tools.heavy_lock import (BUILD, CLASS_UNITS, EXCLUSIVE, EXTENSION, EXIT_HELD_BY_PARENT, GODOT, PYTHON,
                              UNITS_ENV, lock_holders, lock_modes, lock_units, units_in_use)


#: A budget with room for exactly one build holder and one godot holder.
#: A budget that one extension build and one Godot run fill. A budget below
#: the largest class is refused, so the tight-budget cases use that class.
PAIR_BUDGET = {UNITS_ENV: str(CLASS_UNITS[EXTENSION] + CLASS_UNITS[GODOT])}
#: The units a build and a Godot run take, for the in-process grant cases.
BUILD_PAIR_UNITS = CLASS_UNITS[BUILD] + CLASS_UNITS[GODOT]


def overfilling_take_files(lock, kind, count, cap, env, label=None):
    """The mutation for the budget's negative control: `_take_files` without
    all-or-none, so a grant proceeds on whatever units are free."""
    held = []
    for index in range(cap):
        if len(held) == count:
            break
        handle = os.open(f"{lock}.{kind}{index}", os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            os.close(handle)
        else:
            held.append(handle)
    return held


class BudgetCase(LockProcesses):
    """The build class and the unit budget; see LockProcesses."""

    def test_a_build_runs_beside_a_godot_holder_without_waiting(self):
        godot = self.holder("godot")
        code, stderr = self.immediate("build", "--class", "build")
        self.assertEqual(code, 0, stderr)
        self.assertNotIn("waiting", stderr)
        self.assertEqual(self.granted(), ["build"])
        build = self.holder("build")  # both held at once, each in its class
        self.assertEqual(sorted(lock_modes(self.lock)),
                         sorted([(godot.pid, GODOT), (build.pid, BUILD)]))
        self.assertEqual(units_in_use(self.lock), CLASS_UNITS[GODOT] + CLASS_UNITS[BUILD])
        out = io.StringIO()
        heavy_lock_module.status(self.lock, out)
        self.assertIn(f"{build.pid} build:", out.getvalue())
        self.assertIn(f"units in use: {CLASS_UNITS[GODOT] + CLASS_UNITS[BUILD]} of",
                      out.getvalue())

    def test_negative_control_a_budget_without_room_makes_the_build_wait(self):
        """A Godot run and a build under a budget of the largest class's units,
        an extension build's: the budget, not an unheld lock, is what admitted
        the build above."""
        budget = {UNITS_ENV: str(CLASS_UNITS[EXTENSION])}
        godot = self.holder("godot", budget)
        build = self.waiter("build", "--class", EXTENSION, env=budget)
        self.assertIn(f"{godot.pid} (godot)", build.first_line)
        time.sleep(0.5)
        self.assertEqual(self.granted(), [])
        self.release(godot)
        self.assertEqual(self.finish(build), ["build"])

    def test_a_build_holder_excludes_an_exclusive_run(self):
        build = self.holder("build")
        exclusive = self.waiter("exclusive")
        self.assertIn(f"held by pid {build.pid} (build)", exclusive.first_line)
        time.sleep(0.5)
        self.assertEqual(self.granted(), [])
        self.release(build)
        self.assertEqual(self.finish(exclusive), ["exclusive"])

    def test_an_exclusive_holder_excludes_a_build(self):
        holder = self.holder()
        build = self.waiter("build", "--class", "build")
        self.assertIn(f"held by pid {holder.pid} (exclusive)", build.first_line)
        time.sleep(0.5)
        self.assertEqual(self.granted(), [])
        self.release(holder)
        self.assertEqual(self.finish(build), ["build"])

    def test_negative_control_with_no_holder_both_start_at_once(self):
        for name, flags in (("exclusive", ()), ("build", ("--class", "build"))):
            code, stderr = self.immediate(name, *flags)
            self.assertEqual(code, 0, stderr)
            self.assertNotIn("waiting", stderr)
        self.assertEqual(self.granted(), ["exclusive", "build"])

    def test_a_full_budget_makes_every_shared_arrival_wait(self):
        build = self.holder(EXTENSION, PAIR_BUDGET)
        godot = self.holder("godot", PAIR_BUDGET)
        budget = int(PAIR_BUDGET[UNITS_ENV])
        self.assertEqual(units_in_use(self.lock), budget)
        second_godot = self.waiter("second-godot", "--class", "godot", env=PAIR_BUDGET)
        self.assertIn(f"{build.pid} (extension)", second_godot.first_line)
        time.sleep(0.5)
        self.assertLessEqual(units_in_use(self.lock), budget)
        self.assertEqual(self.granted(), [])
        self.release(godot)
        self.assertEqual(self.finish(second_godot), ["second-godot"])
        second_build = self.waiter("second-build", "--class", EXTENSION, env=PAIR_BUDGET)
        time.sleep(0.5)
        self.assertEqual(self.granted(), ["second-godot"])
        self.release(build)
        self.assertEqual(self.finish(second_build), ["second-godot", "second-build"])

    def grant_in_turn(self, classes, budget):
        """Grant each class in turn in this process, as a head would; returns
        the units the granted holders take, by class, never more than fit."""
        handles, taken = [], 0
        try:
            for lock_class in classes:
                need = heavy_lock_module._Need.of(lock_class, 1, 3, budget)
                held = heavy_lock_module._try_grant(str(self.lock), need)
                if held is not None:
                    handles += held
                    taken += need.units
                self.assertLessEqual(taken, budget, f"{classes} overfilled {budget} units")
            return taken
        finally:
            for handle in handles:
                os.close(handle)

    def test_the_budget_is_never_exceeded(self):
        budget = BUILD_PAIR_UNITS
        order = [BUILD, BUILD, GODOT, GODOT, BUILD]
        self.assertEqual(self.grant_in_turn(order, budget), budget)

    def test_negative_control_a_grant_that_is_not_all_or_none_overfills(self):
        budget = BUILD_PAIR_UNITS
        with mock.patch.object(heavy_lock_module, "_take_files", overfilling_take_files), \
                self.assertRaisesRegex(AssertionError, "overfilled"):
            self.grant_in_turn([BUILD, BUILD, GODOT, GODOT, BUILD], budget)

    def test_a_build_arrival_queues_behind_a_waiting_exclusive_run(self):
        """A stream of build holders cannot starve an exclusive waiter: a build
        that would fit beside the current holder still queues behind it."""
        build = self.holder("build")
        exclusive = self.waiter("exclusive")
        later = self.waiter("later", "--class", "build")  # fails here if it jumped the queue
        self.assertIn("1 waiter(s) ahead", later.first_line)
        self.release(build)
        self.assertEqual(self.finish(exclusive, later), ["exclusive", "later"])

    def test_negative_control_without_the_exclusive_waiter_the_build_starts(self):
        self.holder("build")
        code, stderr = self.immediate("later", "--class", "build")
        self.assertEqual(code, 0, stderr)
        self.assertNotIn("waiting", stderr)

    def nested(self, held, requested, env=None):
        flags = ("--class", held)
        return subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", *flags, "--",
             sys.executable, "-c", NESTED_REQUEST, str(self.lock), requested],
            capture_output=True, text=True, timeout=TIMEOUT, env={**self.env, **(env or {})})

    def test_a_request_under_a_shared_holder_of_another_class_is_refused(self):
        for held, requested in ((GODOT, BUILD), (BUILD, GODOT), (BUILD, EXCLUSIVE)):
            done = self.nested(held, requested)
            self.assertEqual(done.returncode, EXIT_HELD_BY_PARENT, (held, requested, done))
            self.assertIn(f"{requested} run was started under a {held}-class holder",
                          done.stderr)

    def test_negative_control_the_same_class_or_an_exclusive_holder_nests(self):
        for held, requested in ((BUILD, BUILD), (EXCLUSIVE, BUILD)):
            done = self.nested(held, requested)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertEqual(done.stdout.split()[0], held)
            self.assertEqual(done.stdout.split()[2], "True")

    def mixed_holders(self, env=None):
        """One cargo run, two Godot runs and a Python suite, all held at once."""
        return [self.holder(BUILD, env), self.holder(GODOT, env), self.holder(GODOT, env),
                self.holder(PYTHON, env)]

    def test_a_cargo_run_two_godot_runs_and_a_python_suite_share_the_budget(self):
        holders = self.mixed_holders()
        expected = CLASS_UNITS[BUILD] + 2 * CLASS_UNITS[GODOT] + CLASS_UNITS[PYTHON]
        self.assertEqual(units_in_use(self.lock), expected)
        self.assertLessEqual(expected, lock_units({}))
        self.assertEqual(sorted(lock_modes(self.lock)),
                         sorted(zip((h.pid for h in holders), (BUILD, GODOT, GODOT, PYTHON))))
        second = self.waiter("second-cargo", "--class", BUILD)
        self.assertIn(f"{holders[0].pid} (build)", second.first_line)
        time.sleep(0.5)
        self.assertEqual(self.granted(), [], "a second cargo run would exceed the budget")
        self.release(holders[0])
        self.assertEqual(self.finish(second), ["second-cargo"])

    def test_negative_control_a_budget_one_unit_short_makes_the_suite_wait(self):
        short = CLASS_UNITS[BUILD] + 2 * CLASS_UNITS[GODOT] + CLASS_UNITS[PYTHON] - 1
        budget = {UNITS_ENV: str(short)}
        cargo, _, _ = self.holder(BUILD, budget), self.holder(GODOT, budget), \
            self.holder(GODOT, budget)
        suite = self.waiter("suite", "--class", PYTHON, env=budget)
        time.sleep(0.5)
        self.assertEqual(self.granted(), [])
        self.release(cargo)
        self.assertEqual(self.finish(suite), ["suite"])

    def test_negative_control_a_budget_with_room_starts_the_second_cargo_run(self):
        room = CLASS_UNITS[BUILD] * 2 + 2 * CLASS_UNITS[GODOT] + CLASS_UNITS[PYTHON]
        budget = {UNITS_ENV: str(room)}
        self.mixed_holders(budget)
        code, stderr = self.immediate("second-cargo", "--class", BUILD, env=budget)
        self.assertEqual(code, 0, stderr)
        self.assertNotIn("waiting", stderr)

    def test_the_extension_class_is_sized_by_the_dev_extension_build(self):
        """ceil(3,609 MB / (0.85 x 512 MB)) = 9, from docs/DEVELOPMENT.md's
        measured dev `cargo build -p psiv-godot`, a literal, not CLASS_UNITS."""
        self.assertEqual(math.ceil(3609 / (0.85 * 512)), 9)
        self.assertEqual(CLASS_UNITS[EXTENSION], 9)

    def test_an_extension_build_shares_the_budget_with_a_build_but_not_two(self):
        """9 + 7 fit the default 16 units; a second extension build (18) waits."""
        extension = self.holder(EXTENSION)
        code, stderr = self.immediate("cargo", "--class", BUILD)
        self.assertEqual(code, 0, stderr)
        self.assertNotIn("waiting", stderr)
        self.assertEqual(units_in_use(self.lock), 9)
        second = self.waiter("second-extension", "--class", EXTENSION)
        self.assertIn(f"{extension.pid} (extension)", second.first_line)
        time.sleep(0.5)
        self.assertEqual(self.granted(), ["cargo"])
        self.release(extension)
        self.assertEqual(self.finish(second), ["cargo", "second-extension"])

    def test_negative_control_an_extension_build_beside_three_godot_runs_and_a_suite_fits(self):
        """9 + 3 + 2 = 14 of 16: the class does not exclude Godot runs."""
        holders = [self.holder(GODOT), self.holder(GODOT), self.holder(GODOT),
                   self.holder(PYTHON)]
        code, stderr = self.immediate("extension", "--class", EXTENSION)
        self.assertEqual(code, 0, stderr)
        self.assertNotIn("waiting", stderr)
        self.assertEqual(len(holders), 4)

    def test_an_exclusive_holder_excludes_every_shared_class_and_they_exclude_it(self):
        """Pack builds and the gate take exclusive: this is what keeps a pack
        rewrite away from every run that reads the pack."""
        for lock_class in (BUILD, GODOT, PYTHON):
            holder = self.holder()
            waiter = self.waiter(lock_class, "--class", lock_class)
            self.assertIn(f"held by pid {holder.pid} (exclusive)", waiter.first_line)
            self.release(holder)
            self.assertEqual(waiter.wait(timeout=TIMEOUT), 0)
            shared = self.holder(lock_class)
            exclusive = self.waiter(f"exclusive-after-{lock_class}")
            self.assertIn(f"held by pid {shared.pid} ({lock_class})", exclusive.first_line)
            self.release(shared)
            self.assertEqual(exclusive.wait(timeout=TIMEOUT), 0)
        self.assertEqual(self.granted(), [step for name in (BUILD, GODOT, PYTHON)
                                          for step in (name, f"exclusive-after-{name}")])

    def test_negative_control_with_no_holder_each_shared_class_starts_at_once(self):
        for lock_class in (BUILD, GODOT, PYTHON):
            code, stderr = self.immediate(lock_class, "--class", lock_class)
            self.assertEqual(code, 0, stderr)
            self.assertNotIn("waiting", stderr)

    def test_a_godot_arrival_queues_behind_a_build_head_waiting_for_units(self):
        """The head waits for units, not the lock file; a smaller arrival that
        would fit now still queues behind it, so a stream of Godot runs cannot
        starve a cargo run either."""
        budget = {UNITS_ENV: str(CLASS_UNITS[BUILD] + CLASS_UNITS[PYTHON])}
        self.holder(PYTHON, budget)
        godot = self.holder(GODOT, budget)
        build = self.waiter("build", "--class", BUILD, env=budget)
        later = self.waiter("later-godot", "--class", GODOT, env=budget)
        self.assertIn("1 waiter(s) ahead", later.first_line)
        time.sleep(0.5)
        self.assertEqual(self.granted(), [])
        self.release(godot)
        self.assertEqual(self.finish(build, later), ["build", "later-godot"])

    def test_negative_control_without_the_build_head_the_godot_run_starts(self):
        budget = {UNITS_ENV: str(CLASS_UNITS[BUILD] + CLASS_UNITS[PYTHON])}
        self.holder(PYTHON, budget)
        self.holder(GODOT, budget)
        code, stderr = self.immediate("godot", "--class", GODOT, env=budget)
        self.assertEqual(code, 0, stderr)
        self.assertNotIn("waiting", stderr)

    def held_slot_files(self):
        """Indexes of this lock's godot slot files someone holds now."""
        held = []
        for path in sorted(self.lock.parent.glob(self.lock.name + ".godot*")):
            handle = os.open(path, os.O_RDWR)
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                held.append(path.name)
            finally:
                os.close(handle)
        return held

    def test_a_godot_take_without_units_gives_its_slot_back(self):
        budget = CLASS_UNITS[BUILD]
        units = heavy_lock_module._take_files(str(self.lock), "unit", budget, budget,
                                              UNITS_ENV, BUILD)  # the budget, all taken
        self.addCleanup(lambda: [os.close(handle) for handle in units])
        need = heavy_lock_module._Need.of(GODOT, 1, 3, budget)
        self.assertIsNone(need.take(str(self.lock)))
        self.assertEqual(self.held_slot_files(), [])

    def test_a_mismatch_after_the_slots_gives_the_slots_and_the_lock_file_back(self):
        budget = CLASS_UNITS[BUILD]
        theirs = heavy_lock_module._take_files(str(self.lock), "unit", 1, budget + 1,
                                               UNITS_ENV, BUILD)  # recorded under another budget
        self.addCleanup(lambda: [os.close(handle) for handle in theirs])
        need = heavy_lock_module._Need.of(GODOT, 1, 3, budget)
        with self.assertRaises(heavy_lock_module.SlotCapMismatch):
            heavy_lock_module._try_grant(str(self.lock), need)
        self.assertEqual(self.held_slot_files(), [])
        self.assertEqual(lock_holders(self.lock), [], "the lock file was given back")

    def test_negative_control_a_successful_take_holds_its_slot(self):
        """The probe above can see a held slot: a take with units free keeps one."""
        need = heavy_lock_module._Need.of(GODOT, 1, 3, CLASS_UNITS[BUILD])
        taken = need.take(str(self.lock))
        self.addCleanup(lambda: [os.close(handle) for handle in taken])
        self.assertEqual(self.held_slot_files(), [f"{self.lock.name}.godot0"])

    def test_a_killed_holders_units_come_back(self):
        budget = {UNITS_ENV: str(CLASS_UNITS[EXTENSION])}
        holder = self.holder(EXTENSION, budget)
        self.assertEqual(units_in_use(self.lock), CLASS_UNITS[EXTENSION])
        holder.send_signal(signal.SIGKILL)
        holder.wait(timeout=TIMEOUT)
        self.assertEqual(units_in_use(self.lock), 0)
        code, stderr = self.immediate("after-kill", "--class", EXTENSION, env=budget)
        self.assertEqual(code, 0, stderr)
        self.assertNotIn("waiting", stderr)

    def test_negative_control_a_live_holders_units_are_kept(self):
        budget = {UNITS_ENV: str(CLASS_UNITS[EXTENSION])}
        holder = self.holder(EXTENSION, budget)
        waiter = self.waiter("waiter", "--class", EXTENSION, env=budget)
        self.assertIn(f"{holder.pid} (extension)", waiter.first_line)
        time.sleep(0.5)
        self.assertEqual(self.granted(), [])
        self.release(holder)
        self.assertEqual(self.finish(waiter), ["waiter"])

    def test_there_is_no_pack_class(self):
        """A pack build rewrites the runtime pack in place, so it must exclude
        every reader: it is exclusive, never a budgeted class."""
        self.assertNotIn("pack", heavy_lock_module.LOCK_CLASSES)
        code, stderr = self.immediate("pack", "--class", "pack")
        self.assertEqual(code, 2, stderr)
        self.assertEqual(self.granted(), [])

    def test_a_held_unit_under_another_budget_is_refused(self):
        self.holder("build", {UNITS_ENV: "16"})  # records 16 in its unit files
        code, stderr = self.immediate("other", "--class", "build", env={UNITS_ENV: "20"})
        self.assertEqual(code, EXIT_HELD_BY_PARENT, stderr)
        self.assertIn(f"{UNITS_ENV}=16, this one has 20", stderr)
        self.assertEqual(self.granted(), [])

    def test_negative_control_the_holders_budget_proceeds(self):
        self.holder("build", {UNITS_ENV: "16"})
        code, stderr = self.immediate("same", "--class", "build", env={UNITS_ENV: "16"})
        self.assertEqual(code, 0, stderr)
        self.assertEqual(self.granted(), ["same"])

    def test_a_budget_below_the_largest_class_is_refused(self):
        with self.assertRaises(ValueError):
            lock_units({UNITS_ENV: str(max(CLASS_UNITS.values()) - 1)})
        self.assertEqual(lock_units({UNITS_ENV: str(max(CLASS_UNITS.values()))}),
                         max(CLASS_UNITS.values()))


if __name__ == "__main__":
    unittest.main()
