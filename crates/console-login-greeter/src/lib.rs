//! The screen the way in is drawn on.
//!
//! It is the login window's child and speaks to it in
//! `console_login_window::protocol`'s lines over its stdin and stdout: it draws the
//! dots, hears the pad, and says `login` with the pattern's letters when Login
//! is pressed. It never sees PAM and never decides anything about who may come
//! in; that is the window's, which runs as root, and this runs as the greeter's
//! own account with nothing but the screen and the pad.
//!
//! It draws with the tree's own painter onto a buffer the kernel shows
//! directly, because before a login there is no compositor to hand it a
//! surface. `display` is that half, `turn` lays the picture onto a panel
//! mounted sideways, and `greeting` and `picture` are what decides and what is
//! drawn, both asked with no machine at all.

pub mod display;
pub mod greeting;
pub mod picture;
pub mod turn;
