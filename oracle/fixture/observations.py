"""What the RAM log shows: the fighters, the queue, and one action's effects.

Everything here reads columns the log actually carries, at the width
`oracle/ram_map.json` gives them. The one convention worth stating up front:
HP is a signed word, so a dead fighter reads 65511 for -25, and `Log.signed`
is what turns the rendering back into the number the cartridge holds.
"""
from .errors import FixtureError

#: `loc_266C` clamps every stored damage to this range, so a
#: `Battle_Heal_Damage_List` word outside it was not written by a damage call:
#: `loc_B6A2`'s nine-word clear leaves `$FFFF` over the first four words, which
#: is why a party action reads `$FFFF` in the party's own slots.
DAMAGE_STORED = (1, 999)

#: The roll table's columns, in order. One row per call: `[frame, roll, role,
#: target, pass, round, action]`. `role` is what the call was for - `priority`,
#: `order`, `hit`, `damage`, `ability`, `ability_reroll`, `formation` or
#: `item_drop` - `target` the fighter id a hit or damage call was aimed at (and
#: `null` otherwise), `pass` which `loc_B6A2` pass a hit roll belongs to, and
#: `round`/`action` the round and acting fighter it sits under. How each role
#: is derived is `oracle/fixture/roles.py`'s subject.
ROLL_COLUMNS = ["frame", "roll", "role", "target", "pass", "round", "action"]

PARTY_NAMES = ["level", "hp", "maxhp", "tp", "maxtp", "status", "str",
               "agi", "agi_bat", "dex", "atk", "dfs", "men", "exp"]
PARTY_IDS = [("alys", 1), ("chaz", 2), ("hahn", 3)]
ENEMY_NAMES = ["id", "hp", "maxhp", "status", "agi_bat", "atk", "dfs",
               "str_bat", "men_bat", "dex_bat"]

#: The log maps three named party members and four numbered enemy slots, so
#: the one-based fighter ids that have an HP column are 1..=3 and 6..=9. A
#: vehicle battle's party-side fighter is id 1 and its HP lives in another
#: column entirely (`vehicle_fighter_hp`); `oracle/fixture/vehicle.py` is that
#: section.
HP_COLUMNS = {1: "alys_hp", 2: "chaz_hp", 3: "hahn_hp",
              6: "e1_hp", 7: "e2_hp", 8: "e3_hp", 9: "e4_hp"}


def hp_column(fighter_id):
    """The log's HP column for a one-based fighter id, when it has one."""
    return HP_COLUMNS.get(fighter_id)


def side_of(fighter_id):
    """`cmpi.w #5, d0 / bgt`: ids 1..=5 are the party, 6..=9 the enemies."""
    return "party" if fighter_id <= 5 else "enemy"


def stat_block(log, frame, prefix, names):
    """{field: value} for one fighter's columns at `frame`."""
    return {name: log.num(frame, f"{prefix}_{name}") for name in names}


def battle_start(log, frame):
    """The party's and enemies' live state at the battle's first frame."""
    party = []
    for prefix, index in PARTY_IDS:
        block = stat_block(log, frame, prefix, PARTY_NAMES)
        party.append({
            "id": index,
            "name": prefix.upper(),
            "level": block["level"],
            "hp": block["hp"],
            "max_hp": block["maxhp"],
            "tp": block["tp"],
            "max_tp": block["maxtp"],
            "status": block["status"],
            "strength": block["str"],
            "mental": block["men"],
            "agility": block["agi_bat"],
            "dexterity": block["dex"],
            "attack": block["atk"],
            "defence": block["dfs"],
            "experience": block["exp"],
        })
    enemies = []
    for slot in range(1, 5):
        max_hp = log.num(frame, f"e{slot}_maxhp")
        hp = log.num(frame, f"e{slot}_hp")
        # An empty slot holds zeros; enemy id `$00` is a real enemy (Helex),
        # so occupancy is the record holding together, not the id.
        if not max_hp or not 0 < hp <= max_hp:
            continue
        block = stat_block(log, frame, f"e{slot}", ENEMY_NAMES)
        enemies.append({
            "formation_slot": slot,
            "id": slot + 5,
            "enemy_id": block["id"],
            "hp": block["hp"],
            "max_hp": block["maxhp"],
            "status": block["status"],
            "strength": block["str_bat"],
            "mental": block["men_bat"],
            "agility": block["agi_bat"],
            "dexterity": block["dex_bat"],
            "attack": block["atk"],
            "defence": block["dfs"],
        })
    return party, enemies


