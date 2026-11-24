{ lib, rustPlatform, pkg-config, wayland, chromiumWorker ? null, webkitWorker ? null, src }:

let
  source = import ./source.nix { inherit lib src; };
in
rustPlatform.buildRustPackage {
  pname = "console-application-browser";
  version = "0.1.0";
  src = source;
  cargoLock.lockFile = src + "/Cargo.lock";

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ wayland ];
  buildFeatures = [ "console-application-browser/checks" ];
  cargoBuildFlags = [ "-p" "console-application-browser" ];
  cargoTestFlags = [ "-p" "console-application-browser" "-p" "console-browser-renderer" "-p" "console-core-web-engine" ];
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";

  postInstall = ''
    desktop="$out/share/applications/console-browser-window.desktop"
    install -Dm644 files/usr/share/applications/console-browser-window.desktop "$desktop"
    substituteInPlace "$desktop" --replace-fail /usr/local/bin/ "$out/bin/"
  '';

  postFixup = lib.optionalString (chromiumWorker != null) ''
    ln -s ${chromiumWorker}/bin/console-browser-renderer-chromium "$out/bin/"
  '' + lib.optionalString (webkitWorker != null) ''
    ln -s ${webkitWorker}/bin/console-browser-renderer-webkit "$out/bin/"
  '';

  meta.mainProgram = "console-application-browser";
}
