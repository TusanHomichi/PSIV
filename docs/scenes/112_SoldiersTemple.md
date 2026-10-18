# The Soldier's Temple island (`$9A`..`$9D`)

Four short events, all of the shape `moveq #n, d0` / `jsr Event_GetAndRunDialogue` and a flag set
(`ps4.asm:153412-153433`). They left the #81 allowlist when the campaign route reached them: the
`island-cave` and `soldiers-temple` chapters of `rust/psiv-campaign/routes/main.json` fire all four
([campaign runner](../campaign/CAMPAIGN_RUNNER.md)).

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

None of the four triggers tests a position, only flags (and, for `$5A`, the chest flag).
`RunEvents` walks the map's event list on every `FieldRoutine_Controls` frame on which the leader
is not mid-step and bit 0 of `Tile_Coll_Run_Event_Flag` is clear (`ps4.asm:116755-116773`,
`:114979-114986`). So `$57`, `$58` and `$59` fire on the first such frame after their map loads,
before the party can take a step, and `$5A` on the first such frame after the Aero-Prism chest
has set `$10D`. The dialogue is the current map's tree.
