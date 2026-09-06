# What every migration is handed, so no migration writes its own `rm`.
#
# Sourced before each migration by `console migrate`, which also sets
# CONSOLE_ATTIC to the directory this run is putting things in and CONSOLE_HOME
# to the home the desktop belongs to. A migration is a shell script and may do
# anything; this is only so that the thing every one of them does is done the
# same way once.
#
# Nothing is deleted. `tools/console-migrate` moved the whole of the rename to
# /var/tmp and said where on its last line, and that attic is still on the
# device -- which is the only reason anybody can now say the rename's sweep did
# what it claimed rather than merely that it ran. A deletion is the same
# operation with nothing left to check it by.

console-attic() {
  at="$1"

  [ -e "$at" ] || [ -L "$at" ] || return 0

  under="$CONSOLE_ATTIC${at}"
  mkdir -p "$(dirname "$under")"
  mv "$at" "$under"
  echo "  $at -> $under"
}

# A unit that is enabled goes on being started whatever happened to the file it
# was enabled from, so the symlink is the thing to take and `disable` is what
# takes it. Failing is fine and quiet: a unit systemd has never heard of is a
# machine that already does not start it, which is the state being asked for.
console-unenable() {
  systemctl --user --global disable "$1" >/dev/null 2>&1 || true
  echo "  disabled $1"
}
