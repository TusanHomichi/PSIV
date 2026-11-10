# Zio barrier palette scene: tape 28 prelude, then C page acknowledgements.
# Explicit map/party/flag/event fixture patches live in tools/certify.py TAPE_44.
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

700 . await_fixture
repeat 400
1 C
15 .
end
