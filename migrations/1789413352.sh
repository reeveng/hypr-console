# Sixteen programs, a unit and a menu entry, under the names they had before
# `[build]` had a rule.
#
# A binary in `[build]` is installed into /usr/local/bin under its own name, and
# an apply installs a name and has never removed one. So a machine that applied
# the commit before this one has both spellings of every renamed program sitting
# beside each other, and -- this is what makes it worth a migration rather than
# a tidy -- the old one still runs. It is the same code compiled a day earlier.
# Nothing tells a person which of the two they have, and nothing here would ever
# say the old one is wrong, because it is not: it is right and it is stale, and
# those look identical from a prompt.
#
# The unit is the half that bites on its own. `console-sky.service` is enabled
# and pulled in by console.target, and the wallpaper's unit is
# `console-wallpaper.service` now. An apply that installs the new one leaves the
# old one enabled beside it: two daemons choosing a picture for one screen, each
# writing over the other every time the weather moves. Stopping comes before
# disabling for the reason the earlier unit rename wrote down -- `disable` takes
# the unit out of the wants and leaves the process running -- and `console apply`
# runs its migrations before any section, so this is the moment there is one of
# each.
#
# The menu entry is smaller and is the one someone sees. `console-music.desktop`
# is what mimeapps.list points at now; `console-music-panel.desktop` is still in
# /usr/share/applications, still names a program that still exists, and so still
# draws a second Music in the launcher, identical to the first.
#
# What is deliberately not swept is the four the rename did not touch:
# `bar-door` and `bar-updating` changed which crate builds them and kept their
# names, so nothing about them left the manifest and nothing is left behind.
#
# sweeps: /usr/local/bin/console-sky
# sweeps: /usr/local/bin/desktop-mode
# sweeps: /usr/local/bin/dictate
# sweeps: /usr/local/bin/download-find
# sweeps: /usr/local/bin/download-get
# sweeps: /usr/local/bin/download-panel
# sweeps: /usr/local/bin/game-mode
# sweeps: /usr/local/bin/game-return
# sweeps: /usr/local/bin/layout-panel
# sweeps: /usr/local/bin/notices-panel
# sweeps: /usr/local/bin/one-format
# sweeps: /usr/local/bin/put-away
# sweeps: /usr/local/bin/sky-press
# sweeps: /usr/local/bin/stick-scroll
# sweeps: /usr/local/bin/switch-language
# sweeps: /usr/local/bin/virtual-keyboard
# sweeps: /etc/systemd/user/console-sky.service
# sweeps: /usr/share/applications/console-music-panel.desktop
# sweeps: enabled console-sky.service

whoever=$(basename "$CONSOLE_HOME")

echo "stopping and disabling the wallpaper under the name it used to have"

systemctl --user -M "$whoever@" stop console-sky.service || echo "  console-sky.service was not running"
systemctl --user -M "$whoever@" disable console-sky.service || echo "  console-sky.service was not enabled"

console-attic /etc/systemd/user/console-sky.service

echo "sweeping the sixteen programs that were named before [build] had a rule"

for at in \
  /usr/local/bin/console-sky \
  /usr/local/bin/desktop-mode \
  /usr/local/bin/dictate \
  /usr/local/bin/download-find \
  /usr/local/bin/download-get \
  /usr/local/bin/download-panel \
  /usr/local/bin/game-mode \
  /usr/local/bin/game-return \
  /usr/local/bin/layout-panel \
  /usr/local/bin/notices-panel \
  /usr/local/bin/one-format \
  /usr/local/bin/put-away \
  /usr/local/bin/sky-press \
  /usr/local/bin/stick-scroll \
  /usr/local/bin/switch-language \
  /usr/local/bin/virtual-keyboard
do
  console-attic "$at"
done

echo "sweeping the Music entry under the name it used to have"

console-attic /usr/share/applications/console-music-panel.desktop
