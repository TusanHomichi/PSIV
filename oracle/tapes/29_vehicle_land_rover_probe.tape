# Tape 29 - retail field boot followed by a documented Land Rover fixture.
#
# The first patch reloads Motavia through the cartridge's own GameMode_LoadFieldMap
# path and sets Principal Confession so RunEvent_ReenterPiata does not redirect
# the fixture. The second patch is the README-approved fixture boundary: after
# that retail map loader has completed, Vehicle_Index is set to Land Rover at a
# clear anchor exactly as the boarding event leaves the mounted field surface.
# The tape then holds right long enough to hit a solid boundary, asks for a
# dismount, acknowledges the refusal window, and leaves a mounted battle-entry
# window. RAM patches are supplied by the replay command, not encoded here.

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

462 . reach_map_fixture
200 . map_loader_settle
400 R mounted_rover_traverse
4 . settle_before_dismount
1 B dismount_probe
120 . refusal_window
4 B refusal_ack
100 . return_to_field
400 . battle_fixture_window
