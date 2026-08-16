# Tape 24 - reach the retail save-slot chooser from a fresh boot.
#
# This uses the existing new-game prelude, then opens the Start/System window
# at the first controllable field frame and confirms its SAVE entry.  It is
# scout evidence only; the native camp SAVE extension is intentionally a
# separate modern route.

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
1 S open_system
120 . system_idle
1 C select_save
120 . save_slots_idle
