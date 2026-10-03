# Tape 34 - the enemy attack animation, mid-round.
#
# Tape 32's prefix (tape 07 up to its command idle at frame 25000), then all
# three party members choose DEFEND: COMD, then for each member four Rights
# (ATTACK -> TECH -> SKILL -> ITEM -> DEFEND) and a C. The enemies' turns come
# between the defenders': Alys defends (window 25294-25340), the left Zoran
# Bult's attack runs its sixteen frames from 25368 (a sprite effect, one
# mapping every two frames), Chaz defends from 25415, and so on.
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

# 25001: C on COMD. The strip is full at 25014.
4 C comd
32 .
# Alys: Right x4 to DEFEND, C.
repeat 4
4 R
10 .
end
4 C defend_alys
32 .
# Chaz
repeat 4
4 R
10 .
end
4 C defend_chaz
32 .
# Hahn
repeat 4
4 R
10 .
end
4 C defend_hahn
# The round plays out; nothing more is pressed.
900 . round
