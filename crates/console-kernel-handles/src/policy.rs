//! What a job refuses the processes inside it, whatever handles they hold.
//!
//! Zircon's job policy, cut to the conditions a browser's job is the case for:
//! making a process, a job, a channel or a piece of memory, and turning memory
//! executable. A handle says what its holder may do to one object; a policy
//! says what a whole tree of processes may never do, and it is the answer to a
//! renderer that has somehow come to hold the executable resource anyway.
//!
//! There is only one action, and it is `Deny`. A job starts with its parent's
//! policy and can only add to it, and only while it is empty, so no process can
//! find itself under a policy that changed after it started and no job can be
//! looser than the job it is in. Zircon has an override flag for letting a
//! child loosen what its parent allowed; there is nothing here to loosen from.

use core::fmt;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    NewProcess,
    NewJob,
    NewChannel,
    NewMemoryObject,
    ReplaceAsExecutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy(u8);

impl Condition {
    fn bit(self) -> Result<u8, Never> {
        Ok(match self {
            Condition::NewProcess => 0b0_0001,
            Condition::NewJob => 0b0_0010,
            Condition::NewChannel => 0b0_0100,
            Condition::NewMemoryObject => 0b0_1000,
            Condition::ReplaceAsExecutable => 0b1_0000,
        })
    }
}

impl Policy {
    pub const ALLOW_ALL: Policy = Policy(0);

    pub fn denying(self, conditions: &[Condition]) -> Result<Policy, Never> {
        let mut denied = self.0;

        for condition in conditions {
            let Ok(bit) = condition.bit();

            denied |= bit;
        }

        Ok(Policy(denied))
    }

    pub fn action(self, condition: Condition) -> Result<PolicyAction, Never> {
        let Ok(bit) = condition.bit();

        Ok(match self.0 & bit {
            0 => PolicyAction::Allow,
            _ => PolicyAction::Deny,
        })
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        to.write_str(match self {
            Condition::NewProcess => "start a process",
            Condition::NewJob => "start a job",
            Condition::NewChannel => "make a channel",
            Condition::NewMemoryObject => "make memory",
            Condition::ReplaceAsExecutable => "make memory executable",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_denied_condition_stays_denied_whatever_is_denied_after_it() -> Result<(), Never> {
        let Ok(policy) = Policy::ALLOW_ALL.denying(&[Condition::NewProcess]);
        let Ok(later) = policy.denying(&[Condition::NewChannel]);

        assert_eq!(later.action(Condition::NewProcess), Ok(PolicyAction::Deny));
        assert_eq!(later.action(Condition::NewChannel), Ok(PolicyAction::Deny));
        assert_eq!(later.action(Condition::NewJob), Ok(PolicyAction::Allow));

        Ok(())
    }
}
