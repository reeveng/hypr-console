//! Whether there is enough room left on the disk to start an apply.
//!
//! `enough` asks whether the machine will still be running in ten minutes.
//! This asks the other question a long write has to ask first, and until now
//! nothing did: whether there is anywhere to put what it is about to write.
//!
//! The machine builds what it runs. An apply compiles every name in `[build]`
//! out of the checkout, downloads whatever packages the manifest names and does
//! not have, and stages a copy of every file beside the one it is replacing --
//! all of it onto the same disk as the games, the videos and whatever the
//! download panel was last told to fetch. A disk that fills partway through
//! that does not say so: the fault arrives as a compile that stopped, a package
//! that would not unpack, a file installed as half of itself. So the question
//! is asked once, at the top, where the answer is still a sentence rather than
//! a mess.
//!
//! # What decides the sizes
//!
//! `A_BUILD` is what an apply wants to itself, and it is the build directory on
//! the device measured and then doubled, because the two things that happen
//! beside a build -- the packages being downloaded and the copies staged next
//! to what they replace -- are on the same disk and are not free either. It is
//! deliberately a size and not a share of the disk: a tenth of this device is
//! room for twenty builds and a tenth of a smaller one is room for none.
//!
//! `AN_EVENING` is the other half of the same question and is only asked of a
//! machine standing still. What the person does after an apply is download
//! things, and a card that waits until an apply cannot run is a card that
//! arrives on the evening it is least welcome.
//!
//! # Why where it went is named and not just how much is gone
//!
//! A number alone leaves somebody looking for the room with a torch. Naming the
//! places is what makes the sentence actionable, and it is why the places are
//! handed in here rather than assumed: what is big on this device is the games,
//! and on the next one it will be something else.
//!
//! Measuring them is a walk over every file in the person's home, which is a
//! minute on a machine with a lot in it, so nobody walks it to answer a
//! question that came back fine. How much room is left is a single cheap
//! reading and it is taken every time; where it went is measured only once that
//! reading is below what an apply wants, which is the moment somebody has to be
//! told where to look. Between the two lines the card says the size and names
//! nothing, because an hourly walk on a machine that is merely getting full is
//! a cost paid for a sentence nobody has to act on yet.

use console_core_never::Never;

const GIGABYTE: u64 = 1024 * 1024 * 1024;

pub const A_BUILD: u64 = 6 * GIGABYTE;

pub const AN_EVENING: u64 = 10 * GIGABYTE;

pub const STANDING: u64 = A_BUILD + AN_EVENING;

const NAMED: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub name: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Left {
    Said(u64),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Room {
    Enough,
    No(String),
}

pub fn words(bytes: u64) -> Result<String, Never> {
    let whole = match bytes.checked_div(GIGABYTE) {
        Some(whole) => whole,
        None => return Ok(format!("{bytes} bytes")),
    };

    Ok(match whole == 0 {
        true => "less than a gigabyte".to_string(),
        false => format!("{whole} GB"),
    })
}

fn where_it_went(went: &[Place]) -> Result<String, Never> {
    let mut biggest: Vec<&Place> = went.iter().filter(|place| place.bytes >= GIGABYTE).collect();
    biggest.sort_by_key(|place| std::cmp::Reverse(place.bytes));

    let named: Vec<String> = biggest
        .into_iter()
        .take(NAMED)
        .map(|place| {
            let Ok(size) = words(place.bytes);

            format!("{} ({size})", place.name)
        })
        .collect();

    Ok(match named.is_empty() {
        true => String::new(),
        false => format!(" Most of the room has gone to {}.", named.join(", ")),
    })
}

pub fn free_in(said: &str) -> Result<Left, Never> {
    let last = said.lines().map(str::trim).rfind(|line| !line.is_empty());

    let bytes = match last {
        Some(bytes) => bytes,
        None => return Ok(Left::Unknown("nothing was said about how much room is left".to_string())),
    };

    Ok(match bytes.parse::<u64>() {
        Ok(bytes) => Left::Said(bytes),
        Err(fault) => Left::Unknown(format!("{bytes:?} is not a number of bytes ({fault})")),
    })
}

pub fn places_in(said: &str, roots: &[String]) -> Result<Vec<Place>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(_, at)| !roots.iter().any(|root| root == at.trim()))
        .filter_map(|(size, at)| {
            let bytes = match size.trim().parse::<u64>() {
                Ok(bytes) => bytes,
                Err(_fault) => return None,
            };

            let name = at.trim().rsplit('/').next()?;

            Some((bytes, name))
        })
        .filter(|(_, name)| !name.starts_with('.'))
        .map(|(bytes, name)| Place { name: name.to_string(), bytes })
        .collect())
}

pub fn before_an_apply(free: u64, went: &[Place]) -> Result<Room, Never> {
    match free >= A_BUILD {
        true => return Ok(Room::Enough),
        false => {},
    }

    let Ok(left) = words(free);
    let Ok(wanted) = words(A_BUILD);
    let Ok(gone) = where_it_went(went);

    Ok(Room::No(format!(
        "there is {left} left on this disk and an apply wants {wanted} to be sure of finishing: it \
         builds the whole desktop before it installs any of it, and a build that runs out of room \
         stops somewhere nobody chose.{gone} Make some room and run it again."
    )))
}

