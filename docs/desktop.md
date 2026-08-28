# The desktop, running here

    tools/legion-desktop run            in a window, to press things
    tools/legion-desktop shot FILE      a picture, at the device's size
    tools/legion-desktop shot a.png --open alacritty
    tools/legion-desktop verify         does the compositor config still parse
    tools/legion-desktop probe          what the compositor thinks it has

The emulator answers what a button does. This answers what the desktop looks
like, which is the other half of the machine and the half nobody could see
without picking it up.

Most of what is worth looking at is ordinary Wayland software reading ordinary
config files, and this laptop runs the same Hyprland the device does, to the
patch version, along with the same wofi and the same alacritty. So the device's
own compositor config is read here, through `dofile`, with one thing changed:
the screen.

## The screen

The device's panel is 1600x2560 at a scale of 2.5, mounted portrait and turned
a quarter turn. What the desktop is actually laid out in is 1024x640, and that
is what this draws on. A bar that fits at some other size is not a bar that
fits.

For a picture there is no window at all. A headless screen is made at exactly
that size and the nested compositor's own window is turned off once it exists,
in that order: turning the window off first leaves nothing to draw on. So a
screenshot is the device's layout and nothing of this machine's.

For pressing things, `run` keeps the window, at whatever size the host gives
it. That one is for trying something, not for judging a layout.

## The staged copy

Every file under `files/` is copied into `.stage/`, and every absolute path
inside those files is rewritten to point back into it. A stylesheet naming
`/usr/share/backgrounds/legion.png` finds the picture that is going to be
installed there, on a machine with no such directory it may write to.

The one file replaced rather than copied is `session-start`. On the device it
hands the environment to systemd and starts `legion.target`; here that would
reach into this machine's own systemd, so the staged copy starts the same
programs directly.

## What this is not

It is not the device, and three things follow.

The controller is not here. Buttons come from `tools/legion-emulate`, which
publishes the devices InputPlumber would publish. What is not tested is
InputPlumber itself.

Nothing is installed. The bar and the wallpaper need `waybar` and `hyprpaper`
on this machine, and without them the desktop comes up without them, quietly.
Everything else the desktop is made of is already here.

The hardware is not here: no battery, no backlight, no Bluetooth, no panel to
be wrong about. A test of the bar's battery reading is a test of the device.

## A trap worth knowing

A Hyprland Lua config keeps one handler per event. A second
`hl.on("hyprland.start", ...)` replaces the first rather than joining it, which
is why the screen here is made from outside the config rather than in it: the
device's own config uses that handler to start the desktop, and taking it would
have started nothing.

`hyprctl keyword` does not work against a Lua config at all. It answers
"keyword can't work with non-legacy parsers. Use eval." So `hyprctl eval` with
a Lua expression is how a running compositor is told anything.
