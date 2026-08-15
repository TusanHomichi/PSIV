#!/usr/bin/env python3
"""Pull battle ground truth out of an oracle log.

Reports, with frame citations: when the battle starts and ends, the enemy
formation and its stats, the turn order the game built and the agility values
behind it, every HP change with the attacker/defender stats at that moment and
the RNG seed, and the XP/meseta awarded. That is the raw material the damage
formula gets fitted against, so nothing here interprets or fits - it only
reports what the cartridge did and lines the numbers up.
"""
import argparse
import csv
import pathlib

# Battle_Routine values, from BattleRoutines (ps4.asm:7524).
BATTLE_ROUTINE = {
    0x00: 'Nothing', 0x02: 'wait', 0x04: 'init', 0x06: 'loc_5380',
    0x08: 'ProcessCOMD', 0x0A: 'ProcessMACRO', 0x0C: 'ProcessRUN',
    0x0E: 'OrderTurns', 0x10: 'loc_576A', 0x12: 'loc_6672',
    0x14: 'loc_6670', 0x16: 'DoAttackEffect', 0x18: 'loc_67AE',
    0x1A: 'loc_67BE', 0x1C: 'loc_5366', 0x1E: 'loc_66B8',
    0x20: 'loc_684A', 0x22: 'loc_68C0',
}

# Game_Mode_Index values, from GameModePtrs (ps4.asm:418).
GAME_MODE = {0x00: 'Sega', 0x04: 'Title', 0x08: 'LoadFieldMap',
             0x0C: 'Field', 0x10: 'LoadBattle', 0x14: 'Battle',
             0x18: 'VInt_Prepare'}

ENEMY_SLOTS = [1, 2, 3, 4]
CHAR_SLOTS = ['chaz', 'alys', 'hahn']


# Which columns the log writes in hex. Without this a value like "0010" parses
# as decimal 10 and every hex field is silently wrong, which is exactly the
# kind of quiet corruption this harness exists to avoid.
HEX_FIELDS = set()


def load_hex_fields():
    import json
    doc = json.loads((pathlib.Path(__file__).parent / 'ram_map.json').read_text())
    return {f['name'] for f in doc['fields'] if f.get('hex')}


def load(path):
    return list(csv.DictReader(
        [l for l in open(path) if not l.startswith('#')]))


