//! Where a person's things are, and where the machine keeps what everybody shares.
//!
//! Three crates each wrote out the directories a `.desktop` file can be in and
//! the three lists had drifted apart. Two of them read `XDG_DATA_DIRS` and the
//! third spelled out three directories, so a machine that set the variable
//! offered a different set of applications on the settings tab than in the
//! menu -- one fault with no argument behind it, because nobody had decided
//! twice, they had written the same thing twice and only fixed one.
//!
//! Where a person's home is had gone the same way, and worse: of the crates
//! that worked it out, two answered `/root` when `HOME` was unset. That is the
//! one nobody would defend out loud. It does not fail, it succeeds against the
//! wrong person's dotfiles, and on a machine where the desktop belongs to
//! somebody who is not root it reads settings nobody wrote and writes settings
//! nobody will find. So `home` is an `Option` here and absent stays absent:
//! meeting the `None` is the caller saying what it does without a home, which
//! is usually nothing, and nothing is the right amount.
//!
//! ## The four bases, and what a caller may say about one
//!
//! The same drift a third time, in the base directories themselves. Where this
//! desktop keeps a person's settings, what it remembers, what it holds for her
//! and what it can make again was spelled out in a dozen crates, each joining
//! its own filename onto its own guess, and the guesses disagreed about the
//! two questions that matter. An unset `HOME` sent two of them to `/tmp`, which
//! is `/root` again with a different address: a model downloaded there is a
//! download nobody will find twice, and a directory there is one anybody can
//! stand in front of. And an `XDG_*_HOME` that is set to nothing was a
//! directory to two of them and no directory to three, so the same empty
//! variable put a panel's notes in the person's home in one program and in
//! whatever directory it happened to be started from in another.
//!
//! [`Base`] is the four of them, each knowing what the environment calls it and
//! what it is when nothing is said. The standard's own rule settles the second
//! question and does it without a case of its own: a value that is not an
//! absolute path is not a directory, so it is ignored and the usual one is
//! used. Empty is not absolute, and neither is the relative path somebody meant
//! to make absolute.
//!
//! `ours` is the desktop's own directory under a base, and it is one word --
//! [`OURS`] -- rather than four spellings of `.config/console`. What goes in it
//! is the owning crate's to name: this crate hands back the directory and has
//! never heard of `scale`, `buttons.toml` or `waited.jsonl`.
//!
//! `ours_under` is the same answer for a home that is not this process's -- the
//! device over ssh, a stage tree standing here -- and it reads no variable,
//! because the environment on this side of the wire is not the one that will
//! open the file. Every caller of it names a machine rather than a process.
//!
//! What is here is the standard's own vocabulary and no more than that -- the
//! base directories, and `applications`, because the desktop entry
//! specification is what names that directory rather than this desktop. What a
//! `.desktop` file *means* is `console-applications`, and it is a caller.
//! `XDG_RUNTIME_DIR` is deliberately not one of these: it is not under a home,
//! it is not what a home is missing when `HOME` is unset, and the one crate
//! that wants a socket path is not asking this question.

use std::path::{Path, PathBuf};

use console_core_never::Never;

pub const DATA: &str = "/usr/local/share:/usr/share";

pub const APPLICATIONS: &str = "applications";

pub const OURS: &str = "console";

fn said(name: &str) -> Result<Option<String>, Never> {
    Ok(match std::env::var(name) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("{name}: {fault}");

            None
        }
    })
}

pub fn home() -> Result<Option<PathBuf>, Never> {
    let said = said("HOME")?;

    Ok(said.map(PathBuf::from))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Base {
    Config,
    State,
    Share,
    Cache,
}

impl Base {
    pub const EVERY: [Base; 4] = [Base::Config, Base::State, Base::Share, Base::Cache];

    pub fn called(self) -> Result<&'static str, Never> {
        Ok(match self {
            Base::Config => "XDG_CONFIG_HOME",
            Base::State => "XDG_STATE_HOME",
            Base::Share => "XDG_DATA_HOME",
            Base::Cache => "XDG_CACHE_HOME",
        })
    }

    pub fn usual(self) -> Result<&'static str, Never> {
        Ok(match self {
            Base::Config => ".config",
            Base::State => ".local/state",
            Base::Share => ".local/share",
            Base::Cache => ".cache",
        })
    }

    pub fn told(self, home: Option<&Path>, said: Option<&str>) -> Result<Option<PathBuf>, Never> {
        let told = said.map(Path::new).filter(|at| at.is_absolute());

        let Ok(usual) = self.usual();

        Ok(match told {
            Some(told) => Some(told.to_path_buf()),
            None => home.map(|home| home.join(usual)),
        })
    }

    pub fn hers(self) -> Result<Option<PathBuf>, Never> {
        let home = home()?;

        let Ok(called) = self.called();

        let said = said(called)?;

        self.told(home.as_deref(), said.as_deref())
    }

    pub fn ours(self) -> Result<Option<PathBuf>, Never> {
        let hers = self.hers()?;

        Ok(hers.map(|at| at.join(OURS)))
    }

    pub fn under(self, home: &Path) -> Result<PathBuf, Never> {
        let Ok(usual) = self.usual();

        Ok(home.join(usual))
    }

    pub fn ours_under(self, home: &Path) -> Result<PathBuf, Never> {
        let under = self.under(home)?;

        Ok(under.join(OURS))
    }
}

