# The desktop, running here

Runs the device's own desktop on this machine, at the device's size.

    console-desktop run                  in a window, to press things
    console-desktop shot FILE            a picture, at the device's size
    console-desktop shot a.png --open alacritty
    console-desktop verify               does the compositor config still parse
    console-desktop probe                what the compositor thinks it has
    console-desktop clean                delete stages nothing is using

`console-desktop` is `cargo run --bin console-desktop --`; `just desktop` and
`just shot` are the two of these anyone types often.

The desktop is laid out in 1024x640 and drawn at two and a half times that, so a
picture comes out 2560x1600. Positions are given in the 1024x640.

Each run copies `files/`, and the programs the device compiles for itself, into
a staged directory of its own and deletes it
afterwards, so two can run side by side and neither can break the other. Set
`CONSOLE_STAGE=mine` to keep one.

## What it cannot answer

There is no controller here. Buttons come from `console-emulate`.

There is no hardware: no battery, no backlight, no Bluetooth.

The wallpaper needs `hyprpaper` installed on this machine. Without it the
desktop comes up without it and says nothing. The bar is this tree's own
program and is staged with the rest of them, but it draws its icons out of a
Nerd Font: on a machine without one they are the boxes fontconfig falls back
to, and the bar is otherwise itself.

## Traps

A Hyprland Lua config keeps one handler per event, so a second one replaces the
first. The screen here is set from outside the config for that reason.

`hyprctl keyword` does not work against a Lua config. Use `hyprctl eval`.

A dispatcher is Lua too, and takes what it takes. Pressing a key at whatever
has the keyboard is `hyprctl dispatch 'hl.dsp.send_shortcut{mods="", key="a"}'`,
a table and not a string. `hl.dsp` holds the names, and one it does not know
comes back as a nil value rather than as a dispatcher no one has.

Which names it holds can be asked without pressing anything. Reading one is
silent whether or not it is there and calling one is not, so `hyprctl eval
'local d = hl.dsp.window.resize({})'` answers either with the arguments that
dispatcher wanted or with the nil value it is. Building a dispatcher is not
dispatching it -- `hl.bind` takes one built -- so the question is safe to ask
on a desktop someone is using, which is how `window.resize` was found to exist
and `window.size` not to.

`HOME` is the stage, not this machine's home. A program that reads a file out
of it reads the copy under the stage, so a setting a picture is meant to show
has to be written there while the run is going rather than here beforehand.

An absolute path compiled into a program is this machine's, not the stage's.
Every file under `files/` is rewritten on the way in, so a path written in one
of them points back into the stage; a path written in Rust is not rewritten by
anything. Ask where the running program is and work out from there --
`console_core_color::spent::beside` finds the palette its own tree spends, and
`console_input_keyboard::asked::beside` finds the keyboard installed next to
whoever is asking. Both of those were `/usr/local/...` once, which is why the
keyboard stood on this stage for as long as it did with nothing able to raise it
and no color on it that this repository spends. On the device the two answers
are the same path, so a program that asks is right in both places and a program
that knows is right in one.

A path in Rust that cannot be worked out from where the program is has to be
asked for instead. `/run/console/updating` is the one of those: the engine is
root and the bar is hers, so the file the strip reads cannot live in either's
home, and on a laptop it is under a directory no one is allowed to make.
`CONSOLE_UPDATING_PATH` is what the staged session is told, the same way it is
told a home and four XDG directories, and the check that looks at the strip
fills a file of its own rather than this machine's `/run`.

A place on the screen is a logical pixel of the screen the picture came off,
which is not the one `hyprland.lua` declares. The nested screen is the device's
mode at whatever scale leaves room on the machine running it, so a row worked
out from the device's own 2.5 is a row somewhere else entirely -- and reading
the wrong row looks exactly like a surface that does not paint. The picture
comes with what the compositor said about its screen at the moment it was
taken, and `Desktop::color` divides by that.

A picture is taken as soon as something reaches the screen, which is before a
panel that is still reading the machine has drawn its rows. A tab that
photographs empty is as likely to be one caught early as one that is broken.
Two things that look the same in a picture: that, and a panel that has died.
A panel dies in a GTK callback and the harness prints nothing at all, so send
its stderr to a file before believing the picture.
