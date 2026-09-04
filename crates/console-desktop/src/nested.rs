//! The compositor's config, with the device's screen swapped for this one.


use console_number::whole_u32;
use console_screen::Screen;

/// What the compositor starts on the device is a systemd target, and the four
/// services under it belong to that machine's session. Starting them here would
/// reach into this machine's own systemd, so the staged copy of session-start
/// runs the same programs directly instead.
pub const SESSION_START: &str = r#"#!/bin/sh
# The staged stand-in for session-start. On the device this hands the
# environment to systemd and starts console.target; here the four pieces are
# started directly, because this machine's systemd is not the device's.

# The bar goes up once the screens have stopped changing, and not before.
#
# This session gets a second screen a moment after it starts and loses its
# first one a moment after that: the device's screen is made at the device's
# size and the window's is turned off, which is `Inside::make_the_screen` and
# takes about two seconds altogether. A bar started before that is built on the
# screen that is about to go, and then built again on the one that arrives.
#
# The second building is what kills it. waybar closes the modules of the screen
# it is finished with, and closing one forks a copy of the bar to start the
# little program the module reads from. The copy never becomes that program: it
# is a whole waybar with the Hyprland module's static still in it, and when its
# one thread ends, glibc exits the process, the static's destructor destroys the
# thread that is running it, and destroying a joinable thread aborts. A core out
# of a bar that had drawn nothing, in five staged sessions in six, and none at
# all in twenty once the bar goes up last.
#
# Settled rather than slept: a list of screens that has not changed for a second
# and a half is a session that has finished being arranged, and the last change
# lands well inside that. Eight seconds is the cap, which is longer than this
# has ever taken and shorter than doing without a bar.
settled=""
steady=0
asked=0
while [ $asked -lt 32 ]; do
    screens=$(hyprctl monitors all -j 2>/dev/null | grep '"name"')
    if [ -n "$screens" ] && [ "$screens" = "$settled" ]; then
        steady=$((steady + 1))
        [ $steady -ge 6 ] && break
    else
        steady=0
    fi
    settled=$screens
    asked=$((asked + 1))
    sleep 0.25
done
[ -x "$(command -v waybar)" ] && waybar &
# The ground, which is what the device shows before console-sky has chosen a
# picture and all this stage ever shows, because it presses none. The colour is
# sourced rather than written here: palette.sh exists to be read by shells, and
# a hex typed into this string would be a colour nothing checks.
[ -x "$(command -v awww-daemon)" ] && { awww-daemon & sleep 1; \
  . /usr/local/lib/console/palette.sh 2>/dev/null; awww clear "${night:-000000}"; }
# The keyboard, started the way the device's unit starts it: the program itself,
# with nothing in front of it. It reads palette.sh on the way in and dresses its
# own command line, so a keyboard on this screen is the keyboard the device has,
# in the colours this repository currently spends.
[ -x "$(command -v virtual-keyboard)" ] && virtual-keyboard &
exit 0
"#;

/// A screen, said the way the compositor is told about one.
fn monitor(output: &str, wide: u32, tall: u32, screen: &Screen, scale: f64) -> String {
    format!(
        "hl.monitor({{\n    \
         output    = \"{output}\",\n    \
         mode      = \"{wide}x{tall}@{}\",\n    \
         position  = \"auto\",\n    \
         scale     = {scale},\n    \
         transform = {},\n\
         }})",
        screen.refresh, screen.transform
    )
}

/// A compositor running as somebody else's window gets one output and calls it
/// WAYLAND-1, at whatever size the host gave the window. That is the one to have
/// while looking at something and pressing things.
///
/// A window has to fit on the screen it is a window on. Density is the only
/// thing given up, and only as far as it must be: the desktop is laid out in the
/// same logical size either way, so everything is where it is on the device and
/// there are simply fewer pixels drawing it.
pub fn in_a_window(screen: &Screen, scale: f64) -> String {
    let (wide, tall) = screen.mode;
    let shown = |size: u32| whole_u32(f64::from(size) * scale / screen.scale);
    monitor("WAYLAND-1", shown(wide), shown(tall), screen, scale)
}

