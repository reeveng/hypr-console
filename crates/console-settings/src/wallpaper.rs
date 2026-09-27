//! The wallpaper tab: which picture is up, and where a new one comes from.
//!
//! Two things a person wants from a wallpaper and cannot otherwise have here.
//! To stop it changing, because they have found one they like and the weather
//! keeps taking it away. And to add one of their own, which on a machine with
//! no file manager and no terminal means putting a file in a directory and
//! being told what happened to it.
//!
//! Adding is the part worth explaining. `wallpaper-render` does the work, and what
//! it does is not copying: a picture is decoded, brought into this palette, cut to
//! the shape of this screen, and written out as something that rests and then
//! stirs. So the row does not say "copy" and it does not say "import", it says
//! what is actually going to happen to her picture.
//!
//! Reading what is on the machine is one half and knowing what to draw from it
//! is the other, and only the second is here, so the tab can be asked what it
//! would show without a machine to ask.

use console_core_never::Never;
use console_panel::page::{Aside, Handler, NOW, Row};

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
    pub dropped: u32,
}

pub fn wallpaper_rows(
    found: &Found<'_>,
    follow: impl Fn(bool) -> Handler,
    show: impl Fn(&str) -> Handler,
    take: Handler,
    find: Handler,
) -> Result<Vec<Row>, Never> {
    let &Found { pictures, following, up, dropped } = found;
    let Ok(weather) = Row::new(
        "Follow the weather",
        Aside(match following {
            true => NOW,
            false => "",
        }),
        follow(!following),
    );
    let mut rows = vec![weather];

    for picture in pictures {
        let Ok(row) = Row::new(
            &picture.says,
            Aside(match picture.name == up {
                true => NOW,
                false => picture.by.as_str(),
            }),
            show(&picture.name),
        );

        rows.push(row);
    }

    match pictures.is_empty() {
        true => {
            let Ok(row) = Row::placeholder("There are no pictures on this machine");

            rows.push(row);
        }
        false => {},
    }

    match dropped > 0 {
        true => {
            let Ok(row) = match dropped {
                1 => Row::new("Add the picture in Pictures/Wallpapers", Aside(""), take),
                many => Row::new(
                    &format!("Add the {many} pictures in Pictures/Wallpapers"),
                    Aside(""),
                    take,
                ),
            };

            rows.push(row);
        }
        false => {},
    }

    let Ok(finding) = Row::new("Find a picture in the files", Aside(""), find);

    rows.push(finding);

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use console_panel::page::{Action, Active};
    use super::*;

    const A_ROW: &str = "a row the tab was to have";

    fn nothing() -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn set() -> Result<Vec<Offered>, Never> {
        let Ok(hers) = Offered::of("her-own-photo");

        Ok(vec![
            Offered {
                name: "star-ride".to_string(),
                says: "Star Ride".to_string(),
                by: "Abi Toads".to_string(),
            },
            hers,
        ])
    }

    fn rows_of(found: &Found<'_>) -> Result<Vec<Row>, Never> {
        let Ok(choose) = nothing();
        let Ok(follow) = nothing();

        wallpaper_rows(
            found,
            |_| {
                let Ok(does) = nothing();

                does
            },
            |_| {
                let Ok(does) = nothing();

                does
            },
            choose,
            follow,
        )
    }

    fn following(dropped: u32) -> Result<Vec<Row>, Never> {
        let Ok(set) = set();

        rows_of(&Found { pictures: &set, following: true, up: "star-ride", dropped })
    }

    fn pinned(up: &str) -> Result<Vec<Row>, Never> {
        let Ok(set) = set();

        rows_of(&Found { pictures: &set, following: false, up, dropped: 0 })
    }

    fn says(rows: &[Row]) -> Result<Vec<&str>, Never> {
        Ok(rows.iter().map(|row| row.says.as_str()).collect())
    }

    #[test]
    fn a_picture_of_hers_is_named_after_its_own_file() {
        for (file, named) in [("her-own-photo", "Her Own Photo"), ("a_quiet_lake", "A Quiet Lake"), ("sunset", "Sunset")] {
            let Ok(offered) = Offered::of(file);

            assert_eq!(offered.says, named);
        }
    }

    #[test]
    fn following_the_weather_is_marked_when_it_is_what_is_happening() -> Result<(), &'static str> {
        let Ok(followed) = following(0);
        let Ok(pinned) = pinned("star-ride");
        let followed = followed.first().ok_or(A_ROW)?;
        let pinned = pinned.first().ok_or(A_ROW)?;

        assert_eq!(followed.now(), Ok(Active::Yes));
        assert_eq!(pinned.now(), Ok(Active::No));

        Ok(())
    }

    #[test]
    fn the_picture_on_the_screen_is_marked_however_it_was_chosen() -> Result<(), &'static str> {
        let Ok(followed) = following(0);
        let Ok(followed_says) = says(&followed);
        let up = followed.get(1).ok_or(A_ROW)?;

        assert_eq!(up.now(), Ok(Active::Yes), "{followed_says:?}");

        let Ok(pinned) = pinned("her-own-photo");
        let Ok(pinned_says) = says(&pinned);
        let up = pinned.get(2).ok_or(A_ROW)?;

        assert_eq!(up.now(), Ok(Active::Yes), "{pinned_says:?}");

        Ok(())
    }

    #[test]
    fn a_picture_that_is_not_up_says_who_drew_it() -> Result<(), &'static str> {
        let Ok(set) = set();
        let Ok(rows) = rows_of(&Found { pictures: &set, following: true, up: "her-own-photo", dropped: 0 });
        let other = rows.get(1).ok_or(A_ROW)?;

        assert_eq!(other.aside, "Abi Toads");

        Ok(())
    }

    #[test]
    fn an_empty_drop_is_not_offered_and_leaves_the_way_in_that_always_works() -> Result<(), &'static str> {
        let Ok(empty) = following(0);
        let Ok(said) = says(&empty);
        let last = empty.last().ok_or(A_ROW)?;

        assert!(!said.iter().any(|says| says.contains("Pictures/Wallpapers")));
        assert_eq!(last.says, "Find a picture in the files");
        assert!(last.does.is_some(), "every row on this tab can be chosen");

        Ok(())
    }

    #[test]
    fn every_row_on_the_tab_does_something() {
        for dropped in [0, 1, 4] {
            let Ok(rows) = following(dropped);

            for row in rows {
                assert_eq!(row.acts(), Ok(Action::Yes), "{:?} does nothing", row.says);
            }
        }
    }

    #[test]
    fn a_drop_with_something_in_it_offers_to_take_it_and_says_how_much() {
        let Ok(one) = following(1);
        let Ok(four) = following(4);
        let Ok(one) = says(&one);
        let Ok(four) = says(&four);

        assert!(one.contains(&"Add the picture in Pictures/Wallpapers"));
        assert!(four.contains(&"Add the 4 pictures in Pictures/Wallpapers"));
    }

    #[test]
    fn a_machine_with_no_pictures_says_so() {
        let Ok(bare) = rows_of(&Found { pictures: &[], following: true, up: "", dropped: 0 });
        let Ok(said) = says(&bare);

        assert!(said.contains(&"There are no pictures on this machine"));
    }
}
