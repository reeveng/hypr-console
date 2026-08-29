//! A word in the corner, on its own, so it can be looked at.
//!
//!     cargo run --example note
//!
//! A panel of one page that says something as it arrives, the way the
//! Wallpaper tab does when a picture has been chosen and is being put up. It
//! takes itself down after six seconds, so this is a surface to catch rather
//! than one to sit and read.

use std::sync::Arc;

use console_panel::page::{Page, Row, Rows};
use console_panel::panel;

fn main() {
    let build = Arc::new(|| {
        vec![
            Page::new(
                "Wallpaper",
                Rows::asked(|| {
                    vec![
                        Row::said("Follow the weather", ""),
                        Row::said("Star Ride", "Abi Toads"),
                    ]
                }),
            )
            .on_arriving(|showing| showing.note("Star Ride is going up")),
        ]
    });
    panel::show(build, 0, None);
}
