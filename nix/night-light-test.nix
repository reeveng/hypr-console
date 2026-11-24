{ pkgs, source, src }:
pkgs.rustPlatform.buildRustPackage {
  pname = "console-night-light-check";
  version = "0.1.0";
  src = source;
  cargoLock.lockFile = src + "/Cargo.lock";
  cargoBuildFlags = [ "-p" "console-night-light" ];
  cargoTestFlags = [ "-p" "console-night-light" ];
  buildType = "debug";
  checkType = "debug";
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";
  CARGO_PROFILE_DEV_DEBUG = "0";
  CARGO_PROFILE_TEST_DEBUG = "0";

  nativeCheckInputs = [ pkgs.python3 ];
  postCheck = ''
    python3 nix/night-light-wayland.py target/x86_64-unknown-linux-gnu/debug/console-warm
  '';
}
