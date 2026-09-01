//! Where the buttons are moved, on a device that is not the one this grew on.
//!
//! Every default in this repository is written in a Legion Go's words. Five of
//! them -- the four paddles and Legion right -- name nothing on an ordinary
//! pad, and the menu, closing, dictation, the screenshot and the settings sit
//! on all five. On such a device this desktop installs, works, and has five
//! promises it cannot keep, which is what `console check` says and what the
//! notice after an apply says.
//!
//! This is where somebody answers. One row per thing the desktop does, what
//! plays it now beside it, and moving one is pressing the button you want it
//! on -- with a trigger held first, if you want it on a layer. Nobody holding
//! a handheld knows which paddle `RightPaddle3` is, and a list of names is the
//! worse screen for the same question.
//!
//! `rows` is the screen and has never seen a machine. `layout-panel` is the
//! screen with one, and `console-asking` is the card that reads the press.

pub mod rows;
pub mod table;
