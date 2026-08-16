# Tape - new game to Piata's item-shop counter, then open the retail shop.
#
# The boot prelude is the checked-in new-game proof.  The final route is the
# route.py output from PiataAcademy_F1 to PiataItemShop, followed by a fixed
# grid walk to the only live Piata shopkeeper at (33,30).  At (33,32), facing
# up, the special probe sees the $C row at (33,31) and reaches the clerk at
# (33,30), producing AcrossCounter rather than an adjacent talk.

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

# route.py: PiataAcademy_F1 -> PiataAcademy -> Piata -> PiataItemShop.
# The terrain route deliberately reaches the opening-act Alys trigger.  Clear
# that real story gate with the same cancel cadence as tape 04, then resume
# the route from its post-scene position.
16 U
192 L
30 . approach_alys
repeat 22
4 B
12 .
end
240 . alys_joined
# Post-scene Chaz is at (37,16); route.py replans the remaining Academy leg
# from that actual position rather than pretending the trigger never moved us.
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

# The terrain route reaches Piata at (17,26) in the live emulator.  Re-route
# around the solid building to its item-shop doorway at (18,39).
16 L
104 D
24 R
120 . warp_to_item_shop

# PiataItemShop loads at the retail destination (33,37), observed at the
# emulator's standing cell (33,36).  The first up step fires the doorway;
# settle after that transition, then approach the clerk.  The counter probe
# is one cell nearer than the pack y coordinate: standing at (33,31), facing
# up, sees the live $C cell at (33,30) and the clerk behind it.
8 U trigger_item_shop
120 . shop_map_settle
40 U
64 . settle_counter
4 C open_shop
120 . shop_open
600 . greeting_drawn
4 C accept_greeting
120 . after_greeting
4 C buy_choice
120 . buy_list
4 C choose_monomate
120 . quantity_screen
4 C quantity_confirm
120 . buy_result_drawn
4 B dismiss_buy_result
120 . shop_menu
4 D sell_cursor
120 . sell_cursor_stable
4 C sell_choice
120 . sell_list
4 C sell_item
120 . sell_confirm
4 C sell_yes
120 . after_sell