#: The fighters `Battle_Turn_Order` holds: one id/agility pair per
#: `Obj_Fighters` slot. `Battle_OrderTurns` runs its insertion loop and its
#: agility passes over nine slots (`moveq #8, d7`, ps4.asm:7757 and 7789) and
#: sorts nine entries (ps4.asm:7793-7801), and `oracle/ram_map.json` logs all
#: nine pairs as `turn_00`..`turn_17`. A log written before that window existed
#: carries six pairs, and `turn_order` reads exactly the ones it has.
ORDER_ENTRIES = 9


def turn_order(log, frame):
    """`Battle_Turn_Order` at `frame`: [(fighter id, ordering value)].

    The queue, as far as the log's own columns go: a capture with seven or more
    fighters needs the window past `turn_11`, or the entries it cannot see are
    simply missing from the fixture (`docs/BATTLE_ORACLE_SWEEP.md`).
    """
    order = []
    for index in range(0, 2 * ORDER_ENTRIES, 2):
        if not log.has(f"turn_{index:02d}"):
            break
        fighter = log.num(frame, f"turn_{index:02d}")
        if fighter:
            order.append((fighter, log.num(frame, f"turn_{index + 1:02d}")))
    return order


def round_frames(log, first, last):
    """The frames where `Battle_Turn_Order` is (re)filled: the round starts."""
    starts = []
    columns = [f"turn_{index:02d}" for index in range(2 * ORDER_ENTRIES)]
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        if any(log.changed(frame, column) for column in columns):
            starts.append(frame)
    return starts


def action_windows(log, first, last, cuts=(), roll_frames=()):
    """[(actor, start, end)]: one per turn the turn engine ran.

    `$FFFF4142` holds a one-based fighter index only while the turn engine is
    driving an action; the command phase leaves other numbers in it, so a
    window opens only on a value that names a fighter slot.

    Three things the field alone cannot say, and the caller's `cuts` and
    `roll_frames` are what settle them:

    * **A round's queue build is a boundary.** An actor that acts last in one
      round and first in the next has the same id in the field throughout, and
      its two turns are still two actions: each cut closes the window that is
      open and the next one opens after it (`cuts` are the frames
      `Battle_Turn_Order` is filled on).
    * **The field holds the *previous* actor until the next turn starts.** The
      frames between a queue build and the first action of the round read the
      id of whoever acted last, so a window must not open on that value - the
      Desrt Leach capture's rounds 3-5 read `6` for five frames before the
      vehicle's turn, and an ability byte the enemy had left behind made one of
      them look like a wasted ability.
    * **A turn that draws nothing is not an action.** `loc_576A` writes the
      queue entry into `$FFFF4142` and only then tests the fighter's status
      (`ps4.asm:8033-8043`), so an actor that fell before its turn arrived
      leaves the field holding *its* id for the rest of the round - a dead
      enemy from f25680 to the next queue build, with no call and no flag
      written in any of those frames. A window opens only where the action's
      own calls show up (`roll_frames` are the frames the trace drew in), which
      is what makes those frames the round's tail rather than a turn.

    The distinction is not bookkeeping: the port has no turn there either, and
    a window built on one asks it to resolve an action the cartridge never ran
    (`docs/BATTLE_ORACLE_SWEEP.md`'s no-swing and phantom clusters).
    """
    cuts = set(cuts)
    rolls = set(roll_frames)
    windows = []
    actor = None
    for frame in range(first, last + 1):
        row = log.by_frame.get(frame)
        if row is None:
            continue
        if frame in cuts:
            if windows:
                windows[-1][2] = min(windows[-1][2], frame - 1)
            actor = None
            continue
        current = log.num(frame, "battle_actor")
        if not 1 <= current <= 9:
            continue
        if current == actor:
            continue
        if frame in rolls:
            if windows:
                windows[-1][2] = min(windows[-1][2], frame - 1)
            windows.append([current, frame, last])
            actor = current
    return [tuple(window) for window in windows]


