{ src ? ../.
, nixpkgs ? let
    lock = builtins.fromJSON (builtins.readFile (src + "/flake.lock"));
  in builtins.fetchTree lock.nodes.nixpkgs.locked
, pkgs ? import nixpkgs { system = "x86_64-linux"; }
, panelFilter ? ""
}:
let
  inherit (pkgs) lib;
  runVM = import ./test-vm.nix { inherit pkgs; };
  source = import ./source.nix { inherit lib src; };
  archive = pkgs.rustPlatform.buildRustPackage {
    pname = "console-test-binaries";
    version = "0.1.0";
    src = source;
    cargoLock.lockFile = src + "/Cargo.lock";
    nativeBuildInputs = [ pkgs.pkg-config pkgs.cargo-nextest ];
    buildInputs = [ pkgs.wayland pkgs.pam pkgs.libxcrypt ];
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";
    CARGO_PROFILE_DEV_DEBUG = "0";
    CARGO_PROFILE_TEST_DEBUG = "0";
    # The all-features tests use portable_simd and Linux uses CommandExt::setsid.
    RUSTC_BOOTSTRAP = "console_core_path_rasterizer,console_operating_system_linux";
    doCheck = false;
    auditable = false;
    dontFixup = true;
    buildPhase = ''
      runHook preBuild
      export CARGO_TARGET_DIR="$PWD/target"
      mkdir -p "$out/bin"
      cargo build --offline --locked --workspace --all-features --bins -j "$NIX_BUILD_CORES"
      cargo nextest archive --offline --locked --workspace --all-features \
        --build-jobs "$NIX_BUILD_CORES" --archive-file "$out/tests.tar.zst"
      printf '%s\n' "$PWD" > "$out/workspace-path"
      find target/debug -maxdepth 1 -type f -executable -exec cp -t "$out/bin" {} +
      runHook postBuild
    '';
    installPhase = "runHook preInstall; runHook postInstall";
  };

  rustVM = { name, module ? {}, arguments ? [], wayland ? false }: runVM {
    inherit name wayland;
    seconds = 1800;
    memory = 6144;
    diskSize = 32768;
    command = [ "${archive}/bin/console-tasks" "guest-test" name archive source ] ++ arguments;
    module = {
      imports = [ module ];
      systemd.services.console-test = {
        path = [ archive ] ++ (with pkgs; [ cargo-nextest gnutar zstd git dbus ]);
        environment = {
          CARGO_TERM_COLOR = "never";
          NEXTEST_SHOW_PROGRESS = "none";
          NEXTEST_NO_INPUT_HANDLER = "true";
        };
        serviceConfig.ExecStartPre = [ "+${archive}/bin/console-tasks guest-workspace ${archive}" ];
      };
      systemd.tmpfiles.rules = with pkgs; [
        "L /bin/bash - - - - ${bash}/bin/bash"
        "L /usr/share/iso-codes - - - - ${isocodes}/share/iso-codes"
        "L /usr/share/X11/xkb - - - - ${xkeyboard_config}/share/X11/xkb"
        "L /usr/share/mime - - - - ${shared-mime-info}/share/mime"
        "L /usr/share/zoneinfo - - - - ${tzdata}/share/zoneinfo"
      ];
      fonts.packages = [ pkgs.dejavu_fonts ];
    };
  };
  desktopModule = {
    fonts.packages = [ pkgs.noto-fonts pkgs.nerd-fonts.fantasque-sans-mono ];
    users.users.console-test = {
      linger = true;
      extraGroups = [ "video" "render" ];
    };
    virtualisation.qemu.options = [ "-vga virtio" ];
    systemd.services.console-test = {
      requires = [ "user@1000.service" ];
      after = [ "user@1000.service" ];
      path = with pkgs; [ hyprland grim procps wtype ];
      environment = {
        XDG_RUNTIME_DIR = "/run/user/1000";
        DBUS_SESSION_BUS_ADDRESS = "unix:path=/run/user/1000/bus";
      };
    };
  };

  wpewebkit = pkgs.callPackage ./wpewebkit.nix {
    webkitBubblewrapPatch = pkgs.path + "/pkgs/by-name/we/webkitgtk_6_0/fix-bubblewrap-paths.patch";
  };
  webkitWorker = pkgs.callPackage ./browser-webkit-worker.nix { inherit wpewebkit src; };
  chromiumWorker = pkgs.callPackage ./browser-chromium-worker.nix { inherit src; };
  browserCheck = { webkit }: import ./browser-test.nix {
    inherit pkgs runVM webkit;
    browser = pkgs.callPackage ./browser.nix ({ inherit chromiumWorker src; }
      // lib.optionalAttrs webkit { inherit webkitWorker; });
  };
in
{
  night-light = import ./night-light-test.nix { inherit pkgs source src; };
  unit = rustVM { name = "unit"; };
  integration = builtins.deepSeq (import ./tests.nix) (rustVM { name = "integration"; });
  e2e-desktop = rustVM { name = "e2e-desktop"; wayland = true; module = desktopModule; };
  e2e-panels = rustVM {
    name = "e2e-panels";
    wayland = true;
    module = desktopModule;
    arguments = lib.optional (panelFilter != "") (pkgs.writeText "console-panel-test-filter" panelFilter);
  };
  e2e-live = rustVM {
    name = "e2e-live";
    module = {
      hardware.uinput.enable = true;
      users.users.console-test.extraGroups = [ "input" "uinput" ];
      services.udev.extraRules = ''KERNEL=="uinput", TAG+="systemd"'';
      systemd.services.console-test = {
        requires = [ "dev-uinput.device" ];
        after = [ "dev-uinput.device" ];
      };
    };
  };
  browser = browserCheck { webkit = true; };
  browser-chromium = browserCheck { webkit = false; };
}
