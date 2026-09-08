//! What this machine is called.
//!
//! The one setting on this panel that is typed rather than chosen, because
//! nobody can offer a list of names a person might give their own device. So
//! it goes through the same one-shot question the Wi-Fi password does: the
//! keyboard comes up, a word comes back, and the row says what it is now.
//!
//! What comes back is checked here rather than by `hostnamectl`, which takes
//! what it is given and leaves a machine that answers to something no other
//! machine on the network can look up. The rule is the one the internet has
//! had since the eighties: letters, digits and hyphens, not starting or ending
//! with one, and short enough to fit in a label.

use console_core_never::Never;

pub const LONGEST: usize = 63;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowed {
    Yes,
    No,
}

pub fn read(said: &str) -> Result<String, Never> {
    Ok(said.trim().to_string())
}

pub fn allowed(name: &str) -> Result<Allowed, Never> {
    let letters = name.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '-');
    let ends = name.starts_with('-') || name.ends_with('-');
    let room = (1..=LONGEST).contains(&name.chars().count());

    Ok(match letters && room && !ends {
        true => Allowed::Yes,
        false => Allowed::No,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(name: &str) -> Allowed {
        let Ok(allowed) = super::allowed(name);

        allowed
    }

    #[test]
    fn a_name_a_network_can_look_up_is_taken() {
        assert_eq!(allowed("legion"), Allowed::Yes);
        assert_eq!(allowed("legion-go"), Allowed::Yes);
        assert_eq!(allowed("go2"), Allowed::Yes);
    }

    #[test]
    fn nothing_is_not_a_name() {
        assert_eq!(allowed(""), Allowed::No);
        assert_eq!(allowed("   "), Allowed::No);
    }

    #[test]
    fn a_name_with_a_space_or_a_dot_in_it_is_refused_rather_than_written() {
        assert_eq!(allowed("my machine"), Allowed::No);
        assert_eq!(allowed("legion.go"), Allowed::No);
        assert_eq!(allowed("légion"), Allowed::No);
    }

    #[test]
    fn a_hyphen_at_either_end_is_the_one_that_looks_fine_and_is_not() {
        assert_eq!(allowed("-legion"), Allowed::No);
        assert_eq!(allowed("legion-"), Allowed::No);
    }

    #[test]
    fn a_name_longer_than_a_label_is_refused_here_rather_than_cut_there() {
        assert_eq!(allowed(&"a".repeat(LONGEST)), Allowed::Yes);
        assert_eq!(allowed(&"a".repeat(LONGEST.saturating_add(1))), Allowed::No);
    }

    #[test]
    fn what_the_machine_says_it_is_called_has_a_newline_on_it() {
        assert_eq!(read("legion\n"), Ok("legion".to_string()));
    }
}
