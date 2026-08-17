# Bespoke NPC receipt: boot through the retail Piata route, then hold on
# Piata's two NPCType6 guards. The guards use the extracted selector-2 fixed
# attempt before EventFlag_PrincipalConfession; their down command is rejected
# by the zero-height leash, so the tape proves facing/order/no-RNG without
# inventing a generic walker.

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
# The terrain route deliberately reaches the opening-act Alys trigger. Clear
# that story gate with the same cancel cadence as tape 04 before continuing.
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

# Piata (map 010) is now live. Walk to the cell immediately south of the two
# guards so their newly registered NPCType6 fixed attempts are on-screen.
8 U
56 R
48 D
16 R
64 D
16 R
32 D
16 R
16 D
120 . bespoke_guards
