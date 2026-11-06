# Tape 36 - retail TELEPIPE town list, teleport message and teleport fade.
#
# The prelude is Tape 22's (Tape 08's deterministic new game to Chaz alone on
# Piata Academy F1). The inventory and town flags are fixture RAM patches
# (oracle/README.md, "Deterministic scene fixtures"); see
# docs/field/TRAVEL.md for the command.
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
120 . settle
300 . field_idle
96 U walk_a
96 D walk_b
60 . rest_after_walk
4 A open_menu
60 . root_open_settle
1 C select_item
120 . item_list_settle
1 C select_telepipe
60 . item_action_settle
1 C select_use
240 . town_list_settle
4 D town_down
12 . town_down_gap
1 C select_town
240 . message_settle
1 C acknowledge
240 . teleport
120 . arrived
