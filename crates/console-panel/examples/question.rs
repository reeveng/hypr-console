//! The question surface, on its own, so it can be looked at.
//!
//!     cargo run --example question
//!
//! A panel of one page that asks as it arrives. Nothing is thrown away by
//! answering it.

use console_panel::page::{Aside, Page, Row, Rows, Subject};
use console_panel::surface;

fn main() {
  let build = || {
    let Ok(asked) = Rows::computed(|| {
      let Ok(row) = Row::text("holiday.jpg", Aside(""));

      vec![row]
    });
    let Ok(page) = Page::new("Pictures", asked);
    let Ok(page_2) = page.on_arriving(|showing| {
      showing.sure(
        "Delete this?",
        Subject("holiday.jpg"),
        &["Delete"],
        Box::new(|_, _| ()),
      );
    });

    vec![page_2]
  };
  let Ok(()) = surface::show(build, 0, None);
}
