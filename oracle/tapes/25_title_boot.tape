# Tape 25 - retail front door through the no-save title menu.
#
# The first 400 frames are untouched power-on playback: Sega's logo fades in,
# holds, and the title routine starts.  The two START presses are the same
# boundary controls used by tape A, retained here so the tape reaches the
# menu in a bounded run while still recording the complete front-door phases.
#
# f25..200  Sega logo fade/hold (the logo mapping is at (12,11), 17x5 cells)
# f401      START skips TitleRoutine_FadingText
# f450      settled title layout: background, PS title, and subtitle
# f500      Press Start and copyright are present
# f605      START leaves TitleRoutine_PressStartButton
# f650      no-save Start-only window opens at (13,12), 14x3 cells

400 . sega_logo_and_title
4 S skip_fade
200 . title_prompt
4 S enter_menu
80 . no_save_menu
