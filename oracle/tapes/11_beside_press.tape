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

# ---------------------------------------------------------------------------
# Tape 11 - beside-press: does Speak reach an NPC that is adjacent but not in
# front of you?
#
# Built on tape 04 so Alys has already joined; she blocks the westward corridor
# until then and walking past her earlier fires her story event.
#
# The first Speak happens in open corridor with nothing adjacent. That is the
# reference: Interaction_ContinueChecks falls through to
# Interaction_DoPlayerNothingMsg when nothing is in range, so this captures the
# "nothing here" message. It matters because a window opening is NOT proof of a
# talk - the whiff opens a window too.
#
# The leader then parks at (17,7) facing RIGHT, held by the wall at (18,7),
# and waits 386 frames. At that frame a map $13 NPC is at rest at (16,7) -
# orthogonally adjacent, directly behind the facing. Speak there.
#
# The wait was measured, not guessed: the NPCs on this map wander, so the tape
# was first run parked and the log scanned for a frame where an at-rest object
# is adjacent but outside the faced cell.
#
# Verdict is the text buffer, not the window flag: if the beside press draws
# byte-identical text to the open-corridor press, it hit the "nothing here"
# path and the NPC was never reached.
# ---------------------------------------------------------------------------
120 . settle
4 C speak_empty
300 . watch_empty
4 C dismiss_a
200 . cleared_a
8 U
160 L
64 U walk_to_npc
60 . arrived
4 R face_right
386 . park
4 C speak_beside
300 . watch_beside
