//! The palette as a list of names and colours.
//!
//! Written as shell assignments because the nested desktop sources it to set
//! its ground before anything else is up, and a shell is what it has there.
//! Everything else reads it as text: `console_colour::spent::read` is the one
//! reader, and the keyboard and the checks both go through it.
//!
//! It is still the keyboard's palette above all. The virtual keyboard takes
//! its colours as arguments and has no configuration file, so something has to
//! turn them into a command line, and that something needs them by name.

use console_colour::Short;
use crate::palette::Palette;
use crate::spend::ROLES;

/// Not aligned on the equals sign the way the stylesheet is aligned: a space
/// before it makes the shell read the name as a command, and a palette that
/// lines up beautifully and stops the keyboard starting is not a trade
/// anybody wants.
pub fn spend(palette: &Palette) -> Result<String, Short> {
    let body = ROLES
        .iter()
        .map(|name| Ok(format!("{name}={}", palette.must(name)?)))
        .collect::<Result<Vec<_>, Short>>()?
        .join("\n");
    Ok(format!(
        "# Written by console-theme from theme/palette.toml.\n\
         # Read by the keyboard and the checks, and sourced by the nested\n\
         # desktop. Nothing that reads it holds a colour of its own.\n\n{body}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn nothing_is_padded_before_the_equals_sign() {
        // A space there makes the shell read the name as a command, and the
        // keyboard then starts with no colours or does not start at all.
        for line in spend(&blossom()).expect("every colour it spends is declared").lines().filter(|l| l.contains('=')) {
            let (name, _) = line.split_once('=').expect("an assignment");
            assert!(!name.ends_with(' '), "{line:?} pads before the equals sign");
            assert!(!name.starts_with(' '), "{line:?} is indented");
        }
    }

    #[test]
    fn every_role_is_assigned_once() {
        let sh = spend(&blossom()).expect("every colour it spends is declared");
        for name in ROLES {
            let count = sh.lines().filter(|l| l.starts_with(&format!("{name}="))).count();
            assert_eq!(count, 1, "{name} is assigned {count} times");
        }
    }

    #[test]
    fn a_value_is_six_hex_digits_with_no_hash_and_no_quotes() {
        // The keyboard builds a command line out of these, and it
        // wants the digits alone. A hash would start a comment in this file
        // anyway.
        for line in spend(&blossom()).expect("every colour it spends is declared").lines().filter(|l| l.contains('=')) {
            let (_, value) = line.split_once('=').expect("an assignment");
            assert_eq!(value.len(), 6, "{line:?}");
            assert!(value.chars().all(|c| c.is_ascii_hexdigit()), "{line:?}");
        }
    }
}
