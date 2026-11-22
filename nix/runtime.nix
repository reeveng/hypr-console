{ pkgs, manifest }:
let
  lib = pkgs.lib;
  decisions = builtins.fromJSON (builtins.readFile ./package-map.json);
  unhandled = builtins.filter (name: !(builtins.hasAttr name decisions)) manifest.packages;
  runtimeDecisions = builtins.filter
    (name: decisions.${name}.role == "runtime") manifest.packages;
  attrs = lib.unique (lib.concatMap (name: decisions.${name}.nix) runtimeDecisions);
  languages = map (name: lib.removePrefix "tesseract-data-" name)
    (builtins.filter (name: lib.hasPrefix "tesseract-data-" name) manifest.packages);
  resolve = attr:
    if attr == "tesseract" then pkgs.tesseract.override { enableLanguages = languages; }
    else lib.attrByPath (lib.splitString "." attr)
      (throw "Missing Console runtime package in pinned Nixpkgs: ${attr}") pkgs;
  # Base-system commands are explicit here: Origin::Arch in the Rust enum is
  # not an instruction to install pacman on NixOS.
  base = with pkgs; [
    bash coreutils util-linux procps systemd dbus iproute2 kmod
  ];
in
assert lib.assertMsg (unhandled == [])
  "Console packages need a NixOS decision: ${lib.concatStringsSep ", " unhandled}";
{
  inherit decisions;
  packages = map resolve attrs ++ base;
  environment = pkgs.buildEnv {
    name = "console-runtime";
    paths = map resolve attrs ++ base;
    pathsToLink = [ "/bin" "/share" ];
    ignoreCollisions = true;
    postBuild = ''
      if [ ! -e "$out/bin/7z" ]; then
        ln -s ${pkgs._7zz}/bin/7zz "$out/bin/7z"
      fi
    '';
  };
}
