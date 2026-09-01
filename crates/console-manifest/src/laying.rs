//! Laying a file down in two halves, so that a deploy has a moment it happens.
//!
//! An apply used to write each file into place as it worked it out: read the
//! source, fill in the marks, and move it over the live one, then on to the
//! next. Every file arrived atomically -- it is written beside its live name
//! and renamed over it, and a rename either happened or did not -- but the set
//! of them did not. Between the first and the last there is a machine running
//! some of one release and some of another, and if the eleventh cannot be
//! written the ten before it are already there with nothing to say so.
//!
//! So it is two halves. Everything is staged first, beside where it goes, and
//! nothing is moved until all of it staged. Then the moves happen one after
//! another with no work between them, which is as close to one moment as a
//! filesystem offers.
//!
//! The old copy is kept by linking rather than by copying, and that is not an
//! optimisation. A hard link is the same inode under a second name, so the
//! program a running service is executing goes on being the program it is
//! executing, and putting it back is another rename rather than a restore. A
//! copy would be a second file that merely resembles it, and the day the two
//! stopped resembling each other is the day nobody could tell.

use std::path::{Path, PathBuf};

/// What a file is called while it is staged and not yet in place.
///
/// Beside where it goes rather than in a directory of its own, because a
/// rename across filesystems is not a rename: it is a copy and a delete, and
/// the whole of what makes this safe is that it is neither.
pub const STAGED: &str = "console-new";

/// What the file that was there is called once something is over it.
pub const KEPT: &str = "console-old";

/// One of those names, for a path.
fn beside(live: &Path, ending: &str) -> PathBuf {
    let name = live.file_name().and_then(|name| name.to_str()).unwrap_or("file");
    live.with_file_name(format!("{name}.{ending}"))
}

/// Where a file waits between being written and being put in place.
pub fn staged(live: &Path) -> PathBuf {
    beside(live, STAGED)
}

/// Where the file it replaces is kept, in case it has to go back.
pub fn kept(live: &Path) -> PathBuf {
    beside(live, KEPT)
}

/// What putting one file back means.
///
/// Two different acts, and telling them apart is the whole of why the plan is
/// written down rather than worked out at the time. A file that replaced
/// another goes back to being that other one. A file that replaced nothing was
/// not there before this apply, and putting it back means it is not there now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Back {
    /// There was a file here, it is kept, and it goes over this one again.
    Kept,
    /// There was nothing here. This is removed.
    Gone,
}

/// One file this apply laid down, and how to undo it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laid {
    /// The live path, as the machine has it.
    pub at: String,
    pub back: Back,
}

/// How to undo an apply, given what it laid down.
///
/// Backwards, because a later file may sit in a directory an earlier one made,
/// and because the order things were done in is the only order anybody can
/// hold in their head when reading what happened.
pub fn undoing(laid: &[Laid]) -> Vec<&Laid> {
    laid.iter().rev().collect()
}

/// What laying a release down needs of a machine.
///
/// A trait so that the order these happen in can be held to without a machine
/// to happen on. What `Deploy` decides -- everything staged before anything
/// moves, what is undone and in which direction -- is the half worth being sure
/// of, and it is the half that could not be asked a question before this,
/// because it called straight into code that writes to `/` and wants to be
/// root.
///
/// That mattered most for the undoing. The undoing is what runs when a deploy
/// has already gone wrong, so it is the least exercised thing here and the
/// worst one to be wrong about: an undo nobody has ever watched is worse than
/// no undo at all, because a machine with one is trusted.
pub trait Lays {
    /// Write a file beside where it goes.
    fn stage(&mut self, from: &Path, live: &str) -> Result<(), String>;
    /// Move a staged file over the live one, keeping what was there.
    fn swap(&mut self, live: &str) -> Result<Back, String>;
    /// Put one file back the way it was.
    fn put_back(&mut self, laid: &Laid) -> Result<(), String>;
    /// Throw away a staged file that is not going to be used.
    fn drop_staged(&mut self, live: &str);
    /// Throw away the copy kept in case a file had to go back.
    fn drop_kept(&mut self, live: &str);
}

/// One file put back, and whether it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undone {
    pub at: String,
    pub fault: Option<String>,
}

