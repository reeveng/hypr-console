{ src ? ../.
, configuration ? src + "/configuration.nix"
, pkgs ? let
    lock = builtins.fromJSON (builtins.readFile (src + "/flake.lock"));
  in import (builtins.fetchTree lock.nodes.nixpkgs.locked) { system = "x86_64-linux"; }
}:
let
  inherit (pkgs) lib;
  programs = import ./native-configuration.nix { inherit lib configuration; };
  source = lib.cleanSourceWith {
    inherit src;
    filter = path: _:
      let
        relative = lib.removePrefix "${toString src}/" path;
        top = builtins.head (lib.splitString "/" relative);
      in builtins.elem top [ ".cargo" "Cargo.toml" "Cargo.lock" "crates" "forks" ];
  };
  calculatorSource = src + "/crates/console-application-calculator";
  calculator = pkgs.runCommand "console-native-calculator" {
    nativeBuildInputs = [ pkgs.rustc pkgs.stdenv.cc.bintools ];
  } ''
    rustc --edition=2024 --crate-type=lib --target=x86_64-unknown-linux-gnu \
      --emit=obj -C opt-level=s -C panic=abort -C no-redzone=yes \
      -o calculator.o ${calculatorSource}/native/entry.rs
    ld --oformat=binary -T ${calculatorSource}/native/link.ld \
      -o "$out" calculator.o
  '';
  system = pkgs.runCommand "console-native-programs" { } ''
    mkdir -p "$out"
    ${lib.optionalString programs.calculator.enable ''
      cp ${calculator} "$out/calculator"
    ''}
  '';
  crossRustc = pkgs.pkgsCross.x86_64-unknown-uefi.buildPackages.rustc;
  uefiSysroot = crossRustc.unwrapped.overrideAttrs (old: {
    # The cross package adds the UEFI sysroot. Rust's host package carries the manuals.
    installPhase = lib.concatStringsSep "\n" (lib.filter
      (line: !(lib.hasPrefix "lndir " (lib.trim line)))
      (lib.splitString "\n" old.installPhase));
  });
  rustPlatform = pkgs.makeRustPlatform {
    rustc = crossRustc.override { rustc-unwrapped = uefiSysroot; sysroot = uefiSysroot; };
    inherit (pkgs) cargo;
  };
  target = "x86_64-unknown-uefi";
in
rustPlatform.buildRustPackage {
  pname = "console-native-generation";
  version = "0.1.0";
  src = source;
  cargoLock.lockFile = src + "/Cargo.lock";
  nativeBuildInputs = [ crossRustc.llvmPackages.lld ];
  CONSOLE_SYSTEM = system;

  auditable = false;
  doCheck = false;
  dontFixup = true;
  buildPhase = ''
    runHook preBuild
    cargo build --offline --locked --release --package console-kernel \
      --target ${target} -j "$NIX_BUILD_CORES"
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    install -Dm644 target/${target}/release/console-kernel.efi "$out/esp/EFI/BOOT/BOOTX64.EFI"
    printf '%s\n' ${lib.escapeShellArg (builtins.toJSON { inherit programs; })} > "$out/configuration.json"
    runHook postInstall
  '';
}
