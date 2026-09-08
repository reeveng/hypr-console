# Two menu entries under names this tree has stopped using.
#
# A desktop entry's filename is not decoration. It is the identity every other
# file uses to point at the thing: `mimeapps.list` names one to say what opens
# a song, and the menu reads the directory these sit in. So the name has to be
# one that still means something here, and two of them had quietly stopped.
#
# `console-music.desktop` named the crate `console-music`, which is
# `console-music-panel` now that what plays the sound is a crate of its own.
# `console-dictate.desktop` named nothing at all: the crate is
# `console-input-dictation`, the binary is `dictate`, and `console-dictate`
# existed in that filename and nowhere else on the machine.
#
# An apply installs a name and has never removed one, so without this a device
# ends up holding both. That is the fault worth saying out loud, because a
# stale entry is not dead weight -- it is a second answer. Two Music rows in
# the menu, both of which start something that works, and a `mimeapps.list`
# pointing at a file that is still sitting there beside the one it meant.
#
# Nothing rebuilds a desktop database here and nothing needs to. This desktop's
# menu reads the directory itself, and mimeapps is read as a file, so the
# entries are right the moment the old ones are gone.
#
# sweeps: /usr/share/applications/console-music.desktop
# sweeps: /usr/share/applications/console-dictate.desktop

echo "sweeping two menu entries under the names they used to have"

console-attic /usr/share/applications/console-music.desktop
console-attic /usr/share/applications/console-dictate.desktop
