//! What the battery does on the way down, and where each of those starts.
//!
//! Three things happen as a battery empties, and all three are one question
//! asked at three depths: say it, say it louder, and stop the machine before
//! the machine is stopped for it. Where each of them starts is a person's own
//! answer -- twenty-five is early on a quiet evening and late in a game -- so
//! the three numbers are settings, kept where the other settings nobody else
//! owns are kept, and this is the shape of them.
//!
//! It is the one reading on this device that moves without anybody pressing
//! anything, which is what makes it worth saying and also what makes it harder
//! than the screen and the volume. Those are raised by the press that caused
//! them and have nowhere else to be. This has no press, so something has to be
//! watching, and whatever watches has to decide when a crossing happened
//! rather than when a number was read.
//!
//! Nothing here reads a clock or raises anything. What is decided and what is
//! done are kept apart as everywhere else here: this says what a reading has
//! come to, `console-battery` is what does it, and `bar-say battery` is the
//! one thing on the machine reading the battery at all.

/// One of the three, deepest last.
///
/// Ordered, because that is the whole of how a reading is judged: what a
/// reading asks for is the deepest step it has reached, and whether to say
/// anything is whether that is deeper than the last thing said.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Step {
    Low,
    Lower,
    Protect,
}

/// The three, shallowest first, which is the order they are drawn in.
pub const EVERY: [Step; 3] = [Step::Low, Step::Lower, Step::Protect];

impl Step {
    /// The word it is asked for by, on a command line and in the file.
    pub fn word(self) -> &'static str {
        match self {
            Step::Low => "low",
            Step::Lower => "lower",
            Step::Protect => "protect",
        }
    }

    pub fn named(word: &str) -> Option<Self> {
        EVERY.into_iter().find(|step| step.word() == word)
    }

    /// The key it is written under.
    pub fn key(self) -> &'static str {
        match self {
            Step::Low => "battery-low",
            Step::Lower => "battery-lower",
            Step::Protect => "battery-protect",
        }
    }

    /// What the row on the Battery tab says.
    ///
    /// Each row is what the machine will do rather than what the number is,
    /// because the number is already beside it. A row called "Low" and a row
    /// called "Lower" would be two words a person has to hold apart; a row
    /// that says it stops the machine is a row nobody has to be told about.
    ///
    /// The last of them does not promise to save anything, and that is
    /// deliberate: whether the session can be put on disk is a fact about the
    /// machine rather than about this setting -- see
    /// `console_settings::stopping` -- and a row that promised it on a device
    /// that cannot would be a lie written where somebody goes to trust it.
    pub fn says(self) -> &'static str {
        match self {
            Step::Low => "Say it is getting low",
            Step::Lower => "Say it is getting really low",
            Step::Protect => "Stop before the battery does",
        }
    }

    /// Where this desktop puts it, until somebody says otherwise.
    ///
    /// The first two are where the icon on the bar already changes colour --
    /// `warning` from twenty-five and `critical` from ten, in
    /// `console_bar::reading` -- so the card and the icon say the same thing
    /// at the same moment rather than disagreeing by a few per cent. The third
    /// is below both, because a machine stopping itself is not a warning and
    /// should never arrive while a warning is still the news.
    pub fn at(self) -> i32 {
        match self {
            Step::Low => 25,
            Step::Lower => 10,
            Step::Protect => 5,
        }
    }

    /// The step as one number, which is how it is kept between readings.
    ///
    /// Nought is nothing said. Whatever is watching the battery is restarted
    /// by the thing that runs it -- waybar starts a module again the moment it
    /// exits -- so what has already been said has to outlive the process that
    /// said it, or a machine sitting at nineteen per cent says so every time
    /// its bar is rebuilt.
    pub fn number(self) -> u32 {
        match self {
            Step::Low => 1,
            Step::Lower => 2,
            Step::Protect => 3,
        }
    }

    /// The same, back again. Nought, and anything nothing is called, is
    /// nothing said.
    pub fn of_number(number: u32) -> Option<Self> {
        EVERY.into_iter().find(|step| step.number() == number)
    }
}

/// Where the three of them stand on this machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Levels {
    pub low: i32,
    pub lower: i32,
    pub protect: i32,
}

impl Default for Levels {
    fn default() -> Self {
        Levels { low: Step::Low.at(), lower: Step::Lower.at(), protect: Step::Protect.at() }
    }
}

