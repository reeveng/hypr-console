{ config, lib, pkgs, consoleSource ? ../., ... }:
let
  cfg = config.services.consoleDesktop;
  source = cfg.source;
  manifest = import ./manifest.nix (source + "/desktop.conf");
  runtime = import ./runtime.nix { inherit pkgs manifest; };
  environment = runtime.environment;
  package = cfg.package;
  balancedPlayback = pkgs.runCommand "console-balanced-playback" {} ''
    install -Dm644 ${source + "/files/etc/wireplumber/wireplumber.conf.d/60-console-balanced-playback.conf"} \
      "$out/share/wireplumber/wireplumber.conf.d/60-console-balanced-playback.conf"
    install -Dm644 ${source + "/files/usr/local/share/wireplumber/scripts/console-balanced-playback.lua"} \
      "$out/share/wireplumber/scripts/console-balanced-playback.lua"
  '';
  applicationVolumes = pkgs.writeTextDir "share/pipewire/pipewire-pulse.conf.d/console-electron.conf"
    (builtins.readFile (source + "/files/home/@user@/.config/pipewire/pipewire-pulse.conf.d/console-electron.conf"));
  playbackGain = pkgs.writeTextDir "share/pipewire/pipewire-pulse.conf.d/60-console-playback-gain.conf"
    (builtins.readFile (source + "/files/etc/pipewire/pipewire-pulse.conf.d/60-console-playback-gain.conf"));
  minimumRust = (builtins.fromTOML (builtins.readFile (source + "/Cargo.toml")))
    .workspace.package.rust-version;
  replace = text: builtins.replaceStrings
    [ "/usr/lib/polkit-kde-authentication-agent-1" "/usr/local/bin/" "/usr/bin/" "@user@" ]
    [ "${pkgs.kdePackages.polkit-kde-agent-1}/libexec/polkit-kde-authentication-agent-1"
      "${package}/bin/" "${environment}/bin/" cfg.user ] text;

  # Retain the project's service definitions, including their restart and
  # confinement choices. The Arch manifest health check is not a NixOS check.
  unitRoot = source + "/files/etc/systemd/user";
  omitted = [ "console.target" "console-health.service" "console-health.timer"
    "console-return.service" "syncthing.service" ];
  unitNames = builtins.filter (name:
    lib.hasPrefix "console" name
    && (builtins.readDir unitRoot).${name} == "regular"
    && !(builtins.elem name omitted)
  ) (builtins.attrNames (builtins.readDir unitRoot));
  wanted = builtins.filter (name: !(builtins.elem name omitted)) manifest.services;
  units = builtins.listToAttrs (map (name: {
    inherit name;
    value.text = replace (builtins.readFile (unitRoot + "/${name}"));
  }) unitNames);
  drops = [ "console-.service.d/restarting.conf" "console-.service.d/confining.conf"
    "console-bar.service.d/opening.conf" "console-input-controller.service.d/opening.conf"
    "console-home.service.d/opening.conf" "console-panels.service.d/opening.conf"
    "console-session.service.d/opening.conf"
    "pipewire-pulse.service.d/console-playback-gain.conf"
    "wireplumber.service.d/console-playback-gain.conf" ];

  seedPrefix = "/home/@user@/.config/console/";
  seedNames = builtins.filter (name: lib.hasPrefix seedPrefix name) manifest.files;
  seeds = pkgs.runCommand "console-configuration-seeds" {} (
    lib.concatMapStringsSep "\n" (name:
      let relative = lib.removePrefix seedPrefix name;
          content = pkgs.writeText (builtins.baseNameOf name)
            (replace (builtins.readFile (source + "/files${name}")));
      in ''install -Dm644 ${content} "$out/${relative}"''
    ) seedNames
  );
  monitor = pkgs.writeText "console-monitor.lua" ''
    hl.monitor({ output = "${cfg.monitor.output}", mode = "preferred",
      position = "auto", scale = ${toString cfg.monitor.scale},
      transform = ${toString cfg.monitor.transform} })
    hl.config({ input = { kb_layout = "${cfg.keyboard.layout}", kb_options = "${cfg.keyboard.options}",
      touchdevice = { output = "${cfg.monitor.output}",
        transform = ${toString cfg.monitor.transform} } } })
  '';
  sessionStart = pkgs.writeShellScript "console-session" ''
    set -eu
    export PATH="${package}/bin:${environment}/bin:$PATH"
    export XDG_DATA_DIRS="${package}/share:${environment}/share:''${XDG_DATA_DIRS:-/run/current-system/sw/share}"
    export XDG_CURRENT_DESKTOP=Hyprland
    export XDG_SESSION_TYPE=wayland
    export XDG_SESSION_DESKTOP=console
    # Copy only missing files in Console's own directory. Existing user files,
    # browser profiles, terminal settings and the other desktop stay theirs.
    ${lib.concatMapStringsSep "\n" (name:
      let relative = lib.removePrefix seedPrefix name;
      in ''
        if [ ! -e "$HOME/.config/console/${relative}" ]; then
          ${pkgs.coreutils}/bin/install -Dm600 ${seeds}/${relative} "$HOME/.config/console/${relative}"
        fi
      ''
    ) seedNames}
    if [ ! -e "$HOME/.config/console/hypr/monitor.lua" ]; then
      ${pkgs.coreutils}/bin/install -Dm600 ${monitor} "$HOME/.config/console/hypr/monitor.lua"
    fi
    trap '${pkgs.systemd}/bin/systemctl --user stop console.target || true' EXIT
    ${package}/bin/console-compositor start "$HOME/.config/console/hypr/hyprland.lua"
  '';
  session = pkgs.runCommand "console-wayland-session" {
    passthru.providedSessions = [ "console" ];
  } ''
    mkdir -p "$out/share/wayland-sessions"
    cat > "$out/share/wayland-sessions/console.desktop" <<EOF
    [Desktop Entry]
    Name=Console
    Comment=The Rust desktop from the Legion Go project
    Exec=${sessionStart}
    Type=Application
    DesktopNames=Hyprland
    EOF
  '';
