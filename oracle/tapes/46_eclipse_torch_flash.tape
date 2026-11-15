# Tape 46 - Event_EclipseTorchUsed ($47), on foot on Dezolis below the Air
# Castle's trigger: the panel, both white flashes and their `loc_58666`
# restores, entered by RAM-patched fixture (#593).
#
# The unpatched prefix is tape 28's input schedule (title boot, START, then the
# B taps that carry Event_GameStart). The fixture lives in the verification
# command, never in the tape: `tools/certify.py` TAPE_46 carries the exact
# `--ram-patch` list. Frame 7000 reloads the field on Dezolis ($001, World_Index
# 1) with Chaz at ($BA0, $D0) and EventFlag_Lashiec ($9B) set; frame 7224, after
# the map-name panel has gone, sets Event_Index to $47 and the field routine to
# Events.
#
# No input follows the fixture until the scene's dialogue: the flashes and
# restores need none.
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

700 . await_fixture
600 . flashes
repeat 10
4 C next_page
76 .
end
