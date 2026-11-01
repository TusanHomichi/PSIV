# The Hunter's Guild: `Event_HuntersGuild`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0066` `Event_HuntersGuild` | `$0718E6..$071D73` code, `$071D74..$071E55` tables and names | `EventPtrs[$66]` at `$05A2B4 + $198` | `hunters_guild.rs`, `HUNTERS_GUILD` (56 ops); the list window in `session/job_board.rs` and `psiv-godot/src/job_board.rs`; `guild/guild.json` in the pack |

- **Trigger:** interaction area `$09` on `HuntersGuild` (`$057`), the cell in front of the
  counter (29, 25): the leader stands at (29, 26) facing up (`Interaction_EventIndexes`).
- **Flags:** every job's are in `GUILD_JOBS` below. The scene sets a row's `taken` on a YES and
  `paid` on the reward, and clears the first row's `taken` and `earned` when Elsydeon retires
  it. Other events set the `earned` flags and read `taken`: the dog's job, for one, takes
  `EventFlag_TinkerbellsDog` (`$1E`, read by [120](120_RockyOwner.md)) to `EventFlag_RockyHome`
  (`$24`), set by the Monsen lane's event.

## Clone audit

The routine was read from the US image through `EventPtrs[$66]`. The clone's listing
(`ps4.asm:150995`) branches on `grand_cross` and `revision`; the US bytes take
`grand_cross=0`, `revision>0`:

- The `cmpi.b #$1F, d0` tree tests at `$071954`, `$0719D2` and `$071C22` and the tree loads
  around them (`$001F2400` before, `$001F1AD0` after) are there. `revision=0` has none.
- The welcome is entry `$0F` and the prompt `$10` (`$071A20`, `$071B4A`), the goodbye `$13`
  (`$071BA8`), the three refusals `$11`, `$12`, `$19`/`$3A` (`$071CF6`, `$071C84`, `$071CAC`).
- The welcome's window is not destroyed before the list opens, and the cursor starts at
  x `$108`, y `$98` (`$071B60`): `grand_cross=1` has its own order and `$100`.
- `HuntersGuildData` and `HuntersGuildMoneyTbl` are the `else` branch's values, at `$071D74`.
  `aiedo_side.rs`'s `the_guilds_table_matches_the_rom` holds `GUILD_JOBS` to the pack's copy of
  those bytes (`psiv_tools/guild_pack.py`), which names them from the routine's own `lea`.

`$12` is the routine's "not offered yet" line as well as "no jobs left". The refusal reads it two
bytes in, past its portrait control. That control names the portrait already up, so the port reads
the whole entry.

## Retail transcription

The scene holds each leg once, as the routine does: `GuildOp` reads the row the routine holds in `d1`,
and the ten `SetDialogueTree`s are its ten `DialogueTreesToRAM` calls (`test_scene_trees.py` counts them).

### Arrival

`.checkquestloop` (`psiv_core::guild_dispatch`) walks the eight rows. A row with `taken` clear is
skipped. The first `taken` row without `earned` says its progress line, unless it is row 0 with
`EventFlag_Elsydeon` set. The first without `paid` pays. A fully paid row counts when its
`available` or `closed` flag is set, and eight counted says `$12`.

| Leg | ROM | Retail | Scene |
|---|---|---|---|
| In progress | `$071950..$071982` | `.doinprogress`: entry byte 4, in tree 27 from `$1F` | `GuildOp::SkipTreeLoad`, `SetDialogueTree`, `GuildOp::RunLine`, `SetDialogueTree`, `End` |
| Retired | `$071984..$0719C9` | `.handlemiledead`: tree 27's entry `$19` (empty in the US tree, so no window), then `EventFlags_Clear` on bytes 0 and 1 | `SetDialogueTree`, `RunDialogue`, `SetDialogueTree`, two `SetFlag`s, `End` |
| Reward | `$0719CA..$071A1F` | `.finishquest`: entry byte 5, `EventFlags_Set` byte 2, `add.l` the money byte times 1000 | as in progress, `GuildOp::SetPaid`, `GuildOp::AddReward`, `End` |
| No jobs | `$071944..$07194B` | `moveq #$12` / `jmp Event_GetAndRunDialogue` | `RunDialogue`, `End` |

