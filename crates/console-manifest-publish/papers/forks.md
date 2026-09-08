# The programs that are not here

This desktop is made of ordinary packages, a set of config files, and one
program that is a fork of somebody else's work. It is not held back from this
copy -- a workspace whose `Cargo.lock` names a crate the copy does not carry is
a copy that will not resolve -- but the licence it arrived under travels with
it, which is the thing that is actually owed.

Nothing here is carried as somebody else's binary any more. Both of the ones
that were are ports now, and each went the same way and for the same reason: a
program on this device that nobody here can read is a program nobody here can
answer for.

## hyprsession, as crates/console-resume

Restores the windows that were open, and keeps saving them. Upstream is
<https://github.com/joshurtree/hyprsession>, and what this was taken from is six
commits past its `v0.2.1`, at `7fd57fc`.

This paper named a different address until that address stopped resolving, and
the wrong one had been sitting here long enough to be believed. Whoever rebuilds
this is rebuilding from a tag and a count of commits, so both are written down:
a fork whose source cannot be found again is a binary nobody can answer for.

What the fork changed, before the port: Hyprland 0.56 moved to a Lua
configuration and the old dispatch path stopped working, so the fork talks to
the compositor the way it now expects, and a terminal comes back in the
directory it reached running what it was running.

It was a binary at `/usr/local/bin/hyprsession` until it became a crate. A
program that closes other people's windows is a program somebody has to be able
to read, and it was the one thing on this device installed from a build nobody
here could check. Ported it also stopped being a second copy of things this tree
keeps once -- where the compositor's socket is, what its event words mean -- and
came under the EXPLICIT rules, which is what turned `no arguments` from the mode
that sweeps the desktop into the mode the unit wants.

To build the public copy: clone upstream at `7fd57fc`, and the port is this
repository's history for `crates/console-resume`.

## kew, which used to be on this list

It was a fork of <https://codeberg.org/ravachol/kew>, carried as a compiled
binary at `/usr/local/bin/kew` and sitting in front of the packaged program on
the path. What the fork changed had grown past the two answers it started as:
`OpenUri`, so a song chosen is the library it came from rather than a playlist
of one; `xesam:url`, so the song playing could be opened where it lives; and
then a use-after-free in the shuffle restore, which freed the playing song under
the thread that answers the bus. Fixing memory safety in somebody else's C, for
the panel this desktop uses most, is where carrying a binary stopped paying.

What plays music now is `crates/console-music-player`, which is this
repository's own and is in this copy like everything else. It links no codec.
ffmpeg is asked for a song as plain samples and pw-cat is handed them, which is
the arrangement kew could not have -- a terminal player that must build anywhere
cannot assume a decoder is installed, which is why ravachol had to bridge
libopus, libvorbis and libfaad into miniaudio by hand. This desktop had already
installed ffmpeg to read tags with.

`xesam:url` did not survive the port and neither did the offer it was for.

## The on-screen keyboard, which used to be on this list

It was wvkbd, a fork carried as a compiled program, and it is now this
repository's own: `crates/console-input-keyboard`, built on the device like
everything else here. What it types it does not carry -- each alphabet is
composed at startup from the system's own xkb symbols, so a language is a word
on the keyboard's command line rather than a table in this tree.

The wvkbd source was kept beside it for a while as the way back, and excluded
from this copy because the patches on top of upstream were a personal adaptation
rather than something to be carried here. The replacement is not young any more
and that source has gone, so no half of that fork is left in this tree.
