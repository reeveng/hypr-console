//! waybar, and the five programs that were its modules.
//!
//! The bar is `console-bar` now: one process drawing one layer surface, with the
//! strip in its own last rows. What it replaced was waybar reading five little
//! programs of ours, each printing a line of JSON that crossed back into CSS as
//! a class name, and the swap is one line of `console-bar.service` -- so an apply
//! restarts the unit and the new bar is what comes up.
//!
//! What that leaves behind is the old half of it, and the half that still runs.
//! `bar-clock`, `bar-door`, `bar-notice`, `bar-say` and `bar-updating` are in
//! /usr/local/bin on every machine that ever applied, and a program nothing
//! starts is only harmless until someone starts it: each of them holds a
//! subscription open and prints until it is killed. The two waybar files are
//! worse in the other direction -- they are a whole working bar, and a person who
//! runs `waybar` at a prompt gets a second bar reserving a second zone across the
//! top of the screen, drawn by a config this tree no longer holds and no longer
//! checks. `bar.css` is what that config imported: the width of the strip,
//! written at every login by `console-scale`, which does not write it any more.
//!
//! The package is deliberately not touched. `waybar` leaving `[packages]` means
//! the manifest stops asking for it, and pacman is the machine's own record of
//! what is installed: removing a package is a decision for whoever is holding the
//! device, and `pacman -Rns waybar` is one command when they want it. Nothing
//! starts it once this has run.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1789848578),
    says: "sweeping waybar's bar and the five programs it read the bar out of",
    steps: &[
        Step::Attic("/usr/local/bin/bar-clock"),
        Step::Attic("/usr/local/bin/bar-door"),
        Step::Attic("/usr/local/bin/bar-notice"),
        Step::Attic("/usr/local/bin/bar-say"),
        Step::Attic("/usr/local/bin/bar-updating"),
        Step::Attic("/home/@user@/.config/waybar/config.jsonc"),
        Step::Attic("/home/@user@/.config/waybar/style.css"),
        Step::Attic("/home/@user@/.config/console/bar.css"),
        Step::RemoveIfEmpty("/home/@user@/.config/waybar"),
    ],
};
