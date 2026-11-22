# Optional first-generation Legion Go evaluation profile, not a disk layout.
# Import alongside nixosModules.console-desktop, then supply user and actual
# hardware-configuration.nix from the target device. See the nixos.md note in
# Orbit.
{ config, lib, pkgs, ... }:
{
  services.consoleDesktop.monitor = {
    output = lib.mkDefault "eDP-1";
    scale = lib.mkDefault 2.5;
    transform = lib.mkDefault 1;
  };
  hardware.cpu.amd.updateMicrocode = true;
  hardware.enableRedistributableFirmware = true;
  hardware.graphics = { enable = true; enable32Bit = true; };
  boot.initrd.kernelModules = [ "amdgpu" ];
  boot.kernelPackages = lib.mkDefault pkgs.linuxPackages_latest;
  programs.steam = {
    enable = true;
    gamescopeSession.enable = true;
  };
  programs.gamescope.enable = true;
  programs.gamemode.enable = true;
  environment.systemPackages = [ pkgs.mangohud pkgs.vulkan-tools ];
  # InputPlumber device overrides and Console <-> Steam switching require a
  # native NixOS implementation before enabling the handheld Console session.
  # Do not import the CachyOS plasmalogin or Limine/mkinitcpio configuration.
}
