# Tape 47 - Ending staff roll, entered by the same explicit RAM fixture
# as tape 38. RunText3 pages without pad input. Continue neutrally through
# the four RunText2 entries, both staff palette loops and the plane clear.
# tools/verify_ending_staff.py records the fixture and named comparison frames.
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
200 . tone_loop

34000 . staff_roll
