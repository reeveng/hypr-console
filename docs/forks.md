# The two programs that are not here

This desktop is made of ordinary packages, a set of config files, and two
programs that are forks of somebody else's work. The forks are not in this
repository. Publishing a compiled program means publishing the source that
made it, and that source belongs to the projects below rather than to this one.

Build each and put it where the manifest expects, then add its path back to the
`[files]` section of `desktop.conf` and run `legion apply`.

## wvkbd, at /usr/local/bin/wvkbd-mobintl

The on-screen keyboard. Upstream is <https://github.com/jjsullivan5196/wvkbd>,
a Wayland keyboard that draws layers and toggles itself on a signal.

What the fork changes: it reads the controller directly. On this device the
keyboard and the controller daemon both see the same pad, and only one of them
may act on a press, so the keyboard takes the pad while it is up and hands it
back when it goes away. One button shows it and puts it away, and no other
button closes it.

    git clone https://github.com/jjsullivan5196/wvkbd
    cd wvkbd && make wvkbd-mobintl
    install -m755 wvkbd-mobintl /usr/local/bin/

## hyprsession, at /usr/local/bin/hyprsession

Restores the windows that were open, and keeps saving them. Upstream is
<https://github.com/Duckonaut/hyprsession>.

What the fork changes: Hyprland 0.56 moved to a Lua configuration and the old
dispatch path stopped working, so the fork talks to the compositor the way it
now expects. Note that its mode is a positional argument: passed as an option
it saves somewhere nothing reads.

    cargo build --release
    install -m755 target/release/hyprsession /usr/local/bin/
