# The OpenGHub package, built from this checkout. Used by flake.nix; callable
# with `pkgs.callPackage ./nix/package.nix { }` outside flakes too.
{
  lib,
  rustPlatform,
  cargo-tauri,
  importNpmLock,
  nodejs_22,
  pkg-config,
  wrapGAppsHook3,
  glib-networking,
  gst_all_1,
  libayatana-appindicator,
  libxkbcommon,
  sqlite,
  pipewire,
  pulseaudio,
  systemd,
  webkitgtk_4_1,
}:

let
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.difference ../. (
      lib.fileset.unions [
        ../nix
        ../flake.nix
        (lib.fileset.maybeMissing ../flake.lock)
        ../docs
        ../README.md
        ../.github
      ]
    );
  };
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "openghub";
  version = (lib.importJSON ../package.json).version;
  inherit src;

  cargoRoot = "src-tauri";
  buildAndTestSubdir = finalAttrs.cargoRoot;
  cargoLock.lockFile = ../src-tauri/Cargo.lock;

  # Reads package-lock.json directly, so there is no npm hash to update.
  npmDeps = importNpmLock { npmRoot = src; };

  nativeBuildInputs = [
    cargo-tauri.hook
    nodejs_22
    importNpmLock.npmConfigHook
    pkg-config
    wrapGAppsHook3
  ];

  buildInputs = [
    glib-networking
    libayatana-appindicator
    sqlite # rusqlite links it even with its bundled feature here
    systemd # libudev for hidapi
    webkitgtk_4_1
    # The screen sampler's GStreamer pipeline (pipewiresrc). wrapGAppsHook3
    # puts the plugins of these packages on GST_PLUGIN_SYSTEM_PATH_1_0.
    gst_all_1.gstreamer
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
    pipewire
  ];

  # Only the .deb: cargo-tauri.hook unpacks it into $out, which gives the
  # binary, the .desktop file, the icons and the udev rule. The deb's
  # postinstall script is not run; on NixOS the module in flake.nix installs
  # the rule.
  tauriBundleType = "deb";

  # Unit tests read /etc and the user's keyboard layout; CI runs them.
  doCheck = false;

  preFixup = ''
    gappsWrapperArgs+=(
      # The tray icon and the macro text mapping load these with dlopen.
      --prefix LD_LIBRARY_PATH : ${
        lib.makeLibraryPath [
          libayatana-appindicator
          libxkbcommon
        ]
      }
      # gst-launch-1.0 for the screen sampler, parec for the audio visualizer.
      --suffix PATH : ${
        lib.makeBinPath [
          gst_all_1.gstreamer
          pulseaudio
        ]
      }
    )
  '';

  meta = {
    description = "Open source, Linux-native reimplementation of Logitech G HUB";
    homepage = "https://github.com/Slyvan25/OpenGHub";
    license = lib.licenses.mit;
    mainProgram = "openghub";
    platforms = lib.platforms.linux;
  };
})
