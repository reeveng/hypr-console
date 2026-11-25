{ lib, rustPlatform, servo, src }:

let
  worker = "crates/console-browser-renderer-servo";
in
rustPlatform.buildRustPackage {
  pname = "console-browser-renderer-servo";
  version = "0.1.0";
  src = import ./source.nix { inherit lib src; };
  cargoRoot = worker;
  buildAndTestSubdir = worker;
  cargoLock.lockFile = src + "/${worker}/Cargo.lock";
  nativeBuildInputs = servo.nativeBuildInputs;
  buildInputs = servo.buildInputs;
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";
  inherit (servo) preConfigure UV_PYTHON NIX_CFLAGS_COMPILE;

  # SpiderMonkey invokes Cargo inside the vendored crate while building its bindings.
  postPatch = ''
    cp .cargo/config.toml "$cargoDepsCopy/.cargo/config.toml"
    for mozjs_dir in "$cargoDepsCopy"/*/mozjs_*/; do
      cp ${worker}/Cargo.lock "$mozjs_dir"
    done
  '';
  doCheck = false;
}
