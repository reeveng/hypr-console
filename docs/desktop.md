# The desktop, running here

Runs the device's own desktop on this machine, at the device's size.

    console-desktop run                  in a window, to press things
    console-desktop shot FILE            a picture, at the device's size
    console-desktop shot a.png --open alacritty
    console-desktop verify               does the compositor config still parse
    console-desktop probe                what the compositor thinks it has
    console-desktop clean                delete stages nothing is using

`console-desktop` is `cargo run --bin console-desktop --`; `just desktop` and
`just shot` are the two of these anybody types often.

The desktop is laid out in 1024x640 and drawn at two and a half times that, so a
picture comes out 2560x1600. Positions are given in the 1024x640.

Each run copies `files/`, and the programs the device compiles for itself, into
a staged directory of its own and deletes it
afterwards, so two can run side by side and neither can break the other. Set
`CONSOLE_STAGE=mine` to keep one.

## What it cannot answer

There is no controller here. Buttons come from `console-emulate`.

There is no hardware: no battery, no backlight, no Bluetooth.

The bar and the wallpaper need `waybar` and `hyprpaper` installed on this
machine. Without them the desktop comes up without them and says nothing.

## Traps

A Hyprland Lua config keeps one handler per event, so a second one replaces the
first. The screen here is set from outside the config for that reason.

`hyprctl keyword` does not work against a Lua config. Use `hyprctl eval`.

A dispatcher is Lua too, and takes what it takes. Pressing a key at whatever
has the keyboard is `hyprctl dispatch 'hl.dsp.send_shortcut{mods="", key="a"}'`,
a table and not a string. `hl.dsp` holds the names, and one it does not know
comes back as a nil value rather than as a dispatcher nobody has.

`HOME` is the stage, not this machine's home. A program that reads a file out
of it reads the copy under the stage, so a setting a picture is meant to show
has to be written there while the run is going rather than here beforehand.

A picture is taken as soon as something reaches the screen, which is before a
panel that is still reading the machine has drawn its rows. A tab that
photographs empty is as likely to be one caught early as one that is broken.
Two things that look the same in a picture: that, and a panel that has died.
A panel dies in a GTK callback and the harness prints nothing at all, so send
its stderr to a file before believing the picture.
