# Tape 45 - Event_FractOozeFound ($7D), a `DialogueWindow::Retained` caller
# (`Event_GetAndRunDialogue2`, `$5ACDC`), from its text to battle 7, entered by
# RAM-patched fixture (#644).
#
# The unpatched prefix is tape 28's input schedule (title boot, START, then the
# B taps that carry Event_GameStart). The fixture lives in the verification
# command, never in the tape: `tools/certify.py` TAPE_45 carries the exact
# `--ram-patch` list. Frame 7000 reloads the field on Monsen ($73, whose tree
# 12 holds the event's entry $2F) with Chaz at (288, 704); frame 7224, after
# the map-name panel has gone, sets Event_Index to $7D and the field routine to
# Events.
#
# C taps page the text. The last one (frame 7799) ends it: the routine returns
# with the window and Rika's portrait still up, and `RunEventBattle`'s entry
# (`loc_58498`) covers them with a white plane-A band from frame 7801.
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
repeat 30
4 C next_page
76 .
end
600 . battle
