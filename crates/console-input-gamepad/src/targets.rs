//! What this desktop asks InputPlumber to make, and how each one is known
//! again on the machine.
//!
//! The profile `router.rs` writes names three target devices, and InputPlumber
//! makes one uinput device for each. Which device on the machine was which used
//! to be decided by an empty physical path, under the name `ByInputPlumber` --
//! but having no physical path is true of every uinput device there is, and
//! Steam publishes a pad of its own the moment it is running on the desktop.
//! The daemon opened that one instead. Nothing emits on it, so every button on
//! the desktop stopped arriving while the profile, the unit and the daemon all
//! read as healthy, and the only thing that said so was a check pressing B at a
//! panel that would not close.
//!
//! A target arrives wearing the identity of the thing it emulates, and which
//! identity that is follows from the name it was asked for. So the name and the
//! two numbers are one fact and are written here together, rather than a word
//! in the profile and a rule somewhere else that has to agree with it.
//!
//! The physical path is still read and now means what it says: an Xbox Elite 2
//! controller somebody plugs in carries the same two numbers as the target
//! emulating one, and is not it.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub vendor: u16,
    pub product: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Mouse,
    Keyboard,
    Pad,
}

pub const ASKED: [Target; 3] = [Target::Mouse, Target::Keyboard, Target::Pad];

impl Target {
    pub fn asked(self) -> Result<&'static str, Never> {
        Ok(match self {
            Target::Mouse => "mouse",
            Target::Keyboard => "keyboard",
            Target::Pad => "xbox-elite",
        })
    }

    pub fn identity(self) -> Result<Identity, Never> {
        Ok(match self {
            Target::Mouse => Identity { vendor: 0x0000, product: 0xffff },
            Target::Keyboard => Identity { vendor: 0x1234, product: 0x5678 },
            Target::Pad => Identity { vendor: 0x045e, product: 0x0b00 },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_targets_arrive_wearing_the_same_identity() {
        for one in ASKED {
            for other in ASKED {
                let Ok(mine) = one.identity();
                let Ok(theirs) = other.identity();

                match one == other {
                    true => {},
                    false => assert_ne!(
                        mine, theirs,
                        "{one:?} and {other:?} cannot be told apart on the machine"
                    ),
                }
            }
        }
    }

    #[test]
    fn every_target_is_asked_for_by_a_name_of_its_own() {
        let mut names: Vec<&str> = Vec::new();

        for target in ASKED {
            let Ok(asked) = target.asked();

            assert!(!names.contains(&asked), "{asked} is asked for twice");
            names.push(asked);
        }
    }
}
