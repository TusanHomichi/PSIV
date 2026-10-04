# Tape 35 - the ship's destination menu, entered by RAM-patched fixture and
# answered with Speak.
#
# The unpatched prefix is tape 28's input schedule (title boot, START, then the
# B taps that carry Event_GameStart), so the cartridge is in field control with
# the opening done. The fixture (frame 7000: Field_Map_Index = Mota Spaceport
# $BF, a map reload; frame 7200: Game_Mode_Routine = $C and Event_Index = $800D)
# lives in the verification command, never in the tape: `tools/certify.py`
# TAPE_35 carries the exact `--ram-patch` list and
# docs/scenes/41_InsideSpaceship.md ("Oracle timeline") the prose.
#
# The menu draws by ~frame 7260 and waits for a button. Speak is pressed at
# 7401 for four frames; what follows (the typed message, the 60-frame wait, the
# takeoff and the flight, 7405 onward) is the cartridge's own, unattended.
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

862 . fixture_window
4 C confirm_speak
2400 . flight
