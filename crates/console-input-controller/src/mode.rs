//! What is in front of you, which is what the buttons are for.
//!
//! This desktop has kept that in three places and owned it in none. The pad's
//! InputPlumber profile is one, and switching it destroys the pad and builds
//! another, so the meaning of a button and the existence of the device
//! carrying it are the same act. A file in the runtime directory is the
//! second, holding which profile was loaded before the keyboard came up, and
//! it is written by whichever program got there. Stopping this daemon outright
//! with SIGSTOP is the third: not a mode at all, but a way of making sure only
//! one of two programs acts on a press.
//!
//! None of those is a fact about the machine. They are notes programs leave
//! each other, and a note is wrong the moment someone restarts without
//! reading it -- which is exactly the shape of the fault where X stopped
//! showing the keyboard until the next reboot.
//!
//! So the mode is read rather than remembered, and it is read from the
//! compositor, which is the only thing that cannot be wrong about what is on
//! its own screen.
//!
//! The mode used to decide the profile too, and it no longer decides anything
//! about the device. The pad wears `router` from login to shutdown: the two
//! profiles that were loaded to hand it over -- one for the keyboard, one for
//! the card that asks which button that was -- are gone, because both were a
//! program saying "this is mine now" through a file six other programs can
//! write. `console_input_focus` says it to the kernel instead, and the kernel
//! is the one thing here that cannot be out of date about who is holding what.
//! `input_handling` is what is left, and it is this daemon's own restraint rather than
//! anything it does to the machine.
//!
//! An app this desktop wrote -- the books, the files, the pictures -- is a window
//! on a workspace like any other, and nothing over the desktop says it is there.
//! What says it is the window in front: one of ours in front is `App`, where a
//! press means what it means in a panel, and anything else is the desktop. A
//! panel opened over one of ours is still a panel.

use console_core_never::Never;
use console_onscreen::{Up, over_the_desktop, up};

pub use console_onscreen::{ASKING, Focused, HOME, KEYBOARD, Over, SYSTEM_SURFACES};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Desktop,
    Tabs,
    App,
    HomeScreen,
    Standing,
    Keyboard,
    Prompt,
}

impl Mode {
    pub fn detect(screens: &[console_compositor::Layer], awake: Woken, focused: Focused) -> Result<Self, Never> {
        let Ok(asking) = up(screens, ASKING);

        match asking {
            Up::OnScreen => return Ok(Mode::Prompt),
            Up::NotThere => {},
        }

        let Ok(keyboard) = up(screens, KEYBOARD);

        match keyboard {
            Up::OnScreen => return Ok(Mode::Keyboard),
            Up::NotThere => {},
        }

        let Ok(over) = over_the_desktop(screens);
        let Ok(home) = up(screens, HOME);

        Ok(match (over, focused) {
            (Over::Some, _) => Mode::Tabs,
            (Over::None, Focused::OurApp) => Mode::App,
            (Over::None, Focused::SomethingElse) => match (home, awake) {
                (Up::OnScreen, Woken::Yes) => Mode::Standing,
                (Up::OnScreen, Woken::No) => Mode::HomeScreen,
                (Up::NotThere, _) => Mode::Desktop,
            },
        })
    }

    pub fn input_handling(self) -> Result<InputHandling, Never> {
        Ok(match matches!(self, Mode::Keyboard | Mode::Prompt) {
            true => InputHandling::Disabled,
            false => InputHandling::Enabled,
        })
    }
}

