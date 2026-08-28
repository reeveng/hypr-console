//! The palette as shell variables, for the keyboard.
//!
//! wvkbd takes its colours as arguments and has no configuration file, so
//! something has to turn them into a command line. `osk-start` sources this
//! and spends them by name.

use crate::palette::Palette;
use crate::spend::ROLES;

/// Not aligned on the equals sign the way the stylesheet is aligned: a space
/// before it makes the shell read the name as a command, and a palette that
/// lines up beautifully and stops the keyboard starting is not a trade
/// anybody wants.
pub fn spend(palette: &Palette) -> String {
    let body = ROLES
        .iter()
        .map(|name| format!("{name}={}", &palette[name]))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# Written by legion-theme from theme/palette.toml.\n\
         # Sourced by osk-start, which holds no colour of its own.\n\n{body}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn nothing_is_padded_before_the_equals_sign() {
        // A space there makes the shell read the name as a command, and the
        // keyboard then starts with no colours or does not start at all.
        for line in spend(&blossom()).lines().filter(|l| l.contains('=')) {
            let (name, _) = line.split_once('=').expect("an assignment");
            assert!(!name.ends_with(' '), "{line:?} pads before the equals sign");
            assert!(!name.starts_with(' '), "{line:?} is indented");
        }
    }

    #[test]
    fn every_role_is_assigned_once() {
        let sh = spend(&blossom());
        for name in ROLES {
            let count = sh.lines().filter(|l| l.starts_with(&format!("{name}="))).count();
            assert_eq!(count, 1, "{name} is assigned {count} times");
        }
    }

    #[test]
    fn a_value_is_six_hex_digits_with_no_hash_and_no_quotes() {
        // osk-start builds a command line out of these, and wvkbd wants the
        // digits alone. A hash would start a comment.
        for line in spend(&blossom()).lines().filter(|l| l.contains('=')) {
            let (_, value) = line.split_once('=').expect("an assignment");
            assert_eq!(value.len(), 6, "{line:?}");
            assert!(value.chars().all(|c| c.is_ascii_hexdigit()), "{line:?}");
        }
    }
}
