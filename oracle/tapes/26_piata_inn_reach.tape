# Tape - new game through the opening Alys gate to Piata's inn.
# The final map-10 leg is a live-object-free terrain walk to the inn door at
# raw source cell (22,30); the pack's y_cell is 31 because the collision grid
# is one row below the emulator's standing pixel cell.

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
120 . settle_after_opening

# Piata Academy F1 -> Academy -> Piata.  The first route reaches the real
# Alys interaction, so clear that story gate before continuing from Chaz's
# post-scene position.
16 U
192 L
30 . approach_alys
repeat 22
4 B
12 .
end
240 . alys_joined
104 L
16 D
56 L
120 . warp_from_13
16 R
24 U
104 R
24 D
120 . warp_from_11
168 D
16 L
88 D
88 L
8 U
120 . warp_from_10

# Live walk from the observed map-10 landing (17,26) to the Piata inn door:
# (16,26) -> (15,31) -> (22,31) -> (22,30), then the door transition.
16 L
40 D
56 R
8 U trigger_piata_inn
120 . inn_map_settle

# Inn clerk is at pack (41,30), reached at emulator standing (41,31).
40 U
64 . settle_counter
4 C open_inn
120 . inn_open
600 . inn_greeting_drawn
4 C accept_inn_greeting
120 . rest_confirm
4 C stay_yes
120 . night_result
300 . night_wake
