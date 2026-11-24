{ lib, rustPlatform, makeWrapper, chromium, src }:

rustPlatform.buildRustPackage {
  pname = "console-browser-renderer-chromium";
  version = "0.1.0";
  src = import ./source.nix { inherit lib src; };
  cargoLock.lockFile = src + "/Cargo.lock";

  nativeBuildInputs = [ makeWrapper ];
  cargoBuildFlags = [ "-p" "console-browser-renderer-chromium" ];
  doCheck = false;
  RUSTC_BOOTSTRAP = "console_operating_system_linux";
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";

  postFixup = ''
    wrapProgram "$out/bin/console-browser-renderer-chromium" \
      --prefix PATH : ${lib.makeBinPath [ chromium ]}
  '';
}
