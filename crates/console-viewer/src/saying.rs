//! What the card says about the one thing it is showing.
//!
//! Y on a picture or a film is *what is this*, the way Y everywhere else on
//! this desktop is what else can be done with the thing in front of you. What
//! it answers is the handful of facts somebody actually wants: how big it is,
//! how much room it takes on the disk, and where it came from.
//!
//! Written here rather than at the drawing so the awkward cases are testable:
//! a size of nothing, a file bigger than the numbers a phone uses, a length
//! that has not been read yet.

use console_number::Float;

use crate::fitting::{self, Size};
use crate::kinds::Kind;
use crate::playing::Along;

/// How many bytes something is, as a person says it.
///
/// The units this desktop uses everywhere else, which are the ones a disk is
/// sold in: a thousand and not 1024. One decimal from a megabyte up, because
/// `4.2 MB` is a size and `4 MB` is a rounding somebody will notice against
/// what the files panel says.
pub fn size(bytes: u64) -> String {
    const STEPS: [(u64, &str); 4] =
        [(1_000_000_000_000, "TB"), (1_000_000_000, "GB"), (1_000_000, "MB"), (1_000, "kB")];

    for (over, unit) in STEPS {
        if bytes >= over {
            return format!("{:.1} {unit}", bytes.float() / over.float());
        }
    }

    format!("{bytes} bytes")
}

/// What the card says a picture is, under its name.
///
/// The shape first because it is what somebody is looking for, then the count
/// where it is worth saying. A megapixel figure on a screenshot is noise; on a
/// photograph it is the thing that says which camera took it.
pub fn about(of: Size, bytes: u64) -> String {
    let shape = fitting::said(of);
    let held = size(bytes);

    match fitting::megapixels(of) >= 1.0 {
        true => format!("{shape} · {:.0} megapixels · {held}", fitting::megapixels(of)),
        false => format!("{shape} · {held}"),
    }
}

/// The one line under the name of a thing being shown, whichever kind it is.
///
/// Anything nothing has said yet is left off rather than written as nought.
/// How long a film runs and how big its picture is are both the decoder's to
/// say and it says neither until it has read the file, which is after the card
/// it is on was first drawn; a card that filled those in would open saying
/// `0 x 0` and correct itself a moment later, which reads as a card that was
/// wrong rather than as one that had not been told.
pub fn under(kind: Kind, of: Size, bytes: u64, along: Along) -> String {
    match (kind, fitting::pixels(of) > 0) {
        (Kind::Picture, true) => about(of, bytes),
        (Kind::Picture, false) => size(bytes),
        (Kind::Film, _) => {
            let mut said: Vec<String> = Vec::new();

            // Where it has got to out of how long it is, which is one thing
            // and not two: a film that has been started has a position whether
            // or not the decoder has said how long it runs, and
            // `playing::said` is what decides which of those to write.
            if along.at > 0 || along.whole > 0 {
                said.push(crate::playing::said(along));
            }

            if fitting::pixels(of) > 0 {
                said.push(fitting::said(of));
            }

            said.push(size(bytes));
            said.join(" · ")
        }
    }
}

/// What a card says where the thing will not open at all.
///
/// One sentence naming what was tried, because the alternative on a device
/// with no terminal is a card that is simply empty and a person who cannot
/// tell a broken file from a broken panel.
pub fn wont_open(name: &str) -> String {
    format!("{name} will not open. It may be damaged, or of a kind this cannot show.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_said_in_the_units_a_disk_is_sold_in() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(999), "999 bytes");
        assert_eq!(size(1_000), "1.0 kB");
        assert_eq!(size(4_200_000), "4.2 MB");
        assert_eq!(size(2_500_000_000), "2.5 GB");
        assert_eq!(size(3_000_000_000_000), "3.0 TB");
    }

    #[test]
    fn a_photograph_says_its_shape_its_count_and_its_room() {
        let said = about(Size::new(4000, 3000), 4_200_000);
        assert!(said.contains("4000 x 3000"), "{said}");
        assert!(said.contains("12 megapixels"), "{said}");
        assert!(said.contains("4.2 MB"), "{said}");
    }

    /// A megapixel figure on a small picture is noise.
    #[test]
    fn something_smaller_than_a_megapixel_does_not_say_so() {
        let said = about(Size::new(32, 32), 900);
        assert!(!said.contains("megapixel"), "{said}");
        assert!(said.contains("32 x 32"), "{said}");
    }

    #[test]
    fn a_film_says_where_it_has_got_to_first() {
        let said = under(Kind::Film, Size::new(1920, 1080), 700_000_000, Along::new(0, 7325));
        assert!(said.starts_with("0:00 of 2:02:05"), "{said}");
        assert!(said.contains("1920 x 1080"), "{said}");
    }

    /// The row is read while the film runs, so it is the position that moves
    /// and the length that stays.
    #[test]
    fn a_film_part_way_through_says_both_ends_of_it() {
        let said = under(Kind::Film, Size::new(0, 0), 700_000_000, Along::new(65, 7325));
        assert!(said.starts_with("1:05 of 2:02:05"), "{said}");
    }

    /// Until the decoder has said, the length is left out rather than said as
    /// nothing.
    #[test]
    fn a_film_of_unread_length_says_what_it_knows() {
        let said = under(Kind::Film, Size::new(1920, 1080), 700_000_000, Along::new(12, 0));
        assert!(!said.contains(" of "), "{said}");
        assert!(said.starts_with("0:12"), "{said}");
        assert!(said.contains("1920 x 1080"), "{said}");
    }

    /// A film is asked about the decoder drawing it, and for the first drawing
    /// the decoder has not read the file: neither number is known, and a card
    /// that wrote them in would open saying `0:00 · 0 x 0` and correct itself.
    #[test]
    fn a_film_nothing_has_read_yet_says_only_how_much_room_it_takes() {
        let said = under(Kind::Film, Size::new(0, 0), 7_800, Along::default());
        assert_eq!(said, "7.8 kB");
    }

    /// The same rule for a photograph whose header would not be read. There is
    /// no shape to say, and a shape of nought is not one.
    #[test]
    fn a_picture_with_no_shape_read_says_only_how_much_room_it_takes() {
        assert_eq!(under(Kind::Picture, Size::new(0, 0), 2_500, Along::default()), "2.5 kB");
    }

    #[test]
    fn a_picture_and_a_film_are_said_differently() {
        let of = Size::new(1920, 1080);
        assert_ne!(
            under(Kind::Picture, of, 100, Along::default()),
            under(Kind::Film, of, 100, Along::new(0, 7325))
        );
    }

    /// A person with no terminal has to be able to tell a broken file from a
    /// broken panel.
    #[test]
    fn something_that_will_not_open_says_which_thing_it_was() {
        let said = wont_open("beach.jpg");
        assert!(said.starts_with("beach.jpg"), "{said}");
        assert!(said.ends_with('.'), "{said}");
    }
}
