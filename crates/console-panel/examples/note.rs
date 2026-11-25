//! A word in the corner, on its own, so it can be looked at.
//!
//!     cargo run --example note
//!
//! A panel of one page that says something as it arrives, the way the
//! Wallpaper tab does when a picture has been chosen and is being put up. It
//! takes itself down after six seconds, so this is a surface to catch rather
//! than one to sit and read.

use console_panel::page::{Aside, Page, Row, Rows};
use console_panel::surface;

fn main() {
  let build = || {
    let Ok(asked) = Rows::computed(|| {
      let Ok(weather) = Row::text("Follow the weather", Aside(""));
      let Ok(picture) = Row::text("Star Ride", Aside("Abi Toads"));

      vec![weather, picture]
    });
    let Ok(page) = Page::new("Wallpaper", asked);
    let Ok(page_2) = page.on_arriving(|_| {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Console"),
        "Star Ride is going up",
        None,
      );
    });

    vec![page_2]
  };
  let Ok(()) = surface::show(build, 0, None);
}
