{ lib, rustPlatform, pkg-config, makeWrapper, mesa, wpewebkit, python3, withTests ? false
, glib-networking, gst_all_1, src, brotli, libidn2, libnghttp2, libpsl
, libselinux, libsepol, pcre2, sqlite, util-linuxMinimal, zstd }:

let
  source = import ./source.nix { inherit lib src; };
  worker = "crates/console-browser-renderer-webkit";
  gstreamerPlugins = with gst_all_1; [
    gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav
  ];
in
rustPlatform.buildRustPackage {
  pname = "console-browser-renderer-webkit";
  version = "0.1.0";
  src = source;
  cargoRoot = worker;
  buildAndTestSubdir = worker;
  cargoLock.lockFile = src + "/${worker}/Cargo.lock";

  nativeBuildInputs = [ pkg-config makeWrapper ];
  # pkg-config reads GLib and libsoup's private requirements while resolving WPE.
  buildInputs = [
    wpewebkit brotli libidn2 libnghttp2 libpsl libselinux libsepol pcre2 sqlite
    util-linuxMinimal zstd
  ];
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C target-cpu=x86-64";
  doCheck = false;
  postBuild = lib.optionalString withTests ''
    set -o pipefail
    test_binary=$(cargo test --manifest-path ${worker}/Cargo.toml --offline --locked \
      --no-run --message-format=json \
      | ${python3}/bin/python3 ${./test-executables.py})
    install -Dm755 "$test_binary" "$out/libexec/console-webkit-tests"
  '';

  postFixup = ''
    wrapProgram "$out/bin/console-browser-renderer-webkit" \
      --set-default __EGL_VENDOR_LIBRARY_DIRS "/run/opengl-driver/share/glvnd/egl_vendor.d:${mesa}/share/glvnd/egl_vendor.d" \
      --prefix GIO_EXTRA_MODULES : ${glib-networking}/lib/gio/modules \
      --prefix GST_PLUGIN_SYSTEM_PATH_1_0 : ${lib.makeSearchPath "lib/gstreamer-1.0" gstreamerPlugins}
  '';
}
