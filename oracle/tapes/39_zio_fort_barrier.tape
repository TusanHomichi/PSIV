# Tape 39 - Event_ZioFortBarrier ($30) to its Palette_Line_4 ramp
# (`loc_6EE16`), entered by RAM-patched fixture.
#
# The unpatched prefix is tape 28's input schedule (title boot, START, then the
# B taps that carry Event_GameStart). The fixture (frame 7000: Zio Fort $82
# reloaded with Chaz, Rune, Gryz, Rika and Demi below the courtyard's
# barrier, After Alys Death $63 set; frame 7200: Event_Index = $30 and the
# field routine to Events) lives in the verification command, never in the
# tape: the exact `--ram-patch` list is in #471 until `tools/certify.py`
# carries it as TAPE_39.
#
# C taps page through dialogue $44; the ring walk and the ramp run
# unattended.
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

repeat 8
4 C next_page
76 .
end

1200 . ring_and_ramp
