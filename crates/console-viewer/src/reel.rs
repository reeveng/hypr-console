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

use crate::kinds::{self, Kind};

/// One thing in a folder that this panel can show.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shot {
    /// Its name in the folder, which is also what the card says at the top.
    pub name: String,
    pub kind: Kind,
}

impl Shot {
    pub fn new(name: &str, kind: Kind) -> Self {
        Shot { name: name.to_string(), kind }
    }
}

/// The showable things in a folder, and which one is in front.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reel {
    shots: Vec<Shot>,
    at: usize,
}

impl Reel {
    /// The reel a folder makes, standing on the thing that was opened.
    ///
    /// The listing is given as it would be drawn -- names and the type the
    /// machine says each one is -- and everything that is neither a picture
    /// nor a film is dropped. The order is the listing's own and is not sorted
    /// again here: what a person is walking is the folder they were just
    /// looking at, and a second opinion about its order would put the next
    /// photograph somewhere they did not leave it.
    ///
    /// Standing on a name that is not in the reel -- which is what happens
    /// when the thing opened is the one thing in the folder this cannot show
    /// -- leaves the finger at the start rather than refusing. There is still
    /// a folder to walk.
    pub fn of(listing: &[(String, String)], opened: &str) -> Option<Reel> {
        let shots: Vec<Shot> = listing
            .iter()
            .filter_map(|(name, mime)| kinds::of(mime).map(|kind| Shot::new(name, kind)))
            .collect();

        if shots.is_empty() {
            return None;
        }

        let at = shots.iter().position(|shot| shot.name == opened).unwrap_or(0);

        Some(Reel { shots, at })
    }

    /// What is being shown.
    pub fn showing(&self) -> &Shot {
        &self.shots[self.at]
    }

    /// How many there are to walk.
    pub fn many(&self) -> usize {
        self.shots.len()
    }

    /// Which one this is, counting from one, as a person would say it.
    pub fn which(&self) -> usize {
        self.at + 1
    }

    /// Walk, by however many, wrapping at either end.
    ///
    /// It wraps for the reason every other list on this desktop wraps: the
    /// card the browser add-on draws, the menu and the panels all come round,
    /// so a thumb held on the d-pad never arrives at a press that does
    /// nothing. A reel that stopped dead at the last photograph would be the
    /// one list on the machine that did, and the person holding it would find
    /// that out by pressing and getting nothing back.
    pub fn step(&mut self, by: isize) {
        // Every reel has something in it -- `of` returns nothing for an empty
        // folder, and there is no other way to make one -- so there is always
        // somewhere to step to and `many` is never nought.
        let many = self.shots.len();

        // Done forwards whichever way the press went, because stepping back n
        // is stepping forward `many - n`. That keeps the whole sum in `usize`,
        // where the only conversion is `unsigned_abs`, which cannot fail --
        // and a conversion that cannot fail is a conversion with no default
        // behind it to be wrong about.
        let forward = match by >= 0 {
            true => by.unsigned_abs() % many,
            false => many - (by.unsigned_abs() % many),
        };

        self.at = (self.at + forward) % many;
    }

    /// Stand on a named thing, if the reel holds one.
    ///
    /// What a folder being read again is for: the panel re-reads after a file
    /// is thrown away or renamed, and the person should be left looking at
    /// whatever they were looking at rather than back at the start.
    pub fn stand_on(&mut self, name: &str) -> Stood {
        match self.shots.iter().position(|shot| shot.name == name) {
            Some(at) => {
                self.at = at;
                Stood::OnIt
            }
            None => Stood::NotThere,
        }
    }

    /// Every name in the reel, in order. What the folder looks like to a
    /// caller that wants to draw the whole run rather than one of it.
    /// Everything in it, in the order a folder is walked.
    ///
    /// For the tab that draws the folder as a list rather than walking it one
    /// press at a time. `names` is not enough there: a row has to say which
    /// kind of thing it is, because a folder holding both draws them
    /// differently.
    pub fn every(&self) -> &[Shot] {
        &self.shots
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.shots.iter().map(|shot| shot.name.as_str())
    }
}

