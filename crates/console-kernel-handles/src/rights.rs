//! What a handle lets its holder do, one right to a bit.
//!
//! Zircon's list, cut to what a process born empty needs before anything
//! else: to read and write through a handle, to run what it names, to make a
//! second handle to the same thing, to give a handle away, to start a process
//! or a job inside a job, and to end one. A right is only
//! ever taken off on the way to another process, never put on, and the one
//! exception is `Execute`, which only `Kernel::replace_as_executable` adds and only to a
//! handle that gives up `Write` in the same step.

use core::fmt;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Right {
    Read,
    Write,
    Execute,
    Duplicate,
    Transfer,
    ManageJob,
    Destroy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contains {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(u8);

impl Right {
    fn bit(self) -> Result<u8, Never> {
        Ok(match self {
            Right::Read => 0b0_0001,
            Right::Write => 0b0_0010,
            Right::Execute => 0b0_0100,
            Right::Duplicate => 0b0_1000,
            Right::Transfer => 0b1_0000,
            Right::ManageJob => 0b10_0000,
            Right::Destroy => 0b100_0000,
        })
    }
}

impl Rights {
    pub const NONE: Rights = Rights(0);

    pub fn from_rights(rights: &[Right]) -> Result<Rights, Never> {
        let mut held = Rights::NONE;

        for right in rights {
            let Ok(with) = held.union(*right);

            held = with;
        }

        Ok(held)
    }

    pub fn union(self, right: Right) -> Result<Rights, Never> {
        let Ok(bit) = right.bit();

        Ok(Rights(self.0 | bit))
    }

    pub fn difference(self, right: Right) -> Result<Rights, Never> {
        let Ok(bit) = right.bit();

        Ok(Rights(self.0 & !bit))
    }

    pub fn contains(self, right: Right) -> Result<Contains, Never> {
        let Ok(bit) = right.bit();

        Ok(match self.0 & bit {
            0 => Contains::No,
            _ => Contains::Yes,
        })
    }

    pub fn subset_of(self, wider: Rights) -> Result<Contains, Never> {
        Ok(match self.0 & !wider.0 {
            0 => Contains::Yes,
            _ => Contains::No,
        })
    }
}

impl fmt::Display for Right {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        to.write_str(match self {
            Right::Read => "read",
            Right::Write => "write",
            Right::Execute => "execute",
            Right::Duplicate => "duplicate",
            Right::Transfer => "transfer",
            Right::ManageJob => "manage the job",
            Right::Destroy => "destroy",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_right_taken_off_is_no_longer_held_and_the_rest_are() -> Result<(), Never> {
        let Ok(both) = Rights::from_rights(&[Right::Read, Right::Write]);
        let Ok(read) = both.difference(Right::Write);

        assert_eq!(read.contains(Right::Write), Ok(Contains::No));
        assert_eq!(read.contains(Right::Read), Ok(Contains::Yes));

        Ok(())
    }

    #[test]
    fn fewer_rights_are_within_more_and_not_the_other_way() -> Result<(), Never> {
        let Ok(read) = Rights::from_rights(&[Right::Read]);
        let Ok(both) = Rights::from_rights(&[Right::Read, Right::Write]);

        assert_eq!(read.subset_of(both), Ok(Contains::Yes));
        assert_eq!(both.subset_of(read), Ok(Contains::No));
        assert_eq!(Rights::NONE.subset_of(read), Ok(Contains::Yes));

        Ok(())
    }
}
