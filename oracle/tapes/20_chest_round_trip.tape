# Tape A - power-on to the first player-controllable frame of a new game.
#
# Proves: the whole boot path is reachable deterministically from power-on,
# and answers the overworld lane's new-game question from the behaviour side
# (starting map, starting position, starting party).
#
# Why the presses land where they do, all read out of the disassembly:
#   f226   GameMode_Title begins (GameModePtrs, ps4.asm:418).
#   f<400  TitleRoutine_FadingText is running; a START here skips the fade.
#   f454+  TitleRoutine_PressStartButton polls Joypad_Pressed for START over
#          a $233 = 563 frame window (ps4.asm, TitleRoutine_PressStartButton).
#   f629   TitleRoutine_PickOption. With empty SRAM CheckPS4String fails and
#          the code takes Title_NoSavedData, which opens window 4 and waits
#          for Speak|Camp|Start in Joypad_Pressed.
#   f902   GameMode_Field, Event_GameStart ($9F) begins.
#   The Speak mashing below advances that scene's dialogue. It runs across
#   maps $11 -> $5E -> $54 -> $00 -> $13 and needs no input after f6230.
#   f6230  Event_PiataChazAlone ($A0) starts and plays unattended.
#   f6456  first sustained FieldRoutine_Controls frame - the answer.

400 . skip_fade
4 S start1
200 . wait
4 S start2
250 . menu_appears
4 S confirm_start
300 . into_field

# Advance Event_GameStart. 336 iterations of (4 held, 12 released) gives a
# clean Joypad_Pressed edge every 16 frames and ends at f6239, after the last
# state change the scene needs from us.
repeat 336
4 B
12 .
end

# Event_PiataChazAlone runs unattended here; control returns at f6456.
400 . await_control

# --- Alys joins -------------------------------------------------------------
# Walking west along the pixel-row-16 corridor runs into NPCAlysPiata (map $13
# object 7, dialogue 41) parked at cell (37,15). She blocks the only corridor
# to the doorway on purpose: talking to her is the story gate. Event $03 fires
# on approach at f7155 and Current_Party_Slots becomes 0100FFFF at f7478 -
# Alys in slot 1, Chaz in slot 2, which is Event_AlysFound.
#
# Cancel (B) advances the scene's pages and stays inert once field control is
# back, so an overrun press cannot perturb the state being measured.
120 . settle
16 U
88 L
30 . approach_alys
repeat 22
4 B
12 .
end
240 . alys_joined

120 . nav_settle
8 U
8 L
32 U
32 L
16 U
30 . rest
300 . office_settle

64 U walk_up_to_npc
40 . facing
4 C speak
repeat 250
4 B
12 .
end
600 . after

80 D back_out
300 . settle_after_warp
repeat 200
4 B
12 .
end
600 . after

120 . nav_settle
48 D
48 L
30 . rest
8 L
16 D
56 L
120 . warp_from_13
300 . cross
8 D
16 R
32 U
40 R
30 . rest
32 R
48 U
16 R
30 . rest
8 R
16 U
120 . warp_from_11
300 . cross

180 . nb_settle
24 U up_a
8 L left_a
24 U up_b
60 . facing_blockers
4 C speak
repeat 250
4 B
12 .
end
600 . after

120 . nav_settle
40 U
40 R
120 . warp_from_12
300 . cross

# ---------------------------------------------------------------------------
# Tape 20 - opening a chest, pressing it again, and leaving and coming back.
#
# Three questions in one run: does a re-press on the same visit grant a second
# item (and if not, where is the guard), does the chest come back open after a
# map transition, and does anything about a chest ever touch $FFFFF140.
#
# The route below reaches (41,8), immediately east of the Dimate chest at
# (40,8); the chest's own cell is excluded from the search because objects
# block. A straight walk west from the spawn does NOT work - a wall at col 46
# stops the party at (47,8), which is what the first attempt hit.
#
# Speak there opens the chest. That sets ChestFlag 24 -> $FFFFF123 bit 7 - the
# $F120 bank, NOT the $F140 one the constants file calls Chest_Flags. Tape 21
# opens the neighbouring chest (flag 25 -> $FFFFF123 bit 6) to show that is the
# bit rule and not a coincidence. (Opening the basement's own chests is a
# harmless save change.)
# ---------------------------------------------------------------------------
200 . settle
72 D
48 L
32 U
8 L
40 U
40 . arrived
# The route's last leg moves up, so the party arrives facing UP. Pressing Left
# targets the chest's own cell, which blocks, so the party turns in place to
# face it without moving - the geometry Interaction_ChkObjects wants.
4 L face_chest
40 . at_chest
4 C open_chest

# One dismissal press then quiet. Tape 19's Speak mash re-enters ItemFound on a
# 32-frame cycle and leaves almost no frames in field control, so a walk
# appended after it never moves.
500 . opened
4 C dismiss
400 . control_back

# Same-visit re-press, standing still. This is the guard test with no walking
# in it, so no random-battle roll can land between the two presses: the party
# is still on (41,8) facing the chest, and the only thing that changed since
# the first press is the flag bit. Whether Game_Mode_Routine reaches $24 at all
# says which of the two candidate guards retail uses - the LoadTreasureChests
# test (chest never becomes interactable again) or the ItemFound entry test
# (routine entered, grant skipped).
4 C repress_same_visit
300 . after_repress

# The re-press opens a window of its own, and an undismissed window holds
# Game_Mode_Routine at $24 forever - the first cut of this tape left it open
# and the party never took another step, which silently voided the whole
# round trip below. Dismiss it before walking.
4 C dismiss2
300 . control_back2

# Back to the stairs: the approach above, reversed.
40 D
8 R
32 D
48 R
72 U
200 . at_stairs

# Landing on the stairs cell fires the doorway by itself. Coming back the other
# way does not: the party is placed ON the destination cell, and the cartridge
# will not fire a warp from the cell you were placed on, so step off and back.
500 . left_map
8 D step_off
60 .
8 U step_on
600 . returned

# Walk to the chest again and press Speak. If the chest is CLOSED and grants a
# second Dimate, the item is duplicable by revisiting - which is what an
# unwritten chest flag implies.
#
# The two 25-step walks in this half are the tape's only real fragility.
# RunRandomBattles (ps4.asm:116840) counts down $FFFFECE4 from 10 on map entry
# and then rolls RNG_Seed & $1F == 0 on every completed step, so 50 steps is
# 40 rolls at 1/32 and an untuned tape loses this coin flip more often than
# not. The `300 . control_back2` wait above is the tuning knob: idle frames
# shift the seed those rolls read without consuming a single step.
# `python3 -m oracle.scripts.sweep_encounter` swept it; 300 walks both legs
# clean.
72 D
48 L
32 U
8 L
40 U
60 . back_at_chest
4 L face_chest2
40 .
4 C press_again
600 . watch_again
