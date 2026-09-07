//! A folder, as the run of things this panel can show, and where you are in it.
//!
//! Opening one photograph out of a folder of two hundred is opening the
//! folder. Nobody presses A on a holiday picture meaning to look at exactly
//! that one and then leave; they mean to start there and walk. So what the
//! panel holds is not a file, it is a reel: the folder in the order the files
//! panel would list it, with everything that is not a picture or a film taken
//! out, and a finger on the one being shown.
//!
//! Taking the others out is the part worth saying. A folder from a camera has
//! a `.thm` and a `.xmp` beside every photograph, and a folder of films has
//! subtitles and a `.nfo`. Left in the reel they are things the d-pad can
//! land on that the card cannot draw, so *next* would sometimes do nothing and
//! nobody could tell why. Left out, next is always the next thing there is to
//! look at, which is what the press means.
//!
//! Nothing here reads a disk. The listing is handed in, the same way
//! `console_files::looking::under` is handed its reading, so a reel can be
//! asked about without a folder to ask of.

use console_core_never::Never;

use crate::kinds::{self, Kind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shot {
    pub name: String,
    pub kind: Kind,
}

impl Shot {
    pub fn new(name: &str, kind: Kind) -> Result<Self, Never> {
        Ok(Shot { name: name.to_string(), kind })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Reel {
    shots: Vec<Shot>,
    at: usize,
}

impl Reel {
    pub fn of(listing: &[(String, String)], opened: &str) -> Result<Option<Reel>, Never> {
        let shots: Vec<Shot> = listing
            .iter()
            .filter_map(|(name, mime)| {
                let Ok(kind) = kinds::of(mime);

                kind.map(|kind| {
                    let Ok(shot) = Shot::new(name, kind);

                    shot
                })
            })
            .collect();

        match shots.is_empty() {
            true => return Ok(None),
            false => {},
        }

        let at = shots.iter().position(|shot| shot.name == opened).unwrap_or(0);

        Ok(Some(Reel { shots, at }))
    }

    pub fn showing(&self) -> Result<&Shot, Never> {
        static NOTHING: Shot = Shot { name: String::new(), kind: Kind::Picture };

        Ok(match self.shots.get(self.at).or_else(|| self.shots.first()) {
            Some(shot) => shot,
            None => &NOTHING,
        })
    }

    pub fn many(&self) -> Result<usize, Never> {
        Ok(self.shots.len())
    }

    pub fn which(&self) -> Result<usize, Never> {
        Ok(self.at.saturating_add(1))
    }

    pub fn step(&mut self, by: isize) -> Result<(), Never> {
        let many = self.shots.len();

        let forward = match by >= 0 {
            true => by.unsigned_abs().checked_rem(many).unwrap_or(0),
            false => many.saturating_sub(by.unsigned_abs().checked_rem(many).unwrap_or(0)),
        };

        self.at = self.at.saturating_add(forward).checked_rem(many).unwrap_or(0);

        Ok(())
    }

    pub fn stand_on(&mut self, name: &str) -> Result<Stood, Never> {
        Ok(match self.shots.iter().position(|shot| shot.name == name) {
            Some(at) => {
                self.at = at;
                Stood::OnIt
            }
            None => Stood::NotThere,
        })
    }

    pub fn every(&self) -> Result<&[Shot], Never> {
        Ok(&self.shots)
    }

    pub fn names(&self) -> Result<impl Iterator<Item = &str>, Never> {
        Ok(self.shots.iter().map(|shot| shot.name.as_str()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stood {
    OnIt,
    NotThere,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> Vec<(String, String)> {
        [
            ("beach.jpg", "image/jpeg"),
            ("beach.jpg.xmp", "application/rdf+xml"),
            ("boat.png", "image/png"),
            ("notes.txt", "text/plain"),
            ("swim.mp4", "video/mp4"),
        ]
        .iter()
        .map(|(name, mime)| ((*name).to_string(), (*mime).to_string()))
        .collect()
    }

    fn of(listing: &[(String, String)], opened: &str) -> Reel {
        let Ok(reel) = Reel::of(listing, opened);

        reel.expect("a reel")
    }

    fn reel(opened: &str) -> Reel {
        of(&folder(), opened)
    }

    fn showing(reel: &Reel) -> &Shot {
        let Ok(shot) = reel.showing();

        shot
    }

    fn stepped(reel: &mut Reel, by: isize) -> &Shot {
        let Ok(()) = reel.step(by);

        showing(reel)
    }

    #[test]
    fn a_reel_opens_standing_on_the_thing_that_was_opened() {
        assert_eq!(showing(&reel("boat.png")).name, "boat.png");
        assert_eq!(reel("boat.png").which(), Ok(2));
        assert_eq!(showing(&reel("swim.mp4")).name, "swim.mp4");
    }

    #[test]
    fn what_this_cannot_show_is_not_in_the_reel() {
        let reel = reel("beach.jpg");
        let Ok(names) = reel.names();

        assert_eq!(names.collect::<Vec<_>>(), ["beach.jpg", "boat.png", "swim.mp4"]);
        assert_eq!(reel.many(), Ok(3));
    }

    #[test]
    fn a_picture_and_a_film_are_both_in_it_and_know_which_they_are() {
        assert_eq!(showing(&reel("swim.mp4")).kind, Kind::Film);
        assert_eq!(showing(&reel("beach.jpg")).kind, Kind::Picture);
    }

    #[test]
    fn the_folders_own_order_is_kept() {
        let listing: Vec<(String, String)> = [("z.jpg", "image/jpeg"), ("a.jpg", "image/jpeg")]
            .iter()
            .map(|(name, mime)| ((*name).to_string(), (*mime).to_string()))
            .collect();
        let reel = of(&listing, "z.jpg");
        let Ok(names) = reel.names();

        assert_eq!(names.collect::<Vec<_>>(), ["z.jpg", "a.jpg"]);
    }

    #[test]
    fn walking_goes_forward_and_back() {
        let mut reel = reel("beach.jpg");

        assert_eq!(stepped(&mut reel, 1).name, "boat.png");
        assert_eq!(stepped(&mut reel, 1).name, "swim.mp4");
        assert_eq!(stepped(&mut reel, -1).name, "boat.png");
    }

    #[test]
    fn walking_off_either_end_comes_round() {
        let mut reel = reel("swim.mp4");

        assert_eq!(stepped(&mut reel, 1).name, "beach.jpg");
        assert_eq!(stepped(&mut reel, -1).name, "swim.mp4");
    }

    #[test]
    fn a_step_of_more_than_the_whole_reel_still_lands_somewhere() {
        let mut reel = reel("beach.jpg");

        assert_eq!(stepped(&mut reel, 7).name, "boat.png");
        assert_eq!(stepped(&mut reel, -7).name, "beach.jpg");
        assert_eq!(stepped(&mut reel, 0).name, "beach.jpg");
    }

    #[test]
    fn a_folder_with_one_picture_in_it_is_a_reel() {
        let listing = vec![("beach.jpg".to_string(), "image/jpeg".to_string())];
        let mut reel = of(&listing, "beach.jpg");

        assert_eq!(reel.many(), Ok(1));
        assert_eq!(stepped(&mut reel, 1).name, "beach.jpg");
        assert_eq!(stepped(&mut reel, -1).name, "beach.jpg");
    }

    #[test]
    fn a_folder_with_nothing_to_show_is_no_reel_at_all() {
        let listing = vec![("notes.txt".to_string(), "text/plain".to_string())];

        assert_eq!(Reel::of(&listing, "notes.txt"), Ok(None));
        assert_eq!(Reel::of(&[], "beach.jpg"), Ok(None));
    }

    #[test]
    fn opening_something_unshowable_still_opens_the_folder() {
        let reel = reel("notes.txt");

        assert_eq!(showing(&reel).name, "beach.jpg");
        assert_eq!(reel.which(), Ok(1));
    }

    #[test]
    fn a_reel_read_again_can_be_put_back_where_it_was() {
        let mut reel = reel("beach.jpg");

        assert_eq!(reel.stand_on("swim.mp4"), Ok(Stood::OnIt));
        assert_eq!(showing(&reel).name, "swim.mp4");
        assert_eq!(reel.stand_on("gone.jpg"), Ok(Stood::NotThere));
        assert_eq!(showing(&reel).name, "swim.mp4", "left where it was");
    }

    #[test]
    fn which_one_this_is_is_counted_the_way_a_person_says_it() {
        let mut reel = reel("beach.jpg");

        assert_eq!((reel.which(), reel.many()), (Ok(1), Ok(3)));

        let Ok(()) = reel.step(2);

        assert_eq!((reel.which(), reel.many()), (Ok(3), Ok(3)));
    }
}
