# The session keeper, under the name it was somebody else's program by.
#
# `/usr/local/bin/hyprsession` was a fork carried as a binary because its source
# was upstream's and this tree does not publish other people's work as its own.
# The source is here now, ported, as `crates/console-resume`, and `[build]`
# names `console-resume` -- so what the manifest installs has a new name and the
# old program is left in place on every machine that ever applied the old one.
#
# It cannot be left there. `console-session.service` now starts
# `console-resume`, but the old binary is still on the path under a name a
# person might type, and typing it is not harmless: its bare invocation is the
# mode that closes every window on the screen and rebuilds the desktop out of a
# file. That is the fault this port exists to end, and leaving a copy of it in
# /usr/local/bin leaves the fault reachable.
#
# The unit is not swept and is not disabled, because it is not the old
# program's. `console-session.service` was this tree's before the fork arrived
# and stays this tree's after it goes; what changed is one `ExecStart` line, and
# an apply rewrites the file, reloads and starts it again a few stages after
# this one. What this does do is stop it first. Migrations run before packages,
# build, files and services, so at this moment the old binary is still running
# under the new unit's name, and a program that is still running is a program
# still writing to the directory below.
#
# The sessions it saved go to the attic with it. They are in
# ~/.local/share/hyprsession and the new program keeps its own under
# ~/.local/share/console/resume, so nothing there is read again by anything --
# and a directory nothing reads, sitting under the name of the program that made
# it, is the thing somebody finds in a year and cannot tell from a live one.
# Swept is not deleted: it is in the attic this run prints on its last line,
# with whatever ~/.config/hyprsession held beside it, and a person who wants a
# window arrangement back can go and read it there.
#
# sweeps: /usr/local/bin/hyprsession

echo "sweeping the session keeper under its old name"

whoever=$(basename "$CONSOLE_HOME")

systemctl --user -M "$whoever@" stop console-session.service >/dev/null 2>&1 || true

console-attic /usr/local/bin/hyprsession
console-attic "$CONSOLE_HOME/.local/share/hyprsession"
console-attic "$CONSOLE_HOME/.config/hyprsession"
