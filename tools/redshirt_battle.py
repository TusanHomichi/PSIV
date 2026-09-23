"""Redshirt consumer for authored psiv-core battle-decision cases.

One operation executes one complete Battle::round in the Rust probe. This is a
projected RES/CROSSCUT/attack slice of tools/native_bioplant.gd's fixed policy,
not the full five-character policy and not ordinary native menu input. RIMIT
and Defend are legal additional choices. The fixture is synthetic, not a
retail encounter; no ROM, pack, save, dialogue, image or future RNG is sent to
the selector. Redshirt owns sequencing, budgets, provider calls and replay.
"""

from __future__ import annotations

import argparse
import asyncio
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import subprocess

from redshirt.adapter_stdio import serve_stdio
from redshirt.runner import Candidate, Observation, Stop, Verdict


SCHEMA = "psiv-redshirt-battle-adapter-v3"
PROBE_SCHEMA = "psiv-synthetic-battle-v1"
FIXTURE_SCHEMA = "psiv-synthetic-battle-fixture-v1"
POLICY = "projected-bioplant-res-crosscut-attack-v1"
ATTACK_POLICY = "attack-only-first-offered-v1"
THREAT_POLICY = "visible-threat-skill-heuristic-v1"

# Explanatory text, not an alternative combat implementation. Audited against
# the synthetic example and battle/{action,damage,order,skill,technique}.rs;
# see docs/REDSHIRT_BATTLE.md's mechanics-briefing source audit. These numerical
# ranges apply to this example's fixed equipment/stats, not arbitrary PSIV play.
MECHANICS_V1 = {
    "scope": "Rules of this synthetic battle only. One Pilot, max HP 100, attack 24, "
             "mental 14, agility/dexterity 18, defence 23. Enemies use plain attacks. "
             "Resources do not regenerate during this battle. "
             "Ranges are bounds, not forecasts; future random rolls are unknown.",
    "turns": "In a mixed round, each queued able fighter can act at most once, "
             "in descending agility plus "
             "random jitter 0..floor(highest queued agility/2)-1; ties use lower fighter ID. "
             "The opening may give the party a free round. Killing or sleeping a foe before "
             "its turn prevents that attack. If Pilot dies before acting, the selected "
             "command does nothing and costs nothing. Victory needs every foe dead within 12 rounds.",
    "attack": "Free single-target attack can miss or crit. For either side, r is a hidden "
              "0..63 roll: r+attacker.dexterity-target.agility <=4 misses, >58 crits, "
              "otherwise normal. Let S be the sum of 16 hidden 0..7 draws (0..112), "
              "A=attacker.attack, D=target.defence. Damage on hit is "
              "floor((floor((S+8)*A/64)+A+2*bonus)*factor/4)-D, clamped 1..999. "
              "bonus=0 normally or floor(A/4) on crit; factor=2 normally. "
              "Pilot normal damage is 13..34 minus target defence; crit adds 6 before clamping.",
    "crosscut": "Costs one skill use and zero TP. Up to TWO damage applications to the "
                "same target, with NO ordinary accuracy/miss/crit check. Each uses the "
                "damage formula with A=24, bonus=80, factor=2: 93..114 minus target "
                "defence per hit, clamped 1..999. Each hit rolls separately. If the first "
                "kills, the second is skipped and does not retarget.",
    "res": "Costs 3 TP; restores 23..36 HP (30 at S=56), capped at max HP. "
           "No miss roll. Healing occurs on Pilot's turn, not when selected.",
    "rimit": "Costs 10 TP once to attempt sleep on each living enemy, no damage. "
             "With these mental stats/resistances it succeeds if a hidden 0..63 roll >18 "
             "(45 of 64 roll values, not a guarantee). Already sleeping targets are unaffected. "
             "Sleep prevents action. Each sleeper may wake at round end, including the "
             "casting round, if a fresh roll is odd; sleep has no fixed duration.",
    "defend": "Free; changes incoming physical factor from 2 to 1 AFTER Pilot acts "
              "until round end. It halves the damage-formula term BEFORE subtracting "
              "defence, not final HP loss. It cannot protect against a faster attack "
              "that already landed; it deals no damage and restores no HP or resources.",
}


