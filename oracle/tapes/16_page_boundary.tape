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
# Tape C - talking to an NPC.
#
# From the first controllable frame Chaz is at (768,288). Holding UP walks him
# to (752,160), where he is stopped by the field object at (752,144) - an NPC,
# not terrain (coll_up reads 00 there). That leaves him one cell below it and
# facing UP, which is exactly the geometry Interaction_ChkObjects accepts.
#
# Speak (Genesis C, ButtonSpeak = bit 5) then opens the dialogue; the presses
# after it advance and close the pages.
# ---------------------------------------------------------------------------
120 . settle
96 U walk_to_npc
30 . facing_npc
4 C speak

# ---------------------------------------------------------------------------
# Tape 16 - holding Speak across a page boundary.
#
# The NPC's line is two pages. With no input page 1 draws 58 characters at one
# per 3 frames and finishes at f7371; the window then sits indefinitely.
#
# This holds Speak continuously from before page 1 finishes through well past
# it. The question it settles: does a held button advance a finished page, or
# does the page wait for a fresh press?
#
# Measured on Text_Buffer ($FFFF7000) writes, as tapes 13 and 15 are. Compare
# against the same tape with the hold removed, and against a press placed after
# page 1 completes.
# ---------------------------------------------------------------------------
10 . gap
500 C hold
900 . watch
