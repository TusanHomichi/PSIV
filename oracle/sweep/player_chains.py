"""Source-chain derivation for the player's ledger; no ROM records live here."""
import re


class Chains:
    def __init__(self, path):
        self.lines = path.read_text().splitlines()
        self.labels = {line[:-1]: i + 1 for i, line in enumerate(self.lines)
                       if re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*:', line)}
        self.objects = {}
        for label, start in self.labels.items():
            if not re.fullmatch(r'BattleObjsGroup\d+Ptrs', label):
                continue
            for line in self.lines[start:]:
                if re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*:', line):
                    break
                match = re.match(r'\s*dc\.l\s+(\w+)\s*;\s*(\$?[0-9A-F]+)\b', line)
                if match and match[1] in self.labels:
                    number = match[2]
                    id_ = int(number[1:], 16) if number.startswith('$') else int(number)
                    if id_ in self.objects:
                        raise ValueError(f'duplicate battle object ${id_:X}')
                    self.objects[id_] = match[1]
        self.effects = {}
        start = self.labels['AbilityEffectsOffs']
        for line in self.lines[start:self.labels['AbilityEffect_None'] - 1]:
            match = re.match(r'\s*dc\.w\s+(\w+)-AbilityEffectsOffs\s*;\s*(\$?[0-9A-F]+)\b', line)
            if match:
                self.effects[int(match[2][1:], 16) if match[2].startswith('$') else int(match[2])] = match[1]

    def cite(self, label):
        return f'`{label}` (ps4.asm:{self.labels[label]})'

    def table(self, label, count):
        found = []
        for line in self.lines[self.labels[label]:]:
            match = re.match(r'\s*dc\.w\s+(\w+)-' + label, line)
            if match:
                found.append(match[1])
                if len(found) == count:
                    return found
        raise ValueError(f'{label}: expected {count} entries')

    def body(self, label, prefix):
        start = self.labels[label]
        end = next((i for i in range(start, len(self.lines))
                    if self.lines[i].startswith(prefix) and self.lines[i].endswith(':')), start + 30)
        return '\n'.join(self.lines[start:end])

    def animation(self, entry, prefix):
        body = self.body(entry, prefix)
        if entry == 'SkillObj_DblSlash':
            labels = ['BattleObj_DblSlash1', 'BattleObj_DblSlash2', 'DblSlash_SecondAttack']
        elif entry == 'SkillObj_Disrupt':
            labels = ['BattleObj_Disrupt']
        elif entry == 'TechObj_Feeve':
            labels = ['BattleObj_BuffLight']
        else:
            matches = re.findall(r'move\.w\s+#\$([0-9A-F]+),\s*(?:d[01]\b|\(a1\))', body)
            objects = list(dict.fromkeys(self.objects[int(value, 16)]
                                        for value in matches if int(value, 16) in self.objects))
            if not objects:
                raise ValueError(f'no animation object for {entry}')
            labels = objects
        return ' -> '.join(self.cite(label) for label in [entry, *labels])

    def row(self, kind, record):
        if kind == 'technique' and record['id'] > 38:
            label = 'Win_TechActionNotRyuka' if record['id'] == 39 else 'Win_TechActionNotHinas'
            return [kind, str(record['id']), record['display_name'],
                    self.cite(label), self.cite(label),
                    (record['targeting']['target'] or f"range {record['targeting']['raw'] & 15}"), 'field travel', '-', '-', '-']
        table, count, prefix = ('CharTech_TechObjsOffs', 38, 'TechObj_') if kind == 'technique' else ('CharSkill_SkillObjsOffs', 55, 'SkillObj_')
        entries = self.table(table, count)
        entry = entries[record['id'] - (1 if kind == 'technique' else 0)]
        effect = record['effect_id']
        chain = ['GetTechEffectAndRange' if kind == 'technique' else 'GetSkillEffectAndRange',
                 self.effects[effect]]
        if effect == 1:
            chain += ['loc_281E' if kind == 'technique' else 'loc_2836', 'loc_266C']
        elif effect in (15, 18, 23):
            chain += ['loc_2EC4' if kind == 'technique' else 'loc_2ED8', 'loc_2F02']
            if effect == 15:
                chain += ['loc_2F40']
        classes = {1: 'damage', 2: 'instant death', 3: 'attack down', 6: 'agility down',
                   7: 'sleep', 8: 'tech seal', 9: 'attack up', 10: 'defence up',
                   11: 'mental defence up', 12: 'agility up', 13: 'resistance byte',
                   14: 'wake characters', 15: 'TP restoration', 18: 'HP healing',
                   19: 'cure poison', 20: 'cure paralysis', 21: 'quarter revival',
                   22: 'full revival', 23: 'heal and revive humans', 38: 'dexterity up'}
        return [kind, str(record['id']), record['display_name'],
                ' -> '.join(self.cite(label) for label in chain),
                self.animation(entry, prefix), (record['targeting']['target'] or f"range {record['targeting']['raw'] & 15}"),
                classes[effect], '-', '-', '-']
