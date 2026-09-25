//! What the weather is here, and what it is going to be.
//!
//! `here` is where the machine is, taken from the timezone its clock keeps.
//! `conditions` is the WMO code open-meteo answers with, and what it is called:
//! grouped into the handful of skies there is a wallpaper for, and named one by
//! one for somebody reading a forecast. `forecast` is the week ahead asked
//! for at once, which is what the forecast panel draws.
//!
//! These were the wallpaper's until the forecast panel became the second thing
//! to ask. Two crates asking one service is two readings of one answer, so the
//! service moved here and both of them ask this crate instead, and neither of
//! them had to bring a toolkit or a picture renderer along to do it.

pub mod conditions;
pub mod forecast;
pub mod here;
