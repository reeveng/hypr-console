# The four installed names the crates' rename took with it.
#
# `console-controller` became `console-input-controller` and `console-keyboard`
# became `console-input-keyboard`, so the units those two run under, the drop-in
# beside one of them, and the two desktop entries named after `console-viewer`
# and `console-download` are all installed under names the manifest has stopped
# saying. Everything else in that rename was a crate, which is a directory here
# and nothing at all on a machine.
#
# The two units are why this is not tidying. A machine that applied the commit
# before this one has both of them enabled and pulled in by console.target, and
# an apply that installs the new pair leaves the old pair beside it: two daemons
# reading the same pad, two keyboards answering the same key.
#
# Stopping is the half that is easy to forget. `disable` takes the unit out of
# the wants and leaves the process running, and the programs behind these two
# units did not change their names -- only the units did -- so what is running
# would keep running against the same binary the new unit starts a second copy
# of. `console apply` runs its migrations before any section, so this is the
# moment there is one of each: stop, then disable, then move the files.
#
# sweeps: /etc/systemd/user/console-controller.service
# sweeps: /etc/systemd/user/console-controller.service.d/opening.conf
# sweeps: /etc/systemd/user/console-keyboard.service
# sweeps: /usr/share/applications/console-download.desktop
# sweeps: /usr/share/applications/console-viewer.desktop
# sweeps: enabled console-controller.service
# sweeps: enabled console-keyboard.service

whoever=$(basename "$CONSOLE_HOME")

echo "stopping and disabling the pair the rename replaced"

for unit in \
  console-controller.service \
  console-keyboard.service
do
  systemctl --user -M "$whoever@" stop "$unit" || echo "  $unit was not running"
  systemctl --user -M "$whoever@" disable "$unit" || echo "  $unit was not enabled"
done

echo "sweeping what the manifest stopped naming"

for at in \
  /etc/systemd/user/console-controller.service \
  /etc/systemd/user/console-controller.service.d/opening.conf \
  /etc/systemd/user/console-controller.service.d \
  /etc/systemd/user/console-keyboard.service \
  /usr/share/applications/console-download.desktop \
  /usr/share/applications/console-viewer.desktop
do
  console-attic "$at"
done
