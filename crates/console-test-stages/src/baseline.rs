//! What every check costs relative to the others, carried in the tree.
//!
//! `lasting` keeps what each check took on the machine it ran on, and that
//! table is the truth about that machine. What it cannot do is exist before
//! the first run: a device nobody has ever checked knows nothing at all, so
//! its first run counts checks rather than time, and a run divided into equal
//! checks tells somebody the two-second one and the three-minute one are the
//! same wait. That is the run somebody is most likely to be watching, because
//! it is the one on a machine they have just put together.
//!
//! So a table travels with the source as well. It is not a second opinion
//! about this machine and it never overrules one: for any check this device
//! has timed itself, the device's own number is used and this file is not
//! consulted. It answers only where the device has nothing to say.
//!
//! # Why the numbers transfer at all
//!
//! Because what varies between two machines is mostly one number. A check that
//! opens a panel and waits for it to be drawn is longer than one that reads a
//! file, on every machine, in roughly the same proportion -- what changes is
//! how fast the whole machine is, not which check is the slow one. So the
//! shape of the table transfers and only its scale does not, and the scale is
//! measurable: over the checks a device has timed, its own numbers against
//! these ones are a ratio, and that ratio carries the rest of this file onto
//! that machine. `lasting::pace_of` is that arithmetic and it needs nothing
//! kept anywhere, because both tables are already to hand.
//!
//! Nothing requires a check to be in here. A test used to: it wanted every
//! device-only check to have a length carried for it, and it could not be
//! satisfied at the moment it fired. A length is measured by a device run, a
//! device run is the end of a deploy, and a deploy is what the test was
//! standing in front of -- so the first check written for the device alone was
//! also the last thing that tree could deploy. What it was guarding against
//! was already handled: `Lengths::middle` gives a check nobody has timed the
//! middle of what is known, which is `lasting`'s own answer and has its own
//! test. The number arrives in the commit after the deploy that measured it,
//! which is the only order it can arrive in.
//!
//! A device with nothing measured at all has no ratio to compute and gets
//! these numbers as they stand, which is the honest guess: not a claim about
//! that machine, a claim that it is a handheld like the one these came off.
//!
//! # Where they come from
//!
//! A device run, taken off the device afterwards -- `just lengths` is the
//! whole of it. They are milliseconds because that is the format `lasting`
//! already reads and writes, and using a second one would mean a second parser
//! to disagree with the first. Nothing here is hand-written and nothing needs
//! correcting by hand: a run that finds them wrong is also a run that can
//! replace them.

use console_core_never::Never;

use crate::lasting::{Lengths, reading};

pub const CARRIED: &str = include_str!("../fixtures/lengths");

pub fn lengths() -> Result<Lengths, Never> {
    reading(CARRIED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_that_travels_reads() {
        let Ok(lengths) = lengths();
        let Ok(known) = lengths.known();

        assert!(known > 0, "the carried table is empty, so a fresh device learns nothing from it");
    }

    #[test]
    fn nothing_carried_is_a_length_of_nothing() {
        let Ok(lengths) = lengths();
        let Ok(names) = lengths.names();
        let empty: Vec<&String> = names
            .iter()
            .filter(|name| {
                let Ok(took) = lengths.of(name);

                took.unwrap_or_default().is_zero()
            })
            .collect();

        assert!(empty.is_empty(), "{empty:?} is carried as taking no time at all");
    }

    #[test]
    fn the_slow_ones_and_the_quick_ones_are_a_long_way_apart() {
        let Ok(lengths) = lengths();
        let Ok(names) = lengths.names();
        let mut every: Vec<u128> = names
            .iter()
            .map(|name| {
                let Ok(took) = lengths.of(name);

                took.unwrap_or_default().as_millis()
            })
            .collect();

        every.sort_unstable();

        let (quickest, slowest) = match (every.first(), every.last()) {
            (Some(quickest), Some(slowest)) => (quickest, slowest),
            (None, _) | (_, None) => panic!("the carried table is empty"),
        };

        assert!(
            slowest > &quickest.saturating_mul(10),
            "the carried table has no shape to it, so counting checks would say as much"
        );
    }
}
