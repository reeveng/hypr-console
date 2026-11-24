{ pkgs }:
{ name, command, module ? {}, seconds ? 300, memory ? 4096, diskSize ? null, wayland ? false }:
let
  inherit (pkgs) lib;
  runtime = "/run/console-test-wayland";
  guest = pkgs.nixos ({ ... }: {
    imports = [ (pkgs.path + "/nixos/modules/virtualisation/qemu-vm.nix") module ];
    networking.hostName = "console-test-${name}";
    networking.useDHCP = false;
    system.stateVersion = "26.05";
    boot.kernelParams = [ "systemd.show_status=yes" ];
    virtualisation = {
      memorySize = memory;
      cores = 2;
      diskImage = if diskSize == null then null else "root.qcow2";
      diskSize = if diskSize == null then 1024 else diskSize;
      graphics = false;
      useNixStoreImage = true;
      mountHostNixStore = false;
      sharedDirectories = lib.mkForce {};
      qemu.enableSharedMemory = false;
      qemu.networkingOptions = lib.mkForce [ "-nic none" ];
      qemu.options = [ "-monitor none" "-serial stdio" "-no-reboot" ];
    };

    users.users.console-test = {
      isNormalUser = true;
      uid = 1000;
      extraGroups = [ "systemd-journal" ];
    };
    environment.etc."console-test-vm".text = name;
    hardware.graphics.enable = wayland;
    systemd.services."serial-getty@ttyS0".enable = false;
    systemd.services.weston = lib.mkIf wayland {
      environment.XDG_RUNTIME_DIR = runtime;
      serviceConfig = {
        Type = "notify";
        User = "console-test";
        RuntimeDirectory = "console-test-wayland";
        RuntimeDirectoryMode = "0700";
        ExecStart = "${pkgs.weston}/bin/weston --backend=headless --renderer=pixman --shell=kiosk-shell.so --socket=wayland-console --no-config --modules=systemd-notify.so";
      };
    };
    systemd.services.console-test = {
      wantedBy = [ "multi-user.target" ];
      requires = lib.optional wayland "weston.service";
      after = lib.optional wayland "weston.service";
      environment = { TMPDIR = "/var/tmp"; } // lib.optionalAttrs wayland {
        XDG_RUNTIME_DIR = lib.mkDefault runtime;
        WAYLAND_DISPLAY = "${runtime}/wayland-console";
        LIBGL_ALWAYS_SOFTWARE = "1";
      };
      unitConfig = {
        SuccessAction = "poweroff";
        FailureAction = "poweroff";
      };
      serviceConfig = {
        User = "console-test";
        Type = "oneshot";
        StateDirectory = "console-test";
        TimeoutStartSec = seconds;
        ExecStart = lib.escapeShellArgs command;
        ExecStartPost = ''${pkgs.coreutils}/bin/printf "\nCONSOLE_GUEST_TEST=passed\n"'';
        StandardOutput = "tty";
        StandardError = "inherit";
        TTYPath = "/dev/ttyS0";
      };
    };
  });
in
pkgs.runCommand "console-test-${name}" {} ''
  set -euo pipefail
  mkdir -p "$out"
  scratch=$(mktemp -d)
  trap 'rm -rf -- "$scratch"' EXIT
  env -i TMPDIR="$scratch" USE_TMPDIR=1 \
    ${lib.optionalString (diskSize != null) ''NIX_DISK_IMAGE="$scratch/root.qcow2"''} \
    ${pkgs.coreutils}/bin/timeout --kill-after=10s ${toString (seconds + 240)}s \
    ${lib.getExe guest.config.system.build.vm} </dev/null 2>&1 | tee "$out/console.log"
  tr -d '\r' < "$out/console.log" | grep -x 'CONSOLE_GUEST_TEST=passed' >/dev/null || {
    echo 'The VM exited without reporting a successful test.' >&2
    exit 1
  }
''
