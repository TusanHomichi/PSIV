# Tape 37 - Event_EnterAngerTower ($99) with Elsydeon held: the barrier's tone
# with palette line 2 held, entered by RAM-patched fixture.
#
# The unpatched prefix is tape 28's input schedule (title boot, START, then the
# B taps that carry Event_GameStart), so the cartridge is in field control with
# the opening done and Chaz alone. The fixture (frame 7000: Field_Map_Index =
# Anger Tower $FC from Rykros $2, at the doorway's cell, EventFlag_Elsydeon
# $D9 set, a map reload) lives in the verification command, never in the tape:
# `tools/certify.py` TAPE_37 carries the exact `--ram-patch` list. The map's
# entry trigger (RunEvent_EnterAngerTower) starts the scene by itself.
#
# C taps close the barrier dialogue; the tone and the restore that follow are
# the cartridge's own, unattended.
400 . skip_fade
4 S start1
200 . wait
4 S start2
250 . menu_appears
4 S confirm_start
300 . into_field

repeat 336
4 B
12 .
end

800 . fixture_window

repeat 4
4 C confirm_speak
56 .
end

900 . barrier
