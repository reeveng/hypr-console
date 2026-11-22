{ lib, clangStdenv, fetchurl, replaceVars, addDriverRunpath, webkitBubblewrapPatch
, bison, cmake, gettext, glib, gperf, ninja, perl, pkg-config, python3, ruby, unifdef
, at-spi2-core, bubblewrap, enchant, flite, fontconfig, freetype, gst_all_1
, harfbuzzFull, hyphen, icu, lcms2, libavif, libbacktrace, libdrm, libepoxy, libgbm
, libgcrypt, libgpg-error, libjpeg, libjxl, libmanette, libpng, libseccomp, libsoup_3
, libsysprof-capture
, libtasn1, libwebp, libxkbcommon, libxml2, libxslt, sqlite, systemdLibs, woff2
, xdg-dbus-proxy, zlib }:

clangStdenv.mkDerivation (finalAttrs: {
  pname = "wpewebkit";
  version = "2.54.3";
  src = fetchurl {
    url = "https://wpewebkit.org/releases/wpewebkit-${finalAttrs.version}.tar.xz";
    hash = "sha256-eHr9Mq4j6sBhYhRlOQ9EpGLodKnrq6C2VSHXI3+opeQ=";
  };

  # WPE and the GTK port use the same GLib process launcher. Reuse the
  # nixpkgs patch so the sandbox can find installed libraries and helpers.
  patches = [
    (replaceVars webkitBubblewrapPatch {
      inherit (builtins) storeDir;
      inherit (addDriverRunpath) driverLink;
    })
  ];

  nativeBuildInputs = [
    addDriverRunpath bison cmake gettext glib gperf ninja perl
    perl.pkgs.FileCopyRecursive pkg-config python3 ruby unifdef
  ];
  buildInputs = [
    at-spi2-core enchant flite fontconfig freetype harfbuzzFull hyphen icu
    lcms2 libavif libbacktrace libdrm libepoxy libgbm libgcrypt libgpg-error
    libjpeg libjxl libmanette libpng libseccomp libtasn1 libwebp libxkbcommon
    libxml2 libxslt sqlite systemdLibs woff2 zlib
    gst_all_1.gst-plugins-base gst_all_1.gst-plugins-bad
  ];
  propagatedBuildInputs = [ glib libsoup_3 libsysprof-capture ];

  postPatch = "patchShebangs .";
  cmakeFlags = [
    "-DPORT=WPE"
    "-DENABLE_DOCUMENTATION=OFF"
    "-DENABLE_INTROSPECTION=OFF"
    "-DENABLE_WEBDRIVER=OFF"
    "-DENABLE_WPE_LEGACY_API=OFF"
    "-DENABLE_WPE_PLATFORM_DRM=OFF"
    "-DENABLE_WPE_PLATFORM_WAYLAND=OFF"
    "-DENABLE_WPE_PLATFORM_HEADLESS=ON"
    "-DENABLE_BUBBLEWRAP_SANDBOX=ON"
    "-DBWRAP_EXECUTABLE=${lib.getExe bubblewrap}"
    "-DDBUS_PROXY_EXECUTABLE=${lib.getExe xdg-dbus-proxy}"
  ];
  requiredSystemFeatures = [ "big-parallel" ];
  meta = {
    description = "WebKit with the WPEPlatform headless backend";
    homepage = "https://wpewebkit.org/";
    license = with lib.licenses; [ bsd2 lgpl21Plus ];
    platforms = [ "x86_64-linux" ];
  };
})
