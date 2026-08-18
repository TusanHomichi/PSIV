# Natural power-on route toward the earliest retail Land Rover acquisition.
#
# This is input-only: no RAM patches, scene fixtures, or save state. The
# opening schedule is the proven tape-07 prefix through the first basement
# encounter; the appended route continues through the real story interaction.

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
16 U
88 L
30 . approach_alys
repeat 22
4 B
12 .
end
240 . alys_joined

120 . nav_settle
8 U
8 L
32 U
32 L
16 U
30 . rest
300 . office_settle
64 U walk_up_to_npc
40 . facing
4 C speak
repeat 250
4 B
12 .
end
600 . after_principal

80 D back_out
300 . settle_after_warp
repeat 200
4 B
12 .
end
600 . after_suspicion

120 . nav_settle
48 D
48 L
30 . rest
8 L
16 D
56 L
120 . warp_from_13
300 . cross
8 D
16 R
32 U
40 R
30 . rest
32 R
48 U
16 R
30 . rest
8 R
16 U
120 . warp_from_11
300 . cross

180 . nb_settle
24 U up_a
8 L left_a
24 U up_b
60 . facing_blockers
4 C speak
repeat 250
4 B
12 .
end
600 . after_hahn

120 . nav_settle
40 U
40 R
120 . warp_from_12
300 . cross
300 . settle
96 D
64 L
60 . approached
60 . patrol_start
repeat 40
48 R
8 .
48 L
8 .
end
repeat 400
4 C
12 .
end
600 . battle_return
repeat 100
4 B
12 .
end
120 . first_battle_done

# Academy Basement B1 -> B2, then the B2 interaction rectangle at (15,10).
24 U descend_b1_a
96 L descend_b1_b
32 U descend_b1_c
24 L descend_b1_d
40 U descend_b1_e
40 R descend_b1_f
120 . enter_b1
16 D descend_b2_a
16 L descend_b2_b
64 D descend_b2_c
248 L descend_b2_d
120 . enter_b2
8 L igglanova_align
72 U igglanova_area
600 . containers_wait
4 C containers_close
400 . containers_done
8 L igglanova_align_after_containers
72 U igglanova_area_after_containers
600 . igglanova_wait
repeat 600
4 C
12 .
end
600 . igglanova_battle_wait
4 C igglanova_after_battle_advance
120 . igglanova_after_battle_wait
32 D leave_b2
120 . leave_b2_warp
8 R leave_b1_r
16 U leave_b1_u
232 R leave_b1_east
72 U leave_b1_north
4 R leave_b1_encounter
repeat 400
4 C
12 .
end
600 . leave_b1_battle_return
repeat 100
4 B
12 .
end
120 . leave_b1_battle_done
24 R leave_b1_exit
16 U leave_b1_warp_align
120 . leave_b1_warp
8 D leave_b2_exit_south
48 L leave_b2_exit_west
56 D leave_b2_exit_south2
120 R leave_academy_basement_east
4 R leave_academy_basement_encounter
repeat 400
4 C
12 .
end
600 . leave_academy_basement_battle_return
repeat 100
4 B
12 .
end
120 . leave_academy_basement_battle_done
40 R leave_academy_basement_east_after
72 U leave_academy_basement_north
8 R leave_academy_basement_exit
8 U leave_academy_basement_surface_north
16 D surface_route_down
8 L surface_route_west
56 D surface_route_south
48 R surface_route_east
72 U surface_route_north
8 R surface_route_exit
120 . leave_academy_basement_warp
8 D return_near_basement_south
48 L return_near_basement_west
48 D return_near_basement_south2
8 R return_near_basement_east
16 D return_near_basement_exit_south
120 . return_near_basement_warp
16 D return_academy_south
8 L return_academy_west
48 D return_academy_south2
40 L return_academy_west2
8 D return_academy_south3
56 L return_academy_exit_west
8 L return_academy_exit_align_west
16 D return_academy_exit_align_south
120 . return_academy_warp
8 R return_f1_east
32 U return_f1_north
72 R return_f1_east2
32 U return_f1_north2
32 R return_f1_east3
16 U return_principal_office
4 U principal_office_trigger
120 . return_principal_warp
40 U principal_approach
4 C principal_confession_start
600 . principal_confession_wait
repeat 250
4 C
12 .
end
600 . principal_confession_done
repeat 100
4 B
12 .
end
600 . principal_confession_final