/// For a picture, the window is in the way: its size is the host's to decide and
/// the device's screen is a fixed thing. So a headless output is made at the
/// size the device's screen is, and the window is turned off once it exists.
/// Turning the window off first leaves nothing to draw on and nothing to
/// photograph, which is what "no wl_output" meant.
pub fn headless(screen: &Screen) -> String {
    let (wide, tall) = screen.mode;
    format!(
        "{}\n{}\n{}",
        monitor("HEADLESS-1", wide, tall, screen, screen.scale),
        monitor("HEADLESS-2", wide, tall, screen, screen.scale),
        // And the window, small. It has to exist for the second and a bit it
        // takes to make the headless output and turn this one off, because a
        // compositor with no output at all has nothing to draw on and nothing
        // to photograph. What it cannot be is the size of the device's screen:
        // that is a picture-sized empty rectangle opening over whatever
        // somebody is doing on the machine taking the picture, in the colour a
        // screen with nothing on it is, and then closing again.
        monitor("WAYLAND-1", 320, 200, screen, 1.0)
    )
}

/// The one screen said again after the compositor is up, because a monitor rule
/// written for a screen that does not exist yet is not applied to the one that
/// turns up.
pub fn made_headless(screen: &Screen) -> String {
    let (wide, tall) = screen.mode;
    format!(
        r#"hl.monitor({{ output = "HEADLESS-1", mode = "{wide}x{tall}@{}", position = "auto", scale = {}, transform = {} }})"#,
        screen.refresh, screen.scale, screen.transform
    )
}

/// The whole nested config: the device's, with a screen of this machine's.
///
/// Nothing else is changed. The window rules, the bindings, the look and the
/// absence of animation are all read from the file the device reads, so a change
/// tried here is a change tried there.
pub fn config(screen_said: &str, device_config: &str) -> String {
    format!(
        "\
-- The device's compositor config, with its screen swapped for this one.
--
-- Nothing else is changed. The window rules, the bindings, the look and the
-- absence of animation are all read from the file the device reads, so a
-- change tried here is a change tried there.

{screen_said}

dofile(\"{device_config}\")

-- Said again after the device's config, because that one names the screen the
-- device is mounted on and this is not that screen.
hl.monitor({{ output = \"eDP-1\", disabled = true }})

-- On the device the bare background is the darkest colour in the palette, so
-- that a wallpaper arriving a second after the compositor does not announce
-- itself with a flash of something else. Here that is the one colour it must
-- not be: the wallpaper rests at that same colour, so a screen nothing painted
-- and a screen the garden painted would read alike, and a check comparing them
-- would pass with no wallpaper at all. Nothing is ever this.
hl.config({{ misc = {{ background_color = \"rgb(ff00ff)\" }} }})

-- The bar Hyprland draws over the screen when it was not started by
-- start-hyprland, which is otherwise in every picture taken here.
hl.config({{ misc = {{ disable_watchdog_warning = true }} }})
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go() -> Screen {
        Screen {
            mode: (2560, 1600),
            refresh: 144,
            scale: 2.5,
            transform: 1,
        }
    }

    #[test]
    fn a_picture_is_taken_at_the_devices_own_pixels() {
        let said = headless(&go());
        assert!(said.contains("2560x1600@144"), "{said}");
        assert!(said.contains("HEADLESS-1") && said.contains("HEADLESS-2"));
    }

    /// Only the density is given up, so the desktop is laid out the same.
    #[test]
    fn a_window_too_large_for_this_screen_gives_up_pixels_and_not_layout() {
        let said = in_a_window(&go(), 1.25);
        assert!(said.contains("1280x800@144"), "{said}");
        assert!(said.contains("scale     = 1.25"), "{said}");
    }

    #[test]
    fn the_nested_config_reads_the_devices_own() {
        let said = config("-- a screen", "/somewhere/hyprland.lua");
        assert!(said.contains(r#"dofile("/somewhere/hyprland.lua")"#));
        assert!(
            said.contains("rgb(ff00ff)"),
            "the background has to be a colour nothing is"
        );
    }
}