in
{
  options.services.consoleDesktop = {
    enable = lib.mkEnableOption "the Console desktop from the Legion Go project";
    user = lib.mkOption { type = lib.types.str; description = "Existing desktop account"; };
    source = lib.mkOption {
      type = lib.types.path;
      default = if builtins.isAttrs consoleSource then consoleSource.outPath else consoleSource;
    };
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ./package.nix { src = source; };
    };
    monitor = {
      output = lib.mkOption { type = lib.types.str; default = "eDP-1"; };
      scale = lib.mkOption { type = lib.types.number; default = 1; };
      transform = lib.mkOption { type = lib.types.int; default = 0; };
    };
    keyboard = {
      layout = lib.mkOption { type = lib.types.str; default = "us"; };
      options = lib.mkOption { type = lib.types.str; default = ""; };
    };
  };
  config = lib.mkIf cfg.enable {
    assertions = [
      { assertion = builtins.hasAttr cfg.user config.users.users;
        message = "Console requires an existing users.users account."; }
      { assertion = lib.versionAtLeast pkgs.rustc.version minimumRust;
        message = "Console requires Rust >= ${minimumRust}; selected ${pkgs.rustc.version}."; }
      { assertion = lib.versionAtLeast pkgs.hyprland.version "0.56";
        message = "Console requires a Hyprland version supporting its current Lua configuration (>= 0.56)."; }
    ];
    nix.enable = true;
    nix.settings.extra-experimental-features = [ "nix-command" "flakes" ];
    programs.hyprland.enable = true;
    services.displayManager.sessionPackages = [ session ];
    services.pipewire = {
      enable = true;
      alsa.enable = true;
      pulse.enable = true;
      configPackages = [ applicationVolumes playbackGain ];
      wireplumber.configPackages = [ balancedPlayback ];
    };
    security.rtkit.enable = true;
    security.polkit.enable = true;
    networking.networkmanager.enable = true;
    hardware.bluetooth.enable = true;
    hardware.uinput.enable = true;
    services.power-profiles-daemon.enable = true;
    users.users.${cfg.user}.extraGroups = [ "input" "video" "networkmanager" ];
    services.udev.extraRules = builtins.replaceStrings
      [ "/usr/bin/chgrp" "/usr/bin/chmod" ]
      [ "${pkgs.coreutils}/bin/chgrp" "${pkgs.coreutils}/bin/chmod" ]
      (builtins.concatStringsSep "\n" (map (name: builtins.readFile (source + "/files/etc/udev/rules.d/${name}")) [
        "90-console-backlight.rules"
        "91-console-charge-limit.rules"
      ]));
    environment.systemPackages = [ package environment ];
    environment.pathsToLink = [ "/share/applications" "/share/icons" ];
    fonts.packages = [ pkgs.noto-fonts pkgs.noto-fonts-cjk-sans
      pkgs.noto-fonts-color-emoji pkgs.nerd-fonts.fantasque-sans-mono ];
    systemd.user.units = units // {
      "console.target".text = ''
        [Unit]
        Description=Console desktop
        ConditionUser=${cfg.user}
        Wants=${lib.concatStringsSep " " wanted}
      '';
    };
    environment.etc = builtins.listToAttrs (map (name: {
      name = "systemd/user/${name}";
      value.text = replace (builtins.readFile (unitRoot + "/${name}"));
    }) drops) // {
      "console-auth/console-fingerprint" = lib.mkIf config.services.fprintd.enable {
        text = builtins.replaceStrings
          [ "pam_fprintd.so" "pam_permit.so" ]
          [ "${config.services.fprintd.package}/lib/security/pam_fprintd.so"
            "${config.security.pam.package}/lib/security/pam_permit.so" ]
          (builtins.readFile (source + "/files/etc/console-auth/console-fingerprint"));
      };
      "console-auth/console-lock-password".text = builtins.replaceStrings
        [ "pam_unix.so" "pam_faillock.so" "pam_permit.so" ]
        [ "${config.security.pam.pam_unixModulePath}"
          "${config.security.pam.package}/lib/security/pam_faillock.so"
          "${config.security.pam.package}/lib/security/pam_permit.so" ]
        (builtins.readFile (source + "/files/etc/console-auth/console-lock-password"));
      "console-auth/other".text = builtins.replaceStrings
        [ "pam_deny.so" ] [ "${config.security.pam.package}/lib/security/pam_deny.so" ]
        (builtins.readFile (source + "/files/etc/console-auth/other"));
    };
    # No Arch sudoers, mkinitcpio, display-manager, package-manager or browser
    # policy files are copied. NixOS owns those integration points.
  };
}
