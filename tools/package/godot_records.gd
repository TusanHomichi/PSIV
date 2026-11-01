# Prints Godot's own license and copyright records as one JSON line after
# PSIV-GODOT-RECORDS, for the release's third-party notices
# (tools/package/release.py export, tools/package/notices.py).
extends SceneTree

func _initialize():
    var records := {
        "version": Engine.get_version_info(),
        "license": Engine.get_license_text(),
        "copyright": Engine.get_copyright_info(),
        "licenses": Engine.get_license_info(),
    }
    print("PSIV-GODOT-RECORDS " + JSON.stringify(records))
    quit()