### The board

`.showjobs` on: one text window stays up throughout, `RunText` returns at each entry's terminator
and the routine destroys the window itself. So while the board is up,
`Runtime::dialogue_board_text` makes a window close at its terminator without a press, keep its last
page drawn, open again without the slide, and keep the first entry's portrait.

| Op | ROM | Retail | Scene |
|---|---|---|---|
| welcome | `$071A20..$071A65` | `$0F`, `SFXID_Selection`, `RunText`, `DoMapUpdate_JoypadLoop(#$3B)` | `PlaySound`, `JobBoard Begin`, `RunDialogue`, `JobBoard Hold` |
| list | `$071A66..$071B09` | `Window_Create(#$B)` and `.loadjobtextloop` | `JobBoard ShowList` |
| prompt | `$071B0A..$071B91` | `.loadjobcursorboxes`, `$10`, `InitWindowCursor`, `.selectjobloop` | `JobBoard Boxes`, `RunDialogue`, `JobBoard Choose`, `GuildOp::BranchChoice` |
| goodbye | `$071B92..$071BBB` | `loc_69A32`'s solid box, `clr.w (a4)`, `$13` | the board stamps the row; `RunDialogue` |
| refusals | `$071BDE..$071D19` | the line, `DoMapUpdate_JoypadLoop(#$3B)`, back to the prompt | `RunDialogue`, `JobBoard Hold`, `Jump` |
| offer | `$071C12..$071C7F` | byte 3 (its `$F5` yes/no), the wait, `tst.b Yes_No_Option`; YES sets byte 0 | `GuildOp::SkipTreeLoad`, `SetDialogueTree`, `GuildOp::RunLine`, `SetDialogueTree`, `JobBoard Hold`, `BranchChoice`, `GuildOp::SetTaken`, `Jump` |
| end | `$071D1A..$071D73` | `.terminateguildwindowloop`, then the three `Window_Destroy`s | `JobBoard Linger`, `JobBoard Close` |

**The picked row** (`psiv_core::guild_choice`) tests `paid`, then `available`, then `closed`. All
eight offer entries reach a `$F5` once `closed` is clear, so `Yes_No_Option` is never stale: rows 0
and 7 open with `$FA $D9`, and those rows refuse before their offer once `$D9` is set.

**The rows** (`psiv_core::job_row`). A paid job keeps its name in `$A000`. Otherwise "not offered"
swaps in `GuildText_ListingPending`, and it or `closed` writes `$E000`, so a closed job keeps its name
in the same colour. A row's colour is its palette line: line 2 is the window font's own, and lines 1
and 3 are `HuntersGuild`'s map palette. Index 15 of each is the font's face: white, red and yellow.

**The cursor.** `Win_UpdateCursorUpDown` wraps at seven. A press ends `.selectjobloop` before the cursor
moves. `Window_Option_Index_2` keeps its row across a refusal or a NO, and the routine zeroes it on the
way out. In `.terminateguildwindowloop` the cursor object is already gone, so Up and Down only sound.

## Evidence and limits

- State: `rust/psiv-core/tests/hunters_guild_scenes.rs` holds the arrival, the choice and the row
  rules to each flag combination, and walks every leg's entries and steps, tree 27 included.
- Pad-driven: `rust/psiv-campaign/tests/aiedo_side.rs` stands at the counter on a hand-built save and
  takes the dog's job on the pad, cancels, and collects a reward, through the session and its
  mode. The route does not visit the guild.
- Derived, not measured on the oracle: the list window's opening (two columns, then four a frame,
  from `Window_Draw`'s pattern in [41](41_InsideSpaceship.md)).
- Deliberate deviation, for the frames only: the three `Window_Destroy`s at the end are not animated.
  The scene ends on the press, where the cartridge spends the destroy frames with no pad read.
- `$FD` waits advance on Speak alone, as every port window does; the cartridge also takes Cancel and Camp.
