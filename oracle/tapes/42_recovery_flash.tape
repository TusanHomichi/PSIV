# Tape 42 - Event_Recovery ($21): the recovery tile's three palette flashes
# (`loc_6D3BA` / `loc_6D40A`), entered by RAM-patched fixture.
#
# The unpatched prefix is tape 28's input schedule (title boot, START, then the
# B taps that carry Event_GameStart), so the cartridge is in field control
# with the opening done and Chaz alone on map $13, as tape 38 leaves it. The
# fixture (frame 7200: Event_Index = $21 and the field routine to Events)
# lives in the verification command, never in the tape: `tools/certify.py`
# TAPE_42 carries the exact `--ram-patch` list. The event takes no input.
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

1100 . fixture_window_and_flashes
