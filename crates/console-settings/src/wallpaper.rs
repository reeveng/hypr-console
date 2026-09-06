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

use console_never::Never;
use console_panel::page::{Does, NOW, Row};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub name: String,
    pub says: String,
    pub by: String,
}

impl Offered {
    pub fn of(name: &str) -> Result<Self, Never> {
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

        Ok(Offered { name: name.to_string(), says, by: String::new() })
    }
}

pub struct Found<'a> {
    pub pictures: &'a [Offered],
    pub following: bool,
    pub up: &'a str,
    pub dropped: usize,
}

pub fn wallpaper_rows(
    found: &Found<'_>,
    follow: impl Fn(bool) -> Does,
    show: impl Fn(&str) -> Does,
    take: Does,
    find: Does,
) -> Result<Vec<Row>, Never> {
    let &Found { pictures, following, up, dropped } = found;
    let Ok(weather) = Row::new(
        "Follow the weather",
        match following {
            true => NOW,
            false => "",
        },
        follow(!following),
    );
    let mut rows = vec![weather];

    for picture in pictures {
        let Ok(row) = Row::new(
            &picture.says,
            match picture.name == up {
                true => NOW,
                false => picture.by.as_str(),
            },
            show(&picture.name),
        );

        rows.push(row);
    }

    match pictures.is_empty() {
        true => {
            let Ok(row) = Row::nothing("There are no pictures on this machine");

            rows.push(row);
        }
        false => {},
    }

    match dropped > 0 {
        true => {
            let Ok(row) = match dropped {
                1 => Row::new("Add the picture in Pictures/Wallpapers", "", take),
                many => Row::new(
                    &format!("Add the {many} pictures in Pictures/Wallpapers"),
                    "",
                    take,
                ),
            };

            rows.push(row);
        }
        false => {},
    }

    let Ok(finding) = Row::new("Find a picture in the files", "", find);

    rows.push(finding);

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use console_panel::page::{Acts, InEffect};
    use super::*;

    fn of(name: &str) -> Offered {
        let Ok(offered) = Offered::of(name);

        offered
    }

    fn nothing() -> Does {
        let Ok(does) = Does::and_stay(|_| ());

        does
    }

    fn now(row: &Row) -> InEffect {
        let Ok(now) = row.now();

        now
    }

    fn acts(row: &Row) -> Acts {
        let Ok(acts) = row.acts();

        acts
    }

    fn set() -> Vec<Offered> {
        vec![
            Offered {
                name: "star-ride".to_string(),
                says: "Star Ride".to_string(),
                by: "Abi Toads".to_string(),
            },
            of("her-own-photo"),
        ]
    }

    fn rows(following: bool, up: &str, dropped: usize) -> Vec<Row> {
        let set = set();
        let found = Found { pictures: &set, following, up, dropped };
        let Ok(rows) = wallpaper_rows(&found, |_| nothing(), |_| nothing(), nothing(), nothing());

        rows
    }

    fn says(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.says.as_str()).collect()
    }

    #[test]
    fn a_picture_of_hers_is_named_after_its_own_file() {
        assert_eq!(of("her-own-photo").says, "Her Own Photo");
        assert_eq!(of("a_quiet_lake").says, "A Quiet Lake");
        assert_eq!(of("sunset").says, "Sunset");
    }

    #[test]
    fn following_the_weather_is_marked_when_it_is_what_is_happening() {
        assert_eq!(now(&rows(true, "star-ride", 0)[0]), InEffect::Yes);
        assert_eq!(now(&rows(false, "star-ride", 0)[0]), InEffect::No);
    }

    #[test]
    fn the_picture_on_the_screen_is_marked_however_it_was_chosen() {
        let following = rows(true, "star-ride", 0);
        assert_eq!(now(&following[1]), InEffect::Yes, "{:?}", says(&following));
        let pinned = rows(false, "her-own-photo", 0);
        assert_eq!(now(&pinned[2]), InEffect::Yes, "{:?}", says(&pinned));
    }

    #[test]
    fn a_picture_that_is_not_up_says_who_drew_it() {
        assert_eq!(rows(true, "her-own-photo", 0)[1].aside, "Abi Toads");
    }

    #[test]
    fn an_empty_drop_is_not_offered_and_leaves_the_way_in_that_always_works() {
        let empty = rows(true, "star-ride", 0);
        assert!(!says(&empty).iter().any(|says| says.contains("Pictures/Wallpapers")));
        let last = empty.last().expect("a row");
        assert_eq!(last.says, "Find a picture in the files");
        assert!(last.does.is_some(), "every row on this tab can be chosen");
    }

    #[test]
    fn every_row_on_the_tab_does_something() {
        for dropped in [0, 1, 4] {
            for row in rows(true, "star-ride", dropped) {
                assert_eq!(acts(&row), Acts::Yes, "{:?} does nothing", row.says);
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

    #[test]
    fn a_machine_with_no_pictures_says_so() {
        let found = Found { pictures: &[], following: true, up: "", dropped: 0 };
        let Ok(bare) = wallpaper_rows(&found, |_| nothing(), |_| nothing(), nothing(), nothing());
        assert!(says(&bare).contains(&"There are no pictures on this machine"));
    }
}
