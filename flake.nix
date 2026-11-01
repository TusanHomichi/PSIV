{
  # `nix develop` for the development shell (docs/DEVELOPMENT.md#current-environment),
  # or direnv through `.envrc`; `nix flake check` for the part of the gate that
  # needs no local inputs, which is what CI runs (.github/workflows/flake.yml).
  # Versions keep their one owner: rust-toolchain.toml pins Rust, and the Godot
  # below is the official build of the version .godot-version pins.
  description = "PSIV";

  inputs.burrow.url = "git+https://github.com/FieldmouseWorks/burrow";

  outputs = { self, burrow }:
    let
      lib = burrow.inputs.nixpkgs.lib;

      # Read as tools/godot.py reads it: the one token, whitespace stripped.
      godotVersion = lib.trim (builtins.readFile ./.godot-version);
      # The official downloads of each Godot version, with Godot's published
      # SHA-512 sums: the editor zip here, the export templates for the
      # release workflow (tools/package/release.py). A version bump in
      # .godot-version fails here until its downloads are added, so the shell
      # never runs a Godot nobody checked (tests/test_flake.py keeps the two
      # in step).
      godotDownloads = (builtins.fromJSON (builtins.readFile ./tools/godot-downloads.json)).${godotVersion}
        or (throw "flake.nix: add Godot ${godotVersion}'s downloads to tools/godot-downloads.json");

      godotFor = pkgs:
        let
          # Xvfb's GLX loads the software rasterizer from LIBGL_DRIVERS_PATH; its
          # built-in path is NixOS's /run/opengl-driver, which a host without
          # graphics configured lacks, and without GLX Godot falls back to EGL.
          # Each wrapper sets the path for its own process only, so other GL
          # programs started from the shell keep the host's drivers.
          driPath = "${pkgs.mesa}/lib/dri";

          # What the official binary loads at run time: X11 and software OpenGL
          # for the captures under Xvfb (tools/certify.py), fonts, audio and input.
          godotLibraries = with pkgs; [
            libx11 libxcursor libxinerama libxext libxrandr libxrender libxi
            libxkbcommon fontconfig libglvnd mesa vulkan-loader wayland
            alsa-lib libpulseaudio dbus udev speechd-minimal
          ];
        in
        {
          godot = pkgs.stdenv.mkDerivation {
            pname = "godot-official";
            version = godotVersion;
            src = pkgs.fetchurl {
              url = "https://github.com/godotengine/godot/releases/download/${godotVersion}-stable/${godotDownloads.editor.file}";
              sha512 = godotDownloads.editor.sha512;
            };
            nativeBuildInputs = [ pkgs.unzip pkgs.autoPatchelfHook pkgs.makeWrapper ];
            dontConfigure = true;
            dontBuild = true;
            dontStrip = true;
            unpackPhase = "unzip -q $src";
            installPhase = ''
              install -Dm755 Godot_v${godotVersion}-stable_linux.x86_64 $out/libexec/godot
              makeWrapper $out/libexec/godot $out/bin/godot \
                --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath godotLibraries} \
                --set __GLX_VENDOR_LIBRARY_NAME mesa \
                --set LIBGL_DRIVERS_PATH ${driPath}
            '';
          };

          xvfb = pkgs.runCommand "xvfb-swrast" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
            makeWrapper ${pkgs.xvfb}/bin/Xvfb $out/bin/Xvfb --set LIBGL_DRIVERS_PATH ${driPath}
          '';
        };

      # The checks' Python, with what tools/ and tests/ import.
      pythonFor = pkgs: pkgs.python3.withPackages (ps: [ ps.pyyaml ps.pillow ps.numpy ]);

      # The Rust toolchain rust-toolchain.toml pins, plus `extra` components.
      toolchainFor = pkgs: extra:
        let spec = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain; in
        pkgs.rust-bin.fromRustupToolchain (spec // { components = (spec.components or [ ]) ++ extra; });

      # psiv-sound compiles the Nuked OPN2 core from the Genesis Plus GX tree
      # oracle/build_core.sh checks out (rust/psiv-sound/build.rs). The tree is
      # third-party and not committed, so the checks fetch the same commit,
      # read from that script.
      gpgxCommit = builtins.head (builtins.match ".*\nCOMMIT=([0-9a-f]{40})\n.*"
        (builtins.readFile ./oracle/build_core.sh));
      gpgxFor = pkgs: pkgs.fetchFromGitHub {
        owner = "libretro";
        repo = "Genesis-Plus-GX";
        rev = gpgxCommit;
        hash = "sha256-tu/FTJDP+auXCDQ5DUbSLrRyCRdlD3dhPv2/PeBqeAc=";
      };

      # The Rust tests that load the ROM-derived runtime-pack, which CI never
      # has: they panic without it instead of skipping (#15). The rest of the
      # workspace runs. Names are exact, so a renamed test runs again and fails
      # until it is listed, and a new test that needs the pack fails CI until
      # it skips on its own or is listed here.
      packTests = [
        # rust/psiv-core/src/battle/replay/data.rs: these skip on their own
        # without a pack (#149) and are listed so the check skips them by name.
        "battle::engine::tests::replay::replay::data::barrier_capture_rejects_a_wrong_mdef_or_party_command"
        "battle::engine::tests::replay::replay::data::every_fixture_replays_as_recorded"
        # rust/psiv-godot/src/audio.rs
        "audio::tests::debug_track_bank_contains_the_pack_path"
        "audio::tests::every_extracted_record_can_start_three_driver_ticks"
        "audio::tests::real_piata_track_first_ticks_are_deterministic_and_include_dac"
        "audio::tests::the_live_machine_keeps_no_trace_and_tracing_changes_no_pcm"
        # rust/psiv-godot/src/battle/art.rs
        "battle::art::tests::igglanova_boss_uses_pack_positions_palette_and_event_background"
        "battle::art::tests::retail_battle_art_pack_has_the_declared_shape"
        # rust/psiv-runtime/src/effects.rs
        "effects::tests::rescued_zema_and_petrified_birth_valley_select_their_actual_palette_sheets"
        # rust/psiv-runtime/src/scene_crash_tests.rs
        "scene_map_tests::crash::crash_landing_writes_five_live_bg_frames_before_flag_and_reload"
        # rust/psiv-runtime/src/scene_map_tests.rs
        "scene_map_tests::alarm_blocks_for_eight_stages_in_each_direction_before_dialogue"
        "scene_map_tests::birth_valley_door_animates_collision_then_warps_and_survives_reload"
        "scene_map_tests::elevators_open_ride_step_out_close_and_return_using_the_real_tables"
        # rust/psiv-runtime/src/scene_vahal_tests.rs
        "scene_map_tests::vahal::belts_carry_the_party_to_the_first_cell_off_the_belt_and_the_terminal_flag_swaps_the_event"
        "scene_map_tests::vahal::the_dominators_aftermath_runs_from_its_trigger_with_the_real_party"
        "scene_map_tests::vahal::the_first_vahal_platform_rides_down_and_back_through_the_ordinary_trigger"
        "scene_map_tests::vahal::the_four_weapon_plant_platforms_carry_the_party_fourteen_rows_down"
        "scene_map_tests::vahal::the_second_vahal_platform_rises_from_the_bottom_when_its_flag_is_clear"
        "scene_map_tests::vahal::the_story_triggers_fire_once_on_their_flags_and_say_their_lines"
        "scene_map_tests::vahal::the_terminals_flip_the_belt_flag_of_the_half_the_leader_stands_in"
        "scene_map_tests::vahal::walking_the_belt_off_its_entry_box_starts_nothing"
        "scene_map_tests::vahal::wren_installs_the_burst_roc_and_the_positron_bolt_from_their_chests"
        # rust/psiv-runtime/src/session/battle/player_tests.rs
        "session::battle::player_tests::session_round_applies_arows_global_event_to_off_party_records"
        "session::battle::player_tests::tp_event_updates_only_its_recipient_at_the_played_beat"
        # rust/psiv-runtime/src/suites/camp_equipment.rs
        "suites::camp_equipment::alys_can_equip_two_slashers_and_keep_them_after_reload"
        # rust/psiv-runtime/src/suites/camp_order.rs
        "suites::camp_order::camp_order_is_atomic_preserves_positions_and_stats_and_survives_save"
        # rust/psiv-runtime/src/suites/player_abilities.rs
        "suites::player_abilities::player_ability_dispatch_inventory"
        # rust/psiv-runtime/tests/session_map_updates.rs
        "camera_damping_runs_between_driver_latch_and_camera_commit"
        "canceller_chest_input_defers_the_story_write_until_its_window_closes"
        "menu_and_dialogue_frames_park_updates_but_not_the_absolute_clock"
        "missing_dma_and_conveyor_buffers_are_reported_without_invented_effects"
        "positional_encounter_override_preserves_the_unsigned_inclusive_boundary"
        "rykros_countdown_and_single_rng2_reload_share_the_field_seed"
        "tower_chest_conjunctions_set_story_flags_only_when_all_bits_hold"
        "tunnels_and_sine_scroll_emit_exact_sparse_presentation_writes"
        "unused_chest_clear_and_music_writes_reach_persistent_state"
        "zelan_canceller_requires_the_chest_bit_not_inventory_or_temporary_flags"
        # rust/psiv-runtime/tests/session_map_updates_palette.rs
        "edge_clear_rotate_and_blob_copies_follow_the_map_list_order"
        "edge_sliding_blob_and_signed_scroll_keep_separate_counters"
        "no_update_entry_has_no_palette_or_state_side_effects"
        "palette_counters_are_independent_and_wrap_at_their_own_table_bounds"
        "shutdown_gates_stop_counters_and_reunion_bypasses_both"
        "strip_lights_use_the_local_duration_byte_and_park_it_in_a_menu"
      ];

      # burrow.lib.project's Rust checks build a workspace from its own
      # directory, and psiv-sound's build script reads ../../oracle, outside
      # rust/. These are the same three checks (fmt, clippy on all targets
      # with warnings as errors, the workspace tests) over the whole tree.
      # The tests run in the test profile, as `cargo test` builds them
      # (rust/Cargo.toml states its overflow checks and debug assertions).
      rustChecks = pkgs:
        let
          craneLib = (burrow.inputs.crane.mkLib pkgs).overrideToolchain (toolchainFor pkgs [ ]);
          common = {
            src = lib.cleanSource self;
            pname = "psiv";
            version = "0.1.0";
            strictDeps = true;
            cargoToml = ./rust/Cargo.toml;
            cargoLock = ./rust/Cargo.lock;
            postUnpack = "cd $sourceRoot/rust && sourceRoot=.";
            preBuild = ''
              mkdir -p ../oracle/gpgx-src/core/sound
              cp ${gpgxFor pkgs}/core/sound/ym3438.{c,h} ../oracle/gpgx-src/core/sound/
            '';
            cargoExtraArgs = "--locked --workspace";
            CARGO_PROFILE = "";
          };
          cargoArtifacts = craneLib.buildDepsOnly common;
        in
        {
          rust-fmt = craneLib.cargoFmt (common // { cargoExtraArgs = "--all"; });
          rust-clippy = craneLib.cargoClippy (common // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });
          rust-test = craneLib.cargoTest (common // {
            inherit cargoArtifacts;
            cargoTestExtraArgs = "--no-fail-fast -- --exact "
              + lib.concatMapStringsSep " " (name: "--skip ${name}") packTests;
          });
        };

      # The documentation and size checks need no local inputs; the first
      # skips the ps4.asm citations without the disassembly
      # (docs/DEVELOPMENT.md#run-checks). Both list the change's files with
      # git (tools/repo_files.py), so they run in a fresh repository holding
      # the tree.
      docsCheck = pkgs: {
        docs = pkgs.runCommand "psiv-docs" { nativeBuildInputs = [ (pythonFor pkgs) pkgs.git ]; } ''
          cp -r --no-preserve=mode ${self} tree
          cd tree
          git init -q
          python3 tools/check_docs.py
          touch $out
        '';
        # The 1,000-line rule (tools/size_guard.py).
        size = pkgs.runCommand "psiv-size" { nativeBuildInputs = [ (pythonFor pkgs) pkgs.git ]; } ''
          cp -r --no-preserve=mode ${self} tree
          cd tree
          git init -q
          python3 tools/size_guard.py
          touch $out
        '';
      };

      project = burrow.lib.project {
        inherit self;
        # The official Godot build is x86-64 Linux.
        systems = [ "x86_64-linux" ];
        python = { };
        packages = pkgs:
          let g = godotFor pkgs; in
          [
            (toolchainFor pkgs [ "rust-src" "rust-analyzer" ])
            g.godot
            g.xvfb
            pkgs.shellcheck
            pkgs.git
            pkgs.pkg-config
          ];
        checks = pkgs: rustChecks pkgs // docsCheck pkgs;
      };
    in
    project // {
      devShells = lib.mapAttrs (system: shells: shells // {
        default = shells.default.overrideAttrs (old:
          let pkgs = burrow.inputs.nixpkgs.legacyPackages.${system}; in
          {
            # The checks' Python goes first on PATH, ahead of the plain one
            # burrow's shell brings with ruff and uv.
            nativeBuildInputs = [ (pythonFor pkgs) ] ++ old.nativeBuildInputs;
            shellHook = (old.shellHook or "") + ''
              export PSIV_GODOT=${(godotFor pkgs).godot}/bin/godot
            '';
          });
      }) project.devShells;
    };
}
