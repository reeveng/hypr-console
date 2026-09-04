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

use console_door::{Up, up};

/// The surfaces that are always there.
///
/// A layer is not somewhere you are just because it is drawn. The wallpaper,
/// the bar and a notification card are furniture: they are on the screen while
/// you are on the desktop, and the buttons still mean what the desktop means.
///
/// `updating` is the thin strip under the bar. It is a second waybar
/// with a name of its own, and waybar puts that name on the layer, so it
/// arrives here as `updating` rather than as something starting `waybar`. It
/// is furniture for the reason the bar is: it is part of the bar, it covers
/// nothing, and it is on the screen for every minute the machine is running
/// rather than for the minute an apply takes.
///
/// Left out of this list it read as a panel that was up for ever, and that is
/// a state nothing refuses to work in: the profile is the same one either way
/// and the daemon still acts, so nothing failed out loud. What it cost was
/// every job written for the desktop and not for a chooser, which on this
/// machine is the shoulders -- they moved between tabs there were none of
/// instead of between workspaces, from the moment the strip was added.
pub const FURNITURE: [&str; 7] = [
    "awww-daemon",
    "waybar",
    "updating",
    "virtual-keyboard",
    "notifications",
    "mako",
    HOME,
];

/// The apps drawn over the wallpaper, which is what a bare desktop looks like
/// on this machine now.
///
/// Furniture, and for the same reason the strip under the bar is: it is up for
/// every minute the machine is running, so read as a panel it would be a panel
/// that never closed, and every job written for the desktop -- the shoulders,
/// Game Mode, the browser -- would be gone from the moment it started. It is
/// what is *behind* everything else rather than something you are in.
///
/// It does change what two buttons mean, and that is [`Mode::Home`] below, out
/// of the same reading rather than out of a second one.
pub const HOME: &str = "console-home";

/// Which of these the keyboard is.
///
/// What the running program publishes, not what this repository calls it.
/// `up` matches on the front of a namespace, and the keyboard sets this in
/// `keyboard/src/surface.rs`, which the device compiles from this workspace.
/// `keyboard/tests/the_namespace` holds this against the four places outside
/// that file which have to agree -- the crate's `[[bin]]`, the manifest's
/// `[build]`, the unit's `ExecStart`, and the toggle's `pkill` pattern --
/// because moving one without the others is exactly how this broke: a daemon
/// that never sees the keyboard never stands down, and nothing fails out loud.
pub const KEYBOARD: &str = "virtual-keyboard";

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
    /// The desktop, with the home screen drawn on it and nothing over that.
    ///
    /// Everything the desktop does, it still does: this is the desktop, and
    /// the apps are what is on it. Two buttons are the home screen's, because
    /// there is now something under the d-pad to open -- A opens it, Y says
    /// what else can be done with it.
    ///
    /// Read off the compositor like the rest, and not out of whether the
    /// program is running: the home screen puts its surface away while a
    /// window is over it, and a process that is alive behind a full-screen
    /// game is not a home screen anybody is looking at.
    Home,
    /// The home screen, awake: a highlight is up and it is standing on a
    /// square.
    ///
    /// The difference between this and [`Mode::Home`] is who owns A. A
    /// highlight is a claim on a button -- while one is drawn, A is the thing
    /// under it -- and a home screen that drew one from the moment it appeared
    /// claimed A for every minute the machine was on, leaving the touchpad a
    /// pointer with nothing to press. So it starts asleep, A is the pointer's
    /// button, and the d-pad is what wakes it.
    ///
    /// Which the compositor cannot answer: the surface is on the screen either
    /// way, and what changed is what is drawn on it. So this one is read from
    /// the note the home screen leaves in `console_door::awake`, and it is the
    /// only part of the mode that is not read off the screen itself.
    Standing,
    /// The on-screen keyboard is up.
    ///
    /// The daemon acts on nothing here. Both it and the keyboard read the same pad,
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
    /// what put it there. Read as the keyboard, presses reach the keyboard and type
    /// into the question. Read as the question, a keyboard that really is up
    /// goes unanswerable for the few seconds the card is on the screen, and
    /// then comes back. The second is the cheaper of the two to be wrong
    /// about, and it is the one that clears itself.
    pub fn seen(screens: &serde_json::Value, awake: Awake) -> Self {
        if up(screens, ASKING) == Up::OnScreen {
            return Mode::Asking;
        }

        if up(screens, KEYBOARD) == Up::OnScreen {
            return Mode::Keyboard;
        }

        match anything_over_the_desktop(screens) {
            Over::Something => Mode::Tabs,
            Over::Nothing => match (up(screens, HOME), awake) {
                (Up::OnScreen, Awake::Yes) => Mode::Standing,
                (Up::OnScreen, Awake::No) => Mode::Home,
                (Up::NotThere, _) => Mode::Desktop,
            },
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
            Mode::Desktop | Mode::Tabs | Mode::Home | Mode::Standing => console_pad::router::NAME,
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
    pub fn acts(self) -> Acts {
        match matches!(self, Mode::Keyboard | Mode::Asking) {
            true => Acts::NotReading,
            false => Acts::OnPresses,
        }
    }
}

/// Whether the home screen has a highlight up.
///
/// The home screen's own word, from the crate both sides of it read: it is the
/// one part of where you are that the compositor cannot answer, so it is not
/// read off the screen and it is not this crate's to define either.
pub use console_door::Awake;

/// Whether the daemon acts on presses just now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acts {
    /// It does, and a bound button carries out its job.
    OnPresses,
    /// Something else is reading the pad -- the keyboard, or a question on the
    /// screen -- and a press acted on here would be a press acted on twice.
    NotReading,
}

