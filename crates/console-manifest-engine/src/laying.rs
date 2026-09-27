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
//! stopped resembling each other is the day no one could tell.

use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::unapplied::Unapplied;

pub const STAGED: &str = "console-new";

pub const KEPT: &str = "console-old";

fn beside(live: &Path, ending: &str) -> Result<Option<PathBuf>, Never> {
    let name = match live.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return Ok(None),
    };

    Ok(Some(live.with_file_name(format!("{name}.{ending}"))))
}

pub fn staged(live: &Path) -> Result<Option<PathBuf>, Never> {
    beside(live, STAGED)
}

pub fn backup_path(live: &Path) -> Result<Option<PathBuf>, Never> {
    beside(live, KEPT)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Back {
    Retained,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laid {
    pub at: String,
    pub back: Back,
}

pub fn undoing(laid: &[Laid]) -> Result<Vec<&Laid>, Never> {
    Ok(laid.iter().rev().collect())
}

pub trait Lays {
    fn stage(&mut self, from: &Path, live: &str) -> Result<(), Unapplied>;

    fn swap(&mut self, live: &str) -> Result<Back, Unapplied>;

    fn put_back(&mut self, laid: &Laid) -> Result<(), Unapplied>;

    fn drop_staged(&mut self, live: &str);

    fn drop_kept(&mut self, live: &str);

    fn presence(&self, live: &str) -> Back;

    fn note(&mut self, laid: &[Laid]) -> Result<(), Unapplied>;

    fn forget_note(&mut self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undone {
    pub at: String,
    pub put: Put,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Put {
    Back,
    NotBack(String),
}

#[derive(Debug, Default)]
pub struct Deploy {
    staged: Vec<String>,
    laid: Vec<Laid>,
}

impl Deploy {
    pub fn stage(&mut self, lays: &mut impl Lays, from: &Path, live: &str) -> Result<(), Unapplied> {
        lays.stage(from, live)?;
        self.staged.push(live.to_string());
        Ok(())
    }

    pub fn swap(&mut self, lays: &mut impl Lays) -> Result<Vec<Undone>, Unapplied> {
        let plan: Vec<Laid> = self
            .staged
            .iter()
            .map(|live| Laid { at: live.clone(), back: lays.presence(live) })
            .collect();
        lays.note(&plan)?;

        for live in std::mem::take(&mut self.staged) {
            match lays.swap(&live) {
                Ok(back) => self.laid.push(Laid { at: live, back }),
                Err(fault) => {
                    let Ok(()) = self.abandon(lays);
                    let Ok(put_back) = self.undo(lays);

                    return Err(match put_back.is_empty() {
                        true => fault,
                        false => Unapplied::WentBack(Box::new(fault)),
                    });
                }
            }
        }

        Ok(Vec::new())
    }

    pub fn abandon(&mut self, lays: &mut impl Lays) -> Result<(), Never> {
        for live in std::mem::take(&mut self.staged) {
            lays.drop_staged(&live);
        }

        Ok(())
    }

    pub fn undo(&mut self, lays: &mut impl Lays) -> Result<Vec<Undone>, Never> {
        let laid = std::mem::take(&mut self.laid);
        let Ok(undoing) = undoing(&laid);
        let mut undone: Vec<Undone> = Vec::new();

        for one in undoing {
            let at = one.at.clone();

            let put = match lays.put_back(one) {
                Ok(()) => Put::Back,
                Err(fault) => Put::NotBack(fault.to_string()),
            };

            undone.push(Undone { at, put });
        }

        lays.forget_note();

        Ok(undone)
    }

    pub fn settle(&mut self, lays: &mut impl Lays) -> Result<(), Never> {
        for one in std::mem::take(&mut self.laid) {
            lays.drop_kept(&one.at);
        }

        lays.forget_note();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    #[test]
    fn a_file_waits_and_is_kept_beside_where_it_goes() {
        let live = Path::new("/usr/local/bin/launcher");

        assert_eq!(staged(live), Ok(Some(PathBuf::from("/usr/local/bin/launcher.console-new"))));
        assert_eq!(backup_path(live), Ok(Some(PathBuf::from("/usr/local/bin/launcher.console-old"))));
    }

    #[test]
    fn what_waits_is_in_the_directory_it_is_going_into() -> Result<(), Failure> {
        let live = Path::new("/etc/systemd/user/console-bar.service");
        let Ok(staged) = staged(live);
        let Ok(backup_path) = backup_path(live);
        let staged = staged.ok_or("nowhere to stage it")?;
        let backup_path = backup_path.ok_or("nowhere to keep it")?;

        assert_eq!(staged.parent(), live.parent());
        assert_eq!(backup_path.parent(), live.parent());

        Ok(())
    }

    #[test]
    fn undoing_a_file_that_replaced_nothing_removes_it() {
        let laid = Laid { at: "/usr/local/bin/new-thing".to_string(), back: Back::Closed };

        assert_eq!(undoing(std::slice::from_ref(&laid)), Ok(vec![&laid]));
        assert_eq!(laid.back, Back::Closed);
    }

    #[test]
    fn what_was_there_survives_being_replaced_and_comes_back_the_same_thing() -> Result<(), Failure> {
        use std::os::unix::fs::MetadataExt;

        let here = console_core_temporary_directories::fresh("laying")?;

        let live = here.join("a-program");
        let Ok(staged) = staged(&live);
        let Ok(backup_path) = backup_path(&live);
        let staged = staged.ok_or("nowhere to stage it")?;
        let backup_path = backup_path.ok_or("nowhere to keep it")?;

        console_core_atomic_writes::whole(&live, b"the one that is running")?;

        let before = std::fs::metadata(&live)?;
        let was = before.ino();

        std::fs::hard_link(&live, &backup_path)?;
        console_core_atomic_writes::whole(&staged, b"the new one")?;
        std::fs::rename(&staged, &live)?;

        let now = std::fs::read(&live)?;
        let aside = std::fs::read(&backup_path)?;
        let kept_as = std::fs::metadata(&backup_path)?;

        assert_eq!(now, b"the new one");
        assert_eq!(aside, b"the one that is running");
        assert_eq!(kept_as.ino(), was);

        std::fs::rename(&backup_path, &live)?;

        let back = std::fs::metadata(&live)?;

        assert_eq!(back.ino(), was);

        let _ = std::fs::remove_dir_all(&here);

        Ok(())
    }

    #[test]
    fn no_file_is_laid_down_by_copying_it() -> Result<(), Failure> {
        let machine = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/machine.rs");
        let held = std::fs::read_to_string(machine)?;
        let copies: Vec<&str> = held
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .filter(|line| line.contains("fs::copy"))
            .collect();

        assert!(copies.is_empty(), "a file is laid down by copying it again: {copies:?}");

        Ok(())
    }

    #[derive(Default)]
    struct Paper {
        on: std::collections::BTreeMap<String, String>,
        waiting: std::collections::BTreeMap<String, String>,
        aside: std::collections::BTreeMap<String, String>,
        noted: Vec<Laid>,
        wont_note: bool,
        wont_stage: Vec<String>,
        wont_swap: Vec<String>,
        wont_put_back: Vec<String>,
        asked: Vec<String>,
    }

    impl Paper {
        fn holding(&self, live: &str) -> Result<Option<&str>, Never> {
            Ok(self.on.get(live).map(String::as_str))
        }
    }

    impl Lays for Paper {
        fn stage(&mut self, from: &Path, live: &str) -> Result<(), Unapplied> {
            self.asked.push(format!("stage {live}"));

            match self.wont_stage.iter().any(|which| which == live) {
                true => {
                    return Err(Unapplied::Staging(
                        live.to_string(),
                        "will not stage",
                        std::io::Error::other("the paper machine"),
                    ));
                },
                false => {},
            }

            let held = match from.file_name() {
                Some(named) => named.to_string_lossy().to_string(),
                None => {
                    return Err(Unapplied::Staging(
                        live.to_string(),
                        "names no file",
                        std::io::Error::other("the paper machine"),
                    ));
                },
            };

            self.waiting.insert(live.to_string(), held);

            Ok(())
        }

        fn swap(&mut self, live: &str) -> Result<Back, Unapplied> {
            self.asked.push(format!("swap {live}"));

            match self.wont_swap.iter().any(|which| which == live) {
                true => {
                    return Err(Unapplied::Staging(
                        live.to_string(),
                        "will not go into place",
                        std::io::Error::other("the paper machine"),
                    ));
                },
                false => {},
            }

            let coming = match self.waiting.remove(live) {
                Some(coming) => coming,
                None => {
                    return Err(Unapplied::Staging(
                        live.to_string(),
                        "nothing is staged for it",
                        std::io::Error::other("the paper machine"),
                    ));
                },
            };
            let back = match self.on.insert(live.to_string(), coming) {
                None => Back::Closed,
                Some(was) => {
                    self.aside.insert(live.to_string(), was);

                    Back::Retained
                },
            };

            Ok(back)
        }

        fn put_back(&mut self, laid: &Laid) -> Result<(), Unapplied> {
            self.asked.push(format!("put back {}", laid.at));

            match self.wont_put_back.iter().any(|which| which == &laid.at) {
                true => {
                    return Err(Unapplied::NothingKept(laid.at.clone()));
                },
                false => {},
            }

            match laid.back {
                Back::Retained => {
                    let was = self.aside.remove(&laid.at).ok_or_else(|| Unapplied::NothingKept(laid.at.clone()))?;

                    self.on.insert(laid.at.clone(), was);
                },
                Back::Closed => {
                    self.on.remove(&laid.at);
                },
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

        fn presence(&self, live: &str) -> Back {
            match self.on.contains_key(live) {
                true => Back::Retained,
                false => Back::Closed,
            }
        }

        fn note(&mut self, laid: &[Laid]) -> Result<(), Unapplied> {
            self.asked.push(format!("note {}", laid.len()));

            match self.wont_note {
                true => {
                    return Err(Unapplied::Directory(
                        PathBuf::from("the plan"),
                        std::io::Error::other("the paper machine"),
                    ));
                }
                false => {},
            }

            self.noted = laid.to_vec();

            Ok(())
        }

        fn forget_note(&mut self) {
            self.asked.push("forget note".to_string());
            self.noted.clear();
        }
    }

    fn machine_with(held: &[(&str, &str)]) -> Result<Paper, Never> {
        Ok(Paper {
            on: held.iter().map(|(at, was)| (at.to_string(), was.to_string())).collect(),
            ..Paper::default()
        })
    }

    #[test]
    fn staging_changes_nothing_and_swapping_changes_all_of_it() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.stage(&mut paper, Path::new("/source/new two"), "/bin/two")?;
        assert_eq!(paper.holding("/bin/one"), Ok(Some("old one")));
        assert_eq!(paper.holding("/bin/two"), Ok(Some("old two")));

        deploy.swap(&mut paper)?;
        assert_eq!(paper.holding("/bin/one"), Ok(Some("new one")));
        assert_eq!(paper.holding("/bin/two"), Ok(Some("new two")));

        Ok(())
    }

    #[test]
    fn a_release_that_cannot_be_staged_whole_is_not_laid_down_at_all() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_stage.push("/bin/two".to_string());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        let refused = deploy.stage(&mut paper, Path::new("/source/new two"), "/bin/two");

        assert!(matches!(refused, Err(Unapplied::Staging(..))), "it staged what would not stage: {refused:?}");
        let Ok(()) = deploy.abandon(&mut paper);

        assert_eq!(paper.holding("/bin/one"), Ok(Some("old one")));
        assert_eq!(paper.holding("/bin/two"), Ok(Some("old two")));
        assert!(paper.waiting.is_empty(), "something is still staged: {:?}", paper.waiting);

        Ok(())
    }

    #[test]
    fn a_move_that_will_not_go_puts_back_the_ones_that_did() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_swap.push("/bin/two".to_string());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.stage(&mut paper, Path::new("/source/new two"), "/bin/two")?;
        let fault = match deploy.swap(&mut paper) {
            Ok(_) => return Err(Failure::from("the second went into place")),
            Err(fault) => fault,
        };

        assert!(fault.to_string().contains("went back"), "the fault does not say it went back: {fault}");
        assert_eq!(paper.holding("/bin/one"), Ok(Some("old one")));
        assert_eq!(paper.holding("/bin/two"), Ok(Some("old two")));

        Ok(())
    }

    #[test]
    fn undoing_takes_away_what_replaced_nothing() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.stage(&mut paper, Path::new("/source/brand new"), "/bin/two")?;
        deploy.swap(&mut paper)?;
        assert_eq!(paper.holding("/bin/two"), Ok(Some("brand new")));

        let Ok(_) = deploy.undo(&mut paper);
        assert_eq!(paper.holding("/bin/one"), Ok(Some("old one")));
        assert_eq!(paper.holding("/bin/two"), Ok(None));

        Ok(())
    }

    #[test]
    fn the_undoing_happens_in_the_order_it_was_done_in_reversed() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.stage(&mut paper, Path::new("/source/new two"), "/bin/two")?;
        deploy.swap(&mut paper)?;
        paper.asked.clear();
        let Ok(_) = deploy.undo(&mut paper);

        assert_eq!(paper.asked, ["put back /bin/two", "put back /bin/one", "forget note"]);

        Ok(())
    }

    #[test]
    fn a_file_that_will_not_go_back_does_not_keep_the_others_out_of_place() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_put_back.push("/bin/two".to_string());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.stage(&mut paper, Path::new("/source/new two"), "/bin/two")?;
        deploy.swap(&mut paper)?;

        let Ok(undone) = deploy.undo(&mut paper);

        match undone.as_slice() {
            [refused, back] => {
                assert!(matches!(refused.put, Put::NotBack(_)), "the one that refuses says so");
                assert_eq!(back.put, Put::Back, "the one that can go back went back");
            },
            other => return Err(Failure::from(format!("two were to be undone and {} were", other.len()))),
        }

        assert_eq!(paper.holding("/bin/one"), Ok(Some("old one")));
        assert_eq!(paper.holding("/bin/two"), Ok(Some("new two")));

        Ok(())
    }

    #[test]
    fn settling_lets_go_and_leaves_nothing_to_put_back() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.swap(&mut paper)?;
        let Ok(()) = deploy.settle(&mut paper);

        assert!(paper.aside.is_empty(), "something is still kept: {:?}", paper.aside);
        let Ok(_) = deploy.undo(&mut paper);
        assert_eq!(paper.holding("/bin/one"), Ok(Some("new one")));

        Ok(())
    }

    #[test]
    fn the_plan_is_written_down_before_anything_moves() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.stage(&mut paper, Path::new("/source/brand new"), "/bin/two")?;
        paper.asked.clear();
        deploy.swap(&mut paper)?;

        assert_eq!(paper.asked.first().map(String::as_str), Some("note 2"), "{:?}", paper.asked);
        assert_eq!(
            paper.noted,
            [
                Laid { at: "/bin/one".to_string(), back: Back::Retained },
                Laid { at: "/bin/two".to_string(), back: Back::Closed },
            ]
        );

        Ok(())
    }

    #[test]
    fn the_plan_says_which_files_replaced_something_and_which_replaced_nothing() {
        let Ok(paper) = machine_with(&[("/bin/one", "old one")]);
        assert_eq!(paper.presence("/bin/one"), Back::Retained);
        assert_eq!(paper.presence("/bin/two"), Back::Closed);
    }

    #[test]
    fn a_swap_that_cannot_be_written_down_does_not_happen() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one")]);
        paper.wont_note = true;
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        let refused = deploy.swap(&mut paper);

        assert!(matches!(refused, Err(Unapplied::Directory(..))), "it swapped with no way back: {refused:?}");
        assert_eq!(paper.holding("/bin/one"), Ok(Some("old one")));

        Ok(())
    }

    #[test]
    fn the_note_goes_whether_the_release_stood_up_or_was_put_back() -> Result<(), Failure> {
        let Ok(mut paper) = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();
        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.swap(&mut paper)?;
        let Ok(()) = deploy.settle(&mut paper);
        assert!(paper.asked.contains(&"forget note".to_string()), "{:?}", paper.asked);

        let Ok(mut paper) = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();
        deploy.stage(&mut paper, Path::new("/source/new one"), "/bin/one")?;
        deploy.swap(&mut paper)?;
        paper.asked.clear();
        let Ok(_) = deploy.undo(&mut paper);
        assert!(paper.asked.contains(&"forget note".to_string()), "{:?}", paper.asked);

        Ok(())
    }

    #[test]
    fn an_apply_is_undone_in_the_order_it_was_done_in_reversed() {
        let laid = [
            Laid { at: "/usr/local/bin/one".to_string(), back: Back::Retained },
            Laid { at: "/usr/local/bin/two".to_string(), back: Back::Closed },
        ];
        let Ok(undoing) = undoing(&laid);
        let order: Vec<&str> = undoing.iter().map(|one| one.at.as_str()).collect();

        assert_eq!(order, ["/usr/local/bin/two", "/usr/local/bin/one"]);
    }
}
