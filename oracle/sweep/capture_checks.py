"""Evidence predicates for damage captures."""

def observed_rounds(fixture: dict, ability: int) -> list[int]:
    return sorted({r["round"] for r in fixture["rounds"]
                   if any(a.get("ability") == ability and a["kind"] == "ability"
                          and any(t.get("damage") is not None for t in a["targets"])
                          for a in r["actions"])})


def out_of_lane(fixture: dict, allowed: set[int]) -> list[int]:
    return sorted({a["ability"] for r in fixture["rounds"] for a in r["actions"]
                   if a.get("ability", 0) and a["ability"] not in allowed})
