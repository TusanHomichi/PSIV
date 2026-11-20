# Tape 48 - Ending credits renderer, ramp and third tone loop, entered by
# the same explicit RAM fixture as tapes 38 and 47. RunText3 pages without
# pad input. Continue neutrally past the staff roll through all three
# credits stages, the 14-step credits palette ramp and the third
# PaletteIncreaseTone loop to white.
# tools/verify_ending_credits.py records the fixture and named comparison frames.
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

45000 . staff_and_credits
