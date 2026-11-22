{ lib, rustPlatform, rustc, pkg-config
, wayland, pam, libxcrypt, isocodes, xkeyboard_config, shared-mime-info
, tzdata, runtimeShell, src }:
let
  source = import ./source.nix { inherit lib src; };
  manifest = import ./manifest.nix (src + "/desktop.conf");
  minimum = (builtins.fromTOML (builtins.readFile (src + "/Cargo.toml")))
    .workspace.package.rust-version;
in
rustPlatform.buildRustPackage {
  pname = "legion-console";
  version = "0.1.0";
  src = source;
  cargoLock.lockFile = src + "/Cargo.lock";

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ wayland pam libxcrypt ];
  cargoBuildFlags = lib.concatMap (name: [ "--bin" name ]) manifest.build;
  # The upstream gate remains `just ready`. Device and display tests cannot run
  # as a headless Nix package check; the port's build is not a substitute for it.
  doCheck = false;

  # Prevent the checkout's target-cpu=native from producing machine-specific
  # artifacts under a shared Nix derivation identity.
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";
  preBuild = lib.optionalString (lib.versionOlder rustc.version minimum) ''
    echo "Console needs Rust >= ${minimum}; selected Nixpkgs provides ${rustc.version}." >&2
    exit 1
  '';

  postPatch = ''
    # Transform only the build sandbox, never the development checkout.
    find crates -name '*.rs' -exec sed -i \
      -e "s|/usr/local/bin|$out/bin|g" \
      -e 's|"/usr/local/lib/console|"${source}/files/usr/local/lib/console|g' \
      -e "s|/etc/console|${source}|g" \
      -e 's|/usr/share/iso-codes|${isocodes}/share/iso-codes|g' \
      -e 's|/usr/share/X11/xkb|${xkeyboard_config}/share/X11/xkb|g' \
      -e 's|/usr/share/mime/globs2|${shared-mime-info}/share/mime/globs2|g' \
      -e 's|/usr/share/zoneinfo|${tzdata}/share/zoneinfo|g' \
      -e "s|/usr/share/icons/console-placeholder.svg|$out/share/icons/console-placeholder.svg|g" \
      -e 's|/usr/lib/signal-desktop/signal-desktop|signal-desktop|g' {} +
  '';

  postInstall = ''
    mkdir -p "$out/libexec/arch-only" "$out/share/console"
    cp -r theme "$out/share/console/"
    mkdir -p "$out/share/applications" "$out/share/icons"
    cp files/usr/share/applications/console-*.desktop "$out/share/applications/"
    cp files/usr/share/icons/console-placeholder.svg "$out/share/icons/"
    sed -i \
      -e "s|/usr/local/bin/|$out/bin/|g" \
      -e 's|/usr/bin/|/run/current-system/sw/bin/|g' \
      -e "s|/usr/share/icons/console-placeholder.svg|$out/share/icons/console-placeholder.svg|g" \
      "$out"/share/applications/*.desktop

    # These commands currently write Arch-managed files or operate the
    # handheld's Arch game session. Keep them out of routine use on NixOS.
    # Their NixOS backends are an explicit remaining port task.
    for program in console console-engine console-machine session-game; do
      mv "$out/bin/$program" "$out/libexec/arch-only/$program"
      cat > "$out/bin/$program" <<'GUARD'
#!${runtimeShell}
echo "This Console administration command still requires its NixOS backend. Manage system changes through the migration flake; do not run console apply on NixOS." >&2
exit 64
GUARD
      chmod +x "$out/bin/$program"
    done

    cat > "$out/bin/steamos-session-select" <<'GUARD'
#!${runtimeShell}
echo "Handheld game-mode switching is not configured for this ThinkPad session." >&2
exit 64
GUARD
    chmod +x "$out/bin/steamos-session-select"
  '';

  passthru = { inherit manifest minimum; };
  meta = {
    description = "Rust Console desktop from the Legion Go project, with a staged NixOS port";
    license = lib.licenses.agpl3Plus;
    platforms = [ "x86_64-linux" ];
  };
}