/// One release, staged and then put in place.
///
/// An apply used to lay each file down as it worked it out. Every file arrived
/// atomically, but the set of them did not: between the first and the last
/// there is a machine running some of one release and some of another, and a
/// file that could not be written left the ones before it already in place with
/// nothing recording that they were.
///
/// So this holds the two halves apart. Everything is staged beside where it
/// goes, and nothing moves until all of it staged. Then the moves happen one
/// after another with no work between them, which is as close to one moment as
/// a filesystem offers. What each move replaced is kept, so the whole of it can
/// go back if what came up does not run.
#[derive(Debug, Default)]
pub struct Deploy {
    /// Live paths written beside their names and not yet moved over them.
    staged: Vec<String>,
    /// What has been moved into place, in the order it was, and how to undo it.
    laid: Vec<Laid>,
}

impl Deploy {
    /// Write one file beside where it goes.
    pub fn stage(&mut self, lays: &mut impl Lays, from: &Path, live: &str) -> Result<(), String> {
        lays.stage(from, live)?;
        self.staged.push(live.to_string());
        Ok(())
    }

    /// Move all of it into place.
    ///
    /// Nothing between the moves, on purpose. Every read, every decision and
    /// every fallible thing has already happened; what is left is renames,
    /// which is the shortest this can be made.
    ///
    /// A move that will not go undoes the ones before it. Half a release put
    /// down is the state this whole arrangement exists to make impossible, and
    /// it is not made better by being the state the failure leaves behind.
    pub fn swap(&mut self, lays: &mut impl Lays) -> Result<Vec<Undone>, String> {
        for live in std::mem::take(&mut self.staged) {
            match lays.swap(&live) {
                Ok(back) => self.laid.push(Laid { at: live, back }),
                Err(fault) => {
                    self.abandon(lays);
                    let put_back = self.undo(lays);
                    return Err(match put_back.is_empty() {
                        true => fault,
                        false => format!("{fault} (and what was already down went back)"),
                    });
                }
            }
        }
        Ok(Vec::new())
    }

    /// Throw away what was staged and is not going to be used.
    pub fn abandon(&mut self, lays: &mut impl Lays) {
        for live in std::mem::take(&mut self.staged) {
            lays.drop_staged(&live);
        }
    }

    /// Put the machine back the way it was.
    ///
    /// Backwards, because a file may sit in a directory a file before it made,
    /// and because the order things were done in is the only order anybody can
    /// hold in their head while reading what happened.
    ///
    /// Every one is tried even where one fails. Stopping at the first would
    /// leave a machine half undone, which is the state this exists to get out
    /// of rather than a second version of it.
    pub fn undo(&mut self, lays: &mut impl Lays) -> Vec<Undone> {
        let laid = std::mem::take(&mut self.laid);
        undoing(&laid)
            .into_iter()
            .map(|one| Undone { at: one.at.clone(), fault: lays.put_back(one).err() })
            .collect()
    }

