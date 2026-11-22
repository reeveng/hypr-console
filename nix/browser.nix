{ lib, rustPlatform, pkg-config, makeWrapper, wayland, chromium, webkitWorker ? null, src }:

let
  source = import ./source.nix { inherit lib src; };
in
rustPlatform.buildRustPackage {
  pname = "console-browser";
  version = "0.1.0";
  src = source;
  cargoLock.lockFile = src + "/Cargo.lock";

  nativeBuildInputs = [ pkg-config makeWrapper ];
  buildInputs = [ wayland ];
  buildFeatures = [ "console-browser-window/checks" ];
  cargoBuildFlags = [ "-p" "console-browser-window" ];
  cargoTestFlags = [ "-p" "console-browser-window" "-p" "console-browser-renderer" "-p" "console-core-web-engine" ];
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";

  postInstall = ''
    desktop="$out/share/applications/console-browser-window.desktop"
    install -Dm644 files/usr/share/applications/console-browser-window.desktop "$desktop"
    substituteInPlace "$desktop" --replace-fail /usr/local/bin/ "$out/bin/"
  '';

  postFixup = ''
    wrapProgram "$out/bin/console-browser-window" \
      --prefix PATH : ${lib.makeBinPath [ chromium ]}
  '' + lib.optionalString (webkitWorker != null) ''
    ln -s ${webkitWorker}/bin/console-browser-renderer-webkit "$out/bin/"
  '';

  meta.mainProgram = "console-browser-window";
}
