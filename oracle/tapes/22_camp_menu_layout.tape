# Tape 22 - retail camp-menu layout captures.
#
# The prelude is Tape 08's deterministic new-game route.  It leaves Chaz
# alone on Piata Academy F1 with the camp menu's first option selected.
#
# Menu controls are the retail functions from ps4.constants.asm: ButtonSpeak
# (physical C) and ButtonCamp (physical A) confirm; ButtonCancel (physical B)
# returns; Up/Down move the edge-triggered cursor.  Release frames between
# directional pulses are intentional: Win_UpdateCursorUpDown reads
# Joypad_Pressed, not Joypad_Held.

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
1 . camp_root_idle

1 C select_item
120 . item_open_settle
1 . camp_item_idle

1 S exit_item_message
60 . field_after_item
4 A reopen_menu
60 . root_reopen_settle
1 . root_before_state

1 D state_down_1
1 . state_gap_1
1 D state_down_2
1 . state_gap_2
1 D state_down_3
1 . state_gap_3
1 D state_down_4
1 . state_gap_4
1 . camp_state_cursor
1 C select_state
120 . state_open_settle
1 . camp_state_idle
1 C select_status
120 . stats_open_settle
1 . camp_stats_idle

1 B close_stats
120 . after_state