/// What a step set to this means: nothing, ever.
///
/// Nought rather than a word, because the row is a level and a level walks to
/// its end. What the row says there is *never*, so nobody has to work out that
/// a battery warning at nought per cent is a warning that cannot arrive.
pub const NEVER: i32 = 0;

impl Levels {
    pub fn at(self, step: Step) -> i32 {
        match step {
            Step::Low => self.low,
            Step::Lower => self.lower,
            Step::Protect => self.protect,
        }
    }

    /// The same, with one of them moved, and the order kept.
    ///
    /// A step can only move in the room between the two either side of it, and
    /// stops at the end of it the way every level here stops at nought and a
    /// hundred. The alternative was to let one pass another and put them back
    /// in order afterwards, which is a row that answers a press by moving a
    /// different row.
    pub fn with(self, step: Step, level: i32) -> Self {
        let (floor, ceiling) = match step {
            Step::Low => (self.lower, 100),
            Step::Lower => (self.protect, self.low),
            Step::Protect => (NEVER, self.lower),
        };
        let level = level.clamp(floor, ceiling);
        let mut levels = self;

        match step {
            Step::Low => levels.low = level,
            Step::Lower => levels.lower = level,
            Step::Protect => levels.protect = level,
        }

        levels
    }

    /// In order and inside the ends, whatever was written.
    ///
    /// The file is written by hand as readily as by the panel, and three
    /// numbers out of order is not something to argue with: what a person
    /// plainly meant is that the deeper of two is the deeper of two.
    pub fn sane(self) -> Self {
        let low = self.low.clamp(NEVER, 100);
        let lower = self.lower.clamp(NEVER, low);
        let protect = self.protect.clamp(NEVER, lower);
        Levels { low, lower, protect }
    }

    /// What the settings file says, with this desktop's own answers under it.
    pub fn read(said: &str) -> Self {
        let settings = crate::read(said);
        let one = |step: Step| {
            settings
                .iter()
                .find(|(key, _)| key == step.key())
                .and_then(|(_key, value)| match value.parse() {
                    Ok(level) => Some(level),
                    // The line is there and is not a number, so the built-in
                    // level below stands. Said out loud: a level somebody set
                    // and mistyped used to read exactly like one never set.
                    Err(fault) => {
                        eprintln!("console-defaults: {}: {value:?}: {fault}", step.key());

                        None
                    }
                })
                .unwrap_or_else(|| step.at())
        };
        Levels { low: one(Step::Low), lower: one(Step::Lower), protect: one(Step::Protect) }
            .sane()
    }

    /// The same, off the file where it lives.
    pub fn here() -> Self {
        let at = crate::where_();

        // No file is ordinary: nobody has set a level and the built-in ones
        // stand. A file that is there and will not be read gives the same
        // levels and is not the same fact, so it is said rather than folded in.
        let said = match std::fs::read_to_string(&at) {
            Ok(said) => said,
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(fault) => {
                eprintln!("console-defaults: {}: {fault}", at.display());

                String::new()
            }
        };

        Levels::read(&said)
    }

    /// Write one of them down, leaving the file's other lines alone.
    pub fn set(self, step: Step, level: i32) -> Self {
        let moved = self.with(step, level);
        crate::set(step.key(), &moved.at(step).to_string());
        moved
    }

    /// The deepest step this charge has reached, if it has reached one.
    ///
    /// A step set to `NEVER` is not reached by anything. Without that, a
    /// person who turned the stopping off would find a battery at nought per
    /// cent had reached it.
    pub fn reached(self, charge: i32) -> Option<Step> {
        EVERY.into_iter().rev().find(|step| {
            let level = self.at(*step);
            level != NEVER && charge <= level
        })
    }
}

// ------------------------------------------------------------------ the reading

/// The charge and whether it is on the mains, out of the kernel.
///
/// Every battery on the machine is looked at and the first that answers is the
/// one, because a handheld has one. The name is matched by its front, not by a
/// number: this device calls its battery `BATT`, and every laptop anybody has
/// written one of these against calls it `BAT0`.
pub fn charge() -> String {
        let Ok(supplies) = std::fs::read_dir("/sys/class/power_supply") else {
            return String::new();
        };

        supplies
            .flatten()
            .map(|supply| supply.path())
            .filter(|at| at.file_name().is_some_and(|name| name.to_string_lossy().starts_with("BAT")))
            .filter_map(|at| {
                // A supply that will not answer both questions is not a battery
                // this can report on. Reading /sys is allowed to fail here --
                // a supply can be unplugged between the listing and the read --
                // and it is the one place in this crate where that is ordinary.
                let (Ok(capacity), Ok(status)) = (
                    std::fs::read_to_string(at.join("capacity")),
                    std::fs::read_to_string(at.join("status")),
                ) else {
                    return None;
                };

                Some(format!("{} {}", capacity.trim(), status.trim()))
            })
            .next()
            .unwrap_or_default()
    }