# Leave the office and Piata for the Motavia overworld after the confession.
40 D office_exit_approach
120 . office_exit_warp
48 D academy_f1_exit_south
56 L academy_f1_exit_west
16 D academy_f1_exit_align
56 L academy_f1_exit_gate
120 . academy_exit_warp
16 R piata_academy_exit_east
24 U piata_academy_exit_north
104 R piata_academy_exit_east2
24 D piata_academy_exit_south
120 . piata_academy_warp
168 D piata_exit_south
16 L piata_exit_west
128 D piata_exit_south2
8 R piata_exit_align
24 D piata_overworld_warp
120 . piata_inn_route_settle
# Piata's live inn is still open here.  Use the natural town path, recover the
# occupied party, then leave through the same south-gate route from the inn's
# real return cell.
72 D piata_inn_route_01
8 L piata_inn_route_02
32 D piata_inn_route_03
56 L piata_inn_door_approach
8 U piata_inn_door
700 . piata_inn_map_settle
120 . piata_inn_map_loaded
64 U piata_inn_map_transition_probe
64 . piata_inn_map_ready
48 U piata_inn_clerk_approach
64 . piata_inn_counter
4 C piata_inn_open
120 . piata_inn_open
600 . piata_inn_greeting
4 C piata_inn_accept_greeting
120 . piata_inn_rest_confirm
4 C piata_inn_stay_yes
120 . piata_inn_night_result
300 . piata_inn_recovered
4 B piata_inn_close
120 . piata_inn_field_return
56 D piata_inn_exit_approach
8 D piata_inn_exit
120 . piata_inn_return_map
48 D piata_after_inn_route_01
8 R piata_after_inn_route_02
32 D piata_after_inn_route_03
8 R piata_after_inn_route_04
32 D piata_after_inn_route_05
48 R piata_after_inn_route_06
8 R piata_after_inn_exit_step
24 D piata_after_inn_exit_south
120 . piata_after_inn_world

# Cross the natural Motavia/The Edge boundary and enter Zema.
8 D zema_route_south
16 R zema_route_east
48 U zema_route_north1
16 R zema_route_east2
40 U zema_route_north2
32 R zema_route_east3
152 U zema_route_north3
120 . motavia_to_edge
8 U edge_route_north
120 . edge_to_motavia_north
56 U zema_approach_north
24 R zema_approach_east
32 U zema_approach_north2
96 R zema_approach_east2
16 U zema_approach_north3
16 R zema_approach_east3
# The field encounter is real; leave through the retail RUN command so the
# travel tape does not turn a random formation into a combat fixture.
120 . zema_route_battle_menu
4 D zema_route_run_down1
20 .
4 D zema_route_run_down2
20 . zema_route_run_selected
4 C zema_route_run_confirm
600 . zema_route_run_wait
4 C zema_route_run_result_dismiss
120 . zema_route_run_field_settle
600 . zema_route_battle_done
24 U zema_approach_north4
16 R zema_approach_east4
24 U zema_approach_north5
8 R zema_approach_east5
8 U zema_approach_north6
8 R zema_approach_east6
8 U zema_approach_north7
8 R zema_approach_east7
8 U zema_approach_north8
8 R zema_approach_east8
8 U zema_approach_north9
8 R zema_approach_east9
16 U zema_gate_north
72 R zema_gate_east
32 U zema_gate_north2
# This formation refuses RUN on the retail roll; resolve it with ordinary
# command-entry inputs rather than pretending the escape succeeded.
repeat 400
4 C
12 .
end
600 . zema_gate_battle_wait
repeat 100
4 B
12 .
end
120 . zema_gate_battle_done
8 U zema_to_zema_01
16 R zema_to_zema_02
24 U zema_to_zema_03
16 R zema_to_zema_04
24 U zema_to_zema_05
8 R zema_to_zema_06
8 U zema_to_zema_07
8 R zema_to_zema_08
8 U zema_to_zema_09
8 R zema_to_zema_10
8 U zema_to_zema_11
8 R zema_to_zema_12
8 U zema_to_zema_13
8 R zema_to_zema_14
16 U zema_to_zema_15
72 R zema_to_zema_16
# A natural three-enemy formation interrupts this route.  Its retail run
# chance is zero; finish it before the remaining path inputs are consumed.
repeat 400
4 C
12 .
end
600 . zema_to_zema_battle_wait
repeat 100
4 B
12 .
end
120 . zema_to_zema_battle_done
# The planner-only route hits a foreground wall after the ambush.  This short
# closed-loop authoring pass walked the accepted prefix around that wall; the
# committed tape below is still a static, input-only schedule.
32 U zema_to_zema_17
120 . zema_road_replan_settle
8 D zema_road_replan_01
72 R zema_road_replan_02
16 U zema_road_replan_03
30 . zema_road_replan_rest
16 U zema_road_replan_04
64 R zema_road_replan_05
8 U zema_road_replan_06
120 . zema_town_warp
300 . zema_town_cross