def num(r, k):
    v = r.get(k)
    if v in (None, ''):
        return None
    return int(v, 16) if k in HEX_FIELDS else int(v)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log')
    a = ap.parse_args()
    global HEX_FIELDS
    HEX_FIELDS = load_hex_fields()
    rows = load(a.log)
    if not rows:
        raise SystemExit('empty log')

    # --- battle window -----------------------------------------------------
    in_battle = [r for r in rows if r.get('game_mode') in ('0010', '0014')]
    if not in_battle:
        print('no battle in this log (game_mode never reached $10/$14)')
        return
    first, last = int(in_battle[0]['frame']), int(in_battle[-1]['frame'])
    print(f'battle frames {first}-{last} ({last - first + 1} frames)')

    byf = {int(r['frame']): r for r in rows}
    at_start = byf[first]
    print(f"  entered from map {at_start.get('map_index')} "
          f"priority={at_start.get('battle_priority')} "
          f"enemy_count={at_start.get('enemy_count')}")

    # --- formation ---------------------------------------------------------
    settle = byf.get(min(first + 240, last), at_start)
    print('\nenemies at battle start:')
    for e in ENEMY_SLOTS:
        eid = num(settle, f'e{e}_id')
        hp = num(settle, f'e{e}_hp')
        if not hp:
            continue
        print(f"  enemy {e}: id={eid} hp={hp}/{num(settle, f'e{e}_maxhp')} "
              f"lvl={num(settle, f'e{e}_level')} "
              f"agi={num(settle, f'e{e}_agi')}/{num(settle, f'e{e}_agi_bat')} "
              f"str={num(settle, f'e{e}_str')} dex={num(settle, f'e{e}_dex')} "
              f"atk={num(settle, f'e{e}_atk')} dfs={num(settle, f'e{e}_dfs')}")
    print('party at battle start:')
    for c in CHAR_SLOTS:
        if f'{c}_hp' not in settle:
            continue
        print(f"  {c}: hp={num(settle, f'{c}_hp')}/{num(settle, f'{c}_maxhp')} "
              f"tp={num(settle, f'{c}_tp')}/{num(settle, f'{c}_maxtp')} "
              f"lvl={num(settle, f'{c}_level')} exp={num(settle, f'{c}_exp')} "
              f"agi={num(settle, f'{c}_agi')}/{num(settle, f'{c}_agi_bat')} "
              f"str={num(settle, f'{c}_str')} dex={num(settle, f'{c}_dex')} "
              f"atk={num(settle, f'{c}_atk')} dfs={num(settle, f'{c}_dfs')}")

    # --- battle routine timeline ------------------------------------------
    print('\nbattle routine timeline:')
    prev = None
    for r in rows:
        f = int(r['frame'])
        if not (first <= f <= last):
            continue
        k = (r.get('battle_routine'), r.get('battle_routine_2'))
        if k != prev:
            br = num(r, 'battle_routine')
            print(f"  f{f:<7} Battle_Routine={r.get('battle_routine')} "
                  f"({BATTLE_ROUTINE.get(br, '?')}) "
                  f"Routine_2={r.get('battle_routine_2')} "
                  f"comd={r.get('battle_total_comd')}")
            prev = k

    # --- turn order --------------------------------------------------------
    order_rows = [r for r in rows if first <= int(r['frame']) <= last
                  and any(r.get(f'turn_{i:02d}', '0000') != '0000'
                          for i in range(12))]
    if order_rows:
        r = order_rows[0]
        print(f"\nBattle_Turn_Order first populated at f{r['frame']}:")
        words = [r.get(f'turn_{i:02d}', '') for i in range(12)]
        print('  raw words:', ' '.join(words))
        print('  (entries are 4 bytes: fighter index word, then agility one '
              'byte in; see Battle_OrderTurns, ps4.asm)')

    # --- HP changes = damage events ---------------------------------------
    print('\nHP changes (damage/heal events):')
    tracked = [(f'e{e}_hp', f'enemy{e}') for e in ENEMY_SLOTS] + \
              [(f'{c}_hp', c) for c in CHAR_SLOTS]
    prev_vals = {k: num(byf[first], k) for k, _ in tracked if k in byf[first]}
    for f in range(first, last + 1):
        r = byf.get(f)
        if not r:
            continue
        for key, label in tracked:
            if key not in r:
                continue
            v = num(r, key)
            p = prev_vals.get(key)
            if p is not None and v is not None and v != p:
                delta = v - p
                hits = ' '.join(f"{r.get(f'hit_{i:02d}', '')}"
                                for i in range(10))
                dmgs = ' '.join(f"{r.get(f'dmg_{i:02d}', '')}"
                                for i in range(9))
                print(f"  f{f:<7} {label:7s} hp {p:5d} -> {v:5d} "
                      f"({delta:+d})  rng={r.get('rng_seed')}")
                print(f"           hit_flags: {hits}")
                print(f"           dmg_list:  {dmgs}")
            prev_vals[key] = v

    # --- rewards -----------------------------------------------------------
    print('\nrewards:')
    for c in CHAR_SLOTS:
        k = f'{c}_exp'
        if k not in rows[0]:
            continue
        before = num(byf[first], k)
        after = num(rows[-1], k)
        if before is not None and after is not None and after != before:
            print(f"  {c} exp {before} -> {after} (+{after - before})")
    if 'current_money' in rows[0]:
        b, aft = num(byf[first], 'current_money'), num(rows[-1], 'current_money')
        if b is not None and aft is not None:
            print(f"  meseta {b} -> {aft} (+{aft - b})")
    if 'battle_meseta_total' in rows[0]:
        vals = {num(byf[f], 'battle_meseta_total') for f in range(first, last + 1)
                if f in byf}
        print(f"  battle_meseta_total values seen: {sorted(v for v in vals if v)}")


if __name__ == '__main__':
    main()
