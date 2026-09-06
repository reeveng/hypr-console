//! When the card gets out of the way, and what brings it back.
//!
//! A film is watched, not operated. What is written under it -- the name and
//! the size, the bar saying how far along it is, the presses that start and
//! stop it -- is there to be used and then to be gone, which is what everything
//! else that plays a film does: the controls come up under a hand and leave
//! again when the hand does.
//!
//! Nothing here reads a clock. What is handed in is how long it has been since
//! the last press, so the whole of this can be asked about without anybody
//! waiting for it.

use std::time::Duration;

use console_never::Never;

pub const QUIET: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awake {
    Yes,
    No,
}

pub fn awake(since: Duration) -> Result<Awake, Never> {
    Ok(match since < QUIET {
        true => Awake::Yes,
        false => Awake::No,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Awake, QUIET, awake};

    #[test]
    fn a_card_just_pressed_is_awake() {
        assert_eq!(awake(Duration::ZERO), Ok(Awake::Yes));
        assert_eq!(awake(QUIET - Duration::from_millis(1)), Ok(Awake::Yes));
    }

    #[test]
    fn a_card_nobody_has_touched_is_only_the_picture() {
        assert_eq!(awake(QUIET), Ok(Awake::No));
        assert_eq!(awake(Duration::from_secs(600)), Ok(Awake::No));
    }

    #[test]
    fn the_quiet_is_longer_than_the_tick_that_would_end_it() {
        assert!(QUIET >= Duration::from_secs(2));
    }
}