/// Whether anything that is not furniture is on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// Something you are in is up.
    Something,
    /// Only the wallpaper and the furniture, which is the desktop itself.
    Nothing,
}

/// Whether any surface that is not furniture is on the screen.
///
/// Asked this way round on purpose. The panels are a list that grows, and a
/// list of them here would be a second register of what a panel is, out of
/// date the first time somebody writes one. The furniture is short and does
/// not grow: everything else that puts a layer up is something you are in.
fn anything_over_the_desktop(screens: &serde_json::Value) -> Over {
    let over = screens
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| screen.get("levels")?.as_object())
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten()
        .filter(|surface| surface.get("h").and_then(serde_json::Value::as_i64).unwrap_or(1) > 0)
        .filter_map(|surface| surface.get("namespace")?.as_str())
        .any(|named| !FURNITURE.iter().any(|known| named.starts_with(known)));

    match over {
        true => Over::Something,
        false => Over::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers(said: &str) -> serde_json::Value {
        serde_json::from_str(said).expect("layers")
    }

    /// What the compositor answers with nothing up: the wallpaper, the bar,
    /// and the strip under the bar. All three are there on an idle desktop, so
    /// all three belong in the fixture that says nothing is up. The strip was
    /// missing from it, and that is why this file went on passing while the
    /// shoulders on the device did the wrong job.
    const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"waybar","h":38},{"namespace":"updating","h":2}]}}}"#;

    #[test]
    fn the_wallpaper_and_the_bar_are_not_somewhere_you_are() {
        assert_eq!(Mode::seen(&layers(NOTHING_UP), Awake::No), Mode::Desktop);
    }

    /// The strip is a row of bar and it is up always, so reading it as a
    /// panel is reading the desktop as a chooser for the whole of a session.
    #[test]
    fn the_strip_under_the_bar_is_the_bar() {
        let said = r#"{"eDP-1":{"levels":{"2":[{"namespace":"updating","h":2}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Desktop);
    }

    #[test]
    fn a_panel_over_the_desktop_is_tabs() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600}],
            "3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Tabs);
    }

    /// A panel this file has never heard of is still a panel. The list that
    /// grows is the panels, so the list written down is the furniture.
    #[test]
    fn a_panel_nobody_told_this_about_is_still_a_panel() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"whatever-panel","h":900}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Tabs);
    }

    #[test]
    fn the_keyboard_being_up_is_the_keyboard() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"virtual-keyboard","h":520}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Keyboard);
    }

    /// It can be summoned over a panel as well as over the desktop, and what
    /// the buttons are for while it is up is the keyboard's business either
    /// way. Read the other way round this was a panel with the pad answering
    /// to something that was not in front.
    #[test]
    fn the_keyboard_over_a_panel_is_still_the_keyboard() {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"settings-panel","h":1562},
            {"namespace":"virtual-keyboard","h":520}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Keyboard);
    }

    /// The keyboard is started --hidden and stays for the session, so being
    /// listed is not being up.
    #[test]
    fn a_keyboard_with_no_height_is_not_up() {
        let said = r#"{"eDP-1":{"levels":{
            "2":[{"namespace":"waybar","h":38}],
            "3":[{"namespace":"virtual-keyboard","h":0}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Desktop);
    }

    /// A notification is drawn over the desktop and is not somewhere you are.
    /// Counted as a panel, every card that arrived would have taken the pad.
    #[test]
    fn a_notification_card_is_not_a_panel() {
        let said = r#"{"eDP-1":{"levels":{
            "2":[{"namespace":"waybar","h":38}],
            "3":[{"namespace":"notifications","h":140}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Desktop);
    }

    /// The one thing the mode decides today, and the whole of what replaces
    /// stopping this process with a signal.
    #[test]
    fn the_daemon_acts_everywhere_except_under_the_keyboard() {
        assert_eq!(Mode::Desktop.acts(), Acts::OnPresses);
        assert_eq!(Mode::Tabs.acts(), Acts::OnPresses);
        assert_eq!(Mode::Keyboard.acts(), Acts::NotReading);
        assert_eq!(Mode::Asking.acts(), Acts::NotReading);
    }

    /// Nothing said at all is the desktop, which is where this daemon starts
    /// and what it falls back to when the compositor cannot be asked.
    #[test]
    fn a_compositor_that_says_nothing_leaves_you_on_the_desktop() {
        assert_eq!(Mode::seen(&layers("{}"), Awake::No), Mode::Desktop);
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
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Asking);
        assert_eq!(Mode::seen(&layers(said), Awake::No).profile(), "asking");
    }

    /// Only the card carries the namespace, never the panel that raises it, so
    /// walking the rows of a setup screen is walking the rows of a panel.
    #[test]
    fn the_panel_that_asks_is_a_panel_until_it_asks() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"setup-panel","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Tabs);
    }

    /// A question and a keyboard at once is a layer that outlived what put it
    /// there, and the question is the cheaper of the two to be wrong about.
    #[test]
    fn a_question_wins_over_a_keyboard_left_up() {
        let said = r#"{"eDP-1":{"levels":{"3":[
            {"namespace":"virtual-keyboard","h":520},
            {"namespace":"console-asking","h":300}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Asking);
    }

    /// Coming out of the keyboard over a panel lands on the profile the panel
    /// is driven by and not the keyboard's, because the answer is read rather
    /// than recalled. Recalled, this was the one case `osk-hook` had to guard.
    #[test]
    fn leaving_the_keyboard_over_a_panel_leaves_the_keyboards_profile() {
        let said = r#"{"eDP-1":{"levels":{"3":[{"namespace":"settings-panel","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Tabs);
        assert_eq!(Mode::seen(&layers(said), Awake::No).profile(), Mode::Desktop.profile());
    }

    /// The apps over the wallpaper. Nothing is in front of you -- this is what
    /// a bare desktop looks like on this machine -- and two buttons are the
    /// home screen's while it is drawn.
    #[test]
    fn the_home_screen_is_the_desktop_with_the_apps_on_it() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "2":[{"namespace":"waybar","h":38}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Home);
        assert_eq!(Mode::Home.acts(), Acts::OnPresses);
        assert_eq!(Mode::Home.profile(), console_pad::router::NAME);
    }

    /// The same screen, once the d-pad has been used on it. Nothing about the
    /// compositor's answer has changed -- the surface was already drawn -- and
    /// A has changed hands, which is the whole reason this is a mode of its
    /// own rather than something the home screen keeps to itself.
    #[test]
    fn the_home_screen_with_a_highlight_up_is_somewhere_else_to_be() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "2":[{"namespace":"waybar","h":38}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::Yes), Mode::Standing);
        assert_eq!(Mode::Standing.acts(), Acts::OnPresses);
        assert_eq!(Mode::Standing.profile(), console_pad::router::NAME);
    }

    /// Awake with a panel over it is the panel. Whatever the home screen was
    /// standing on, it is behind something now, and the note it left saying so
    /// outlives the moment -- a note is only ever the second question.
    #[test]
    fn a_panel_over_an_awake_home_screen_is_still_a_panel() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "2":[{"namespace":"waybar","h":38},{"namespace":"launcher","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::Yes), Mode::Tabs);
    }

    /// And a panel opened over it is a panel, not a home screen. The home
    /// screen is furniture: what it is behind decides, and it is behind
    /// everything.
    #[test]
    fn a_panel_over_the_home_screen_is_still_a_panel() {
        let said = r#"{"eDP-1":{"levels":{
            "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
            "3":[{"namespace":"launcher","h":1562}]}}}"#;
        assert_eq!(Mode::seen(&layers(said), Awake::No), Mode::Tabs);
    }

    /// The home screen puts its surface away while a window is over it, so a
    /// machine playing a game full screen is a desktop and not a home screen.
    #[test]
    fn no_home_screen_on_the_screen_is_the_desktop_it_always_was() {
        assert_eq!(Mode::seen(&layers(NOTHING_UP), Awake::No), Mode::Desktop);
    }
}