pub use console_onscreen::Woken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputHandling {
    Enabled,
    Disabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    fn seen_with(said: &str, awake: Woken, focused: Focused) -> Result<Mode, Failure> {
        let value: serde_json::Value = serde_json::from_str(said)?;
        let layers = console_compositor::said_of(console_compositor::Layers, value)?;
        let Ok(mode) = Mode::detect(&layers, awake, focused);

        Ok(mode)
    }

    fn seen(said: &str, awake: Woken) -> Result<Mode, Failure> {
        seen_with(said, awake, Focused::SomethingElse)
    }

    fn seen_over_ours(said: &str) -> Result<Mode, Failure> {
        seen_with(said, Woken::No, Focused::OurApp)
    }

    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"console-bar","h":40}]}}}"#;

    #[test]
    fn the_wallpaper_and_the_bar_are_not_somewhere_you_are() -> Result<(), Failure> {
        let mode = seen(NOTHING_UP, Woken::No)?;

        assert_eq!(mode,Mode::Desktop);

        Ok(())
    }

    #[test]
    fn one_of_our_apps_in_front_is_the_app_even_with_the_home_screen_under_it() -> Result<(), Failure> {
        let mode = seen_over_ours(NOTHING_UP)?;

        assert_eq!(mode, Mode::App);

        let under = r#"{"eDP-1":{"levels":{
            "1":[{"namespace":"console-home","h":1600}],
            "2":[{"namespace":"console-bar","h":40}]}}}"#;

        let over_home = seen_over_ours(under)?;

        assert_eq!(over_home, Mode::App);

        Ok(())
    }

    #[test]
    fn a_panel_over_one_of_our_apps_is_still_a_panel() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","h":1562}]}}}"#;

        let mode = seen_over_ours(said)?;

        assert_eq!(mode, Mode::Tabs);

        Ok(())
    }

    #[test]
    fn a_panel_over_the_desktop_is_tabs() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600}],
            "3":[{"namespace":"settings-panel","h":1562}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Tabs);

        Ok(())
    }

    #[test]
    fn a_panel_no_one_told_this_about_is_still_a_panel() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"whatever-panel","h":900}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Tabs);

        Ok(())
    }

    #[test]
    fn the_keyboard_being_up_is_the_keyboard() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"console-keyboard","h":520}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Keyboard);

        Ok(())
    }

    #[test]
    fn the_keyboard_over_a_panel_is_still_the_keyboard() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"settings-panel","h":1562},
            {"namespace":"console-keyboard","h":520}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Keyboard);

        Ok(())
    }

    #[test]
    fn a_keyboard_with_no_height_is_not_up() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{
            "2":[{"namespace":"console-bar","h":40}],
            "3":[{"namespace":"console-keyboard","h":0}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Desktop);

        Ok(())
    }

    #[test]
    fn a_notification_card_is_not_a_panel() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{
            "2":[{"namespace":"console-bar","h":40}],
            "3":[{"namespace":"notifications","h":140}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Desktop);

        Ok(())
    }

    #[test]
    fn the_daemon_acts_everywhere_except_under_the_keyboard() {
        assert_eq!(Mode::Desktop.input_handling(), Ok(InputHandling::Enabled));
        assert_eq!(Mode::Tabs.input_handling(), Ok(InputHandling::Enabled));
        assert_eq!(Mode::App.input_handling(), Ok(InputHandling::Enabled));
        assert_eq!(Mode::Keyboard.input_handling(), Ok(InputHandling::Disabled));
        assert_eq!(Mode::Prompt.input_handling(), Ok(InputHandling::Disabled));
    }

    #[test]
    fn a_compositor_that_says_nothing_leaves_you_on_the_desktop() -> Result<(), Failure> {
        let mode = seen("{}", Woken::No)?;

        assert_eq!(mode, Mode::Desktop);
        assert_eq!(Mode::default(), Mode::Desktop);

        Ok(())
    }

    #[test]
    fn a_card_asking_which_button_you_pressed_is_the_question() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"settings-panel","h":1562},
            {"namespace":"console-asking","h":300}]}}}"#;
        let asking = seen(said, Woken::No)?;

        assert_eq!(asking, Mode::Prompt);
        assert_eq!(asking.input_handling(), Ok(InputHandling::Disabled));

        Ok(())
    }

    #[test]
    fn the_panel_that_asks_is_a_panel_until_it_asks() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"setup-panel","h":1562}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Tabs);

        Ok(())
    }

    #[test]
    fn a_question_wins_over_a_keyboard_left_up() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"console-keyboard","h":520},
            {"namespace":"console-asking","h":300}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Prompt);

        Ok(())
    }

    #[test]
    fn leaving_the_keyboard_over_a_panel_is_the_daemon_reading_again() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        let tabs = seen(said, Woken::No)?;

        assert_eq!(tabs, Mode::Tabs);
        assert_eq!(tabs.input_handling(), Ok(InputHandling::Enabled));

        Ok(())
    }

    #[test]
    fn the_home_screen_is_the_desktop_with_the_apps_on_it() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "2":[{"namespace":"console-bar","h":40}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::HomeScreen);
        assert_eq!(Mode::HomeScreen.input_handling(), Ok(InputHandling::Enabled));

        Ok(())
    }

    #[test]
    fn the_home_screen_with_a_highlight_up_is_somewhere_else_to_be() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "2":[{"namespace":"console-bar","h":40}]}}}"#;

        let mode = seen(said, Woken::Yes)?;

        assert_eq!(mode,Mode::Standing);
        assert_eq!(Mode::Standing.input_handling(), Ok(InputHandling::Enabled));

        Ok(())
    }

    #[test]
    fn a_panel_over_an_awake_home_screen_is_still_a_panel() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "2":[{"namespace":"console-bar","h":40},{"namespace":"launcher","h":1562}]}}}"#;

        let mode = seen(said, Woken::Yes)?;

        assert_eq!(mode,Mode::Tabs);

        Ok(())
    }

    #[test]
    fn a_panel_over_the_home_screen_is_still_a_panel() -> Result<(), Failure> {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "3":[{"namespace":"launcher","h":1562}]}}}"#;

        let mode = seen(said, Woken::No)?;

        assert_eq!(mode,Mode::Tabs);

        Ok(())
    }

    #[test]
    fn no_home_screen_on_the_screen_is_the_desktop_it_always_was() -> Result<(), Failure> {
        let mode = seen(NOTHING_UP, Woken::No)?;

        assert_eq!(mode,Mode::Desktop);

        Ok(())
    }
}
