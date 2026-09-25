//! What a migration is, what it says it sweeps, and what it does.
//!
//! A migration was a shell script, on the argument that moving files someone
//! installed has no better language. What nearly every one of them did was
//! three things -- move a path into the attic, stop a unit, disable one -- and
//! the few that did more read one line out of one file and wrote it into
//! another. That is a short list of steps, and a list of steps is data: the
//! engine carries them out in the same code that takes back what the manifest
//! stopped naming, and a migration that names a step the engine does not know
//! is one the compiler refuses rather than one bash reaches halfway through.
//!
//! So the claim is no longer written beside the body. What a migration answers
//! for is what its steps move and stand down, in the words [`holds`] puts it
//! in -- a path for a file, and what holds a unit there for a unit. The gate
//! used to read a `# sweeps:` header and never the body, because a script that
//! mentions a name in a comment has not thereby swept it; a step has no
//! comment to mention it in, so the body is the claim.
//!
//! A migration is named for the moment of the commit that needs it, which
//! omarchy arrived at for the reason that matters here too: it sorts into
//! history order without a counter anyone has to keep, and two people writing
//! one on the same afternoon get different moments without talking to each
//! other.
//!
//! `left-on-purpose` stays a file in `migrations/`, read where it is: it is a
//! list of names with a reason beside each, and the engine reads it on the
//! machine from the tree it is applying.
//!
//! [`holds`]: crate::holds

use crate::{Section, holds};
use console_core_never::Never;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

pub const UNDER: &str = "migrations";

pub const ON_PURPOSE: &str = "left-on-purpose";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment(pub u64);

impl fmt::Display for Moment {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(to, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub moment: Moment,
    pub says: &'static str,
    pub steps: &'static [Step],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Attic(&'static str),
    AtticEach(Each),
    Stop(&'static str),
    Disable(&'static str),
    DisableGlobally(&'static str),
    Terminate(&'static str),
    RemoveIfEmpty(&'static str),
    Rewrite(Rewrite),
    CopySetting(CopySetting),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Each {
    pub under: &'static str,
    pub named: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rewrite {
    pub at: &'static str,
    pub was: &'static str,
    pub becomes: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopySetting {
    pub from: &'static str,
    pub key: &'static str,
    pub into: &'static str,
}

pub fn sweeps(migration: &Migration) -> Result<BTreeSet<String>, Never> {
    Ok(migration
        .steps
        .iter()
        .filter_map(|step| {
            let (section, entry) = match step {
                Step::Attic(at) => (Section("[files]"), *at),
                Step::Disable(unit) | Step::DisableGlobally(unit) => (Section("[services]"), *unit),
                Step::AtticEach(_)
                | Step::Stop(_)
                | Step::Terminate(_)
                | Step::RemoveIfEmpty(_)
                | Step::Rewrite(_)
                | Step::CopySetting(_) => return None,
            };

            let Ok(held) = holds(section, entry);

            held
        })
        .collect())
}

pub fn all_claimed(every: &[Migration]) -> Result<BTreeSet<String>, Never> {
    Ok(every
        .iter()
        .flat_map(|migration| {
            let Ok(claimed) = sweeps(migration);

            claimed
        })
        .collect())
}

pub fn on_purpose(said: &str) -> Result<BTreeSet<String>, Never> {
    Ok(said
        .lines()
        .map(|line| line.split('#').next().map_or("", str::trim))
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn setting(said: &str, copy: &CopySetting) -> Result<Option<String>, Never> {
    Ok(said
        .lines()
        .find_map(|line| line.trim_start().strip_prefix(copy.key))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned))
}

pub fn rewritten(said: &str, rewrite: &Rewrite) -> Result<Option<String>, Never> {
    let found = said.lines().any(|line| line == rewrite.was);

    Ok(match found {
        true => Some(
            said.split_inclusive('\n')
                .map(|line| match line.strip_suffix('\n') == Some(rewrite.was) || line == rewrite.was {
                    true => line.replacen(rewrite.was, rewrite.becomes, 1),
                    false => line.to_string(),
                })
                .collect(),
        ),
        false => None,
    })
}

pub fn beside(root: &Path) -> Result<PathBuf, Never> {
    Ok(root.join(UNDER))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MUSIC: CopySetting = CopySetting {
        from: "/home/@user@/.config/kew/kewrc",
        key: "path=",
        into: "/home/@user@/.config/console/music",
    };

    const AUTOLOGIN: Rewrite = Rewrite {
        at: "/etc/plasmalogin.conf.d/zz-steamos-autologin.conf",
        was: "Session=hyprland.desktop",
        becomes: "Session=console.desktop",
    };

    fn of(steps: &'static [Step]) -> Migration {
        Migration { moment: Moment(1), says: "", steps }
    }

    #[test]
    fn a_migration_claims_what_it_moves_and_what_it_disables() {
        let Ok(claimed) = sweeps(&of(&[
            Step::Attic("/usr/local/bin/console-poke"),
            Step::Attic("/home/@user@/.config/mako/config"),
            Step::Disable("console-sky.service"),
            Step::DisableGlobally("console-well.timer"),
        ]));

        assert!(claimed.contains("/usr/local/bin/console-poke"));
        assert!(claimed.contains("/home/@user@/.config/mako/config"));
        assert!(claimed.contains("enabled console-sky.service"));
        assert!(claimed.contains("enabled console-well.timer"));
        assert_eq!(claimed.len(), 4);
    }

    #[test]
    fn stopping_a_unit_is_not_claiming_it() {
        let Ok(claimed) = sweeps(&of(&[Step::Stop("console-session.service"), Step::Terminate("kew")]));

        assert!(claimed.is_empty());
    }

    #[test]
    fn what_was_left_on_purpose_reads_one_name_a_line_with_its_reason() {
        let said = "# why each of these needs nothing\nconsole-timings  # moved by hand\nmkinitcpio\n";
        let Ok(left) = on_purpose(said);

        assert!(left.contains("console-timings"));
        assert!(left.contains("mkinitcpio"));
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn a_line_that_is_only_a_reason_names_nothing() {
        let Ok(left) = on_purpose("# nothing has left yet\n");

        assert!(left.is_empty());
    }

    #[test]
    fn a_setting_is_read_from_the_first_line_that_says_it() {
        let Ok(said) = setting("volume=80\n  path= /home/ada/Music \npath=/elsewhere\n", &MUSIC);

        assert_eq!(said.as_deref(), Some("/home/ada/Music"));
    }

    #[test]
    fn a_setting_that_is_empty_or_absent_is_not_there() {
        let Ok(empty) = setting("path=\n", &MUSIC);
        let Ok(absent) = setting("volume=80\n", &MUSIC);

        assert_eq!(empty, None);
        assert_eq!(absent, None);
    }

    #[test]
    fn a_line_that_says_the_old_session_is_rewritten_and_nothing_else_is() {
        let Ok(said) = rewritten("[Autologin]\nUser=ada\nSession=hyprland.desktop\n", &AUTOLOGIN);

        assert_eq!(said.as_deref(), Some("[Autologin]\nUser=ada\nSession=console.desktop\n"));
    }

    #[test]
    fn a_file_that_already_says_something_else_is_left_as_it_is() {
        let Ok(gamescope) = rewritten("Session=gamescope-wayland.desktop\n", &AUTOLOGIN);
        let Ok(longer) = rewritten("Session=hyprland.desktop.old\n", &AUTOLOGIN);

        assert_eq!(gamescope, None);
        assert_eq!(longer, None);
    }
}