# Zema's inn doors are story-locked until the Igglanova flag; do not fake a
# recovery here.  The Piata receipt above is the natural healing stop.

# The retail road enters Birth Valley B1 to discover the petrified Holt.
304 U zema_birth_valley_climb
120 . zema_to_birth_valley
40 U birth_valley_b1_route1
8 R birth_valley_b1_route2
112 U birth_valley_b1_route3
8 L birth_valley_b1_route4
# A natural four-enemy ambush interrupts the first B1 bend.  Its retail run
# chance is zero; resolve it before the remaining path resumes.
repeat 400
4 C
12 .
end
600 . birth_valley_b1_first_battle_wait
repeat 100
4 B
12 .
end
120 . birth_valley_b1_first_battle_done
120 . birth_valley_b1_replan_settle
72 U birth_valley_b1_replan_01
8 L birth_valley_b1_replan_02
16 U birth_valley_b1_replan_03
30 . birth_valley_b1_replan_rest1
40 U birth_valley_b1_replan_04
56 R birth_valley_b1_replan_05
30 . birth_valley_b1_replan_rest2
96 R birth_valley_b1_replan_06
30 . birth_valley_b1_replan_rest3
24 R birth_valley_b1_replan_07
8 U birth_valley_b1_replan_08
8 R birth_valley_b1_replan_09
8 U birth_valley_b1_replan_10
16 R birth_valley_b1_replan_11
32 U birth_valley_b1_replan_12
30 . birth_valley_b1_replan_rest4
32 U birth_valley_b1_replan_13
64 L birth_valley_b1_replan_14
8 U birth_valley_b1_replan_15
8 L birth_valley_b1_replan_16
120 . birth_valley_b1_enter
80 U holt_approach1
16 L holt_approach2
8 U holt_approach3
8 L holt_approach4
80 U holt_approach5a
8 U holt_approach5b
# A second natural encounter interrupts the final approach.  This formation
# has a zero retail run chance, so resolve it with ordinary command inputs.
120 . holt_battle_menu
repeat 400
4 C
12 .
end
600 . holt_battle_wait
repeat 100
4 B
12 .
end
120 . holt_battle_done
80 U holt_approach5c
8 R holt_align
56 U holt_face
4 C holt_scene_open
12 . holt_scene_gap
repeat 400
4 B
12 .
end
120 . holt_scene_done
# Cutscene_ProfHolt returns to Zema; leave through its south gate.
304 D zema_south_gate
# No battle interrupts this corrected departure; the old exploratory fight
# block only pressed field inputs and was not a retail receipt.
120 . zema_south_gate_settle
600 . motavia_after_birth_valley
8 D zema_south_gate_final
120 . motavia_after_zema_again
600 . motavia_after_zema_settle
# This is the verified natural prefix boundary.  GettingLandRover remains
# downstream of the Zema/Krup/Tonoe/Rune/Zio/Rika route and is deliberately
# not represented by a fake RAM-patch receipt.
120 . natural_prefix_holt_world