def _encoded(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def _hash(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def clean_env() -> dict[str, str]:
    """Never pass a TypeSafe credential into the adapter's probe children."""
    return {key: value for key, value in os.environ.items() if not key.startswith("TYPESAFE_")}


def _unique_pairs(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("fixture_duplicate_key")
        result[key] = value
    return result


def _fixture_bytes(path: Path) -> bytes:
    if path.stat().st_size > 4096:
        raise ValueError("fixture_too_large")
    raw = path.read_bytes()
    if len(raw) > 4096:
        raise ValueError("fixture_too_large")
    return raw


class BattleAdapter:
    setup_mode = "reset"

    def __init__(self, engine: Path, case: str | None = None, *,
                 fixture: Path | None = None, baseline: str = "projected",
                 briefing: str = "minimal"):
        if (case is None) == (fixture is None):
            raise ValueError("choose_case_or_fixture")
        if baseline not in {"projected", "attack", "threat"}:
            raise ValueError("invalid_baseline")
        if briefing not in {"minimal", "mechanics-v1"}:
            raise ValueError("invalid_briefing")
        self.briefing = briefing
        self.engine = engine.resolve(strict=True)
        self.fixture = fixture.resolve(strict=True) if fixture is not None else None
        self.fixture_hash = None
        authored = None
        if self.fixture is not None:
            raw = _fixture_bytes(self.fixture)
            self.fixture_hash = _hash(raw)
            authored = json.loads(raw, object_pairs_hook=_unique_pairs)
        self._probe_args = (["--fixture", str(self.fixture)] if self.fixture is not None
                            else ["--case", case])
        self.engine_hash = _hash(self.engine.read_bytes())
        self.adapter_hash = _hash(Path(__file__).read_bytes())
        description = subprocess.run(
            [str(self.engine), *self._probe_args, "--describe"],
            env=clean_env(), stdin=subprocess.DEVNULL, capture_output=True,
            check=True, timeout=5,
        )
        config = json.loads(description.stdout)
        if (not isinstance(config, dict) or config.get("schema") != PROBE_SCHEMA
                or config.get("id") != (case if case is not None else authored.get("id"))
                or config.get("round_limit") != 12
                or (self.fixture is not None and config.get("source") != FIXTURE_SCHEMA)):
            raise ValueError("invalid_probe_config")
        if authored is not None:
            if (config["seed"] != authored["seed"]
                    or {key: config["party"][key] for key in ("hp", "tp", "crosscut_uses")}
                    != authored["party"]
                    or [{key: foe[key] for key in ("hp", "attack", "defence", "agility", "dexterity")}
                        for foe in config["foes"]] != authored["foes"]):
                raise ValueError("fixture_config_mismatch")
        if not self._identity_unchanged():
            raise ValueError("adapter_identity_changed")
        self.case = config["id"]
        self.config = config
        self.identity = {
            "schema": SCHEMA,
            "engine_sha256": self.engine_hash,
            "adapter_sha256": self.adapter_hash,
            "case": self.case,
            "seed": config["seed"],
            "config_sha256": _hash(_encoded(config)),
            "policy": {"projected": POLICY, "attack": ATTACK_POLICY,
                       "threat": THREAT_POLICY}[baseline],
            "briefing": briefing,
        }
        if self.fixture_hash is not None:
            self.identity["fixture_sha256"] = self.fixture_hash
        self.process: asyncio.subprocess.Process | None = None
        self._observed: dict | None = None
        self._initial: dict | None = None
        self._last_checked: dict | None = None
        self._pending: tuple[dict, dict] | None = None
        self._failed = False
        self._ever_ko = False

    def _identity_unchanged(self) -> bool:
        try:
            return (_hash(self.engine.read_bytes()) == self.engine_hash
                    and _hash(Path(__file__).read_bytes()) == self.adapter_hash
                    and (self.fixture is None
                         or _hash(_fixture_bytes(self.fixture)) == self.fixture_hash))
        except (OSError, ValueError):
            return False

    async def _rpc(self, request: dict) -> dict:
        process = self.process
        if process is None or process.returncode is not None or process.stdin is None or process.stdout is None:
            raise Stop("probe_unavailable")
        wire = _encoded(request) + b"\n"
        if len(wire) > 4096:
            raise Stop("probe_request_size")
        try:
            process.stdin.write(wire)
            await asyncio.wait_for(process.stdin.drain(), 3)
            raw = await asyncio.wait_for(process.stdout.readline(), 3)
        except (OSError, asyncio.TimeoutError, ValueError) as exc:
            raise Stop("probe_io") from exc
        if not raw or len(raw) > 32768:
            raise Stop("probe_reply_size")
        try:
            response = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise Stop("probe_reply_json") from exc
        if not isinstance(response, dict) or type(response.get("ok")) is not bool:
            raise Stop("probe_reply_shape")
        return response

    async def _state(self) -> dict:
        response = await self._rpc({"op": "state"})
        if response.get("ok") is not True or not isinstance(response.get("state"), dict):
            raise Stop("probe_state")
        return response["state"]

    async def reset(self) -> None:
        await self.close()
        if not self._identity_unchanged():
            raise Stop("adapter_identity_changed")
        self.process = await asyncio.create_subprocess_exec(
            str(self.engine), *self._probe_args,
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL, env=clean_env(),
        )
        response = await self._rpc({"op": "reset"})
        if response.get("ok") is not True or not isinstance(response.get("state"), dict):
            raise Stop("probe_reset")
        if not self._identity_unchanged():
            raise Stop("adapter_identity_changed")
        self._initial = response["state"]
        self._observed = None
        self._last_checked = self._initial
        self._pending = None
        self._failed = False
        self._ever_ko = False

    async def observe(self) -> Observation:
        state = await self._state()
        self._observed = state
        view = {
            "goal": "Win without party KO; then conserve TP and skill uses.",
            "step": "One selected command resolves a full round; enemies use plain attacks.",
            "status_bits": {"poison": 1, "paralysis": 2, "knocked_out": 4,
                            "sleep": 8, "techniques_sealed": 16,
                            "secondary_sleep": 32, "android_knocked_out": 64},
            "round": state["round"], "round_limit": state["round_limit"],
            "party": state["party"], "enemies": state["enemies"],
            "last_effects": ({"selected": state["last"]["selected"],
                              "command_applied": state["last"]["applied"],
                              "skipped_reason": state["last"]["skipped_reason"],
                              "party_hp_delta": state["last"]["party_hp_delta"],
                              "party_hp_damage": state["last"]["party_hp_damage"],
                              "enemy_hp_lost": state["last"]["enemy_hp_lost"],
                              "healed": state["last"]["healed"],
                              "sleep_effective": state["last"]["sleep_effective"]}
                             if state["last"] is not None else None),
        }
        if self.briefing == "mechanics-v1":
            view["rules"] = dict(MECHANICS_V1)
        if len(_encoded(view)) > 4096:
            raise Stop("observation_size")
        terminal = None
        if state["outcome"] != "ongoing":
            terminal = state["outcome"]
        elif state["round"] >= state["round_limit"]:
            terminal = "round_limit"
        return Observation(
            environment=f"psiv-synthetic-battle:{self.engine_hash}",
            epoch=self.identity["config_sha256"],
            guard=f"{state['revision']}:{_hash(_encoded(state))}",
            view=view, ready=terminal is None, terminal=terminal,
        )

    def candidates(self, observation: Observation) -> list[Candidate]:
        state = self._observed
        if state is None or observation.guard != f"{state['revision']}:{_hash(_encoded(state))}":
            raise Stop("candidate_state")
        if not observation.ready:
            return []
        return [Candidate(entry["id"], entry["description"],
                          {"action": entry["id"], "revision": state["revision"]})
                for entry in state["legal"]]

    async def verify(self) -> None:
        if not self._identity_unchanged():
            raise Stop("adapter_identity_changed")
        if self._observed is None or await self._state() != self._observed:
            raise Stop("stale_probe_state")

    async def execute(self, operation: dict) -> dict:
        before = self._observed
        if (before is None or set(operation) != {"action", "revision"}
                or type(operation["revision"]) is not int
                or operation["revision"] != before["revision"]
                or operation["action"] not in {entry["id"] for entry in before["legal"]}):
            raise Stop("unavailable_action")
        if await self._state() != before:
            raise Stop("stale_probe_state")
        self._pending = (before, operation.copy())
        receipt = await self._rpc({"op": "step", **operation})
        if receipt.get("ok") is not True:
            raise Stop(receipt.get("error", "probe_step_failed"))
        if receipt.get("acknowledged") is not True:
            raise Stop("probe_receipt")
        return {"acknowledged": True, "revision": receipt.get("revision")}

    @staticmethod
    def _healthy(state: dict) -> bool:
        return bool(state["party"]) and all(
            fighter["hp"] > 0 and fighter["status"] & 0x44 == 0
            for fighter in state["party"]
        )

    @staticmethod
    def _valid_state(state: dict) -> bool:
        if (state.get("schema") != "psiv-synthetic-battle-state-v1"
                or state.get("outcome") not in {"ongoing", "victory", "defeat"}
                or state.get("round_limit") != 12
                or not isinstance(state.get("party"), list)
                or not isinstance(state.get("enemies"), list)):
            return False
        for fighter in state["party"] + state["enemies"]:
            if (type(fighter.get("hp")) is not int or type(fighter.get("max_hp")) is not int
                    or not 0 <= fighter["hp"] <= fighter["max_hp"]
                    or any(type(fighter.get(key)) is not int or fighter[key] < 0
                           for key in ("attack", "defence", "agility", "dexterity", "mental"))):
                return False
        return True

    def _comparison(self, state: dict, useful: bool | None) -> dict:
        return {"complete": (not self._failed and not self._ever_ko
                             and state["outcome"] == "victory" and self._healthy(state)),
                "useful_action": useful}

    async def evaluate(self, phase: str, operation: dict | None) -> Verdict:
        state = await self._state()
        valid = self._valid_state(state) and state.get("case") == self.case
        self._ever_ko |= not valid or not self._healthy(state)
        if phase == "reset":
            initial = self._initial
            ok = bool(valid and initial == state and state["revision"] == 0
                      and state["round"] == 0 and state["outcome"] == "ongoing"
                      and self._healthy(state)
                      and state["party"][0]["hp"] == self.config["party"]["hp"]
                      and state["party"][0]["tp"] == self.config["party"]["tp"]
                      and state["party"][0]["crosscut_uses"] == self.config["party"]["crosscut_uses"]
                      and [foe["hp"] for foe in state["enemies"]]
                      == [foe["hp"] for foe in self.config["foes"]])
            self._failed |= not ok
            return Verdict(ok, False, {"comparison": self._comparison(state, None),
                                       "rounds": state["round"], "initial_state_ok": ok})
        if phase == "final":
            ok = bool(valid and self._pending is None and not self._failed
                      and self._last_checked == state)
            return Verdict(ok, False, {"comparison": self._comparison(state, None),
                                       "rounds": state["round"],
                                       "party_hp": state["party"][0]["hp"],
                                       "party_tp": state["party"][0]["tp"],
                                       "crosscut_uses": state["party"][0]["crosscut_uses"],
                                       "enemy_hp_total": sum(f["hp"] for f in state["enemies"])})
        if phase != "after" or self._pending is None or operation != self._pending[1]:
            self._failed = True
            return Verdict(False, False, {"comparison": self._comparison(state, False),
                                          "operation_match": False})
        before, selected = self._pending
        self._pending = None
        action = selected["action"]
        former = before["party"][0]
        pilot = state["party"][0]
        last = state.get("last") or {}
        advance = (state["revision"] == before["revision"] + 1
                   and state["round"] == before["round"] + 1
                   and state["round"] <= 12)
        roster = ([f["id"] for f in before["party"]] == [f["id"] for f in state["party"]]
                  and [f["id"] for f in before["enemies"]] == [f["id"] for f in state["enemies"]]
                  and all(a["max_hp"] == b["max_hp"] for a, b in zip(before["party"] + before["enemies"],
                                                                     state["party"] + state["enemies"])))
        enemy_damage = sum(a["hp"] - b["hp"] for a, b in zip(before["enemies"], state["enemies"]))
        monotone = all(a["hp"] >= b["hp"] for a, b in zip(before["enemies"], state["enemies"]))
        skipped_reason = last.get("skipped_reason")
        # The authored foes only plain-attack. The sole accepted no-cost skip
        # here is their preemptive KO, with a terminal core defeat and HP zero.
        skipped = (last.get("applied") is False and last.get("rejected") is False
                   and skipped_reason in {"dead", "defeat_before_turn"}
                   and state["outcome"] == "defeat" and pilot["hp"] == 0)
        expected_tp = (0 if skipped else
                       3 if action.startswith("res_") else 10 if action == "rimit_all" else 0)
        expected_skill = 0 if skipped else 1 if action.startswith("crosscut_") else 0
        resources = (former["tp"] - pilot["tp"] == expected_tp
                     and former["crosscut_uses"] - pilot["crosscut_uses"] == expected_skill)
        effects_match = (type(last.get("healed")) is int and last["healed"] >= 0
                         and last.get("party_hp_delta") == pilot["hp"] - former["hp"]
                         and last.get("party_hp_damage")
                         == former["hp"] + last["healed"] - pilot["hp"]
                         and last.get("enemy_hp_lost") == enemy_damage
                         and type(last.get("sleep_effective")) is bool)
        command_seen = (last.get("revision") == state["revision"]
                        and last.get("selected") == action
                        and last.get("rejected") is False
                        and ((last.get("applied") is True and skipped_reason is None)
                             or skipped))
        useful = bool(enemy_damage > 0 or last.get("healed", 0) > 0
                      or last.get("sleep_effective") is True)
        ok = bool(valid and advance and roster and monotone and resources
                  and command_seen and effects_match)
        self._failed |= not ok
        self._last_checked = state
        return Verdict(ok, useful, {"comparison": self._comparison(state, useful),
                                    "rounds": state["round"], "party_hp": pilot["hp"],
                                    "party_tp": pilot["tp"],
                                    "crosscut_uses": pilot["crosscut_uses"],
                                    "enemy_hp_total": sum(f["hp"] for f in state["enemies"]),
                                    "enemy_damage": enemy_damage,
                                    "healed": last.get("healed", 0),
                                    "effective_control": last.get("sleep_effective", False),
                                    "round_advanced": advance, "roster_ok": roster,
                                    "resource_costs_ok": resources,
                                    "command_observed": command_seen,
                                    "command_skipped": skipped,
                                    "skipped_reason": skipped_reason,
                                    "effects_match": effects_match})

    async def close(self) -> None:
        process, self.process = self.process, None
        if process is None:
            return
        try:
            if process.returncode is None and process.stdin and process.stdout:
                process.stdin.write(b'{"op":"close"}\n')
                await asyncio.wait_for(process.stdin.drain(), 1)
                await asyncio.wait_for(process.stdout.readline(), 1)
                await asyncio.wait_for(process.wait(), 1)
        except (OSError, asyncio.TimeoutError, ValueError):
            if process.returncode is None:
                try:
                    process.kill()
                except ProcessLookupError:
                    pass
        finally:
            if process.returncode is None:
                try:
                    process.kill()
                except ProcessLookupError:
                    pass
            await process.wait()


class ProjectedBioPlantBaseline:
    """Only the one-actor RES/CROSSCUT/attack branches of native_bioplant.gd.

    No SANER, GELUN, BROSE, CRASH, VORTEX, WAT, multi-actor heal coordination,
    item or camp choices are representable here. RIMIT and Defend remain legal
    model candidates, but the native fixed policy does not choose them.
    """

    name = POLICY

    async def select(self, request: dict) -> str:
        # This sees only the exact selector request, not a probe or adapter.
        view = request["observation"]
        offered = request["candidates"]
        living = [foe for foe in view["enemies"] if foe["hp"] > 0]
        if not living:
            return "stop"
        pilot = view["party"][0]
        if pilot["hp"] * 10 < pilot["max_hp"] * 7:
            choice = f"res_{pilot['id']}"
            if choice in offered:
                return choice
        strongest = max(living, key=lambda foe: (foe["hp"], foe["id"]))
        choice = f"crosscut_{strongest['id']}"
        if strongest["hp"] >= 40 and choice in offered:
            return choice
        weakest = min(living, key=lambda foe: (foe["hp"], foe["id"]))
        choice = f"attack_{weakest['id']}"
        return choice if choice in offered else "stop"


class AttackOnlyBaseline:
    """First offered attack ID in lexical order, with no probe-only knowledge."""

    name = ATTACK_POLICY

    async def select(self, request: dict) -> str:
        return next((choice for choice in sorted(request["candidates"])
                     if choice.startswith("attack_")), "stop")


class ThreatSkillBaseline:
    """Visible-state heuristic for this synthetic solo battle, not a simulator.

    Incoming/outgoing hits and RES use representative values, not forecasts or
    calibrated probabilities. Initiative jitter, misses/crits, variable healing,
    RIMIT failure and wake-up can all defeat the estimate. Only the selector's
    observation and offered candidate IDs are read; Rust resolves every round.
    """

    name = THREAT_POLICY

    async def select(self, request: dict) -> str:
        view = request["observation"]
        offered = request["candidates"]
        if not offered:
            raise ValueError("no_candidates")
        fallback = "stop" if "stop" in offered else min(offered)
        living = [foe for foe in view["enemies"] if foe["hp"] > 0]
        if not living:
            return fallback
        pilot = view["party"][0]
        bits = view["status_bits"]
        sleep_mask = bits["sleep"] | bits["secondary_sleep"]
        awake = [foe for foe in living if (foe["status"] & sleep_mask) == 0]
        threat = {foe["id"]: max(1, foe["attack"] - pilot["defence"])
                  if (foe["status"] & sleep_mask) == 0 else 0 for foe in living}
        incoming = sum(threat.values())
        largest = max(threat.values(), default=0)

        def priority(foe: dict) -> tuple[int, int, int, int]:
            return (threat[foe["id"]], foe["agility"], -foe["hp"], -foe["id"])

        # A nominal minimum-damage, no-normal-miss kill saves a charge only
        # when other awake foes leave a conservative current-round HP margin.
        ordinary_kills = [foe for foe in living
                          if f"attack_{foe['id']}" in offered
                          and pilot["dexterity"] - foe["agility"] > 4
                          and foe["hp"] <= max(1, 13 - foe["defence"])
                          and (len(living) == 1
                               or pilot["hp"] > 2 * (incoming - threat[foe["id"]]))]
        if ordinary_kills:
            return f"attack_{max(ordinary_kills, key=priority)['id']}"

        charged = ([foe for foe in living if f"crosscut_{foe['id']}" in offered]
                   if pilot["crosscut_uses"] > 0 else [])
        guaranteed_skill_kills = [foe for foe in charged
                                  if foe["hp"] <= 2 * max(1, 93 - foe["defence"])]
        if guaranteed_skill_kills:
            return f"crosscut_{max(guaranteed_skill_kills, key=priority)['id']}"

        danger = pilot["hp"] <= incoming + largest
        # With two acting foes, representative RES healing (30 HP) cannot
        # outrun >=30 incoming. This narrow RIMIT attempt is still fallible.
        if (not any(choice.startswith("crosscut_") for choice in offered)
                and len(awake) == 2 and incoming >= 30 and danger
                and all(pilot["agility"] >= foe["agility"] for foe in awake)
                and pilot["tp"] >= 10 and "rimit_all" in offered):
            return "rimit_all"

        missing_hp = pilot["max_hp"] - pilot["hp"]
        if ("res_" + str(pilot["id"]) in offered and pilot["tp"] >= 3
                and ((danger and missing_hp >= 23) or pilot["hp"] < incoming)):
            return f"res_{pilot['id']}"

        if charged:
            return f"crosscut_{max(charged, key=priority)['id']}"

        attacks = [foe for foe in living if f"attack_{foe['id']}" in offered]
        if not attacks:
            return fallback

        def attack_priority(foe: dict) -> tuple[Fraction, int, int, int, int]:
            representative = max(1, 24 - foe["defence"])
            needed = (foe["hp"] + representative - 1) // representative
            return (Fraction(threat[foe["id"]], needed), *priority(foe))

        return f"attack_{max(attacks, key=attack_priority)['id']}"


async def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--case")
    source.add_argument("--fixture", type=Path)
    parser.add_argument("--baseline", choices=("projected", "attack", "threat"),
                        default="projected")
    parser.add_argument("--briefing", choices=("minimal", "mechanics-v1"), default="minimal")
    args = parser.parse_args()
    # Redshirt's Rust controller also strips this variable before spawning
    # the trusted worker. Keep direct/test invocation from retaining it.
    for key in tuple(os.environ):
        if key.startswith("TYPESAFE_"):
            os.environ.pop(key)
    adapter = BattleAdapter(args.engine, args.case, fixture=args.fixture, baseline=args.baseline,
                            briefing=args.briefing)
    provider = {"projected": ProjectedBioPlantBaseline,
                "attack": AttackOnlyBaseline,
                "threat": ThreatSkillBaseline}[args.baseline]()
    await serve_stdio(adapter, provider=provider)


if __name__ == "__main__":
    asyncio.run(main())
