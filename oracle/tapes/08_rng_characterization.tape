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

# ---------------------------------------------------------------------------
# Tape F - RNG characterisation.
#
# PSIV has ONE seed, RNG_Seed ($FFFFEF0C, constants:2328), and two update
# routines over it. UpdateRNGSeed (ps4.asm:86066) rewrites the whole longword;
# UpdateRNGSeed2 (ps4.asm:86098) only does `ror` on the high word. The entire
# disassembly contains exactly two writes to the location, one in each.
#
# The log is analysed by predicting UpdateRNGSeed forward from each frame's
# seed and counting how many applications are needed to reach the next frame's
# value. That count is the number of RNG calls that frame, which is how the
# consumers are identified without instrumenting the emulator.
#
# Phases below: field idle (no input), field walking, camp-menu idle, field
# again. The title and cutscene phases come free from the prelude.
# ---------------------------------------------------------------------------
120 . settle
300 . field_idle
96 U walk_a
96 D walk_b
60 . rest_after_walk
4 A open_menu
300 . menu_idle
4 B close_menu
120 . after_menu
