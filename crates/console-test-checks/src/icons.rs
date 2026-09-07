//! Every icon a surface asks for is an icon the theme has.
//!
//! The colours have this already: `every_name_the_desktop_asks_for_is_defined`
//! crosses every name a stylesheet asks for against the palette, because a GTK
//! stylesheet that names a colour nobody defined drops the declaration and
//! carries on. An icon name nobody has is the same fault with a louder ending
//! -- GTK draws the broken square -- and until now nothing crossed the names
//! against a theme.
//!
//! It is on this tier and not in the suite because the theme is a package the
//! device installs. The machine this is written on has not got it, so a test
//! here could only ever have asked a different question and answered it
//! confidently.

use console_panel::icons::{EVERY, Icon};
use console_test_stages::checking::{Body, Check, Done, cannot, empty};
use console_test_stages::device::Device;

pub const ICONS: Check = Check {
    name: "300-every-icon-is-one-the-theme-has",
    about: "Every icon name a panel asks for is one the device's icon theme can draw.",
    feature: "icons",
    since: "2026-09-04",
    bodies: &[Body::Device(there)],
};

fn there(stage: &mut Device) -> Done {
    let Ok(theme) = stage.icon_theme();

    match theme.is_empty() {
        true => return cannot("the machine's gtk-4.0 settings name no icon theme"),
        false => {},
    }

    let names: Vec<&str> = EVERY
        .iter()
        .map(|icon| {
            let Ok(named) = Icon::name(*icon);

            named
        })
        .collect();

    let Ok(missing) = stage.icons_missing(&theme, &names);

    empty(&missing, || {
        format!("{theme} has nothing for {missing:?}, so those are drawn as broken squares")
    })
}
