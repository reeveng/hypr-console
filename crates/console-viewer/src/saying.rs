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

use console_never::Never;
use console_number_conversion::Float;

use crate::fitting::{self, Size};
use crate::kinds::Kind;
use crate::playing::Along;

pub fn size(bytes: u64) -> Result<String, Never> {
    const STEPS: [(u64, &str); 4] =
        [(1_000_000_000_000, "TB"), (1_000_000_000, "GB"), (1_000_000, "MB"), (1_000, "kB")];

    for (over, unit) in STEPS {
        let Ok(held) = bytes.float();
        let Ok(step) = over.float();

        match bytes >= over {
            true => return Ok(format!("{:.1} {unit}", held / step)),
            false => {},
        }
    }

    Ok(format!("{bytes} bytes"))
}

pub fn about(of: Size, bytes: u64) -> Result<String, Never> {
    let Ok(shape) = fitting::said(of);
    let Ok(held) = size(bytes);
    let Ok(megapixels) = fitting::megapixels(of);

    Ok(match megapixels >= 1.0 {
        true => format!("{shape} · {megapixels:.0} megapixels · {held}"),
        false => format!("{shape} · {held}"),
    })
}

pub fn under(kind: Kind, of: Size, bytes: u64, along: Along) -> Result<String, Never> {
    let Ok(pixels) = fitting::pixels(of);

    match (kind, pixels > 0) {
        (Kind::Picture, true) => about(of, bytes),
        (Kind::Picture, false) => size(bytes),
        (Kind::Film, _) => {
            let mut said: Vec<String> = Vec::new();

            match along.at > 0 || along.whole > 0 {
                true => {
                    let Ok(playing) = crate::playing::said(along);

                    said.push(playing);
                },
                false => {},
            }

            match pixels > 0 {
                true => {
                    let Ok(shape) = fitting::said(of);

                    said.push(shape);
                },
                false => {},
            }

            let Ok(held) = size(bytes);

            said.push(held);

            Ok(said.join(" · "))
        }
    }
}

pub fn wont_open(name: &str) -> Result<String, Never> {
    Ok(format!("{name} will not open. It may be damaged, or of a kind this cannot show."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(wide: u32, tall: u32) -> Size {
        let Ok(size) = Size::new(wide, tall);

        size
    }

    fn along(at: u64, whole: u64) -> Along {
        let Ok(along) = Along::new(at, whole);

        along
    }

    fn said_of(kind: Kind, of: Size, bytes: u64, along: Along) -> String {
        let Ok(said) = under(kind, of, bytes, along);

        said
    }

    #[test]
    fn a_size_is_said_in_the_units_a_disk_is_sold_in() {
        assert_eq!(size(0), Ok("0 bytes".to_string()));
        assert_eq!(size(999), Ok("999 bytes".to_string()));
        assert_eq!(size(1_000), Ok("1.0 kB".to_string()));
        assert_eq!(size(4_200_000), Ok("4.2 MB".to_string()));
        assert_eq!(size(2_500_000_000), Ok("2.5 GB".to_string()));
        assert_eq!(size(3_000_000_000_000), Ok("3.0 TB".to_string()));
    }

    #[test]
    fn a_photograph_says_its_shape_its_count_and_its_room() {
        let Ok(said) = about(sized(4000, 3000), 4_200_000);

        assert!(said.contains("4000 x 3000"), "{said}");
        assert!(said.contains("12 megapixels"), "{said}");
        assert!(said.contains("4.2 MB"), "{said}");
    }

    #[test]
    fn something_smaller_than_a_megapixel_does_not_say_so() {
        let Ok(said) = about(sized(32, 32), 900);

        assert!(!said.contains("megapixel"), "{said}");
        assert!(said.contains("32 x 32"), "{said}");
    }

    #[test]
    fn a_film_says_where_it_has_got_to_first() {
        let said = said_of(Kind::Film, sized(1920, 1080), 700_000_000, along(0, 7325));

        assert!(said.starts_with("0:00 of 2:02:05"), "{said}");
        assert!(said.contains("1920 x 1080"), "{said}");
    }

    #[test]
    fn a_film_part_way_through_says_both_ends_of_it() {
        let said = said_of(Kind::Film, sized(0, 0), 700_000_000, along(65, 7325));

        assert!(said.starts_with("1:05 of 2:02:05"), "{said}");
    }

    #[test]
    fn a_film_of_unread_length_says_what_it_knows() {
        let said = said_of(Kind::Film, sized(1920, 1080), 700_000_000, along(12, 0));

        assert!(!said.contains(" of "), "{said}");
        assert!(said.starts_with("0:12"), "{said}");
        assert!(said.contains("1920 x 1080"), "{said}");
    }

    #[test]
    fn a_film_nothing_has_read_yet_says_only_how_much_room_it_takes() {
        let said = said_of(Kind::Film, sized(0, 0), 7_800, Along::default());

        assert_eq!(said, "7.8 kB");
    }

    #[test]
    fn a_picture_with_no_shape_read_says_only_how_much_room_it_takes() {
        let said = said_of(Kind::Picture, sized(0, 0), 2_500, Along::default());

        assert_eq!(said, "2.5 kB");
    }

    #[test]
    fn a_picture_and_a_film_are_said_differently() {
        let of = sized(1920, 1080);

        assert_ne!(
            said_of(Kind::Picture, of, 100, Along::default()),
            said_of(Kind::Film, of, 100, along(0, 7325))
        );
    }

    #[test]
    fn something_that_will_not_open_says_which_thing_it_was() {
        let Ok(said) = wont_open("beach.jpg");

        assert!(said.starts_with("beach.jpg"), "{said}");
        assert!(said.ends_with('.'), "{said}");
    }
}
