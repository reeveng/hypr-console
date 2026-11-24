{ pkgs, runVM, browser, webkit ? true }:

let
  engine = if webkit then "webkit" else "chromium";
in
runVM {
  name = "browser-${engine}";
  memory = 3072;
  wayland = true;
  command = [
    "${pkgs.dbus}/bin/dbus-run-session"
    "--dbus-daemon=${pkgs.dbus}/bin/dbus-daemon"
    "--"
    "${browser}/bin/console-browser-check"
    "--guest-${engine}"
    "${browser}/bin/console-application-browser"
  ];
  module.fonts.packages = [ pkgs.dejavu_fonts ];
}