def enemy_slots(log, frame):
    """The one-based fighter ids of the enemies the log shows at `frame`.

    An occupied slot is one whose record holds together - a maximum and a
    current HP inside it - not one whose id byte is nonzero: enemy id `$00` is
    a real enemy (Helex, formation `$5E`), while an empty slot holds zeros and
    the field's leftovers hold whatever the last map left there (tape 07's
    slot 2 reads 6956 of 7680 before the formation lands, which is why the
    current HP has to be inside the maximum).
    """
    slots = []
    for slot in range(1, 5):
        hp = log.num(frame, f"e{slot}_hp")
        max_hp = log.num(frame, f"e{slot}_maxhp")
        if max_hp and 0 < hp <= max_hp:
            slots.append(slot + 5)
    return slots


def enemies_loaded(log, first, last):
    """The frame the whole formation was written into RAM.

    Before it the enemy slots hold whatever the field left there - tape 07's
    slot 1 read 12063 over a maximum of 46 - so the test is a full, occupied
    record, not a nonzero byte (`enemy_slots`). That alone is not enough: the
    formation is written **one slot per frame** from `Fighter_Enemy_1`
    (`ps4.asm:11952`), so the frame the *first* record lands on holds a
    one-enemy formation that the cartridge never fought. `enemy_count` is the
    number the load wrote for the whole formation, and it is written before the
    records are: the start frame is the first one whose occupied slots are all
    of them.

    The sweep's formation `$13` is the shape: `enemy_count` reads 2 from
    f24821, that frame holds one SandNewt record and f24822 holds both. The
    fixture built on f24821 seated one enemy, so its port-side queue, its rolls
    and its whole round were a different battle from the cartridge's
    (`docs/BATTLE_ORACLE_SWEEP.md`, the queue cluster).

    A log with no such frame is a capture this extractor cannot start: the
    count and the records never agreed, and guessing between them would seat a
    formation the cartridge did not fight.
    """
    seen = None
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        count = log.num(frame, "enemy_count")
        if count <= 0:
            # A frame before the load holds no records to read: the cells are
            # the field's leftovers, and a capture that does not carry them at
            # all would raise on the read.
            continue
        slots = enemy_slots(log, frame)
        if not slots:
            continue
        if seen is None or len(slots) > seen[2]:
            seen = (frame, count, len(slots))
        if len(slots) == count:
            return frame
    if seen is not None:
        frame, count, occupied = seen
        raise FixtureError(
            f"the formation never loads whole between {first} and {last}: "
            f"enemy_count reads {count} from f{frame} but no frame holds more "
            f"than {occupied} intact record(s)")
    raise FixtureError(f"no formation is loaded between {first} and {last}")


def wiped_out(log, slots, first, last, columns=None):
    """The frame the last of `slots` reads zero or less, else None.

    One side's wipe is what decides a battle: the enemies' wipe is a victory
    and the party side's is a defeat, so the frame is the battle's end either
    way and the tape's own tail (the game-over or victory sequence) is not part
    of it.

    `columns` is the fighter-id to HP-column map; a vehicle battle replaces the
    party side's slot 1 with `vehicle_fighter_hp`
    (`oracle/fixture/vehicle.py`), because the members' columns never move
    there.
    """
    columns = HP_COLUMNS if columns is None else columns
    for frame in range(first, last + 1):
        if frame not in log.by_frame:
            continue
        if all(log.signed(frame, columns[slot]) <= 0 for slot in slots):
            return frame
    return None


def decided_frame(log, enemy_ids, party_ids, first, last, columns=None):
    """The frame the battle's outcome was decided, else None.

    `party_ids` is empty for a vehicle battle, whose party side is the vehicle
    rather than the members: `oracle/fixture/vehicle.py` supplies the frame the
    vehicle is wrecked and this takes whichever side fell first.
    """
    frames = []
    if enemy_ids:
        frames.append(wiped_out(log, enemy_ids, first, last, columns))
    if party_ids:
        frames.append(wiped_out(log, party_ids, first, last, columns))
    seen = [frame for frame in frames if frame is not None]
    return min(seen) if seen else None