pub fn on_a_machine_standing(free: u64, went: &[Place]) -> Result<Room, Never> {
    match free >= STANDING {
        true => return Ok(Room::Enough),
        false => {},
    }

    let Ok(left) = words(free);
    let Ok(wanted) = words(A_BUILD);
    let Ok(gone) = where_it_went(went);

    Ok(Room::No(format!(
        "There is {left} left. The next update wants {wanted} of that to itself, and what is over \
         is what there is for anything you save or download.{gone}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(name: &str, bytes: u64) -> Place {
        Place { name: name.to_string(), bytes }
    }

    fn asking(free: u64) -> Room {
        let Ok(room) = before_an_apply(free, &[]);

        room
    }

    fn said(room: Room) -> String {
        match room {
            Room::No(said) => said,
            Room::Enough => panic!("it was allowed"),
        }
    }

    #[test]
    fn an_apply_wants_room_for_the_build_it_is_about_to_run() {
        assert_eq!(asking(A_BUILD), Room::Enough);
        assert!(matches!(asking(A_BUILD - 1), Room::No(_)));
    }

    #[test]
    fn a_machine_standing_still_is_asked_for_the_evening_as_well_as_the_apply() {
        let Ok(room) = on_a_machine_standing(A_BUILD, &[]);
        assert!(
            matches!(room, Room::No(_)),
            "room for the apply and nothing after it, and the card waited for the refusal"
        );

        let Ok(room) = on_a_machine_standing(STANDING, &[]);
        assert_eq!(room, Room::Enough);
    }

    #[test]
    fn the_refusal_says_what_is_wrong_and_what_would_fix_it() {
        let said = said(asking(GIGABYTE));
        assert!(said.contains("1 GB left"), "{said}");
        assert!(said.contains("6 GB"), "{said}");
        assert!(said.contains("Make some room"), "{said}");
    }

    #[test]
    fn a_refusal_names_the_biggest_places_and_the_size_of_each() {
        let Ok(room) = before_an_apply(
            GIGABYTE,
            &[place("Videos", 8 * GIGABYTE), place("Steam", 122 * GIGABYTE)],
        );
        let said = said(room);

        assert!(said.contains("Steam (122 GB), Videos (8 GB)"), "{said}");
    }

    #[test]
    fn nothing_worth_clearing_is_nothing_said_about_it() {
        let Ok(room) = before_an_apply(GIGABYTE, &[place("Downloads", 4096)]);
        let said = said(room);

        assert!(!said.contains("Most of the room"), "{said}");
        assert!(!said.contains("Downloads"), "{said}");
    }

    #[test]
    fn only_the_few_worth_naming_are_named() {
        let went: Vec<Place> = (1..8u64)
            .map(|which| {
                let bytes = match which.checked_mul(GIGABYTE) {
                    Some(bytes) => bytes,
                    None => return place("nowhere", 0),
                };

                place(&format!("place{which}"), bytes)
            })
            .collect();
        let Ok(room) = before_an_apply(GIGABYTE, &went);
        let said = said(room);

        assert!(said.contains("place7 (7 GB), place6 (6 GB), place5 (5 GB)"), "{said}");
        assert!(!said.contains("place4"), "{said}");
    }

    #[test]
    fn what_the_disk_says_is_the_number_under_the_heading() {
        assert_eq!(free_in("     Avail\n1795046354944\n"), Ok(Left::Said(1795046354944)));
    }

    #[test]
    fn a_disk_that_would_not_say_is_not_a_disk_with_no_room() {
        let Ok(left) = free_in("");
        assert!(matches!(left, Left::Unknown(_)), "{left:?}");

        let Ok(left) = free_in("df: /nowhere: No such file or directory");
        assert!(matches!(left, Left::Unknown(_)), "{left:?}");
    }

    #[test]
    fn the_places_are_what_is_under_the_ones_asked_about_and_not_the_ones_asked_about() {
        let roots = vec!["/home/somebody".to_string(), "/home/somebody/.local/share".to_string()];
        let said = "\
8000000000\t/home/somebody/Videos
131000000000\t/home/somebody
122000000000\t/home/somebody/.local/share/Steam
123000000000\t/home/somebody/.local/share
";
        let Ok(places) = places_in(said, &roots);

        assert_eq!(
            places,
            vec![
                place("Videos", 8000000000),
                place("Steam", 122000000000),
            ]
        );
    }

    #[test]
    fn a_folder_the_person_never_made_is_not_one_they_can_be_asked_to_clear() {
        let roots = vec!["/home/somebody".to_string()];
        let said = "123000000000\t/home/somebody/.local\n2000000000\t/home/somebody/Music\n";
        let Ok(places) = places_in(said, &roots);

        assert_eq!(places, vec![place("Music", 2000000000)]);
    }

    #[test]
    fn a_size_under_a_gigabyte_is_said_in_words_rather_than_as_a_nought() {
        assert_eq!(words(0), Ok("less than a gigabyte".to_string()));
        assert_eq!(words(GIGABYTE - 1), Ok("less than a gigabyte".to_string()));
        assert_eq!(words(GIGABYTE), Ok("1 GB".to_string()));
    }
}
