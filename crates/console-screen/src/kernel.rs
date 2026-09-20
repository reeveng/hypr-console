//! What the panel is, asked before there is a compositor to ask.
//!
//! A Lua config's monitor block is read as the compositor starts, which is the
//! one moment nothing can be asked of it -- so the block this desktop ships was
//! one machine's numbers, and a machine with another panel wore them. The kernel
//! is there the whole time: `/sys/class/drm` holds a directory per connector,
//! `status` says whether anything is on the end of it, and the first line of
//! `modes` is the mode the panel says it prefers.
//!
//! That is everything the block needs. The resolution goes in without a rate,
//! because `modes` does not carry one and a compositor handed a resolution alone
//! picks the fastest rate the panel has at it -- so the 144 that used to be
//! written down is the compositor's to find. The other two are what it cannot
//! work out at all: the quarter turn, which is [`Mounted`] off the panel's own
//! mode, and the scale, which is this desktop's canvas over the pixels along the
//! edge the desktop runs across.
//!
//! `console apply` writes it, because an apply is the one thing that happens on
//! a machine before anybody logs into it. The file is not in the manifest for
//! the same reason the controller's router profile is not: what it holds is read
//! off the machine it is on, so a tree that carried a copy would be carrying one
//! machine's answer for all of them again.

use std::path::{Path, PathBuf};

use console_core_geometry::Size;
use console_core_never::Never;

use crate::{Canvas, INTERNAL, Mounted};

pub const DRM: &str = "/sys/class/drm";

pub const UNDER: &str = "hypr";

pub const NAMED: &str = "monitor.lua";

pub const MODES: &str = "modes";

pub const STATUS: &str = "status";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plugged {
    Into,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    pub named: String,
    pub mode: Size<u32>,
}

impl Panel {
    pub fn mounted(&self) -> Result<Mounted, Never> {
        Mounted::of(self.mode)
    }

    pub fn across(&self) -> Result<u32, Never> {
        let Ok(mounted) = self.mounted();

        Ok(match mounted {
            Mounted::Sideways => self.mode.tall,
            Mounted::Upright => self.mode.wide,
        })
    }

    pub fn scale(&self, canvas: Canvas) -> Result<f64, Never> {
        let Ok(across) = self.across();

        Ok(f64::from(across) / f64::from(canvas.0.max(1)))
    }

