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

/// How long the card is left alone before it is only the picture.
///
/// Long enough to reach for a second press after making one, and short enough
/// that a film is not watched through a strip of buttons. It is a whole number
/// of seconds because the card is redrawn on a tick of one, so anything
/// finer is a number nothing can act on.
pub const QUIET: Duration = Duration::from_secs(4);

/// Whether the rows under the picture are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awake {
    Yes,
    No,
}

/// Whether the card is awake, given how long since the last press.
///
/// One rule for a film and for a photograph. A photograph has less to get out
/// of the way and gains less by it, but a card that hides its rows on one kind
/// and keeps them on the other is a card whose behaviour has to be learned
/// twice, and a photograph looked at for a while is the case where the picture
/// wants the whole card just as much.
///
/// Nothing here asks whether the film is running. A film stopped on a frame is
/// a frame somebody stopped on to look at, which is the picture wanting the
/// room rather than an argument for keeping the buttons over it.
pub fn awake(since: Duration) -> Awake {
    match since < QUIET {
        true => Awake::Yes,
        false => Awake::No,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Awake, QUIET, awake};

    #[test]
    fn a_card_just_pressed_is_awake() {
        assert_eq!(awake(Duration::ZERO), Awake::Yes);
        assert_eq!(awake(QUIET - Duration::from_millis(1)), Awake::Yes);
    }

    /// Left alone, the rows go and the picture has the card.
    #[test]
    fn a_card_nobody_has_touched_is_only_the_picture() {
        assert_eq!(awake(QUIET), Awake::No);
        assert_eq!(awake(Duration::from_secs(600)), Awake::No);
    }

    /// The tick that redraws the card is one second, so a quiet shorter than a
    /// couple of ticks is rows that come and go while somebody is still
    /// reaching.
    #[test]
    fn the_quiet_is_longer_than_the_tick_that_would_end_it() {
        assert!(QUIET >= Duration::from_secs(2));
    }
}