def hit_flag_column(fighter_id):
    """The log's `Fighters_Hit_Flags` column for a one-based fighter id.

    The array is indexed by **slot** (`loc_B6A2` writes `-$1(a0,d6.w)` for a
    one-based `d6`), so fighter id 1 reads `hit_00` and enemy slot 1 - id 6 -
    reads `hit_05`.
    """
    return f"hit_{fighter_id - 1:02d}"


#: The three bytes `loc_B6A2` can leave in a slot: `$00` and `$01` are the
#: verdicts of its chance roll, `$FF` is "not targeted, or missed". Anything
#: else is not a hit flag at all, which is how a frame whose bytes were written
#: by something else - `$FFFF4150` is battle scratch that the round's tail
#: reuses - is told apart from a pass.
HIT_FLAG_VALUES = ("00", "01")
NOT_TARGETED = "FF"

#: How many `Fighters_Hit_Flags` slots the log carries: ids 1..=9 are `d6`
#: 1..=9 in `st -$1(a0,d6.w)`, i.e. `hit_00`..`hit_08`.
HIT_FLAGS = 9


def hit_flags(log, frame):
    """The nine `Fighters_Hit_Flags` bytes, by fighter id, at `frame`.

    Slot 0 of the array is fighter id 1 (`hit_flag_column`), which is also how
    the log's columns are numbered: `hit_00` is id 1 and `hit_05` is id 6.
    """
    return [log.raw(frame, hit_flag_column(id_))
            for id_ in range(1, HIT_FLAGS + 1)]


def pass_frame(log, start, end, roll_frames):
    """The frame the action's last `loc_B6A2` pass left its verdicts on.

    `loc_B6A2` (`ps4.asm:17492`) presets all nine `Fighters_Hit_Flags` to `$FF`
    and then writes one byte per slot in the acting window
    (`st -$1(a0,d6.w)`), so everything a slot's byte can mean is decided by the
    action's **last** pass - the one `Fighter_TakeDamage` (`ps4.asm:3564-3571`)
    reads. A swing whose passes all land in one frame already holds that state;
    a vehicle swing draws one pass per frame (`loc_AF9C`: two for the Ice
    Digger, three for the others) and the later frames overwrite the earlier
    ones, which is why this is the *last* frame the flags moved on rather than
    the first.

    Not every frame the flags move on is a pass. `$FFFF4150` is battle scratch:
    when a round runs out of actors the bytes above `$FFFF4100` are reused, and
    the sweep's formation `$10` writes `$0605` over `$FFFF4142` and `03 06 04`
    into three hit slots at f25808. A pass is a frame that also **drew**: it
    rolls once per living slot in its window (`loc_B716`), or - for an ability,
    whose arm loads its own object - is the frame the action's own ability roll
    is on. So a frame counts only when it is one of the action's `roll_frames`
    *and* its nine bytes are all verdicts.

    Reading the state rather than the change is what makes a repeat visible: a
    pass that resolves the same slot with the verdict the previous action left
    there changes nothing (`formation_37`'s Hahn hits enemy 6 for 1 where Alys
    had just done the same), and a change-based read loses the target entirely.
    A window whose pass drew nothing - every slot in it already down - falls
    back to its own first roll frame, which for an ability is the frame its
    `Enemy_Attack` roll is on.
    """
    rolls = set(roll_frames)
    verdicts = HIT_FLAG_VALUES + (NOT_TARGETED,)
    found = None
    previous = hit_flags(log, start - 1) if start - 1 in log.by_frame else None
    for frame in range(start, end + 1):
        if frame not in log.by_frame:
            continue
        values = hit_flags(log, frame)
        if frame in rolls and previous is not None and values != previous \
                and all(value in verdicts for value in values):
            found = frame
        previous = values
    if found is not None:
        return found
    return next((frame for frame in range(start, end + 1)
                 if frame in rolls), start)


