# Tape 32 - the battle command strip and the technique window.
#
# Tape 07 up to frame 25000, the settled command-idle frame that
# `oracle/frames/frame_25000.png` certifies (docs/scenes/SCENE_PRESENTATION.md),
# then a scripted walk through the per-character windows of that same battle:
# Alys (fighter slot 1) opens her strip, steps to TECH, and opens the
# technique list. Every line before `patrol_start` is tape 07's, verbatim.
# Frame numbers are 1-based `retro_run()` frames.
#
# Why the patrol is spelled out: tape 07 walks R then L (48 held, 8 released,
# twice per 112-frame cycle) while the battle waits, and frame 25000 falls 16
# frames into that cycle's third R (24761 + 2 * 112 = 24985). Reproducing the
# cycle to that frame keeps every input the battle saw identical to tape 07's,
# which the red cursor's blink and the round's rolls both depend on.
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

300 . settle
96 D
64 L
60 . approached

60 . patrol_start
repeat 2
48 R
8 .
48 L
8 .
end
16 R battle_idle_25000

# 25001: C on COMD. The strip opens (its window grows 2 cells a frame and is
# full at 25014, where the cursor object is created).
4 C comd
76 . strip_idle

# 25081: Right moves the cursor from ATTACK to TECH.
4 R tech_icon
# 25085-25164: the strip at rest, cursor on TECH. Frame 25134 is the pair.
80 . strip_tech_rest

# 25165: C on TECH. The technique window opens (full 6 frames later).
4 C tech_open
# 25169-25500: the technique window at rest. Frame 25400 is the pair.
336 . tech_rest