/// How full it is, and whether it is filling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Charge {
    /// Nothing on a machine with no battery, or one whose battery would not
    /// answer. Told apart from nought per cent, which is a machine about to
    /// stop rather than a machine that has no battery to stop for.
    pub percent: Option<i32>,
    pub filling: Filling,
}

/// Whether the battery is going up or down.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filling {
    /// It is on the mains, so nothing here has anything to warn about.
    Yes,
    /// It is running down, which is the only case any of this is about.
    #[default]
    No,
}

impl Charge {
    pub fn of(said: &str) -> Self {
        let mut words = said.split_whitespace();
        let percent = words.next().and_then(|word| match word.parse() {
            Ok(percent) => Some(percent),
            // The machine answered with something that is not a number, which
            // is not the same as a machine with no battery in it.
            Err(fault) => {
                eprintln!("console-defaults: the battery said {word:?}: {fault}");

                None
            }
        });
        let filling = match words.next().is_some_and(|word| word == "Charging" || word == "Full") {
            true => Filling::Yes,
            false => Filling::No,
        };
        Charge { percent, filling }
    }
}

// ----------------------------------------------------------------- the crossing

/// How far a reading has to come back up before a step is armed again.
///
/// A charge reading wobbles a point either way, and a step armed the moment
/// the number rises off it is a card that arrives again on the next wobble.
/// Three points is more than the wobble and less than a step of the level.
pub const MARGIN: i32 = 3;

/// What a reading asks for, and what to remember of it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Said {
    /// The step to act on now, where this reading has reached a new one.
    pub act: Option<Step>,
    /// The deepest thing said so far, to be handed back with the next reading.
    pub told: Option<Step>,
}

