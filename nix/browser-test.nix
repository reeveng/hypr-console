{ pkgs, runVM, browser, engine, servo ? null, webkitTests ? null }:

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
  ] ++ pkgs.lib.optional (engine == "servo") "${servo}/bin/console-browser-renderer-servo";
  module = {
    fonts.packages = [ pkgs.dejavu_fonts ];
    systemd.services.console-test.serviceConfig.ExecStartPre = pkgs.lib.optional (webkitTests != null) webkitTests;
  };
}
