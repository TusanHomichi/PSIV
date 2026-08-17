# Tape 27 - the complete retail opening scene, from title boot through the
# Piata hand-off. This is intentionally the same deterministic input schedule
# as Tape A, with a scene-presentation name so frame captures and layout
# assertions have a stable oracle target.
#
# The opening scene is Event_GameStart ($9F): title image load, two prologue
# pages, 20-step text ramps, long holds, map tour, and final Saved_Sound_Index.

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

400 . await_control
