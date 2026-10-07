# The Soldier's Temple island (`$9A`..`$9D`)

Four short events, all of the shape `moveq #n, d0` / `jsr Event_GetAndRunDialogue` and a flag set
(`ps4.asm:153412-153433`). They left the #81 allowlist with lane C9, which met the first of them on
the way from Seth's island to the temple ([the C9 log](../campaign/RUNNER_LOG_ENDGAME.md)).

- **Data:** `rust/psiv-core/src/scenes/soldiers_temple.rs`.
- **Clone audit:** each body was read from the US image through `EventPtrs[$9A..$9D]`
  (`$05A2B4 + 4 * n`) and every instruction word matches the clone's label bodies. No `revision`
  or `grand_cross` conditional touches them.

| Event | Routine | Retail bytes | Trigger | Ops |
|---|---|---|---|---|
| `$009A` | `Event_SoldiersTempleCaveDialogue1` | `$073880..$073891` | `RunEvent_SoldiersTempleCaveDialogue1` (`$57`, `ps4.asm:116405`), `IslandCave_F1` `$93`, `$C2` clear | dialogue `$0E`, set `$C2` (`EventFlag_SethConversation1`) |
| `$009B` | `Event_SoldiersTempleCaveDialogue2` | `$073892..$0738A3` | `$58` (`:116413`), `IslandCave_F3` `$98`, `$C3` clear | dialogue `$0F`, set `$C3` (`SethConversation2`) |
| `$009C` | `Event_SoldiersTempleReached` | `$0738A4..$0738B5` | `$59` (`:116421`), `SoldiersTempleOutside` `$99`, `$C4` clear | dialogue `$10`, set `$C4` (`SoldiersTemple`) |
| `$009D` | `Event_AeroPrismFound` | `$0738B6..$0738D1` | `$5A` (`:116429`), `SoldiersTemple` `$9A`, chest `$10D` set and `$C8` clear | dialogue `$11`, set `$C9` (`AeroPrism2`) then `$C8` (`AeroPrism1`) |

The triggers fire on a map load: none carries a position box, so each scene runs the moment the
party arrives, before the first step (`RunEvents` on the first field frame). The dialogue is the
current map's tree.
