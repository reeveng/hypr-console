# The programs that are not here

This desktop is made of ordinary packages, a set of config files, and two
programs that are forks of somebody else's work. Neither is in this copy.
Publishing a compiled program means publishing the source that made it, and
both of these sources are upstream's, so the binaries are not carried without
them.

Build each one and put it where the manifest expects, then add its path back to
the `[files]` section of `desktop.conf` and run `console apply`.

## hyprsession, at /usr/local/bin/hyprsession


Restores the windows that were open, and keeps saving them. Upstream is
<https://github.com/Duckonaut/hyprsession>.

What the fork changes: Hyprland 0.56 moved to a Lua configuration and the old
dispatch path stopped working, so the fork talks to the compositor the way it
now expects. Note that its mode is a positional argument: passed as an option
it saves somewhere nothing reads.

    cargo build --release
    install -m755 target/release/hyprsession /usr/local/bin/

## kew, at /usr/local/bin/kew

Plays the music, and is everything the Music panel's buttons are: it reads the
library, holds the playlist, decodes, and answers on MPRIS. Upstream is
<https://codeberg.org/ravachol/kew>.

The `kew` package is installed as well and is what brings the libraries this
links; the fork sits in front of its program on the path.

What the fork changes: two answers on the bus. `OpenUri` is the larger of them.
Started with a song on its command line, kew plays that song and stops -- a
playlist of one, where next goes nowhere. Told the same song over the bus, the
fork builds the playlist out of the whole library around it, so the song asked
for plays and everything else follows it, once each, round for ever. That is
what a song pressed in the Music panel does, and without it a press restarts
the player on one file. The other is `xesam:url`, which says what file the song
playing is in, so that Y over it can open the file.

    make
    install -m755 kew /usr/local/bin/

## The on-screen keyboard, which used to be on this list

It was wvkbd, a fork carried as a compiled program, and it is now this
repository's own: `crates/keyboard`, built on the device like everything else
here. What it types it does not carry -- each alphabet is composed at startup
from the system's own xkb symbols, so a language is a word on the keyboard's
command line rather than a table in this tree.

The wvkbd source was kept beside it for a while as the way back, and excluded
from this copy because the patches on top of upstream were a personal adaptation
rather than something to be carried here. The replacement is not young any more
and that source has gone, so no half of that fork is left in this tree.
