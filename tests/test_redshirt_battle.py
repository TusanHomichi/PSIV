"""Focused synthetic battle/Redshirt adapter checks; build example first.

Caller supplies Redshirt through PYTHONPATH. These checks never call a model.
"""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

try:
    from redshirt import Limits, run
    from redshirt.runner import Stop
    from tools.redshirt_battle import (AttackOnlyBaseline, BattleAdapter,
                                      ProjectedBioPlantBaseline, ThreatSkillBaseline,
                                      _encoded, clean_env)
except ModuleNotFoundError as exc:
    if exc.name != "redshirt":
        raise
    REDSHIRT_MISSING = "Redshirt is not on PYTHONPATH; supply the pinned ignored checkout"
else:
    REDSHIRT_MISSING = None


ROOT = Path(__file__).resolve().parents[1]
ENGINE = Path(os.environ.get(
    "PSIV_REDSHIRT_BATTLE_ENGINE",
    ROOT / "rust/target/debug/examples/redshirt_battle",
))
LIMITS = (Limits(inputs=12, requests=12, captures=0, seconds=90,
                 operation_seconds=5, final_seconds=5, no_progress=3)
          if REDSHIRT_MISSING is None else None)
CASES = (
    "calm_single", "wounded_single", "split_pair", "sturdy_pair",
    "lean_single", "wounded_pair", "uneven_pair", "thin_reserve",
)


def authored_fixture() -> dict:
    # Deliberately a test-only synthetic pressure edge, not an experiment case.
    return {"schema": "psiv-synthetic-battle-fixture-v1", "id": "ko_validation",
            "seed": 0x13572468,
            "party": {"hp": 5, "tp": 18, "crosscut_uses": 2},
            "foes": [{"hp": 38, "attack": 31, "defence": 4,
                      "agility": 18, "dexterity": 9}]}


def visible_foe(id: int, hp: int, attack: int, *, defence: int = 4,
                agility: int = 9, status: int = 0) -> dict:
    return {"id": id, "name": f"Foe {id}", "hp": hp, "max_hp": hp,
            "attack": attack, "defence": defence, "agility": agility,
            "dexterity": 12, "mental": 8, "status": status}


def threat_request(foes: list[dict], offered: set[str], *, hp: int = 45,
                   tp: int = 18, charges: int = 1) -> dict:
    # Full selector-visible shape; these are policy test scenarios, not engine
    # fixtures or new evaluation cases. Candidate IDs are explicit on purpose.
    view = {"goal": "Win without party KO; then conserve TP and skill uses.",
            "step": "One selected command resolves a full round; enemies use plain attacks.",
            "status_bits": {"poison": 1, "paralysis": 2, "knocked_out": 4,
                            "sleep": 8, "techniques_sealed": 16,
                            "secondary_sleep": 32, "android_knocked_out": 64},
            "round": 0, "round_limit": 12, "last_effects": None,
            "party": [{"id": 1, "name": "Pilot", "hp": hp, "max_hp": 100,
                       "tp": tp, "max_tp": 24, "crosscut_uses": charges,
                       "attack": 24, "defence": 23, "agility": 18,
                       "dexterity": 18, "mental": 14, "status": 0}],
            "enemies": foes}
    return {"observation": view, "remaining_inputs": 12, "version": 1,
            "candidates": {candidate: candidate for candidate in offered | {"stop"}}}


class ThreatSkillBaselineTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        if REDSHIRT_MISSING is not None:
            self.skipTest(REDSHIRT_MISSING)
        self.policy = ThreatSkillBaseline()

    async def choose(self, request: dict) -> str:
        choice = await self.policy.select(request)
        self.assertIn(choice, request["candidates"])
        return choice

    async def test_trivial_ordinary_kill_conserves_tp_and_charge(self):
        request = threat_request(
            [visible_foe(6, 9, 40, defence=2)],
            {"attack_6", "crosscut_6", "res_1", "rimit_all", "defend"}, hp=20)
        self.assertEqual(await self.choose(request), "attack_6")

    async def test_burst_removal_precedes_healing(self):
        request = threat_request(
            [visible_foe(6, 80, 60), visible_foe(7, 100, 40)],
            {"attack_6", "attack_7", "crosscut_6", "crosscut_7", "res_1"}, hp=25)
        self.assertEqual(await self.choose(request), "crosscut_6")

    async def test_weak_kill_does_not_distract_from_dangerous_foe(self):
        foes = [visible_foe(6, 5, 25, defence=2), visible_foe(7, 80, 60)]
        offered = {"attack_6", "attack_7", "crosscut_6", "crosscut_7", "res_1"}
        request = threat_request(foes, offered, hp=45)
        # Killing Foe 6 normally leaves an estimated 37 incoming from Foe 7;
        # 45 HP lacks the conservative >2x margin, so remove the threat.
        self.assertEqual(await self.choose(request), "crosscut_7")
        # With ample HP the same free, no-normal-miss kill saves the charge.
        self.assertEqual(await self.choose(threat_request(foes, offered, hp=90)),
                         "attack_6")

    async def test_heal_when_no_foe_can_be_removed(self):
        request = threat_request(
            [visible_foe(6, 200, 40), visible_foe(7, 200, 38)],
            {"attack_6", "attack_7", "crosscut_6", "crosscut_7", "res_1"}, hp=40)
        self.assertEqual(await self.choose(request), "res_1")
        # Even a capped heal is preferred when one-round representative
        # incoming exceeds current HP and no immediate removal is available.
        urgent = threat_request(
            [visible_foe(6, 200, 71), visible_foe(7, 200, 72)],
            {"attack_6", "attack_7", "res_1"}, hp=90, charges=0)
        self.assertEqual(await self.choose(urgent), "res_1")

    async def test_visible_resources_and_offered_menu_prevent_skill_overspend(self):
        foes = [visible_foe(6, 90, 35)]
        # An inconsistent offered skill cannot make the selector spend a zero
        # visible balance; a real request excludes it from the legal menu.
        no_charge = threat_request(foes, {"attack_6", "crosscut_6"},
                                   hp=90, tp=0, charges=0)
        self.assertEqual(await self.choose(no_charge), "attack_6")
        unavailable = threat_request(foes, {"attack_6", "res_1"},
                                     hp=90, tp=0, charges=1)
        self.assertEqual(await self.choose(unavailable), "attack_6")

    async def test_awake_target_outranks_stronger_sleeper(self):
        request = threat_request(
            [visible_foe(6, 80, 80, status=8), visible_foe(7, 80, 40)],
            {"attack_6", "attack_7", "crosscut_6", "crosscut_7"}, hp=90)
        self.assertEqual(await self.choose(request), "crosscut_7")
        no_charge = threat_request(request["observation"]["enemies"],
                                   {"attack_6", "attack_7"}, hp=90, charges=0)
        self.assertEqual(await self.choose(no_charge), "attack_7")

    async def test_rimit_only_in_two_foe_no_charge_danger_window(self):
        foes = [visible_foe(6, 90, 40), visible_foe(7, 90, 42, agility=12)]
        offered = {"attack_6", "attack_7", "res_1", "rimit_all"}
        danger = threat_request(foes, offered, hp=45, charges=0)
        self.assertEqual(await self.choose(danger), "rimit_all")
        healthy = threat_request(foes, offered, hp=90, charges=0)
        self.assertNotEqual(await self.choose(healthy), "rimit_all")
        with_charge = threat_request(foes, offered | {"crosscut_6"}, hp=45)
        self.assertNotEqual(await self.choose(with_charge), "rimit_all")
        too_slow = threat_request([foes[0], visible_foe(7, 90, 42, agility=19)],
                                  offered, hp=45, charges=0)
        self.assertNotEqual(await self.choose(too_slow), "rimit_all")
        one_asleep = threat_request([foes[0], visible_foe(7, 90, 42, status=8)],
                                    offered, hp=45, charges=0)
        self.assertNotEqual(await self.choose(one_asleep), "rimit_all")
        no_tp = threat_request(foes, offered, hp=45, tp=0, charges=0)
        self.assertNotEqual(await self.choose(no_tp), "rimit_all")

    async def test_attack_fallback_uses_threat_per_estimated_hits(self):
        request = threat_request(
            [visible_foe(6, 100, 50, defence=5),
             visible_foe(7, 15, 38, defence=5)],
            {"attack_6", "attack_7"}, hp=90, tp=0, charges=0)
        self.assertEqual(await self.choose(request), "attack_7")
        self.assertEqual(await self.choose(threat_request(
            request["observation"]["enemies"], {"attack_6"},
            hp=90, tp=0, charges=0)), "attack_6")
        tied = threat_request(
            [visible_foe(7, 80, 40), visible_foe(6, 80, 40)],
            {"attack_7", "attack_6"}, hp=90, tp=0, charges=0)
        self.assertEqual(await self.choose(tied), "attack_6")

    async def test_host_metadata_and_briefing_cannot_change_choice(self):
        request = threat_request(
            [visible_foe(6, 80, 45)], {"attack_6", "crosscut_6", "res_1"}, hp=40)
        original = await self.choose(request)
        decorated = json.loads(json.dumps(request))
        decorated.update({"identity": {"seed": 999, "case": "never_read"},
                          "epoch": "irrelevant", "guard": "irrelevant"})
        decorated["observation"]["rules"] = {"arbitrary": "irrelevant"}
        self.assertEqual(await self.choose(decorated), original)

    async def test_unknown_policy_is_rejected_before_engine_access(self):
        with self.assertRaisesRegex(ValueError, "invalid_baseline"):
            BattleAdapter(Path("/no/engine/needed"), "calm_single", baseline="unknown")


class BattleAdapterTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        if REDSHIRT_MISSING is not None:
            self.skipTest(REDSHIRT_MISSING)
        if not ENGINE.is_file():
            self.skipTest(f"build the psiv-core redshirt_battle example first: {ENGINE}")
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def fixture_path(self, value: dict | str) -> Path:
        path = Path(self.tmp.name) / "authored.json"
        path.write_text(json.dumps(value) if isinstance(value, dict) else value)
        return path

    async def test_fixed_cases_are_four_plus_four_and_source_bound(self):
        result = subprocess.run([str(ENGINE), "--list-cases"], check=True,
                                capture_output=True, text=True, timeout=5)
        rows = json.loads(result.stdout)
        self.assertEqual(tuple(row["id"] for row in rows), CASES)
        self.assertEqual([row["split"] for row in rows].count("calibration"), 4)
        self.assertEqual([row["split"] for row in rows].count("held_out"), 4)
        self.assertEqual(len({row["seed"] for row in rows}), 8)
        adapter = BattleAdapter(ENGINE, "split_pair")
        self.assertEqual(adapter.identity["seed"], rows[2]["seed"])
        await adapter.reset()
        try:
            observation = await adapter.observe()
            self.assertLessEqual(len(_encoded(observation.view)), 4096)
            self.assertNotIn("seed", _encoded(observation.view).decode())
            self.assertNotIn("split_pair", _encoded(observation.view).decode())
            self.assertIsNone(observation.view["last_effects"])
            for fighter in observation.view["party"] + observation.view["enemies"]:
                self.assertTrue({"attack", "defence", "agility", "dexterity", "mental"}
                                <= fighter.keys())
            ids = {candidate.id for candidate in adapter.candidates(observation)}
            self.assertEqual(ids, {"attack_6", "attack_7", "defend", "res_1",
                                   "rimit_all", "crosscut_6", "crosscut_7"})
            check = await adapter.evaluate("reset", None)
            self.assertTrue(check.ok)
            self.assertIsNone(check.checks["comparison"]["useful_action"])
        finally:
            await adapter.close()

    async def test_probe_rejects_stale_and_unavailable_without_moving(self):
        adapter = BattleAdapter(ENGINE, "calm_single")
        await adapter.reset()
        try:
            before = await adapter._state()
            stale = await adapter._rpc({"op": "step", "revision": 99, "action": "attack_6"})
            unavailable = await adapter._rpc({"op": "step", "revision": 0, "action": "crosscut_7"})
            self.assertEqual(stale, {"ok": False, "error": "stale_revision"})
            self.assertEqual(unavailable, {"ok": False, "error": "unavailable_action"})
            self.assertEqual(await adapter._state(), before)
        finally:
            await adapter.close()

    async def test_mechanics_briefing_changes_only_information(self):
        fixture = authored_fixture()
        fixture["party"] = {"hp": 100, "tp": 24, "crosscut_uses": 2}
        fixture["foes"] = [{"hp": 200, "attack": 1, "defence": 4,
                            "agility": 9, "dexterity": 9}]
        path = self.fixture_path(fixture)
        sparse = BattleAdapter(ENGINE, fixture=path)
        rich = BattleAdapter(ENGINE, fixture=path, briefing="mechanics-v1")
        self.assertNotEqual(sparse.identity, rich.identity)
        await sparse.reset()
        await rich.reset()
        try:
            # Exercise every exposed command kind through the same real engine.
            for action in ("defend", "rimit_all", "res_1", "attack_6", "crosscut_6"):
                a, b = await sparse.observe(), await rich.observe()
                self.assertLessEqual(len(_encoded(b.view)), 4096)
                self.assertNotIn("rules", a.view)
                plain = dict(b.view)
                rules = plain.pop("rules")
                self.assertEqual(plain, a.view)
                self.assertTrue({"attack", "crosscut", "res", "rimit", "defend"} <= rules.keys())
                self.assertNotIn(fixture["id"], _encoded(b.view).decode())
                self.assertNotIn(str(fixture["seed"]), _encoded(b.view).decode())
                self.assertEqual(sparse.candidates(a), rich.candidates(b))
                operation = next(c.operation for c in sparse.candidates(a) if c.id == action)
                self.assertEqual(await sparse.execute(operation), await rich.execute(operation))
                self.assertEqual(await sparse.evaluate("after", operation),
                                 await rich.evaluate("after", operation))
                self.assertEqual(await sparse._state(), await rich._state())
            self.assertEqual(await sparse.evaluate("final", None),
                             await rich.evaluate("final", None))
        finally:
            await sparse.close()
            await rich.close()

    async def test_briefing_rejects_unknown_mode_and_does_not_weaken_size_limit(self):
        with self.assertRaisesRegex(ValueError, "invalid_briefing"):
            BattleAdapter(ENGINE, "calm_single", briefing="unknown")
        adapter = BattleAdapter(ENGINE, "calm_single", briefing="mechanics-v1")
        await adapter.reset()
        try:
            # Negative control: extra explanatory information must still be bounded.
            with mock.patch("tools.redshirt_battle.MECHANICS_V1", {"oversize": "x" * 4097}):
                with self.assertRaisesRegex(Stop, "observation_size"):
                    await adapter.observe()
        finally:
            await adapter.close()

    async def test_acknowledged_but_omitted_command_fails_independent_check(self):
        adapter = BattleAdapter(ENGINE, "calm_single")
        await adapter.reset()
        try:
            observation = await adapter.observe()
            operation = next(candidate.operation for candidate in adapter.candidates(observation)
                             if candidate.id == "attack_6")
            original_rpc = adapter._rpc

            async def omit_step(request):
                if request.get("op") == "step":
                    return {"ok": True, "acknowledged": True, "revision": 1}
                return await original_rpc(request)

            adapter._rpc = omit_step
            receipt = await adapter.execute(operation)
            self.assertTrue(receipt["acknowledged"])
            checked = await adapter.evaluate("after", operation)
            self.assertFalse(checked.ok)
            self.assertFalse(checked.checks["round_advanced"])
            self.assertFalse(checked.checks["command_observed"])
            final = await adapter.evaluate("final", None)
            self.assertFalse(final.ok)
        finally:
            await adapter.close()

    async def test_fixture_schema_rejection_and_content_bound_identity(self):
        fixture = authored_fixture()
        path = self.fixture_path(fixture)
        adapter = BattleAdapter(ENGINE, fixture=path)
        self.assertEqual(adapter.identity["seed"], fixture["seed"])
        self.assertIn("fixture_sha256", adapter.identity)
        self.assertEqual(adapter.config["source"], fixture["schema"])
        await adapter.reset()
        try:
            observation = await adapter.observe()
            self.assertNotIn(fixture["id"], _encoded(observation.view).decode())
            self.assertNotIn(str(fixture["seed"]), _encoded(observation.view).decode())
            self.assertEqual(observation.view["party"][0]["hp"], 5)
        finally:
            await adapter.close()
        changed = authored_fixture()
        changed["party"]["hp"] = 6
        self.fixture_path(changed)
        with self.assertRaisesRegex(Stop, "adapter_identity_changed"):
            await adapter.reset()
        self.assertNotEqual(adapter.identity["fixture_sha256"],
                            BattleAdapter(ENGINE, fixture=path).identity["fixture_sha256"])

        invalid = []
        extra = authored_fixture()
        extra["ambush_chance"] = 1
        invalid.append(extra)
        zero_hp = authored_fixture()
        zero_hp["party"]["hp"] = 0
        invalid.append(zero_hp)
        too_many = authored_fixture()
        too_many["foes"] *= 3
        invalid.append(too_many)
        bad_seed = authored_fixture()
        bad_seed["seed"] = 1.5
        invalid.append(bad_seed)
        bool_attack = authored_fixture()
        bool_attack["foes"][0]["attack"] = True
        invalid.append(bool_attack)
        for index, value in enumerate(invalid):
            with self.subTest(index=index):
                self.fixture_path(value)
                result = subprocess.run([str(ENGINE), "--fixture", str(path), "--describe"],
                                        capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 2, result.stdout)
                with self.assertRaises((subprocess.CalledProcessError, ValueError)):
                    BattleAdapter(ENGINE, fixture=path)
        duplicate = '{"schema":"psiv-synthetic-battle-fixture-v1","id":"a","id":"b",' \
                    '"seed":1,"party":{"hp":5,"tp":18,"crosscut_uses":2},' \
                    '"foes":[{"hp":38,"attack":31,"defence":4,"agility":18,"dexterity":9}]}'
        self.fixture_path(duplicate)
        direct = subprocess.run([str(ENGINE), "--fixture", str(path), "--describe"],
                                capture_output=True, text=True, timeout=5)
        self.assertEqual(direct.returncode, 2, direct.stdout)
        with self.assertRaisesRegex(ValueError, "fixture_duplicate_key"):
            BattleAdapter(ENGINE, fixture=path)

    async def test_forged_nondefeat_skip_does_not_pass_verdict(self):
        adapter = BattleAdapter(ENGINE, "calm_single")
        await adapter.reset()
        try:
            observation = await adapter.observe()
            operation = next(candidate.operation for candidate in adapter.candidates(observation)
                             if candidate.id == "attack_6")
            await adapter.execute(operation)
            actual = await adapter._state()
            forged = json.loads(json.dumps(actual))
            forged["last"]["applied"] = False
            forged["last"]["skipped_reason"] = "incapacitated"

            async def forged_state():
                return forged

            adapter._state = forged_state
            verdict = await adapter.evaluate("after", operation)
            self.assertFalse(verdict.ok)
            self.assertFalse(verdict.checks["command_skipped"])
            self.assertFalse(verdict.checks["command_observed"])
        finally:
            await adapter.close()

    async def test_preemptive_ko_is_valid_loss_and_omission_remains_invalid(self):
        path = self.fixture_path(authored_fixture())
        for action in ("attack_6", "res_1", "rimit_all", "crosscut_6"):
            with self.subTest(action=action):
                adapter = BattleAdapter(ENGINE, fixture=path, baseline="attack")
                await adapter.reset()
                try:
                    observation = await adapter.observe()
                    operation = next(candidate.operation for candidate in adapter.candidates(observation)
                                     if candidate.id == action)
                    await adapter.execute(operation)
                    verdict = await adapter.evaluate("after", operation)
                    self.assertTrue(verdict.ok, verdict)
                    self.assertFalse(verdict.progress)
                    self.assertFalse(verdict.checks["comparison"]["complete"])
                    self.assertTrue(verdict.checks["command_skipped"])
                    self.assertEqual(verdict.checks["skipped_reason"], "defeat_before_turn")
                    self.assertEqual(verdict.checks["party_hp"], 0)
                    self.assertEqual(verdict.checks["party_tp"], 18)
                    self.assertEqual(verdict.checks["crosscut_uses"], 2)
                    observation = await adapter.observe()
                    self.assertEqual(observation.terminal, "defeat")
                    self.assertEqual(observation.view["last_effects"]["party_hp_delta"], -5)
                    self.assertEqual(observation.view["last_effects"]["party_hp_damage"], 5)
                    self.assertFalse(observation.view["last_effects"]["command_applied"])
                    final = await adapter.evaluate("final", None)
                    self.assertTrue(final.ok)
                    self.assertFalse(final.checks["comparison"]["complete"])
                finally:
                    await adapter.close()
        report = await run(BattleAdapter(ENGINE, fixture=path, baseline="attack"),
                           Path(self.tmp.name) / "ko_loss", provider=AttackOnlyBaseline(),
                           limits=LIMITS)
        self.assertTrue(report["cleanup"])
        self.assertTrue(report["final"]["ok"])
        self.assertFalse(report["final"]["checks"]["comparison"]["complete"])

    async def test_both_baselines_use_only_same_offered_request(self):
        offered = {"rimit_all": {}, "attack_7": {}, "attack_6": {}, "res_1": {},
                   "crosscut_7": {}, "defend": {}}
        view = {"party": [{"id": 1, "hp": 49, "max_hp": 100}],
                "enemies": [{"id": 6, "hp": 38}, {"id": 7, "hp": 43}]}
        request = {"observation": view, "candidates": offered}
        self.assertEqual(await ProjectedBioPlantBaseline().select(request), "res_1")
        self.assertEqual(await AttackOnlyBaseline().select(request), "attack_6")
        self.assertEqual(await AttackOnlyBaseline().select(
            {"observation": view, "candidates": {"defend": {}}}), "stop")
        self.assertNotEqual(BattleAdapter(ENGINE, "wounded_single").identity["policy"],
                            BattleAdapter(ENGINE, "wounded_single", baseline="attack")
                            .identity["policy"])
        self.assertEqual(BattleAdapter(ENGINE, "wounded_single", baseline="threat")
                         .identity["policy"], ThreatSkillBaseline.name)

    async def test_rimit_is_additional_effective_control_with_real_tp_cost(self):
        adapter = BattleAdapter(ENGINE, "calm_single")
        await adapter.reset()
        try:
            observation = await adapter.observe()
            operation = next(candidate.operation for candidate in adapter.candidates(observation)
                             if candidate.id == "rimit_all")
            await adapter.execute(operation)
            verdict = await adapter.evaluate("after", operation)
            self.assertTrue(verdict.ok)
            self.assertTrue(verdict.progress)
            self.assertTrue(verdict.checks["effective_control"])
            self.assertEqual(verdict.checks["party_tp"], observation.view["party"][0]["tp"] - 10)
            self.assertEqual(verdict.checks["enemy_damage"], 0)
        finally:
            await adapter.close()

    async def test_baseline_preflight_all_cases_and_one_concrete_replay(self):
        baseline = ProjectedBioPlantBaseline()
        for case in CASES:
            with self.subTest(case=case):
                output = Path(self.tmp.name) / case
                report = await run(BattleAdapter(ENGINE, case), output,
                                   provider=baseline, limits=LIMITS)
                self.assertEqual(report["stop"], "victory", report)
                self.assertTrue(report["cleanup"])
                self.assertTrue(report["final"]["ok"])
                self.assertTrue(report["final"]["checks"]["comparison"]["complete"])
                self.assertLessEqual(report["final"]["checks"]["rounds"], 12)
                self.assertGreater(report["final"]["checks"]["party_hp"], 0)
                self.assertTrue(report["replayable"])
        replay = json.loads((Path(self.tmp.name) / "wounded_pair" / "replay.json").read_text())
        result = await run(BattleAdapter(ENGINE, "wounded_pair"),
                           Path(self.tmp.name) / "wounded_pair_replay",
                           replay=replay, limits=LIMITS)
        self.assertTrue(result["replay_complete"], result)
        self.assertEqual(result["requests"], 0)
        self.assertEqual(result["stop"], "replay_complete")

    async def test_typesafe_environment_is_excluded_from_probe_children(self):
        with mock.patch.dict(os.environ, {"TYPESAFE_API_KEY": "sentinel",
                                       "TYPESAFE_TEST_SECRET": "sentinel"}):
            environment = clean_env()
            self.assertNotIn("TYPESAFE_API_KEY", environment)
            self.assertNotIn("TYPESAFE_TEST_SECRET", environment)


if __name__ == "__main__":
    unittest.main()
