//! The question surface, on its own, so it can be looked at.
//!
//!     cargo run --example question
//!
//! A panel of one page that asks as it arrives. Nothing is thrown away by
//! answering it.

use std::sync::Arc;

use console_panel::page::{Page, Row, Rows};
use console_panel::panel;

fn main() {
    let build = Arc::new(|| {
        vec![Page::new("Pictures", Rows::asked(|| vec![Row::said("holiday.jpg", "")]))
            .on_arriving(|showing| {
                showing.sure("Throw this away?", "holiday.jpg", &["Delete"], Arc::new(|_, _| ()));
            })]
    });
    panel::show(build, 0, None);
}