def action_record(log, actor, start, end, rolls, occupied, columns=None,
                  roll_frames=()):
    """One action: who acted, what the log shows, and the rolls it drew.

    Only the acting side's opponents can be its targets - and only the ones
    the formation actually seated, since an empty slot holds zeros that read as
    a corpse - which is what keeps `loc_B6A2`'s blanking of the hit flags
    (`$FF` over every slot, `$FFFF` over the first four damage words) from
    reading as an observation about the actor's own side. `columns` is the
    fighter-id to HP-column map (see `wiped_out`).

    A target is a slot the action's **pass** resolved - its byte at
    [`pass_frame`] is `$00` or `$01` - or one whose damage word moved, which is
    the same claim read the other way round (the damage routine writes only the
    slots `Fighter_TakeDamage` applies to). The `hit` is that byte: the last
    pass's verdict, which is the byte `Character_Attack`'s second pass and the
    animation's own pass leave behind, not the first frame's.

    `damage` stays the *change* a slot's word shows - the honest reading, since
    a word rewritten to the value it already held cannot be told from one that
    was not written at all - so a resolved slot can carry `damage: null`. What
    the action did to that slot is still pinned by `hp_after`, which is the
    live cell.
    """
    columns = HP_COLUMNS if columns is None else columns
    verdict_frame = pass_frame(log, start, end, roll_frames)
    flags = hit_flags(log, verdict_frame)
    # The frame the flags first moved on, kept for a reader: it is where the
    # pass's own clears and writes become visible.
    hit_frame = next(
        (frame for frame in range(start, end + 1)
         if frame in log.by_frame
         and any(log.changed(frame, f"hit_{slot:02d}")
                 for slot in range(HIT_FLAGS))),
        None)
    opposing = "enemy" if side_of(actor) == "party" else "party"
    targets = []
    for id_ in range(1, 10):
        if side_of(id_) != opposing or id_ not in occupied:
            continue
        damage = f"dmg_{id_ - 1:02d}"

        def stored(frame):
            value = log.signed(frame, damage)
            return value if DAMAGE_STORED[0] <= value <= DAMAGE_STORED[1] else None

        damage_frame = next(
            (frame for frame in range(start, end + 1)
             if frame in log.by_frame and stored(frame) is not None
             and stored(frame - 1) != stored(frame)), None)
        resolved = flags[id_ - 1] in HIT_FLAG_VALUES
        if not resolved and damage_frame is None:
            continue
        hp = log.signed(end, columns[id_])
        targets.append({
            "id": id_,
            "hit": flags[id_ - 1],
            "damage": stored(damage_frame) if damage_frame else None,
            "damage_frame": damage_frame,
            "hp_after": hp,
            "died": hp <= 0,
        })
    return {
        "actor": actor,
        "start_frame": start,
        "end_frame": end,
        "hit_frame": hit_frame,
        "verdict_frame": verdict_frame,
        "rolls": [list(entry) for entry in rolls],
        "roll_count": sum(count for _, count in rolls),
        "targets": targets,
    }


def action_effects(log, start, end, occupied, columns=None):
    """What the action moved on **any** fighter, either side.

    The target list is the acting side's opponents, which is what a swing can
    touch; an enemy ability can touch its own side instead - the sweep's
    TechUser casts RES on itself and its HP rises by 37 - and `docs/
    ENEMY_ABILITIES.md`'s status arms leave their mark in a status byte rather
    than in a damage word. An action's *effect* is therefore read off every
    fighter the formation seated, on both sides, in the cells the log carries:
    HP, the status byte, and the battle stat cells the fixtures' stat blocks
    read.

    Returns `{"hp": [[id, before, after], ...], "status": [...], "stats":
    [[id, field, before, after], ...]}`, each list holding only what moved, so
    an empty fixture key means "nothing moved" rather than "nothing observed".

    A vehicle battle's party side is the vehicle (`columns` maps its HP cell),
    which has no status or stat columns of its own.
    """
    columns = HP_COLUMNS if columns is None else columns
    effects = {"hp": [], "status": [], "stats": []}
    for id_ in sorted(occupied):
        hp_column_ = columns.get(id_)
        if hp_column_ is None or not log.has(hp_column_):
            continue
        before, after = log.signed(start, hp_column_), log.signed(end, hp_column_)
        if before != after:
            effects["hp"].append([id_, before, after])
        names = _effect_fields(id_, columns)
        for name in names:
            column = f"{names[name]}_{name}"
            if not log.has(column):
                continue
            was, now = log.signed(start, column), log.signed(end, column)
            if name == "status":
                # The dead and android-dead bits are a *death*, not an effect:
                # the cartridge sets them when the HP cell goes non-positive
                # (`Battle_KillFighter`), the port reports the same thing as a
                # `Died` event, and a fixture that called the bit a status
                # would ask the comparator for a status the log never had.
                was, now = was & STATUS_EFFECT_BITS, now & STATUS_EFFECT_BITS
            if was == now:
                continue
            if name == "status":
                effects["status"].append([id_, was, now])
            else:
                effects["stats"].append([id_, name, was, now])
    return effects


