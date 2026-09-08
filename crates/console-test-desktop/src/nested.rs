//! The compositor's config, with the device's screen swapped for this one.
//! Whatever the session starts here, it starts the way the device starts it and
//! from the same place the device reads. The stage is the device's tree copied
//! somewhere else with every absolute path inside it pointed back into the
//! copy, so a line here that names `/usr/local` names the staged one by the
//! time it is written -- `staging::rewritten` is the whole of that, and this
//! file is written through it rather than around it.  The keyboard is the
//! reason that is a rule and not a habit. This script used to start it by its
//! bare name off the staged `PATH`, which is one program started the way
//! nothing on the device starts anything: `keyboard-toggle` signals a command
//! line, that command line was the name and not the path, and X did nothing on
//! this stage for as long as it existed. The command is read off
//! `console-input-keyboard.service` now, so the flags the device types with are
//! the flags this stage types with, and neither is written down twice.


use console_core_ini_files::field;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use console_screen::Screen;

pub const UNIT: &str = "files/etc/systemd/user/console-input-keyboard.service";

pub fn started_by(unit: &str) -> Result<Option<String>, Never> {
    let started = field(unit, "Service", "ExecStart")?;

    Ok(started.filter(|command| command.starts_with('/')).map(str::to_string))
}

pub const MARK: &str = "@keyboard@";
pub const GROUND: &str = "@ground@";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wallpaper {
    Started,
    LeftOut,
}

pub fn session_start(keyboard: &str, wallpaper: Wallpaper) -> Result<String, Never> {
    let ground = match wallpaper {
        Wallpaper::Started => THE_GROUND,
        Wallpaper::LeftOut => "",
    };

    Ok(SESSION_START.replace(MARK, keyboard).replace(GROUND, ground))
}

const THE_GROUND: &str = r#"[ -x "$(command -v awww-daemon)" ] && { awww-daemon & sleep 1; \
  . /usr/local/lib/console/palette.sh 2>/dev/null; awww clear "${night:-000000}"; }"#;

const SESSION_START: &str = r#"#!/bin/sh
# The staged stand-in for session-start. On the device this hands the
# environment to systemd and starts console.target; here the pieces are started
# directly, because this machine's systemd is not the device's.

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
#
# A desktop opened by hand starts none of it. The daemon cannot outlive the
# compositor it drew on and it does not end when that one goes: it panics on
# the socket that closed under it, which is a core file and a crash
# notification for every window somebody shut. The sessions console-desktop
# ends itself tell the daemon to go first, and this is the one the compositor's
# own exit ends, where there is no moment left to say it in -- a shutdown
# handler in the config was tried, and it runs after the clients are already
# gone. What Hyprland fills the screen with instead is the device's own night,
# which is the colour this would have painted.
@ground@
# The keyboard, started the way the device's unit starts it, because this is
# that unit's own ExecStart with its paths pointed into the stage. The path
# matters as much as the flags do: `console_input_keyboard::asked` finds the keyboard beside
# itself and signals that command line, so a keyboard started as a bare name is
# a keyboard nothing can raise. It reads palette.sh out of this same tree on the
# way in and dresses its own command line, so a keyboard on this screen is the
# keyboard the device has, in the colours this repository currently spends.
keyboard="@keyboard@"
[ -x "${keyboard%% *}" ] && $keyboard &
exit 0
"#;

fn monitor(
    output: &str,
    wide: u32,
    tall: u32,
    screen: &Screen,
    scale: f64,
) -> Result<String, Never> {
    Ok(format!(
        "hl.monitor({{\n    \
         output    = \"{output}\",\n    \
         mode      = \"{wide}x{tall}@{}\",\n    \
         position  = \"auto\",\n    \
         scale     = {scale},\n    \
         transform = {},\n\
         }})",
        screen.refresh, screen.transform
    ))
}

pub fn in_a_window(screen: &Screen, scale: f64) -> Result<String, Never> {
    let (wide, tall) = screen.mode;
    let shown = |size: u32| {
        let Ok(shown) = whole_u32(f64::from(size) * scale / screen.scale);

        shown
    };

    monitor("WAYLAND-1", shown(wide), shown(tall), screen, scale)
}

