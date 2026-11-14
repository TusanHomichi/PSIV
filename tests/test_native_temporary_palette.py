"""The temporary-object palette driver's failures are final:
`tools/native/native_temporary_palette.gd`.

    PYTHONPATH=. python3 -m unittest tests.test_native_temporary_palette -v

The real driver runs against a stub `field.tscn` (no extension, pack or ROM):
a game whose `debug_play_state` replays a scripted CRAM, Igglanova's sprite
and a camera. Unfaulted, the driver captures `before` and `during` and exits 0.
Each negative control faults one validation the driver makes: the copy on the
build tick, line 3 or a sheet pixel (alpha included) before the first capture,
and line 3 or a sheet pixel while line 1 cycles. Each must exit nonzero, and no
capture may follow the failure, so the `during` capture's success exit cannot
overwrite it. Skipped without the pinned Godot or `xvfb-run` (the viewport
capture needs a renderer).
"""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools import godot

ROOT = Path(__file__).resolve().parents[1]
DRIVER = ROOT / "tools/native/native_temporary_palette.gd"

# The stub game. Each `debug_play_state` call is one driver tick: ticks 1-2 show
# the map's line 3 and no object; tick 3 copies line 3 and builds Igglanova
# (copy fault: no copy); tick 4 is the first on-screen tick, the `before`
# capture; tick 6 cycles line 1 (words 16-19), the `during` capture. A fault
# named `PHASE-line3` or `PHASE-pixels` changes word 50 or one opaque pixel's
# alpha from that phase's tick on.
STUB = r'''extends Node2D

const RAMP := [0, 32, 65, 98, 139, 172, 205, 238]
var fault := OS.get_environment("PSIV_STUB_FAULT")
var calls := 0
var revision := 1
var words := []
var sprite: Sprite2D

func _ready():
    for i in 64:
        words.append(0x002 * ((i % 7) + 1))
    var camera := Camera2D.new()
    camera.position = Vector2(480, 190)
    add_child(camera)
    camera.make_current()

func word(i: int) -> int:
    return ((i & 7) << 1) | (((i >> 3) & 1) << 9) | 0x0E0

func rgba(w: int) -> Color:
    return Color8(RAMP[(w >> 1) & 7], RAMP[(w >> 5) & 7], RAMP[(w >> 9) & 7], 255)

func sheet(hole: bool) -> ImageTexture:
    var image := Image.create(64, 32, false, Image.FORMAT_RGBA8)
    for y in 32:
        for x in 64:
            var index := (x + y) % 14
            if index != 0:
                image.set_pixel(x, y, rgba(words[48 + index]))
    if hole:
        image.set_pixel(1, 0, Color8(0, 0, 0, 0))
    return ImageTexture.create_from_image(image)

func at(kind: String) -> bool:
    var phase := {"before": 4, "during": 6}
    for name in phase:
        if fault == name + "-" + kind and calls >= phase[name]:
            return true
    return false

func debug_play_state() -> String:
    calls += 1
    if calls == 3 and fault != "copy":
        for i in range(48, 62):
            words[i] = word(i - 48)
        revision += 1
    if calls == 3:
        sprite = Sprite2D.new()
        sprite.centered = false
        sprite.position = Vector2(470, 180)
        sprite.region_enabled = true
        sprite.region_rect = Rect2(0, 0, 32, 32)
        sprite.texture = sheet(false)
        add_child(sprite)
    if calls == 6:
        for i in range(16, 20):
            words[i] ^= 0x002
        revision += 1
    if at("line3") and words[50] == word(2):
        words[50] ^= 0x200
        revision += 1
    if at("pixels") and sprite.texture.get_image().get_pixel(1, 0).a > 0:
        sprite.texture = sheet(true)
    var objects := [{"id": 0x188}] if calls >= 3 else []
    return JSON.stringify({"temporary_objects": objects,
        "cram": {"live": true, "revision": revision, "words": words}})
'''

SCENE = '''[gd_scene load_steps=2 format=3]

[ext_resource type="Script" path="res://stub_field.gd" id="1"]

[node name="Field" type="Node2D"]
script = ExtResource("1")
'''


class Driver(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        try:
            cls.godot = godot.resolve()
        except godot.GodotError as error:
            raise unittest.SkipTest(f"no pinned Godot: {error}")
        cls.xvfb = shutil.which("xvfb-run")
        if cls.xvfb is None:
            raise unittest.SkipTest("xvfb-run is not installed")

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="temporary-palette-")
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        project = self.dir / "project"
        project.mkdir()
        (project / "project.godot").write_text(
            '[application]\nconfig/name="stub"\n[rendering]\n'
            'renderer/rendering_method="gl_compatibility"\n')
        (project / "stub_field.gd").write_text(STUB)
        (project / "field.tscn").write_text(SCENE)
        self.project = project

    def run_driver(self, fault=""):
        output = self.dir / (fault or "clean")
        output.mkdir()
        env = {**os.environ, "PSIV_STUB_FAULT": fault,
               "PSIV_TEMPORARY_OUTPUT": str(output), "PSIV_SAVE_DIR": str(self.dir / "saves")}
        proc = subprocess.run(
            [self.xvfb, "-a", str(self.godot), "--path", str(self.project),
             "--rendering-driver", "opengl3", "--audio-driver", "Dummy",
             "--fixed-fps", "60", "--quit-after", "600", "--script", str(DRIVER)],
            capture_output=True, text=True, timeout=180, env=env)
        return proc, proc.stdout + proc.stderr, output

    def test_an_unfaulted_run_captures_both_and_succeeds(self):
        proc, log, output = self.run_driver()
        self.assertEqual(proc.returncode, 0, log)
        self.assertIn("TEMPORARY_BUILT tick=3", log)
        self.assertIn("TEMPORARY_CAPTURE before tick=4", log)
        self.assertIn("TEMPORARY_CAPTURE during tick=6", log)
        for label in ("before", "during"):
            self.assertTrue((output / f"{label}-viewport.png").is_file(), label)

    def assert_fails(self, fault, message, captures=()):
        proc, log, output = self.run_driver(fault)
        self.assertNotEqual(proc.returncode, 0, log)
        self.assertIn(message, log)
        printed = [label for label in ("before", "during")
                   if f"TEMPORARY_CAPTURE {label}" in log]
        self.assertEqual(printed, list(captures), log)
        self.assertFalse((output / "during-viewport.png").exists(), log)

    def test_negative_control_no_copy_on_the_build_tick(self):
        self.assert_fails("copy", "line 3 did not change when Igglanova was built")

    def test_negative_control_line_3_changes_before_the_capture(self):
        self.assert_fails("before-line3", "line 3 changed before the capture")

    def test_negative_control_a_pixel_changes_before_the_capture(self):
        self.assert_fails("before-pixels", "the object changed with another line")

    def test_negative_control_line_3_changes_with_line_1(self):
        self.assert_fails("during-line3", "line 3 changed", captures=("before",))

    def test_negative_control_a_pixel_changes_with_line_1(self):
        self.assert_fails("during-pixels", "the object changed with another line",
                          captures=("before",))


if __name__ == "__main__":
    unittest.main()
