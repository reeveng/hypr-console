//! Leave the player going: the library, in any order, round for ever.
//!
//! Run right after a song is chosen, and for one reason: choosing a song and
//! being handed silence four minutes later is a machine that stops in the
//! middle of the evening and waits to be asked again. A handheld that is being
//! carried about is the last place anybody wants to go back to a panel to hear
//! a second song, so what one press means here is *play*, not *play this*.
//!
//! Which is what the song on the end of this line is for. A player told to
//! open one song used to hold one song, so next and previous had nowhere to go
//! and the evening ended when it did; told by the fork now, it builds the
//! playlist out of the library around that song. The panel asks for the song
//! itself as the press lands, so the answer is instant, and this asks again --
//! because on the press that starts the player there was nobody there to hear
//! the first one.
//!
//! Its own program rather than three more lines in the panel, because it has
//! to wait: the player is being launched and a bus name is being taken while
//! this runs, and neither is instant. The panel hands it to `later`, which is a
//! thread of the panel's own, and the tab is drawn again when it comes back --
//! by which time the two marks on the transport are lit.
//!
//! Nothing here overrules anybody. Both modes are ordinary presses of the two
//! keys the transport already offers, so turning either of them off is somebody
//! saying what they want rather than the machine having never decided.

use console_music::player;
use std::path::PathBuf;

fn main() {
    // Nothing to open where nothing was named, which is somebody running this
    // by hand to put a player that has been left in some other state back the
    // way this desktop leaves it.
    let Some(song) = std::env::args().nth(1).map(PathBuf::from) else {
        player::onward_only();
        return;
    };

    player::onward(&song);
}
