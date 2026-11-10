# Tape 38 - Cutscene_Ending ($8021) through its first dialogue to the
# Elsydeon break (`TextAction_ElsydeonBroken`), entered by RAM-patched fixture.
#
# The unpatched prefix is tape 28's input schedule (title boot, START, then the
# B taps that carry Event_GameStart), so the cartridge is in field control
# with the opening done and Chaz alone on map $13. The fixture (frame 7200:
# Event_Index = $8021 and the field routine to Events) lives in the
# verification command, never in the tape: `tools/certify.py` TAPE_38 carries
# the exact `--ram-patch` list.
#
# C taps page through the ending's first dialogue; the tone and back that
# whiten the screen at the break run inside the text, unattended.
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

1100 . fixture_window

repeat 20
4 C next_page
76 .
end

600 . tone
