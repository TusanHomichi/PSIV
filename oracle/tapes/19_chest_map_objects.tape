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
# Tape 19 - a chest-bearing map's object pool, and opening a chest.
#
# Tape 02's map has no chests, so the comparator has never exercised the slot
# offset chest maps introduce. AcademyBasement ($15) carries 1 NPC (the
# Xanafalgue) and 2 chests, and measurement shows the pool fills NPCs first:
# slot 0 is the Xanafalgue (id 388), slots 1 and 2 are the chests (id $A0), in
# pack record order.
#
# The route below reaches (41,8), immediately east of the Dimate chest at
# (40,8); the chest's own cell is excluded from the search because objects
# block. A straight walk west from the spawn does NOT work - a wall at col 46
# stops the party at (47,8), which is what the first attempt hit.
#
# Speak there opens the chest. That sets ChestFlag 24 -> byte 3, bit 7 -> $80
# at $FFFFF143, giving core-lane's open_chest path its frame timing and its
# bank write. (Opening the basement's own chests is a harmless save change.)
#
# The legs afterwards give the object columns a walk to compare against rather
# than a single standing sample.
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
400 . opened
repeat 40
4 C
12 .
end
200 . chest_done

120 . legs
120 D leg_down
40 . r1
120 R leg_right
40 . r2
120 U leg_up
40 . r3
120 L leg_left
60 . done