pub fn headless(screen: &Screen) -> Result<String, Never> {
    let (wide, tall) = screen.mode;
    let Ok(first) = monitor("HEADLESS-1", wide, tall, screen, screen.scale);
    let Ok(second) = monitor("HEADLESS-2", wide, tall, screen, screen.scale);
    let Ok(window) = monitor("WAYLAND-1", 320, 200, screen, 1.0);

    Ok(format!("{first}\n{second}\n{window}"))
}

pub fn made_headless(screen: &Screen) -> Result<String, Never> {
    let (wide, tall) = screen.mode;
    Ok(format!(
        r#"hl.monitor({{ output = "HEADLESS-1", mode = "{wide}x{tall}@{}", position = "auto", scale = {}, transform = {} }})"#,
        screen.refresh, screen.scale, screen.transform
    ))
}

const THE_COLOUR_NOTHING_IS: &str = "\
-- On the device the bare background is the darkest colour in the palette, so
-- that a wallpaper arriving a second after the compositor does not announce
-- itself with a flash of something else. Here that is the one colour it must
-- not be: the wallpaper rests at that same colour, so a screen nothing painted
-- and a screen the wallpaper painted would read alike, and a check comparing them
-- would pass with no wallpaper at all. Nothing is ever this.
hl.config({ misc = { background_color = \"rgb(ff00ff)\" } })";

const THE_DEVICES_OWN: &str = "\
-- The ground is left the device's own, because this session starts no
-- wallpaper daemon to paint over it and nobody here has to tell a screen the
-- wallpaper painted from a screen nothing did.";

pub fn config(
    screen_said: &str,
    device_config: &str,
    wallpaper: Wallpaper,
) -> Result<String, Never> {
    let ground = match wallpaper {
        Wallpaper::Started => THE_COLOUR_NOTHING_IS,
        Wallpaper::LeftOut => THE_DEVICES_OWN,
    };

    Ok(format!(
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

{ground}

-- The bar Hyprland draws over the screen when it was not started by
-- start-hyprland, which is otherwise in every picture taken here.
hl.config({{ misc = {{ disable_watchdog_warning = true }} }})
"
    ))
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
        let said = headless(&go()).expect("the screens");
        assert!(said.contains("2560x1600@144"), "{said}");
        assert!(said.contains("HEADLESS-1") && said.contains("HEADLESS-2"));
    }

    #[test]
    fn a_window_too_large_for_this_screen_gives_up_pixels_and_not_layout() {
        let said = in_a_window(&go(), 1.25).expect("the window");
        assert!(said.contains("1280x800@144"), "{said}");
        assert!(said.contains("scale     = 1.25"), "{said}");
    }

    #[test]
    fn the_nested_config_reads_the_devices_own() {
        let said = config("-- a screen", "/somewhere/hyprland.lua", Wallpaper::Started)
            .expect("the config");
        assert!(said.contains(r#"dofile("/somewhere/hyprland.lua")"#));
        assert!(
            said.contains("rgb(ff00ff)"),
            "the background has to be a colour nothing is"
        );
    }

    #[test]
    fn a_session_nothing_is_left_to_stop_starts_no_wallpaper() {
        let started = session_start("/k", Wallpaper::Started).expect("the session's start");
        assert!(
            started.contains("awww-daemon"),
            "the session that stops the daemon does not start it: {started}"
        );

        let alone = session_start("/k", Wallpaper::LeftOut).expect("the session's start");
        assert!(
            !alone.contains("awww"),
            "a daemon whose only way out is a core file is started here: {alone}"
        );

        let said = config("-- a screen", "/somewhere/hyprland.lua", Wallpaper::LeftOut)
            .expect("the config");
        assert!(
            !said.contains("ff00ff"),
            "nothing paints this ground, so a colour nothing is is all anybody would see: {said}"
        );
    }
}
