{ pkgs, browser, webkit ? true }:

let
  engine = if webkit then "webkit" else "chromium";
  guest = pkgs.nixos ({ lib, ... }: {
    imports = [ (pkgs.path + "/nixos/modules/virtualisation/qemu-vm.nix") ];

    networking.hostName = "console-browser-test";
    system.stateVersion = "26.05";
    virtualisation = {
      memorySize = 3072;
      cores = 2;
      diskImage = null;
      graphics = false;
      qemu.networkingOptions = lib.mkForce [ "-nic none" ];
    };
    hardware.graphics.enable = true;
    fonts.packages = [ pkgs.dejavu_fonts ];
    users.users.browser = {
      isNormalUser = true;
      createHome = true;
    };
    systemd.services."serial-getty@ttyS0".enable = false;

    systemd.services.weston = {
      environment.XDG_RUNTIME_DIR = "/run/console-browser";
      serviceConfig = {
        User = "browser";
        RuntimeDirectory = "console-browser";
        RuntimeDirectoryMode = "0700";
        ExecStart = "${pkgs.weston}/bin/weston --backend=headless --renderer=pixman --shell=kiosk-shell.so --socket=wayland-console --no-config";
      };
    };

    systemd.services.console-browser-check = {
      wantedBy = [ "multi-user.target" ];
      requires = [ "weston.service" ];
      after = [ "weston.service" ];
      environment = {
        XDG_RUNTIME_DIR = "/run/console-browser";
        WAYLAND_DISPLAY = "wayland-console";
        TMPDIR = "/var/tmp";
        LIBGL_ALWAYS_SOFTWARE = "1";
      };
      unitConfig = {
        SuccessAction = "poweroff";
        FailureAction = "poweroff";
      };
      serviceConfig = {
        Type = "oneshot";
        User = "browser";
        TimeoutStartSec = "210s";
        StandardOutput = "tty";
        StandardError = "inherit";
        TTYPath = "/dev/ttyS0";
        ExecStart = "${pkgs.dbus}/bin/dbus-run-session --dbus-daemon=${pkgs.dbus}/bin/dbus-daemon -- ${browser}/bin/console-browser-check --guest-${engine} ${browser}/bin/console-browser-window";
      };
    };
  });
  vm = guest.config.system.build.vm;
in
pkgs.runCommand "console-browser-${engine}-vm-check" { } ''
  mkdir -p "$out"
  ${browser}/bin/console-browser-check --vm ${pkgs.lib.getExe vm} "$out/console.log"
''
