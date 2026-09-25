//! The question surface, on its own, so it can be looked at.
//!
//!     cargo run --example question
//!
//! A panel of one page that asks as it arrives. Nothing is thrown away by
//! answering it.

use std::sync::Arc;

use console_panel::page::{Aside, Page, Row, Rows, Subject};
use console_panel::surface;

fn main() {
    let build = Arc::new(|| {
        let Ok(asked) = Rows::asked(|| {
            let Ok(row) = Row::said("holiday.jpg", Aside(""));

            vec![row]
        });
        let Ok(page) = Page::new("Pictures", asked);
        let Ok(page) = page.on_arriving(|showing| {
            showing.sure("Delete this?", Subject("holiday.jpg"), &["Delete"], Arc::new(|_, _| ()));
        });

        vec![page]
    });
    let Ok(()) = surface::show(build, 0, None);
}