/// What this reading comes to, given what has already been said.
///
/// Three rules, and each of them is a thing a battery actually does.
///
/// On the mains nothing is said and everything is armed again. A machine that
/// stopped itself at five per cent while it was plugged in and filling would
/// be a machine doing the one thing it is here to prevent.
///
/// A step is said once and not again, so a machine sitting at nineteen per
/// cent for an hour says so once. What is remembered is the deepest step said,
/// not the charge, because the question is which cards have been shown rather
/// than where the battery was.
///
/// A reading is taken every thirty seconds and a battery can fall through two
/// steps between two of them. That owes one card, the deeper: somebody who is
/// about to be told the machine is stopping does not also need to be told it
/// is getting low.
pub fn asked(levels: Levels, charge: i32, filling: Filling, told: Option<Step>) -> Said {
    if filling == Filling::Yes {
        return Said { act: None, told: None };
    }

    // What has been said and is still in force. A step whose level the charge
    // has climbed clear of is a step that can happen again.
    let held = told.filter(|step| charge < levels.at(*step) + MARGIN);

    match levels.reached(charge) {
        Some(now) if held.is_none_or(|before| now > before) => {
            Said { act: Some(now), told: Some(now) }
        }
        _ => Said { act: None, told: held },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole of what a step is: a card that arrives once when the battery
    /// passes it, and not again while it stays under.
    #[test]
    fn a_step_is_said_once_and_not_again_while_it_is_held() {
        let levels = Levels::default();
        let first = asked(levels, 19, Filling::No, None);
        assert_eq!(first, Said { act: Some(Step::Low), told: Some(Step::Low) });
        let again = asked(levels, 19, Filling::No, first.told);
        assert_eq!(again, Said { act: None, told: Some(Step::Low) });
        let lower = asked(levels, 18, Filling::No, again.told);
        assert_eq!(lower.act, None, "still the same step");
    }

    /// A reading every thirty seconds can fall through two steps between two
    /// of them, and the one that is owed is the deeper. Being told the machine
    /// is stopping and then that it is getting low is a machine reading its
    /// own list out backwards.
    #[test]
    fn a_reading_that_falls_through_two_steps_owes_the_deeper_one() {
        let said = asked(Levels::default(), 4, Filling::No, Some(Step::Low));
        assert_eq!(said, Said { act: Some(Step::Protect), told: Some(Step::Protect) });
    }

    /// Nothing is said while it is filling, and everything is armed again. A
    /// machine that stopped itself at five per cent on the mains would be
    /// doing the one thing this is here to prevent.
    #[test]
    fn nothing_happens_to_a_battery_that_is_filling() {
        let said = asked(Levels::default(), 3, Filling::Yes, Some(Step::Lower));
        assert_eq!(said, Said { act: None, told: None });
    }

    /// And once it has climbed clear, the step can happen again -- which is
    /// the case a machine unplugged twice in an evening is.
    #[test]
    fn a_charge_that_climbs_clear_of_a_step_can_meet_it_again() {
        let levels = Levels::default();
        assert_eq!(asked(levels, 40, Filling::No, Some(Step::Low)).told, None);
        assert_eq!(asked(levels, 24, Filling::No, None).act, Some(Step::Low));
    }

    /// A point either way is a battery reading, not a crossing. Without the
    /// margin, a charge hovering on a step raises a card every time it wobbles.
    #[test]
    fn a_reading_wobbling_on_a_step_does_not_say_it_twice() {
        let levels = Levels::default();
        let said = asked(levels, 25, Filling::No, None);
        assert_eq!(said.act, Some(Step::Low));
        assert_eq!(asked(levels, 26, Filling::No, said.told).told, Some(Step::Low));
        assert_eq!(asked(levels, 25, Filling::No, said.told).act, None);
    }

    /// A step turned off is not reached by anything, including nought per
    /// cent, which is where an empty machine spends its last minute.
    #[test]
    fn a_step_set_to_never_is_never_reached() {
        let levels = Levels { low: 25, lower: 10, protect: NEVER };
        assert_eq!(levels.reached(0), Some(Step::Lower));
        assert_eq!(Levels { low: NEVER, lower: NEVER, protect: NEVER }.reached(0), None);
    }

    /// A row moves in the room between its neighbours and stops there, rather
    /// than passing one and putting them back in order behind its back.
    #[test]
    fn a_step_stops_where_the_one_under_it_is() {
        let levels = Levels::default();
        assert_eq!(levels.with(Step::Lower, 30).lower, levels.low);
        assert_eq!(levels.with(Step::Lower, 0).lower, levels.protect);
        assert_eq!(levels.with(Step::Low, 100).low, 100);
    }

    /// Three numbers out of order in a file written by hand are not worth an
    /// argument: what was plainly meant is that the deeper of two is deeper.
    #[test]
    fn a_file_with_the_three_in_the_wrong_order_is_read_in_order() {
        let said = "battery-low=5\nbattery-lower=40\nbattery-protect=90\n";
        assert_eq!(Levels::read(said), Levels { low: 5, lower: 5, protect: 5 });
    }

    #[test]
    fn a_file_that_says_nothing_is_this_desktops_own_answers() {
        assert_eq!(Levels::read(""), Levels::default());
        assert_eq!(Levels::read("search=startpage\n"), Levels::default());
    }

    /// What the kernel writes, as the two things anybody wants off it.
    #[test]
    fn a_charge_is_a_number_and_whether_it_is_filling() {
        assert_eq!(Charge::of("72 Discharging"), Charge { percent: Some(72), filling: Filling::No });
        assert_eq!(Charge::of("100 Full"), Charge { percent: Some(100), filling: Filling::Yes });
        assert_eq!(Charge::of(""), Charge { percent: None, filling: Filling::No });
    }

    /// A machine with no battery is not a machine at nought per cent, and the
    /// difference is whether anything should happen about it.
    #[test]
    fn a_machine_with_no_battery_is_not_a_machine_about_to_stop() {
        assert_eq!(Charge::of("").percent, None);
        assert_eq!(Charge::of("0 Discharging").percent, Some(0));
    }

    /// The step outlives the process that said it, because whatever is
    /// watching the battery is restarted by the thing that runs it.
    #[test]
    fn a_step_survives_as_one_number() {
        for step in EVERY {
            assert_eq!(Step::of_number(step.number()), Some(step));
        }
        assert_eq!(Step::of_number(0), None);
    }

    #[test]
    fn a_step_is_asked_for_by_its_own_word() {
        assert_eq!(Step::named("protect"), Some(Step::Protect));
        assert_eq!(Step::named("nothing"), None);
    }
}
