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
//! each other, and a note is wrong the moment somebody restarts without
//! reading it -- which is exactly the shape of the fault where X stopped
//! showing the keyboard until the next reboot.
//!
//! So the mode is read rather than remembered, and it is read from the
//! compositor, which is the only thing that cannot be wrong about what is on
//! its own screen.

use console_door::up;

/// The surfaces that are always there.
///
/// A layer is not somewhere you are just because it is drawn. The wallpaper,
/// the bar and a notification card are furniture: they are on the screen while
/// you are on the desktop, and the buttons still mean what the desktop means.
pub const FURNITURE: [&str; 5] =
    ["awww-daemon", "waybar", "wvkbd", "notifications", "mako"];

/// Which of these the keyboard is.
pub const KEYBOARD: &str = "wvkbd";

/// The card that asks you to press a button so it can learn which one it is.
///
/// Only the card carries this, never the panel that raised it, so walking the
/// rows of a setup screen still works and only the few seconds of the question
/// are inert.
pub const ASKING: &str = "console-asking";

/// What is in front of you.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// Nothing over it. The stick is a pointer, A is a click.
    #[default]
    Desktop,
    /// A panel is up. It has taken the chooser's buttons, and the d-pad moves
    /// a highlight rather than a pointer.
    Tabs,
    /// The on-screen keyboard is up.
    ///
    /// The daemon acts on nothing here. Both it and wvkbd read the same pad,
    /// and with the keyboard up they would both act on the right stick, which
    /// navigates and scrolls at once and flickers.
    ///
    /// That was done by stopping this process with a signal, which is not the
    /// same thing and cost what a heavy hand costs: stopped is not deaf, the
    /// kernel went on queueing on the devices, and the whole backlog arrived
    /// in one instant when the keyboard went away -- every button pressed
    /// while typing, in order, against a desktop that had moved on. That is
    /// how the machine once left for Game Mode on its own.
    Keyboard,
    /// A card is asking you to press a button, so that it can learn which one
    /// you meant.
    ///
    /// The one place on this device where a press is the answer rather than a
    /// request. Everything the front of the machine does would happen while
    /// binding it: Legion left would leave for Game Mode, X would raise the
    /// keyboard over the question, and the paddles would run the launcher and
    /// take a screenshot. So the pad is handed a profile that sends every
    /// button to a key nothing listens for, and this daemon stands down for
    /// the same reason it stands down under the keyboard -- somebody else is
    /// reading.
    Asking,
}

impl Mode {
    /// What the compositor's layers come to.
    ///
    /// The keyboard wins over a panel, because it is raised over one: the
    /// keyboard can be summoned on top of a panel as well as on top of the
    /// desktop, and what the buttons are for while it is up is the keyboard's
    /// business either way.
    ///
    /// The question wins over both, and it is asked in that order because of
    /// what each way round costs. A question cannot ordinarily be asked with
    /// the keyboard up -- the button that raises the keyboard is inert while
    /// it is being bound -- so the two together means a layer that outlived
    /// what put it there. Read as the keyboard, presses reach wvkbd and type
    /// into the question. Read as the question, a keyboard that really is up
    /// goes unanswerable for the few seconds the card is on the screen, and
    /// then comes back. The second is the cheaper of the two to be wrong
    /// about, and it is the one that clears itself.
    pub fn seen(screens: &serde_json::Value) -> Self {
        if up(screens, ASKING) {
            return Mode::Asking;
        }
        if up(screens, KEYBOARD) {
            return Mode::Keyboard;
        }
        match anything_over_the_desktop(screens) {
            true => Mode::Tabs,
            false => Mode::Desktop,
        }
    }

    /// Which InputPlumber profile the pad wants while you are here.
    ///
    /// A function of the mode, which is why nothing has to remember anything.
    /// Putting the keyboard away used to mean restoring a profile written into
    /// `$XDG_RUNTIME_DIR/console-profile-before-keyboard` when it went up, and
    /// that file carried the usual hazard of a remembered thing: a panel closed
    /// while the keyboard was over it had already put the desktop back, so
    /// laying the remembered profile over that left the pad answering to a
    /// panel which was no longer there. `osk-hook` guarded against exactly that
    /// case and only that one.
    ///
    /// There is nothing to restore here. Leaving the keyboard, the mode is
    /// whatever the compositor now says it is, and the profile is this.
    /// Which profile the pad should be wearing.
    ///
    /// The desktop and a chooser are one profile, and that is the whole point
    /// of the router: they used to be two, a button meant one thing in one and
    /// another in the other, and so every menu opening and closing destroyed
    /// the pad and built a new one -- taking the device the on-screen keyboard
    /// reads and the device this daemon reads with it. What a button means
    /// with a chooser up is decided here now, by a daemon that can see the
    /// chooser.
    pub fn profile(self) -> &'static str {
        match self {
            Mode::Desktop | Mode::Tabs => console_pad::router::NAME,
            Mode::Keyboard => "keyboard",
            Mode::Asking => "asking",
        }
    }

    /// Whether this daemon acts on what arrives at all.
    ///
    /// The one thing the mode decides today. What a button means is the same
    /// on the desktop and under a panel -- the panel takes the chooser's
    /// buttons from InputPlumber and the daemon's own jobs are the same jobs
    /// -- and with the keyboard up the daemon is not the one reading.
    ///
    /// Under the question it is not reading either, and for a second reason
    /// besides. A press being bound is not a press meaning anything yet, and a
    /// daemon that acted on it would carry out the job the button used to do
    /// at the moment somebody was telling the machine it should do another.
    pub fn acts(self) -> bool {
        !matches!(self, Mode::Keyboard | Mode::Asking)
    }
}