pub fn data_under(
    home: Option<&Path>,
    mine: Option<&str>,
    shared: Option<&str>,
) -> Result<Vec<PathBuf>, Never> {
    let mine = Base::Share.told(home, mine)?;

    let mut every: Vec<PathBuf> = mine.into_iter().collect();

    let shared = shared.filter(|said| !said.is_empty()).unwrap_or(DATA);

    every.extend(shared.split(':').filter(|at| !at.is_empty()).map(PathBuf::from));

    Ok(every)
}

pub fn data() -> Result<Vec<PathBuf>, Never> {
    let home = home()?;

    let Ok(called) = Base::Share.called();

    let mine = said(called)?;

    let shared = said("XDG_DATA_DIRS")?;

    data_under(home.as_deref(), mine.as_deref(), shared.as_deref())
}

pub fn applications_under(data: &[PathBuf]) -> Result<Vec<PathBuf>, Never> {
    Ok(data.iter().map(|at| at.join(APPLICATIONS)).collect())
}

pub fn applications() -> Result<Vec<PathBuf>, Never> {
    let data = data()?;

    applications_under(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn hers() -> PathBuf {
        PathBuf::from("/home/somebody")
    }

    #[test]
    fn a_persons_own_share_comes_before_the_machines() {
        let every = ok(data_under(Some(&hers()), None, Some("/usr/share")));

        assert_eq!(
            every,
            vec![PathBuf::from("/home/somebody/.local/share"), PathBuf::from("/usr/share")],
            "hers is first because hers is the one that wins"
        );
    }

    #[test]
    fn nothing_is_said_and_the_standards_answer_is_used() {
        let every = ok(data_under(Some(&hers()), None, None));

        assert_eq!(
            every,
            vec![
                PathBuf::from("/home/somebody/.local/share"),
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        );
    }

    #[test]
    fn an_empty_saying_is_nothing_said() {
        assert_eq!(ok(data_under(None, None, Some(""))), ok(data_under(None, None, None)));
    }

    #[test]
    fn no_home_takes_a_place_away_rather_than_inventing_one() {
        let every = ok(data_under(None, None, Some("/usr/share")));

        assert_eq!(every, vec![PathBuf::from("/usr/share")]);
        assert!(
            !every.iter().any(|at| at.starts_with("/root")),
            "an absent home is absent, not root's"
        );
    }

    #[test]
    fn an_empty_directory_in_the_middle_is_not_a_directory() {
        let every = ok(data_under(None, None, Some("/usr/local/share::/usr/share")));

        assert_eq!(
            every,
            vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]
        );
    }

    #[test]
    fn where_a_person_keeps_her_own_share_is_hers_to_say() {
        let every = ok(data_under(Some(&hers()), Some("/elsewhere/share"), Some("/usr/share")));

        assert_eq!(
            every,
            vec![PathBuf::from("/elsewhere/share"), PathBuf::from("/usr/share")]
        );
    }

    #[test]
    fn every_data_directory_holds_the_applications_in_it() {
        let data = ok(data_under(Some(&hers()), None, Some("/usr/share")));
        let every = ok(applications_under(&data));

        assert_eq!(
            every,
            vec![
                PathBuf::from("/home/somebody/.local/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
    }

    #[test]
    fn each_base_is_where_the_standard_says_it_is() {
        let every: Vec<PathBuf> = Base::EVERY
            .into_iter()
            .filter_map(|base| ok(base.told(Some(&hers()), None)))
            .collect();

        assert_eq!(
            every,
            vec![
                PathBuf::from("/home/somebody/.config"),
                PathBuf::from("/home/somebody/.local/state"),
                PathBuf::from("/home/somebody/.local/share"),
                PathBuf::from("/home/somebody/.cache"),
            ]
        );
    }

    #[test]
    fn a_base_that_is_said_is_taken_as_it_is() {
        let at = ok(Base::Config.told(Some(&hers()), Some("/elsewhere/config")));

        assert_eq!(at, Some(PathBuf::from("/elsewhere/config")));
    }

    #[test]
    fn a_saying_that_is_not_a_directory_is_not_a_saying() {
        for said in ["", "config", "./config", "~/config"] {
            let at = ok(Base::Cache.told(Some(&hers()), Some(said)));

            assert_eq!(
                at,
                Some(PathBuf::from("/home/somebody/.cache")),
                "{said:?} is not an absolute path and cannot be a base"
            );
        }
    }

    #[test]
    fn no_home_and_nothing_said_is_no_base_rather_than_a_relative_one() {
        let every: Vec<Option<PathBuf>> =
            Base::EVERY.into_iter().map(|base| ok(base.told(None, None))).collect();

        assert_eq!(every, vec![None, None, None, None], "a base under nothing is nothing");
    }

    #[test]
    fn a_base_can_be_answered_for_a_home_that_is_not_ours() {
        let at = ok(Base::State.ours_under(Path::new("/home/elsewhere")));

        assert_eq!(at, PathBuf::from("/home/elsewhere/.local/state/console"));
    }

    #[test]
    fn what_this_desktop_keeps_is_one_word_under_each_base() {
        let every: Vec<PathBuf> =
            Base::EVERY.into_iter().map(|base| ok(base.ours_under(&hers()))).collect();

        assert!(
            every.iter().all(|at| at.ends_with(OURS)),
            "the desktop's own directory is the same word under every base: {every:?}"
        );
    }

    #[test]
    fn every_base_is_asked_of_the_environment_by_its_own_name() {
        let every: Vec<&str> = Base::EVERY.into_iter().map(|base| ok(base.called())).collect();

        assert_eq!(
            every,
            vec!["XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"]
        );
    }
}
