{
  # The development shell for Nix hosts (docs/DEVELOPMENT.md#current-environment).
  # `nix develop`, or direnv through `.envrc`, enters it. Versions keep their
  # one owner: rustup reads rust-toolchain.toml, and the Godot below is the
  # official build of the version .godot-version pins.
  description = "PSIV development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      lib = pkgs.lib;

      # Read as tools/godot.py reads it: the one token, whitespace stripped.
      godotVersion = lib.trim (builtins.readFile ./.godot-version);
      # The release zip's hash for each pinned version. A version bump in
      # .godot-version fails here until its hash is added, so the shell never
      # runs a Godot nobody checked (tests/test_flake.py keeps the two in step).
      godotHashes = {
        "4.7.1" = "sha256-x/8U/ShHLI1PGTBD3jAnjc9+UkGh3PdWawLiet2qM7o=";
      };

      # Xvfb's GLX loads the software rasterizer from LIBGL_DRIVERS_PATH; its
      # built-in path is NixOS's /run/opengl-driver, which a host without
      # graphics configured lacks, and without GLX Godot falls back to EGL.
      # Each wrapper sets the path for its own process only, so other GL
      # programs started from the shell keep the host's drivers.
      driPath = "${pkgs.mesa}/lib/dri";

      # What the official binary loads at run time: X11 and software OpenGL for
      # the captures under Xvfb (tools/certify.py), fonts, audio and input.
      godotLibraries = with pkgs; [
        libx11 libxcursor libxinerama libxext libxrandr libxrender libxi
        libxkbcommon fontconfig libglvnd mesa vulkan-loader wayland
        alsa-lib libpulseaudio dbus udev speechd-minimal
      ];

      godot = pkgs.stdenv.mkDerivation {
        pname = "godot-official";
        version = godotVersion;
        src = pkgs.fetchurl {
          url = "https://github.com/godotengine/godot/releases/download/${godotVersion}-stable/Godot_v${godotVersion}-stable_linux.x86_64.zip";
          hash = godotHashes.${godotVersion}
            or (throw "flake.nix: add the hash of Godot ${godotVersion}'s Linux zip to godotHashes");
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

      python = pkgs.python3.withPackages (ps: [ ps.pyyaml ps.pillow ps.numpy ]);
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          python
          godot
          xvfb
          pkgs.rustup
          pkgs.shellcheck
          pkgs.git
          pkgs.pkg-config
        ];
        shellHook = ''
          export PSIV_GODOT=${godot}/bin/godot
        '';
      };
    };
}
