//! Everything the manifest stopped naming after the rename, and never swept.
//!
//! The rename had `tools/console-migrate` behind it and this had nothing. Between
//! the two, a program stopped being a shell script and became a crate, the
//! keyboard stopped being wvkbd and became one of ours, `console-poke` split into
//! two crates named for what they are, and each of those left the name it had
//! been installed under sitting in /usr/local/bin. None of it is running and
//! nothing in the tree reaches for any of it -- the device was read before this
//! was written -- so what this sweeps is dead weight rather than a fault.
//!
//! It is written anyway, and not skipped as harmless, because the mechanism that
//! left them there is the one that nearly left seven `legion-*` units enabled
//! beside their replacements, both wanting the pad. What is inert this time was
//! luck about which things happened to be removed, not a property of anything.
//!
//! The keyboard's own era, which the manifest never named and so the gate cannot
//! ask for. They came in beside the wvkbd fork and went when `virtual-keyboard`
//! became a crate; a sweep that took the four the manifest happens to remember
//! and left these three beside them would be tidying by paperwork.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1788609965),
    says: "sweeping what the manifest stopped naming after the rename",
    steps: &[
        Step::Attic("/etc/firefox/policies/policies.json"),
        Step::Attic("/etc/inputplumber/profiles/desktop.yaml"),
        Step::Attic("/etc/inputplumber/profiles/keyboard.yaml"),
        Step::Attic("/etc/inputplumber/profiles/menu.yaml"),
        Step::Attic("/etc/inputplumber/profiles/tabs.yaml"),
        Step::Attic("/etc/plasmalogin.conf.d/zzz-session.conf"),
        Step::Attic("/home/@user@/.config/gtk-3.0/colors.css"),
        Step::Attic("/home/@user@/.config/gtk-4.0/colors.css"),
        Step::Attic("/home/@user@/.config/hypr/hyprpaper.conf"),
        Step::Attic("/home/@user@/.config/wofi/config"),
        Step::Attic("/home/@user@/.config/wofi/guide.css"),
        Step::Attic("/home/@user@/.config/wofi/style.css"),
        Step::Attic("/usr/local/bin/console-pictures"),
        Step::Attic("/usr/local/bin/console-poke"),
        Step::Attic("/usr/local/bin/console-timings"),
        Step::Attic("/usr/local/bin/home-place"),
        Step::Attic("/usr/local/bin/keyboard-start"),
        Step::Attic("/usr/local/bin/osk"),
        Step::Attic("/usr/local/bin/osk-hook"),
        Step::Attic("/usr/local/bin/osk-start"),
        Step::Attic("/usr/local/bin/power-menu"),
        Step::Attic("/usr/local/bin/wvkbd-mobintl"),
        Step::Attic("/usr/share/applications/console-notices.desktop"),
        Step::Attic("/usr/local/bin/osk-toggle"),
        Step::Attic("/usr/local/bin/wvkbd-kwin"),
        Step::Attic("/home/@user@/.config/hypr/hyprland.lua.working"),
    ],
};