    /// Let go of what was kept, the release having stood up.
    pub fn settle(&mut self, lays: &mut impl Lays) {
        for one in std::mem::take(&mut self.laid) {
            lays.drop_kept(&one.at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_waits_and_is_kept_beside_where_it_goes() {
        let live = Path::new("/usr/local/bin/launcher");
        assert_eq!(staged(live), Path::new("/usr/local/bin/launcher.console-new"));
        assert_eq!(kept(live), Path::new("/usr/local/bin/launcher.console-old"));
    }

    /// Beside it, and not under a directory of this engine's own. A rename
    /// across filesystems is a copy and a delete, and a copy of a program a
    /// service is running is not the program it is running.
    #[test]
    fn what_waits_is_in_the_directory_it_is_going_into() {
        let live = Path::new("/etc/systemd/user/console-bar.service");
        assert_eq!(staged(live).parent(), live.parent());
        assert_eq!(kept(live).parent(), live.parent());
    }

    /// A file that replaced nothing is not put back, it is taken away. Undone
    /// the other way, an apply that failed halfway would leave behind the
    /// programs it had already installed, which is the state it is undoing.
    #[test]
    fn undoing_a_file_that_replaced_nothing_removes_it() {
        let laid = Laid { at: "/usr/local/bin/new-thing".into(), back: Back::Gone };
        assert_eq!(undoing(std::slice::from_ref(&laid)), vec![&laid]);
        assert_eq!(laid.back, Back::Gone);
    }

    /// The property the whole arrangement rests on, held to the filesystem
    /// rather than to a comment.
    ///
    /// A service executing a program goes on executing it while that program
    /// is replaced, because a rename replaces a name and the inode lives on
    /// under whoever still holds it. Linking the old one aside is what keeps
    /// hold of it, so putting it back is the same inode and not a copy that
    /// resembles it.
    ///
    /// Written as a test because the day somebody reaches for `fs::copy` here
    /// -- and it looks like the obvious thing -- everything still passes and a
    /// daemon dies mid-apply six months later.
    #[test]
    fn what_was_there_survives_being_replaced_and_comes_back_the_same_thing() {
        use std::os::unix::fs::MetadataExt;

        let here = std::env::temp_dir().join(format!("console-laying-{}", std::process::id()));
        std::fs::create_dir_all(&here).expect("somewhere to work");
        let live = here.join("a-program");
        std::fs::write(&live, b"the one that is running").expect("the old one");
        let was = std::fs::metadata(&live).expect("its inode").ino();

        std::fs::hard_link(&live, kept(&live)).expect("keeping it");
        std::fs::write(staged(&live), b"the new one").expect("the new one");
        std::fs::rename(staged(&live), &live).expect("putting it in place");

        // The name is the new program; what was running is still there, and
        // still the same file rather than a likeness of it.
        assert_eq!(std::fs::read(&live).unwrap(), b"the new one");
        assert_eq!(std::fs::read(kept(&live)).unwrap(), b"the one that is running");
        assert_eq!(std::fs::metadata(kept(&live)).unwrap().ino(), was);

        // And putting it back is a rename, so it is that same file again.
        std::fs::rename(kept(&live), &live).expect("putting it back");
        assert_eq!(std::fs::metadata(&live).unwrap().ino(), was);

        std::fs::remove_dir_all(&here).ok();
    }

    /// Nothing here copies a file into place, and this is the guard on that.
    ///
    /// `fs::copy` writes through an existing inode where one is there, which
    /// is the one thing that reaches inside a running program: the bytes under
    /// a daemon change while it is executing them. Every laying down here is a
    /// write beside and a rename over, and a change that quietly became a copy
    /// would pass every other test in this file.
    #[test]
    fn no_file_is_laid_down_by_copying_it() {
        let machine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/machine.rs");
        let held = std::fs::read_to_string(machine).expect("the machine half");
        let copies: Vec<&str> = held
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .filter(|line| line.contains("fs::copy"))
            .collect();
        assert!(copies.is_empty(), "a file is laid down by copying it again: {copies:?}");
    }

    /// A machine that is not one, so the order a release is laid down in can
    /// be asked about without root and without a device.
    #[derive(Default)]
    struct Paper {
        /// What is on it, live path to content.
        on: std::collections::BTreeMap<String, String>,
        /// What is staged beside a live path, not yet over it.
        waiting: std::collections::BTreeMap<String, String>,
        /// What was there before something went over it.
        aside: std::collections::BTreeMap<String, String>,
        /// Paths this machine refuses, and at which half.
        wont_stage: Vec<String>,
        wont_swap: Vec<String>,
        wont_put_back: Vec<String>,
        /// Everything asked of it, in order.
        asked: Vec<String>,
    }

    impl Paper {
        fn holding(&self, live: &str) -> Option<&str> {
            self.on.get(live).map(String::as_str)
        }
    }

    impl Lays for Paper {
        fn stage(&mut self, from: &Path, live: &str) -> Result<(), String> {
            self.asked.push(format!("stage {live}"));
            if self.wont_stage.iter().any(|which| which == live) {
                return Err(format!("{live}: will not stage"));
            }
            let held = from.file_name().unwrap().to_string_lossy().to_string();
            self.waiting.insert(live.to_string(), held);
            Ok(())
        }

        fn swap(&mut self, live: &str) -> Result<Back, String> {
            self.asked.push(format!("swap {live}"));
            if self.wont_swap.iter().any(|which| which == live) {
                return Err(format!("{live}: will not go into place"));
            }
            let coming = self.waiting.remove(live).expect("something staged");
            let back = match self.on.insert(live.to_string(), coming) {
                None => Back::Gone,
                Some(was) => {
                    self.aside.insert(live.to_string(), was);
                    Back::Kept
                }
            };
            Ok(back)
        }

        fn put_back(&mut self, laid: &Laid) -> Result<(), String> {
            self.asked.push(format!("put back {}", laid.at));
            if self.wont_put_back.iter().any(|which| which == &laid.at) {
                return Err(format!("{}: will not go back", laid.at));
            }
            match laid.back {
                Back::Kept => {
                    let was = self.aside.remove(&laid.at).expect("something kept");
                    self.on.insert(laid.at.clone(), was);
                }
                Back::Gone => {
                    self.on.remove(&laid.at);
                }
            }
            Ok(())
        }

        fn drop_staged(&mut self, live: &str) {
            self.asked.push(format!("drop staged {live}"));
            self.waiting.remove(live);
        }

        fn drop_kept(&mut self, live: &str) {
            self.asked.push(format!("drop kept {live}"));
            self.aside.remove(live);
        }
    }

    fn from(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!("/source/{name}"))
    }

    fn machine_with(held: &[(&str, &str)]) -> Paper {
        Paper {
            on: held.iter().map(|(at, was)| (at.to_string(), was.to_string())).collect(),
            ..Paper::default()
        }
    }

    /// Nothing moves until all of it is staged. The whole point, held to.
    #[test]
    fn staging_changes_nothing_and_swapping_changes_all_of_it() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("new two"), "/bin/two").expect("staged");
        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), Some("old two"));

        deploy.swap(&mut paper).expect("swapped");
        assert_eq!(paper.holding("/bin/one"), Some("new one"));
        assert_eq!(paper.holding("/bin/two"), Some("new two"));
    }

    /// A file that will not stage leaves the machine exactly as it was, and
    /// the caller throws away what was staged before it.
    #[test]
    fn a_release_that_cannot_be_staged_whole_is_not_laid_down_at_all() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_stage.push("/bin/two".into());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        assert!(deploy.stage(&mut paper, &from("new two"), "/bin/two").is_err());
        deploy.abandon(&mut paper);

        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), Some("old two"));
        assert!(paper.waiting.is_empty(), "something is still staged: {:?}", paper.waiting);
    }

    /// The undoing, which is the thing that only runs when a deploy has already
    /// gone wrong and so is the one nobody watches. A move that will not go
    /// puts back the ones that did.
    #[test]
    fn a_move_that_will_not_go_puts_back_the_ones_that_did() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_swap.push("/bin/two".into());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("new two"), "/bin/two").expect("staged");
        let fault = deploy.swap(&mut paper).expect_err("the second will not go");

        assert!(fault.contains("went back"), "the fault does not say it went back: {fault}");
        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), Some("old two"));
    }

    /// A file this release put where there was none is taken away rather than
    /// put back. Undone the other way it would be left behind, which is the
    /// half-release the undoing exists to clear.
    #[test]
    fn undoing_takes_away_what_replaced_nothing() {
        let mut paper = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("brand new"), "/bin/two").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        assert_eq!(paper.holding("/bin/two"), Some("brand new"));

        deploy.undo(&mut paper);
        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), None);
    }

    /// Backwards, because a file may sit in a directory a file before it made.
    #[test]
    fn the_undoing_happens_in_the_order_it_was_done_in_reversed() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("new two"), "/bin/two").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        paper.asked.clear();
        deploy.undo(&mut paper);

        assert_eq!(paper.asked, ["put back /bin/two", "put back /bin/one"]);
    }

    /// One file refusing to go back does not stop the rest going back. Stopping
    /// at the first would leave a machine half undone, which is the state this
    /// is getting out of rather than a second version of it.
    #[test]
    fn a_file_that_will_not_go_back_does_not_keep_the_others_out_of_place() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_put_back.push("/bin/two".into());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("new two"), "/bin/two").expect("staged");
        deploy.swap(&mut paper).expect("swapped");

        let undone = deploy.undo(&mut paper);
        assert_eq!(undone.len(), 2);
        assert!(undone[0].fault.is_some(), "the one that refuses says so");
        assert!(undone[1].fault.is_none(), "the one that can go back went back");
        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), Some("new two"));
    }

    /// Settling lets go of what was kept, and after it there is nothing to undo
    /// with. A release that has stood up is not one to be walked back on the
    /// next failure of something else.
    #[test]
    fn settling_lets_go_and_leaves_nothing_to_put_back() {
        let mut paper = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        deploy.settle(&mut paper);

        assert!(paper.aside.is_empty(), "something is still kept: {:?}", paper.aside);
        deploy.undo(&mut paper);
        assert_eq!(paper.holding("/bin/one"), Some("new one"));
    }

    /// Backwards, so a file is put back before the directory it needed.
    #[test]
    fn an_apply_is_undone_in_the_order_it_was_done_in_reversed() {
        let laid = [
            Laid { at: "/usr/local/bin/one".into(), back: Back::Kept },
            Laid { at: "/usr/local/bin/two".into(), back: Back::Gone },
        ];
        let order: Vec<&str> = undoing(&laid).iter().map(|one| one.at.as_str()).collect();
        assert_eq!(order, ["/usr/local/bin/two", "/usr/local/bin/one"]);
    }
}