#: The status bits an *effect* can set: everything the status byte holds except
#: the two death bits (`StatusDead` `$04` and `StatusAndroidDead` `$40`,
#: `ps4.constants.asm:66-81`). Those two are a death, which the fixture records
#: as the target's `died`, and the port answers with a `Died` event.
STATUS_EFFECT_BITS = 0xFF & ~(0x04 | 0x40)

#: The stat cells an action's effect is read from, per fighter: the battle
#: values the fixtures' own stat blocks carry (`PARTY_NAMES`, `ENEMY_NAMES`),
#: plus the status byte. HP is read through `columns`, because a vehicle
#: battle's party side keeps its HP in a cell of its own.
EFFECT_FIELDS = {
    "party": ["status", "str", "agi_bat", "dex", "atk", "dfs", "men"],
    "enemy": ["status", "agi_bat", "atk", "dfs", "str_bat", "men_bat",
              "dex_bat"],
}


def _effect_fields(fighter_id, columns):
    """{field: log column prefix} for one fighter, or {} where it has none.

    A vehicle battle's party-side fighter has no status or stat cells of its
    own: the members' columns are the field's, and `columns` is what says the
    id is the vehicle (`vehicle_fighter_hp`).
    """
    if columns.get(fighter_id) == "vehicle_fighter_hp":
        return {}
    prefix = None
    for name, index in PARTY_IDS:
        if index == fighter_id:
            prefix = name
            break
    if prefix is None and 6 <= fighter_id <= 9:
        prefix = f"e{fighter_id - 5}"
    if prefix is None:
        return {}
    side = "party" if fighter_id <= 5 else "enemy"
    return {name: prefix for name in EFFECT_FIELDS[side]}


def compact_leaf_arrays(text):
    """Join every array that holds only scalars onto one line.

    `json.dump(indent=1)` puts each element of each array on its own line, which
    turns a 134-row table into a thousand-line file. The data is untouched -
    only the layout of a leaf array changes, and a leaf array is exactly the
    shape a table's rows have. Strings are tracked so a bracket inside one is
    text and not nesting.
    """
    lines, out, index = text.split("\n"), [], 0
    while index < len(lines):
        line = lines[index]
        out.append(line)
        if line.rstrip().endswith("["):
            depth, block, in_string, escaped = 1, [], False, False
            for offset in range(index + 1, len(lines)):
                candidate = lines[offset]
                for character in candidate:
                    if in_string:
                        if escaped:
                            escaped = False
                        elif character == "\\":
                            escaped = True
                        elif character == '"':
                            in_string = False
                    elif character == '"':
                        in_string = True
                    elif character == "[":
                        depth += 1
                    elif character == "]":
                        depth -= 1
                block.append(candidate)
                if depth == 0:
                    break
            body = [entry.strip() for entry in block[:-1]]
            if body and not any(entry.startswith(("{", "[")) for entry in body):
                joined = ", ".join(entry.rstrip(",") for entry in body)
                # The closing line carries the member's own comma, if any.
                tail = block[-1].strip()[1:]
                out[-1] = f"{line.rstrip()} {joined} ]{tail}"
                index += len(block) + 1
                continue
        index += 1
    return "\n".join(out)
