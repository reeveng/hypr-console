//! The wallpaper tab: which picture is up, and where a new one comes from.
//!
//! Two things a person wants from a wallpaper and cannot otherwise have here.
//! To stop it changing, because they have found one they like and the weather
//! keeps taking it away. And to add one of their own, which on a machine with
//! no file manager and no terminal means putting a file in a directory and
//! being told what happened to it.
//!
//! Adding is the part worth explaining. `sky-press` does the work, and what it
//! does is not copying: a picture is decoded, brought into this palette, cut to
//! the shape of this screen, and written out as something that rests and then
//! stirs. So the row does not say "copy" and it does not say "import", it says
//! what is actually going to happen to her picture.
//!
//! Reading what is on the machine is one half and knowing what to draw from it
//! is the other, and only the second is here, so the tab can be asked what it
//! would show without a machine to ask.

use console_panel::page::{Does, NOW, Row};

/// One picture, as a tab has to know it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub name: String,
    pub says: String,
    pub by: String,
}

impl Offered {
    /// A picture nothing is written down about, which is one of hers.
    ///
    /// The file's own name is all there is, so it is made into something worth
    /// reading: hyphens are spaces and every word gets its capital.
    pub fn of(name: &str) -> Self {
        let says = name
            .split(['-', '_'])
            .filter(|word| !word.is_empty())
            .map(|word| {
                let mut letters = word.chars();

                match letters.next() {
                    Some(first) => first.to_uppercase().chain(letters).collect::<String>(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        Offered { name: name.to_string(), says, by: String::new() }
    }
}

/// What the tab was able to read off the machine.
pub struct Found<'a> {
    /// Every picture this machine can put up.
    pub pictures: &'a [Offered],
    /// Whether the picture changes with the sun and the weather.
    pub following: bool,
    /// The one on the screen now, and empty while the daemon has not said yet:
    /// a tab with nothing marked rather than a tab that is not there.
    pub up: &'a str,
    /// How many files are waiting in the drop.
    pub dropped: usize,
}

/// What the tab holds.
pub fn wallpaper_rows(
    found: &Found<'_>,
    follow: impl Fn(bool) -> Does,
    show: impl Fn(&str) -> Does,
    take: Does,
    find: Does,
) -> Vec<Row> {
    let &Found { pictures, following, up, dropped } = found;
    let mut rows = vec![Row::new(
        "Follow the weather",
        match following {
            true => NOW,
            false => "",
        },
        follow(!following),
    )];

    for picture in pictures {
        rows.push(Row::new(
            &picture.says,
            match picture.name == up {
                // What is on the screen is worth saying even while the weather
                // is choosing it, because otherwise the tab is a list of names
                // with no way to tell which one you are looking at.
                true => NOW,
                false => picture.by.as_str(),
            },
            show(&picture.name),
        ));
    }

    if pictures.is_empty() {
        rows.push(Row::nothing("There are no pictures on this machine"));
    }

    // Said with the number in it, because "add a picture" on a machine holding
    // none of hers is an instruction and not an offer, and the number is what
    // tells the two apart without her having to try it.
    //
    // With nothing in the drop this used to be a line of text telling her to go
    // and put one there, which is the one row on the panel that did nothing
    // when it was pressed. What it was standing in for is under it now.
    if dropped > 0 {
        rows.push(match dropped {
            1 => Row::new("Add the picture in Pictures/Wallpapers", "", take),
            many => Row::new(&format!("Add the {many} pictures in Pictures/Wallpapers"), "", take),
        });
    }

    // The other road in, and the only one a hand holding nothing but the
    // controller can take: the files are where her photographs are, and Y on
    // one of them offers to make it the wallpaper.
    rows.push(Row::new("Find a picture in the files", "", find));
    rows
}

#[cfg(test)]
mod tests {
    use console_panel::page::{Acts, InEffect};
    use super::*;

    fn nothing() -> Does {
        Does::and_stay(|_| ())
    }

    fn set() -> Vec<Offered> {
        vec![
            Offered {
                name: "star-ride".to_string(),
                says: "Star Ride".to_string(),
                by: "Abi Toads".to_string(),
            },
            Offered::of("her-own-photo"),
        ]
    }

    fn rows(following: bool, up: &str, dropped: usize) -> Vec<Row> {
        let set = set();
        let found = Found { pictures: &set, following, up, dropped };
        wallpaper_rows(&found, |_| nothing(), |_| nothing(), nothing(), nothing())
    }

    fn says(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.says.as_str()).collect()
    }

    #[test]
    fn a_picture_of_hers_is_named_after_its_own_file() {
        assert_eq!(Offered::of("her-own-photo").says, "Her Own Photo");
        assert_eq!(Offered::of("a_quiet_lake").says, "A Quiet Lake");
        assert_eq!(Offered::of("sunset").says, "Sunset");
    }

    #[test]
    fn following_the_weather_is_marked_when_it_is_what_is_happening() {
        assert_eq!(rows(true, "star-ride", 0)[0].now(), InEffect::Yes);
        assert_eq!(rows(false, "star-ride", 0)[0].now(), InEffect::No);
    }

    /// The tab is a list of names, and without this there is no way to tell
    /// which of them you are looking at.
    #[test]
    fn the_picture_on_the_screen_is_marked_however_it_was_chosen() {
        let following = rows(true, "star-ride", 0);
        assert_eq!(following[1].now(), InEffect::Yes, "{:?}", says(&following));
        let pinned = rows(false, "her-own-photo", 0);
        assert_eq!(pinned[2].now(), InEffect::Yes, "{:?}", says(&pinned));
    }

    /// Who drew it, where there is nothing more useful to put.
    #[test]
    fn a_picture_that_is_not_up_says_who_drew_it() {
        assert_eq!(rows(true, "her-own-photo", 0)[1].aside, "Abi Toads");
    }

    /// An offer to add nothing is not an offer, it is a puzzle. With the drop
    /// empty there is no row for it at all, and the way in that works whatever
    /// is in the drop is the last row instead.
    #[test]
    fn an_empty_drop_is_not_offered_and_leaves_the_way_in_that_always_works() {
        let empty = rows(true, "star-ride", 0);
        assert!(!says(&empty).iter().any(|says| says.contains("Pictures/Wallpapers")));
        let last = empty.last().expect("a row");
        assert_eq!(last.says, "Find a picture in the files");
        assert!(last.does.is_some(), "every row on this tab can be chosen");
    }

    /// The one row that used to do nothing when it was pressed.
    #[test]
    fn every_row_on_the_tab_does_something() {
        for dropped in [0, 1, 4] {
            for row in rows(true, "star-ride", dropped) {
                assert_eq!(row.acts(), Acts::Yes, "{:?} does nothing", row.says);
            }
        }
    }

    #[test]
    fn a_drop_with_something_in_it_offers_to_take_it_and_says_how_much() {
        assert!(says(&rows(true, "star-ride", 1)).contains(&"Add the picture in Pictures/Wallpapers"));
        assert!(
            says(&rows(true, "star-ride", 4)).contains(&"Add the 4 pictures in Pictures/Wallpapers")
        );
    }

    /// A machine with nothing on it should say so rather than showing a tab
    /// holding one switch and nothing to switch between.
    #[test]
    fn a_machine_with_no_pictures_says_so() {
        let found = Found { pictures: &[], following: true, up: "", dropped: 0 };
        let bare = wallpaper_rows(&found, |_| nothing(), |_| nothing(), nothing(), nothing());
        assert!(says(&bare).contains(&"There are no pictures on this machine"));
    }
}
