//! What the bar says about the machine, and which menu you are in.
//!
//! waybar has modules of its own for the sound, the network, the bluetooth and
//! the battery, and they read the machine perfectly well. What they cannot do
//! is wear a class that something outside them decides, and every one of these
//! icons opens a tab of the settings panel. So the icon could not say whether
//! the thing it opens is already on the screen, which the two doors on the left
//! have said since the day they were written.
//!
//! These are those four readings, said by us, so that the icon lights while its
//! own tab is in front.

pub mod notices;
pub mod reading;
pub mod watch;
