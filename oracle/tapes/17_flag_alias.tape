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
# Tape 17 - where a "temp" event flag write lands, tested on hardware.
#
# SOURCE_NOTES proves from ROM bytes that retail has four flag banks, not five:
# nothing addresses the clone's Temp_Event_Flags at $F156, and the
# "TempEveFlags" calls dispatch through the door at $F140 - which the constants
# file labels Chest_Flags, but tapes 20/21 show chests write $F120 instead. That
# makes a "temp" flag and a chest flag of the same raw id one physical bit.
#
# TempEveFlag_Xanafalgue = $13 is set when the Xanafalgue runs away in the
# Piata basement. Walking a short way into the basement reaches that trigger.
#
# The prediction, from ChestFlags_Set (ps4.asm:116683): flag id N sets bit
# (7 - (N & 7)) of bank byte (N >> 3), so id $13 = 19 lands at $F140 + 2 = 
# $FFFFF142, mask $10. If the alias is real, chestb2 goes 00 -> 10 and every
# byte of the $F156 bank stays 00.
#
# Chest id $13 is ChestFlag_GrbrkTwMoonSlashr - the Garuberk Tower Moon
# Slasher chest, which shares the bit.
# ---------------------------------------------------------------------------
300 . settle
96 D
64 L
40 . approach
200 . watch_flag
