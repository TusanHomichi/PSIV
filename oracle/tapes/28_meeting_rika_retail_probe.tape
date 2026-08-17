# Tape 28 - retail opening followed by a RAM-patched MeetingRika entry.
#
# The RAM writes used with this tape are kept in the verification command, not
# hidden in tape syntax: the host applies them before the named frame and
# proves that every one was reached. The unpatched prefix is the same input
# schedule as Tape 27, so the retail cartridge has completed title boot and
# is in field control before the fixture redirects it to BioPlant B4.

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

1200 . fixture_window