/// Whether a name asked for was in the reel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stood {
    /// It was, and the finger is on it.
    OnIt,
    /// It is gone -- thrown away, or renamed while the card was up.
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

    fn reel(opened: &str) -> Reel {
        Reel::of(&folder(), opened).expect("a reel")
    }

    /// The whole reason a reel exists: pressing A on one photograph opens the
    /// folder standing on it.
    #[test]
    fn a_reel_opens_standing_on_the_thing_that_was_opened() {
        assert_eq!(reel("boat.png").showing().name, "boat.png");
        assert_eq!(reel("boat.png").which(), 2);
        assert_eq!(reel("swim.mp4").showing().name, "swim.mp4");
    }

    /// Everything the card cannot draw is left out, so next is always the next
    /// thing there is to look at.
    #[test]
    fn what_this_cannot_show_is_not_in_the_reel() {
        let reel = reel("beach.jpg");
        assert_eq!(reel.names().collect::<Vec<_>>(), ["beach.jpg", "boat.png", "swim.mp4"]);
        assert_eq!(reel.many(), 3);
    }

    #[test]
    fn a_picture_and_a_film_are_both_in_it_and_know_which_they_are() {
        assert_eq!(reel("swim.mp4").showing().kind, Kind::Film);
        assert_eq!(reel("beach.jpg").showing().kind, Kind::Picture);
    }

    /// The order is the listing's, not a second opinion about it.
    #[test]
    fn the_folders_own_order_is_kept() {
        let listing: Vec<(String, String)> = [("z.jpg", "image/jpeg"), ("a.jpg", "image/jpeg")]
            .iter()
            .map(|(name, mime)| ((*name).to_string(), (*mime).to_string()))
            .collect();
        let reel = Reel::of(&listing, "z.jpg").expect("a reel");
        assert_eq!(reel.names().collect::<Vec<_>>(), ["z.jpg", "a.jpg"]);
    }

    #[test]
    fn walking_goes_forward_and_back() {
        let mut reel = reel("beach.jpg");
        reel.step(1);
        assert_eq!(reel.showing().name, "boat.png");
        reel.step(1);
        assert_eq!(reel.showing().name, "swim.mp4");
        reel.step(-1);
        assert_eq!(reel.showing().name, "boat.png");
    }

    /// The one press that must never do nothing. Every other list on this
    /// desktop comes round, and a reel that stopped would be the only one that
    /// did not.
    #[test]
    fn walking_off_either_end_comes_round() {
        let mut reel = reel("swim.mp4");
        reel.step(1);
        assert_eq!(reel.showing().name, "beach.jpg");
        reel.step(-1);
        assert_eq!(reel.showing().name, "swim.mp4");
    }

    #[test]
    fn a_step_of_more_than_the_whole_reel_still_lands_somewhere() {
        let mut reel = reel("beach.jpg");
        reel.step(7);
        assert_eq!(reel.showing().name, "boat.png");
        reel.step(-7);
        assert_eq!(reel.showing().name, "beach.jpg");
        reel.step(0);
        assert_eq!(reel.showing().name, "beach.jpg");
    }

    /// One thing in the reel is a reel where every press stays where it is,
    /// rather than a reel that cannot be walked at all.
    #[test]
    fn a_folder_with_one_picture_in_it_is_a_reel() {
        let listing = vec![("beach.jpg".to_string(), "image/jpeg".to_string())];
        let mut reel = Reel::of(&listing, "beach.jpg").expect("a reel");
        assert_eq!(reel.many(), 1);
        reel.step(1);
        assert_eq!(reel.showing().name, "beach.jpg");
        reel.step(-1);
        assert_eq!(reel.showing().name, "beach.jpg");
    }

    #[test]
    fn a_folder_with_nothing_to_show_is_no_reel_at_all() {
        let listing = vec![("notes.txt".to_string(), "text/plain".to_string())];
        assert_eq!(Reel::of(&listing, "notes.txt"), None);
        assert_eq!(Reel::of(&[], "beach.jpg"), None);
    }

    /// Opening the one file in the folder this cannot show still gives a reel
    /// of the rest, standing at the start. There is a folder to walk.
    #[test]
    fn opening_something_unshowable_still_opens_the_folder() {
        let reel = reel("notes.txt");
        assert_eq!(reel.showing().name, "beach.jpg");
        assert_eq!(reel.which(), 1);
    }

    /// After a file is thrown away the folder is read again, and the person
    /// should be left where they were rather than at the start.
    #[test]
    fn a_reel_read_again_can_be_put_back_where_it_was() {
        let mut reel = reel("beach.jpg");
        assert_eq!(reel.stand_on("swim.mp4"), Stood::OnIt);
        assert_eq!(reel.showing().name, "swim.mp4");
        assert_eq!(reel.stand_on("gone.jpg"), Stood::NotThere);
        assert_eq!(reel.showing().name, "swim.mp4", "left where it was");
    }

    #[test]
    fn which_one_this_is_is_counted_the_way_a_person_says_it() {
        let mut reel = reel("beach.jpg");
        assert_eq!((reel.which(), reel.many()), (1, 3));
        reel.step(2);
        assert_eq!((reel.which(), reel.many()), (3, 3));
    }
}
