# Dialogue and field travel

Scope: dialogue continuations, choices and resting, and field
travel beyond them, including the post-Rika overworld crossing
and its recovery.

Index: [source and provenance notes](../../SOURCE_NOTES.md).

## 2026-09-12 — Dialogue continuations, choices and resting

**PORT BUG — scene dialogue ignored its caller's yield.** Retail `$F7`
reaches `TextCtrlCode_Terminate3` and saves the following text address before
returning to the event. The old Godot window immediately read the next chunk,
while the runtime automatically acknowledged `RunDialogueResume`. The
post-Igglanova entry (tree 33, entry 18) has two such breaks: Chaz turns
before the second chunk, and Hahn moves before the third. The native window
now preserves the original tree and cursor while those scene ops execute.

**RETAIL FINDING — choice offsets start after both operands.**
`TextCtrlCode_YesNo` (`ps4.asm:142780`) stores the selection in `Yes_No_Option`,
advances past both operands, and calls `GetOffsetByID`. Zero continues in
the current entry; positive values count subsequent `$FF` delimiters. Cancel
selects NO. `Event_ChazHouse` (`:149074`) consults that saved answer after the
response closes. Both branches set its temporary visit flag, but only YES
calls recovery. The port previously discarded the text branches and forced
the scene branch to YES.

**PORT BUG — rests left skills exhausted.** Both native scene recovery and
inn recovery omitted skill uses. `RecoverStats` / `DoCharRecovery`
(`ps4.asm:136503`) refill the current party's HP, TP and eight skill counts
and clear status. `DoVehicleRecovery` then refills all three saved vehicle
use banks; it does not write vehicle HP. The two native paths now share this
core operation, with real-pack tests for YES, NO, deferred answers, and inns.

## Post-Rika overworld crossing and recovery (2026-09-23)

US retail `$053D16..$053D39` (`loc_53D16`) tests event `$35` before writing
FG `$00` and BG `$48` at chunk `(42,33)`. This is a paged-overworld loader
hook, not a MapDataManager entry. It opens Motavia cells `(84..85,66..67)`:
the unpatched collision is water `$9`, and the final BG chunk supplies `$0`.
The prior native runtime ignored the extracted hook list. The repaired pack
and Rust consumer retain the source records, compose both planes and apply
flag-gated collision, raw chunk identity and render patches on map build.
Same-map live page streaming after a flag change remains unimplemented; see
[MAP_EFFECTS.md](../MAP_EFFECTS.md#12-native-overworld-page-hook-consumption-2026-09-23).

Zema's inn row `$068116` is `01 14`, rate 20 per occupied party slot; its
keeper row is `$0683A4`. Five members pay 100. `RecoverStats` starts at
`$0662DA`, not `$0662FE`: the latter is its call to `DoVehicleRecovery`.
`DoCharRecovery` at `$066306` restores HP, TP, persistent status and all eight
skill-use counters. The older address in SHOPS.md and shop_flow.py's docstring
is corrected; no recovery rule changed.

Raw byte receipts and the retained native failure live under
`build/native-post-rika-20260923/`. The original bridge video comparison uses
explicitly isolated SRAM position fixtures, not connected cartridge play.
Their Y word is `(standing_y-1)*16`, accounting for the separately documented
legacy native save-coordinate gap without modifying any campaign save.
