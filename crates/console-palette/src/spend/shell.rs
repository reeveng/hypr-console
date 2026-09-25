//! The palette as a list of names and colors.  Written as shell assignments
//! because the nested desktop sources it to set its ground before anything else
//! is up, and a shell is what it has there. Everything else reads it as text:
//! `console_core_color::palette::read` is the one reader, and the keyboard and
//! the checks both go through it.  It is still the keyboard's palette above
//! all. The virtual keyboard takes its colors as arguments and has no
//! configuration file, so something has to turn them into a command line, and
//! that something needs them by name.

use console_core_color::Short;
use crate::palette::{Palette, Spent};
use crate::spend::ROLES;

pub fn spend(palette: &Palette) -> Result<String, Short> {
    let lines = palette.lines(&ROLES, |Spent { name, color }| format!("{name}={color}"))?;
    let body = lines.join("\n");
    Ok(format!(
        "# Written by console-palette from theme/palette.toml.\n\
         # Read by the keyboard and the checks, and sourced by the nested\n\
         # desktop. Nothing that reads it holds a color of its own.\n\n{body}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn nothing_is_padded_before_the_equals_sign() {
        for line in spend(&blossom()).expect("every color it spends is declared").lines().filter(|line| line.contains('=')) {
            let (name, _) = line.split_once('=').expect("an assignment");
            assert!(!name.ends_with(' '), "{line:?} pads before the equals sign");
            assert!(!name.starts_with(' '), "{line:?} is indented");
        }
    }

    #[test]
    fn every_role_is_assigned_once() {
        let sh = spend(&blossom()).expect("every color it spends is declared");
        for name in ROLES {
            let assigned: Vec<&str> = sh.lines().filter(|line| line.starts_with(&format!("{name}="))).collect();
            assert_eq!(assigned.len(), 1, "{name} is assigned as {assigned:?}");
        }
    }

    #[test]
    fn a_value_is_six_hex_digits_with_no_hash_and_no_quotes() {
        for line in spend(&blossom()).expect("every color it spends is declared").lines().filter(|line| line.contains('=')) {
            let (_, value) = line.split_once('=').expect("an assignment");
            assert_eq!(value.len(), 6, "{line:?}");
            assert!(value.chars().all(|digit| digit.is_ascii_hexdigit()), "{line:?}");
        }
    }
}
