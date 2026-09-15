# The three files that were in another desktop's directory.

# ~/.config/hypr belongs to whatever Hyprland desktop a machine already has.
# This one kept its compositor's config, its idle rules and its evening curve in
# there, which is exactly right while the desktop *is* the machine and is a
# collision the moment it is one session of several: on the laptop this was
# written for, omarchy's own hyprland.lua and hypridle.conf are in that
# directory, and an apply that owned those paths would have installed over them.
#
# So all three moved to ~/.config/console/hypr, the session entry in
# /usr/share/wayland-sessions names the compositor's file, and the two units
# hand their daemons the directory through XDG_CONFIG_HOME. desktop.conf carries
# the argument.
#
# What this takes is the three files at the old paths, which on this device this
# desktop wrote and which nothing reads any more. The directory itself is left:
# hyprlock, hyprpicker and anything else a person installs later will want it,
# and an empty directory costs nothing.
#
# sweeps: /home/@user@/.config/hypr/hyprland.lua
# sweeps: /home/@user@/.config/hypr/hypridle.conf
# sweeps: /home/@user@/.config/hypr/hyprsunset.conf

echo "sweeping the three files that were in ~/.config/hypr"

for at in \
  "$CONSOLE_HOME/.config/hypr/hyprland.lua" \
  "$CONSOLE_HOME/.config/hypr/hypridle.conf" \
  "$CONSOLE_HOME/.config/hypr/hyprsunset.conf"
do
  console-attic "$at"
done

# And which session logs in, which the manifest cannot do for this one. The
# autologin file is carried `theirs` -- `steamos-session-select` rewrites it on
# every crossing to Game Mode and back, so what is in it is a report rather than
# a setting, and an apply never compares it. A machine left naming
# hyprland.desktop would log in through the hyprland package's own entry, which
# runs the compositor with no argument and so reads the config that just moved:
# a stock Hyprland with no binds, on a handheld whose other way in is ssh.
#
# Only this machine has the file at all, and only if it says the old name.
autologin=/etc/plasmalogin.conf.d/zz-steamos-autologin.conf

if grep -q '^Session=hyprland.desktop$' "$autologin" 2>/dev/null; then
  sed -i 's/^Session=hyprland.desktop$/Session=console.desktop/' "$autologin"
  echo "  $autologin now logs in through console.desktop"
fi
