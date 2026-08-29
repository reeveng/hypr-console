//! The sky: which wallpaper is up, and why.
//!
//! One picture on the screen all day is a picture nobody sees after the first
//! week. This chooses between several, by the hour and by the weather outside,
//! and hands the answer to the wallpaper daemon.
//!
//! `grade` is how somebody else's picture is brought into this palette.

pub mod choose;
pub mod covered;
pub mod grade;
pub mod here;
pub mod loops;
pub mod moon;
pub mod place;
pub mod press;
pub mod source;
pub mod sun;
pub mod weather;
