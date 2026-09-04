//! The long way round, on purpose.
//!
//! A check asks one question about one feature. A flow walks the desktop the
//! way a person does -- across programs, across crates -- and asks at every
//! step what that person would see, because most of what goes wrong on this
//! machine goes wrong *between* features: a mode that lingered, a chooser
//! that stacked, a button that was kept. `docs/flows.md` is the strategy and
//! the promises; the tests beside this library are the flows themselves, run
//! at the fast stage on every `just test`.
//!
//! What is in the library is only what every flow needs said once: the
//! compositor's answers, as `hyprctl layers -j` would give them, for the
//! places a flow walks through. They are handed to the stage's `showing`,
//! which reads them with the same `Mode::seen` the daemon reads the real
//! compositor with -- so a flow stands where a person would stand, and what
//! is being checked is the reading and not a second opinion about it.

/// The screen, in the compositor's own words.
pub mod screens {
    /// The bare desktop: the wallpaper and the furniture, and no home screen,
    /// which is what a workspace with a window on it comes to.
    pub const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"waybar","h":38}]}}}"#;

    /// The home screen drawn on the wallpaper, with nothing over it. Whether
    /// it is awake is not in here, because the compositor cannot see a
    /// highlight; the stage carries that half itself.
    pub const THE_HOME_SCREEN: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1600}],
        "2":[{"namespace":"waybar","h":38}]}}}"#;

    /// A panel up over the desktop, which is any panel: what matters to the
    /// mode is that something other than furniture is on the screen.
    pub const A_CHOOSER: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"waybar","h":38}],
        "3":[{"namespace":"settings-panel","h":1562}]}}}"#;

    /// The on-screen keyboard up, where the daemon is not the one reading.
    pub const THE_KEYBOARD: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","h":1600}],
        "2":[{"namespace":"waybar","h":38}],
        "3":[{"namespace":"virtual-keyboard","h":520}]}}}"#;
}
