# Everything the manifest stopped naming after the rename, and never swept.
#
# The rename had `tools/console-migrate` behind it and this had nothing. Between
# the two, a program stopped being a shell script and became a crate, the
# keyboard stopped being wvkbd and became one of ours, `console-poke` split into
# two crates named for what they are, and each of those left the name it had
# been installed under sitting in /usr/local/bin. None of it is running and
# nothing in the tree reaches for any of it -- the device was read before this
# was written -- so what this sweeps is dead weight rather than a fault.
#
# It is written anyway, and not skipped as harmless, because the mechanism that
# left them there is the one that nearly left seven `legion-*` units enabled
# beside their replacements, both wanting the pad. What is inert this time was
# luck about which things happened to be removed, not a property of anything.
#
# sweeps: /etc/firefox/policies/policies.json
# sweeps: /etc/inputplumber/profiles/desktop.yaml
# sweeps: /etc/inputplumber/profiles/keyboard.yaml
# sweeps: /etc/inputplumber/profiles/menu.yaml
# sweeps: /etc/inputplumber/profiles/tabs.yaml
# sweeps: /etc/plasmalogin.conf.d/zzz-session.conf
# sweeps: /home/@user@/.config/gtk-3.0/colors.css
# sweeps: /home/@user@/.config/gtk-4.0/colors.css
# sweeps: /home/@user@/.config/hypr/hyprpaper.conf
# sweeps: /home/@user@/.config/wofi/config
# sweeps: /home/@user@/.config/wofi/guide.css
# sweeps: /home/@user@/.config/wofi/style.css
# sweeps: /usr/local/bin/console-pictures
# sweeps: /usr/local/bin/console-poke
# sweeps: /usr/local/bin/console-timings
# sweeps: /usr/local/bin/home-place
# sweeps: /usr/local/bin/keyboard-start
# sweeps: /usr/local/bin/osk
# sweeps: /usr/local/bin/osk-hook
# sweeps: /usr/local/bin/osk-start
# sweeps: /usr/local/bin/power-menu
# sweeps: /usr/local/bin/wvkbd-mobintl
# sweeps: /usr/share/applications/console-notices.desktop

echo "sweeping what the manifest stopped naming after the rename"

for at in \
  /etc/firefox/policies/policies.json \
  /etc/inputplumber/profiles/desktop.yaml \
  /etc/inputplumber/profiles/keyboard.yaml \
  /etc/inputplumber/profiles/menu.yaml \
  /etc/inputplumber/profiles/tabs.yaml \
  /etc/plasmalogin.conf.d/zzz-session.conf \
  "$CONSOLE_HOME/.config/gtk-3.0/colors.css" \
  "$CONSOLE_HOME/.config/gtk-4.0/colors.css" \
  "$CONSOLE_HOME/.config/hypr/hyprpaper.conf" \
  "$CONSOLE_HOME/.config/wofi/config" \
  "$CONSOLE_HOME/.config/wofi/guide.css" \
  "$CONSOLE_HOME/.config/wofi/style.css" \
  /usr/local/bin/console-pictures \
  /usr/local/bin/console-poke \
  /usr/local/bin/console-timings \
  /usr/local/bin/home-place \
  /usr/local/bin/keyboard-start \
  /usr/local/bin/osk \
  /usr/local/bin/osk-hook \
  /usr/local/bin/osk-start \
  /usr/local/bin/power-menu \
  /usr/local/bin/wvkbd-mobintl \
  /usr/share/applications/console-notices.desktop
do
  console-attic "$at"
done

# The keyboard's own era, which the manifest never named and so the gate cannot
# ask for. They came in beside the wvkbd fork and went when `virtual-keyboard`
# became a crate; a sweep that took the four the manifest happens to remember
# and left these three beside them would be tidying by paperwork.
for at in \
  /usr/local/bin/osk-toggle \
  /usr/local/bin/wvkbd-kwin \
  "$CONSOLE_HOME/.config/hypr/hyprland.lua.working"
do
  console-attic "$at"
done