/// Whether any surface that is not furniture is on the screen.
///
/// Asked this way round on purpose. The panels are a list that grows, and a
/// list of them here would be a second register of what a panel is, out of
/// date the first time somebody writes one. The furniture is short and does
/// not grow: everything else that puts a layer up is something you are in.
fn anything_over_the_desktop(screens: &serde_json::Value) -> bool {
    screens
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| screen.get("levels")?.as_object())
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten()
        .filter(|surface| surface.get("h").and_then(serde_json::Value::as_i64).unwrap_or(1) > 0)
        .filter_map(|surface| surface.get("namespace")?.as_str())
        .any(|named| !FURNITURE.iter().any(|known| named.starts_with(known)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers(said: &str) -> serde_json::Value {
        serde_json::from_str(said).expect("layers")
    }

    /// What the compositor answers with nothing up: the wallpaper and the bar.
    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"waybar","h":38}]}}}"#;

    #[test]
    fn the_wallpaper_and_the_bar_are_not_somewhere_you_are() {
        assert_eq!(Mode::seen(&layers(NOTHING_UP)), Mode::Desktop);
    }

    #[test]
    fn a_panel_over_the_desktop_is_tabs() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600}],
            "3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Tabs);
    }

    /// A panel this file has never heard of is still a panel. The list that
    /// grows is the panels, so the list written down is the furniture.
    #[test]
    fn a_panel_nobody_told_this_about_is_still_a_panel() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"whatever-panel","h":900}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Tabs);
    }

    #[test]
    fn the_keyboard_being_up_is_the_keyboard() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"wvkbd-mobintl","h":520}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Keyboard);
    }

    /// It can be summoned over a panel as well as over the desktop, and what
    /// the buttons are for while it is up is the keyboard's business either
    /// way. Read the other way round this was a panel with the pad answering
    /// to something that was not in front.
    #[test]
    fn the_keyboard_over_a_panel_is_still_the_keyboard() {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"settings-panel","h":1562},
            {"namespace":"wvkbd-mobintl","h":520}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Keyboard);
    }

    /// wvkbd is started --hidden and stays for the session, so being listed is
    /// not being up.
    #[test]
    fn a_keyboard_with_no_height_is_not_up() {
        let said = r#"{"eDP-1":{"levels":{
            "2":[{"namespace":"waybar","h":38}],
            "3":[{"namespace":"wvkbd-mobintl","h":0}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Desktop);
    }

    /// A notification is drawn over the desktop and is not somewhere you are.
    /// Counted as a panel, every card that arrived would have taken the pad.
    #[test]
    fn a_notification_card_is_not_a_panel() {
        let said = r#"{"eDP-1":{"levels":{
            "2":[{"namespace":"waybar","h":38}],
            "3":[{"namespace":"notifications","h":140}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Desktop);
    }

    /// The one thing the mode decides today, and the whole of what replaces
    /// stopping this process with a signal.
    #[test]
    fn the_daemon_acts_everywhere_except_under_the_keyboard() {
        assert!(Mode::Desktop.acts());
        assert!(Mode::Tabs.acts());
        assert!(!Mode::Keyboard.acts());
        assert!(!Mode::Asking.acts());
    }

    /// Nothing said at all is the desktop, which is where this daemon starts
    /// and what it falls back to when the compositor cannot be asked.
    #[test]
    fn a_compositor_that_says_nothing_leaves_you_on_the_desktop() {
        assert_eq!(Mode::seen(&layers("{}")), Mode::Desktop);
        assert_eq!(Mode::default(), Mode::Desktop);
    }

    /// A function of where you are, so nothing has to remember what was
    /// before. The file that did is the third ownerless variable this module
    /// was written to take away.
    #[test]
    fn the_profile_the_pad_wants_is_a_function_of_where_you_are() {
        assert_eq!(Mode::Keyboard.profile(), "keyboard");
        assert_eq!(Mode::Asking.profile(), "asking");
    }

    /// And the desktop and a chooser are the same one, which is what stopped
    /// the pad being destroyed and rebuilt every time a menu opened.
    #[test]
    fn opening_a_menu_does_not_change_the_profile() {
        assert_eq!(Mode::Desktop.profile(), Mode::Tabs.profile());
        assert_eq!(Mode::Desktop.profile(), "router");
    }

    /// The card is raised over a panel, and the panel underneath is still
    /// drawn. Read as a panel, the profile that makes the front of the machine
    /// inert is loaded and then replaced on the next look, and the question is
    /// answered by whatever the button already did.
    #[test]
    fn a_card_asking_which_button_you_pressed_is_the_question() {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"settings-panel","h":1562},
            {"namespace":"console-asking","h":300}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Asking);
        assert_eq!(Mode::seen(&layers(said)).profile(), "asking");
    }

    /// Only the card carries the namespace, never the panel that raises it, so
    /// walking the rows of a setup screen is walking the rows of a panel.
    #[test]
    fn the_panel_that_asks_is_a_panel_until_it_asks() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"setup-panel","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Tabs);
    }

    /// A question and a keyboard at once is a layer that outlived what put it
    /// there, and the question is the cheaper of the two to be wrong about.
    #[test]
    fn a_question_wins_over_a_keyboard_left_up() {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"wvkbd-mobintl","h":520},
            {"namespace":"console-asking","h":300}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Asking);
    }

    /// Coming out of the keyboard over a panel lands on the profile the panel
    /// is driven by and not the keyboard's, because the answer is read rather
    /// than recalled. Recalled, this was the one case `osk-hook` had to guard.
    #[test]
    fn leaving_the_keyboard_over_a_panel_leaves_the_keyboards_profile() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said)), Mode::Tabs);
        assert_eq!(Mode::seen(&layers(said)).profile(), Mode::Desktop.profile());
    }
}
