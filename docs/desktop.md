# The desktop, running here

Runs the device's own desktop on this machine, at the device's size.

    tools/legion-desktop run            in a window, to press things
    tools/legion-desktop shot FILE      a picture, at the device's size
    tools/legion-desktop shot a.png --open alacritty
    tools/legion-desktop verify         does the compositor config still parse
    tools/legion-desktop probe          what the compositor thinks it has
    tools/legion-desktop clean          delete stages nothing is using

The desktop is laid out in 1024x640 and drawn at two and a half times that, so a
picture comes out 2560x1600. Positions are given in the 1024x640.

Each run copies `files/` into a staged directory of its own and deletes it
afterwards, so two can run side by side and neither can break the other. Set
`LEGION_STAGE=mine` to keep one.

## What it cannot answer

There is no controller here. Buttons come from `tools/legion-emulate`.

There is no hardware: no battery, no backlight, no Bluetooth.

The bar and the wallpaper need `waybar` and `hyprpaper` installed on this
machine. Without them the desktop comes up without them and says nothing.

## Two traps

A Hyprland Lua config keeps one handler per event, so a second one replaces the
first. The screen here is set from outside the config for that reason.

`hyprctl keyword` does not work against a Lua config. Use `hyprctl eval`.
