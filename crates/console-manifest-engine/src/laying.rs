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

use console_core_never::Never;

pub const STAGED: &str = "console-new";

pub const KEPT: &str = "console-old";

fn beside(live: &Path, ending: &str) -> Result<PathBuf, Never> {
    let name = live.file_name().and_then(|name| name.to_str()).unwrap_or("file");

    Ok(live.with_file_name(format!("{name}.{ending}")))
}

pub fn staged(live: &Path) -> Result<PathBuf, Never> {
    beside(live, STAGED)
}

pub fn kept(live: &Path) -> Result<PathBuf, Never> {
    beside(live, KEPT)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Back {
    Kept,
    Gone,
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
    fn stage(&mut self, from: &Path, live: &str) -> Result<(), String>;

    fn swap(&mut self, live: &str) -> Result<Back, String>;

    fn put_back(&mut self, laid: &Laid) -> Result<(), String>;

    fn drop_staged(&mut self, live: &str);

    fn drop_kept(&mut self, live: &str);

    fn standing(&self, live: &str) -> Back;

    fn note(&mut self, laid: &[Laid]) -> Result<(), String>;

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
    pub fn stage(&mut self, lays: &mut impl Lays, from: &Path, live: &str) -> Result<(), String> {
        lays.stage(from, live)?;
        self.staged.push(live.to_string());
        Ok(())
    }

    pub fn swap(&mut self, lays: &mut impl Lays) -> Result<Vec<Undone>, String> {
        let plan: Vec<Laid> = self
            .staged
            .iter()
            .map(|live| Laid { at: live.clone(), back: lays.standing(live) })
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
                        false => format!("{fault} (and what was already down went back)"),
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
        let undone: Vec<Undone> = undoing
            .into_iter()
            .map(|one| Undone {
                at: one.at.clone(),
                put: match lays.put_back(one) {
                    Ok(()) => Put::Back,
                    Err(fault) => Put::NotBack(fault),
                },
            })
            .collect();

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

    fn staged(live: &Path) -> PathBuf {
        let Ok(staged) = super::staged(live);

        staged
    }

    fn kept(live: &Path) -> PathBuf {
        let Ok(kept) = super::kept(live);

        kept
    }

    fn undoing(laid: &[Laid]) -> Vec<&Laid> {
        let Ok(undoing) = super::undoing(laid);

        undoing
    }

    #[test]
    fn a_file_waits_and_is_kept_beside_where_it_goes() {
        let live = Path::new("/usr/local/bin/launcher");
        assert_eq!(staged(live), Path::new("/usr/local/bin/launcher.console-new"));
        assert_eq!(kept(live), Path::new("/usr/local/bin/launcher.console-old"));
    }

    #[test]
    fn what_waits_is_in_the_directory_it_is_going_into() {
        let live = Path::new("/etc/systemd/user/console-bar.service");
        assert_eq!(staged(live).parent(), live.parent());
        assert_eq!(kept(live).parent(), live.parent());
    }

    #[test]
    fn undoing_a_file_that_replaced_nothing_removes_it() {
        let laid = Laid { at: "/usr/local/bin/new-thing".into(), back: Back::Gone };
        assert_eq!(undoing(std::slice::from_ref(&laid)), vec![&laid]);
        assert_eq!(laid.back, Back::Gone);
    }

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

        assert_eq!(std::fs::read(&live).unwrap(), b"the new one");
        assert_eq!(std::fs::read(kept(&live)).unwrap(), b"the one that is running");
        assert_eq!(std::fs::metadata(kept(&live)).unwrap().ino(), was);

        std::fs::rename(kept(&live), &live).expect("putting it back");
        assert_eq!(std::fs::metadata(&live).unwrap().ino(), was);

        std::fs::remove_dir_all(&here).ok();
    }

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
        fn holding(&self, live: &str) -> Option<&str> {
            self.on.get(live).map(String::as_str)
        }
    }

    impl Lays for Paper {
        fn stage(&mut self, from: &Path, live: &str) -> Result<(), String> {
            self.asked.push(format!("stage {live}"));
            match self.wont_stage.iter().any(|which| which == live) {
                true => {
                    return Err(format!("{live}: will not stage"));
                }
                false => {},
            }
            let held = from.file_name().unwrap().to_string_lossy().to_string();
            self.waiting.insert(live.to_string(), held);
            Ok(())
        }

        fn swap(&mut self, live: &str) -> Result<Back, String> {
            self.asked.push(format!("swap {live}"));
            match self.wont_swap.iter().any(|which| which == live) {
                true => {
                    return Err(format!("{live}: will not go into place"));
                }
                false => {},
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
            match self.wont_put_back.iter().any(|which| which == &laid.at) {
                true => {
                    return Err(format!("{}: will not go back", laid.at));
                }
                false => {},
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

        fn standing(&self, live: &str) -> Back {
            match self.on.contains_key(live) {
                true => Back::Kept,
                false => Back::Gone,
            }
        }

        fn note(&mut self, laid: &[Laid]) -> Result<(), String> {
            self.asked.push(format!("note {}", laid.len()));

            match self.wont_note {
                true => return Err("the plan could not be written down".to_string()),
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

    fn from(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!("/source/{name}"))
    }

    fn machine_with(held: &[(&str, &str)]) -> Paper {
        Paper {
            on: held.iter().map(|(at, was)| (at.to_string(), was.to_string())).collect(),
            ..Paper::default()
        }
    }

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

    #[test]
    fn a_release_that_cannot_be_staged_whole_is_not_laid_down_at_all() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_stage.push("/bin/two".into());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        assert!(deploy.stage(&mut paper, &from("new two"), "/bin/two").is_err());
        let Ok(()) = deploy.abandon(&mut paper);

        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), Some("old two"));
        assert!(paper.waiting.is_empty(), "something is still staged: {:?}", paper.waiting);
    }

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

    #[test]
    fn undoing_takes_away_what_replaced_nothing() {
        let mut paper = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("brand new"), "/bin/two").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        assert_eq!(paper.holding("/bin/two"), Some("brand new"));

        let Ok(_) = deploy.undo(&mut paper);
        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), None);
    }

    #[test]
    fn the_undoing_happens_in_the_order_it_was_done_in_reversed() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("new two"), "/bin/two").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        paper.asked.clear();
        let Ok(_) = deploy.undo(&mut paper);

        assert_eq!(paper.asked, ["put back /bin/two", "put back /bin/one", "forget note"]);
    }

    #[test]
    fn a_file_that_will_not_go_back_does_not_keep_the_others_out_of_place() {
        let mut paper = machine_with(&[("/bin/one", "old one"), ("/bin/two", "old two")]);
        paper.wont_put_back.push("/bin/two".into());
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("new two"), "/bin/two").expect("staged");
        deploy.swap(&mut paper).expect("swapped");

        let Ok(undone) = deploy.undo(&mut paper);
        assert_eq!(undone.len(), 2);
        assert!(matches!(undone[0].put, Put::NotBack(_)), "the one that refuses says so");
        assert_eq!(undone[1].put, Put::Back, "the one that can go back went back");
        assert_eq!(paper.holding("/bin/one"), Some("old one"));
        assert_eq!(paper.holding("/bin/two"), Some("new two"));
    }

    #[test]
    fn settling_lets_go_and_leaves_nothing_to_put_back() {
        let mut paper = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        let Ok(()) = deploy.settle(&mut paper);

        assert!(paper.aside.is_empty(), "something is still kept: {:?}", paper.aside);
        let Ok(_) = deploy.undo(&mut paper);
        assert_eq!(paper.holding("/bin/one"), Some("new one"));
    }

    #[test]
    fn the_plan_is_written_down_before_anything_moves() {
        let mut paper = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.stage(&mut paper, &from("brand new"), "/bin/two").expect("staged");
        paper.asked.clear();
        deploy.swap(&mut paper).expect("swapped");

        assert_eq!(paper.asked.first().map(String::as_str), Some("note 2"), "{:?}", paper.asked);
        assert_eq!(
            paper.noted,
            [
                Laid { at: "/bin/one".into(), back: Back::Kept },
                Laid { at: "/bin/two".into(), back: Back::Gone },
            ]
        );
    }

    #[test]
    fn the_plan_says_which_files_replaced_something_and_which_replaced_nothing() {
        let paper = machine_with(&[("/bin/one", "old one")]);
        assert_eq!(paper.standing("/bin/one"), Back::Kept);
        assert_eq!(paper.standing("/bin/two"), Back::Gone);
    }

    #[test]
    fn a_swap_that_cannot_be_written_down_does_not_happen() {
        let mut paper = machine_with(&[("/bin/one", "old one")]);
        paper.wont_note = true;
        let mut deploy = Deploy::default();

        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        assert!(deploy.swap(&mut paper).is_err(), "it swapped with no way back");
        assert_eq!(paper.holding("/bin/one"), Some("old one"));
    }

    #[test]
    fn the_note_goes_whether_the_release_stood_up_or_was_put_back() {
        let mut paper = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();
        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        let Ok(()) = deploy.settle(&mut paper);
        assert!(paper.asked.contains(&"forget note".to_string()), "{:?}", paper.asked);

        let mut paper = machine_with(&[("/bin/one", "old one")]);
        let mut deploy = Deploy::default();
        deploy.stage(&mut paper, &from("new one"), "/bin/one").expect("staged");
        deploy.swap(&mut paper).expect("swapped");
        paper.asked.clear();
        let Ok(_) = deploy.undo(&mut paper);
        assert!(paper.asked.contains(&"forget note".to_string()), "{:?}", paper.asked);
    }

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