    pub fn block(&self, canvas: Canvas) -> Result<String, Never> {
        let Ok(mounted) = self.mounted();
        let Ok(transform) = mounted.transform();
        let Ok(across) = self.across();
        let Ok(scale) = self.scale(canvas);
        let Ok(how) = mounted.how_it_is_mounted();

        let named = &self.named;
        let (wide, tall) = (self.mode.wide, self.mode.tall);
        let points = canvas.0;

        Ok(format!(
            "\
-- This machine's panel, written by `console apply` out of what the kernel says
-- it is. It is read after the block in hyprland.lua, which is one machine's
-- numbers, so this is the one that stands.
--
-- It prefers {wide}x{tall}, {how}.
--
-- The desktop is drawn {points} points across and the panel has {across} pixels
-- along that edge, which is the scale.
--
-- The mode names no rate, which is deliberate rather than missing: `modes` does
-- not carry one, and a compositor handed a resolution with no rate picks the
-- fastest the panel has at it. The rate was written here once, as 144, by the
-- one machine that has it.
--
-- The touchscreen reports in the panel's own orientation, so it wears the same
-- quarter as the picture does or a finger lands turned. It is said here rather
-- than in hyprland.lua for the same reason the monitor is: a transform written
-- down is one machine's, and this one is read off the panel it is about.
--
-- Editing this is editing a report. The next apply writes it again.
hl.monitor({{ output = \"{named}\", mode = \"{wide}x{tall}\", position = \"auto\", scale = {scale}, transform = {transform} }})
hl.config({{ input = {{ touchdevice = {{ output = \"{named}\", transform = {transform} }} }} }})
"
        ))
    }
}

pub fn at(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Config.ours_under(home);

    Ok(ours.join(UNDER).join(NAMED))
}

pub fn named(of: &Path) -> Result<Option<String>, Never> {
    let called = match of.file_name() {
        Some(called) => called.to_string_lossy().to_string(),
        None => return Ok(None),
    };

    Ok(called.split_once('-').map(|(_card, connector)| connector.to_string()))
}

pub fn plugged(at: &Path) -> Result<Plugged, Never> {
    let said = match std::fs::read_to_string(at.join(STATUS)) {
        Ok(said) => said,
        Err(_nothing_says_so) => return Ok(Plugged::Nothing),
    };

    Ok(match said.trim() == "connected" {
        true => Plugged::Into,
        false => Plugged::Nothing,
    })
}

pub fn mode(at: &Path) -> Result<Option<Size<u32>>, Never> {
    let said = match std::fs::read_to_string(at.join(MODES)) {
        Ok(said) => said,
        Err(_no_modes_to_read) => return Ok(None),
    };

    let first = match said.lines().next() {
        Some(first) => first.trim().to_string(),
        None => return Ok(None),
    };

    let (wide, tall) = match first.split_once('x') {
        Some(both) => both,
        None => return Ok(None),
    };

    Ok(match (wide.parse(), tall.parse()) {
        (Ok(wide), Ok(tall)) => Some(Size { wide, tall }),
        (_wide, _tall) => None,
    })
}

pub fn found(under: &Path) -> Result<Vec<Panel>, Never> {
    let entries = match std::fs::read_dir(under) {
        Ok(entries) => entries,
        Err(_no_drm_here) => return Ok(Vec::new()),
    };

    let mut every: Vec<Panel> = Vec::new();

    for at in entries.flatten().map(|entry| entry.path()) {
        let Ok(plugged) = plugged(&at);

        match plugged {
            Plugged::Nothing => continue,
            Plugged::Into => {},
        }

        let Ok(named) = named(&at);
        let Ok(mode) = mode(&at);

        match (named, mode) {
            (Some(named), Some(mode)) => every.push(Panel { named, mode }),
            (_named, _mode) => {},
        }
    }

    every.sort_by(|one, other| one.named.cmp(&other.named));

    Ok(every)
}

pub fn panel(under: &Path) -> Result<Option<Panel>, Never> {
    let Ok(every) = found(under);

    let built_in = every.iter().find(|panel| panel.named.starts_with(INTERNAL));

    Ok(built_in.or_else(|| every.first()).cloned())
}

pub fn here() -> Result<Option<Panel>, Never> {
    panel(Path::new(DRM))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drm(whose: &str, said: &[(&str, &str, &str)]) -> PathBuf {
        let at = std::env::temp_dir().join(format!("console-drm-{}-{whose}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);

        for (connector, status, modes) in said {
            let under = at.join(connector);
            let _ = std::fs::create_dir_all(&under);
            let _ = std::fs::write(under.join(STATUS), format!("{status}\n"));
            let _ = std::fs::write(under.join(MODES), modes);
        }

        at
    }

    const HANDHELD: &str = "1600x2560\n1600x2560\n";

    const LAPTOP: &str = "1920x1200\n1920x1080\n";

    #[test]
    fn a_connector_is_named_without_the_card_it_is_on() {
        let Ok(named) = named(Path::new("/sys/class/drm/card1-eDP-1"));

        assert_eq!(named, Some("eDP-1".to_string()));
    }

    #[test]
    fn the_mode_a_panel_prefers_is_the_first_one_it_lists() {
        let at = drm("preferred", &[("card1-eDP-1", "connected", LAPTOP)]);
        let Ok(mode) = mode(&at.join("card1-eDP-1"));

        assert_eq!(mode, Some(Size { wide: 1920, tall: 1200 }));
    }

    #[test]
    fn nothing_is_plugged_into_a_connector_that_says_disconnected() {
        let at = drm("unplugged", &[("card1-DP-1", "disconnected", "")]);
        let Ok(plugged) = plugged(&at.join("card1-DP-1"));

        assert_eq!(plugged, Plugged::Nothing);
    }

    #[test]
    fn the_panel_is_the_built_in_one_whatever_else_is_plugged_in() {
        let at = drm(
            "both",
            &[
                ("card1-DP-1", "connected", "3840x2160\n"),
                ("card1-eDP-1", "connected", HANDHELD),
            ],
        );
        let Ok(panel) = panel(&at);

        assert_eq!(panel, Some(Panel { named: "eDP-1".to_string(), mode: Size { wide: 1600, tall: 2560 } }));
    }

    #[test]
    fn with_no_panel_built_in_it_is_whatever_is_plugged_in() {
        let at = drm("external", &[("card1-DP-2", "connected", "1920x1080\n")]);
        let Ok(panel) = panel(&at);

        assert_eq!(panel, Some(Panel { named: "DP-2".to_string(), mode: Size { wide: 1920, tall: 1080 } }));
    }

    #[test]
    fn a_machine_with_nothing_connected_says_nothing() {
        let at = drm("nothing", &[("card1-HDMI-A-1", "disconnected", "")]);
        let Ok(panel) = panel(&at);

        assert_eq!(panel, None);
    }

    #[test]
    fn a_panel_taller_than_it_is_wide_is_turned_and_scaled_on_its_long_edge() {
        let panel = Panel { named: "eDP-1".to_string(), mode: Size { wide: 1600, tall: 2560 } };
        let Ok(mounted) = panel.mounted();
        let Ok(across) = panel.across();
        let Ok(scale) = panel.scale(crate::DRAWN_AT);

        assert_eq!(mounted, Mounted::Sideways);
        assert_eq!(across, 2560);
        assert!((scale - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn a_laptop_panel_is_not_turned_and_is_scaled_on_its_width() {
        let panel = Panel { named: "eDP-1".to_string(), mode: Size { wide: 1920, tall: 1200 } };
        let Ok(mounted) = panel.mounted();
        let Ok(scale) = panel.scale(crate::DRAWN_AT);

        assert_eq!(mounted, Mounted::Upright);
        assert!((scale - 1.875).abs() < f64::EPSILON);
    }

    #[test]
    fn the_block_names_the_connector_the_turn_and_the_scale_and_no_mode() {
        let panel = Panel { named: "eDP-1".to_string(), mode: Size { wide: 1600, tall: 2560 } };
        let Ok(block) = panel.block(crate::DRAWN_AT);

        assert!(block.contains(
            r#"hl.monitor({ output = "eDP-1", mode = "1600x2560", position = "auto", scale = 2.5, transform = 1 })"#
        ));
        assert!(!block.contains("1600x2560@"));
    }

    #[test]
    fn the_finger_is_read_through_the_same_quarter_the_picture_is_drawn_at() {
        for (mode, transform) in
            [(Size { wide: 1600, tall: 2560 }, 1), (Size { wide: 1920, tall: 1200 }, 0)]
        {
            let panel = Panel { named: "eDP-1".to_string(), mode };
            let Ok(block) = panel.block(crate::DRAWN_AT);

            assert!(
                block.contains(&format!(
                    r#"hl.config({{ input = {{ touchdevice = {{ output = "eDP-1", transform = {transform} }} }} }})"#
                )),
                "{block}"
            );
        }
    }

    #[test]
    fn the_file_is_beside_the_compositors_own_in_this_desktops_directory() {
        let Ok(at) = at(Path::new("/home/ada"));

        assert_eq!(at, PathBuf::from("/home/ada/.config/console/hypr/monitor.lua"));
    }
}

