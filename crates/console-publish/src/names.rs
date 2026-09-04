//! What a copy is not allowed to say, and how it is found out.
//!
//! This used to hold the answers: a person's name, their machine's name on the
//! network, and their controller's serial, written down so the copy could have
//! them taken out on the way past. That worked, and it meant the one file whose
//! job was to keep those three things out of a published repository was the one
//! file in the repository that had all three of them in it.
//!
//! Nothing in the tree says any of them now. The manifest writes `@user@` and
//! the machine fills it in, the device's address is read from `CONSOLE_HOST`,
//! and the capture records no serial. So there is nothing left to take out, and
//! what is left to do is make sure it stays that way.
//!
//! This asks rather than remembers. The names to watch for are gathered when a
//! copy is built: whoever is building it, whatever the device calls itself, and
//! whoever the device belongs to. A copy that says one of them is a copy that is
//! refused, and none of them is written down here or anywhere else.

use std::process::Command;

/// A name a copy must not say, and what it is, for saying so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    pub name: String,
    pub what: &'static str,
}

/// Every name a copy must not say, asked of this machine and of the device.
///
/// The device is asked over ssh, so a device that is off or unset answers
/// nothing and its two names go unwatched. That is said out loud rather than
/// passed over, because a check that quietly checked less is worse than one
/// that did not run.
pub fn watched() -> (Vec<Watched>, Option<String>) {
    let mut names = Vec::new();
    let mut push = |name: String, what| {
        let name = name.trim().to_string();

        // A short or empty name is not a name. `contains("")` is true of
        // everything, and a two letter one is true of half of everything.
        if name.len() > 2 && !names.iter().any(|held: &Watched| held.name == name) {
            names.push(Watched { name, what });
        }
    };

    push(said(&["id", "-un"]), "whoever is building this");
    push(said(&["hostname"]), "what this machine calls itself");

    let missing = match std::env::var("CONSOLE_HOST") {
        Ok(at) if !at.trim().is_empty() => {
            // The address itself holds the device's name, and it is the one
            // name here that needs nothing asked of anything.
            push(at.rsplit('@').next().unwrap_or_default().to_string(), "the device");
            let asked = said(&[
                "ssh",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                &at,
                "hostname; set -- $(ls -1 /home 2>/dev/null); \
                 if [ $# -eq 1 ]; then echo \"$1\"; else id -nu 1000; fi",
            ]);

            match asked.is_empty() {
                true => Some(format!(
                    "{at} did not answer, so the device's own name and the name \
                     of whoever it belongs to were not checked for."
                )),
                false => {
                    let mut lines = asked.lines();
                    push(lines.next().unwrap_or_default().to_string(), "the device");
                    push(
                        lines.next().unwrap_or_default().to_string(),
                        "whoever the device belongs to",
                    );
                    None
                }
            }
        }
        _ => Some(
            "CONSOLE_HOST is not set, so the device's own name and the name of \
             whoever it belongs to were not checked for."
                .to_string(),
        ),
    };
    (names, missing)
}

/// Whichever of the watched names something says, if it says any.
pub fn leaks<'a>(text: &str, names: &'a [Watched]) -> Option<&'a Watched> {
    names.iter().find(|watched| says(text, &watched.name) == Says::TheName)
}

/// Whether text says a name, rather than merely holding its letters.
///
/// A name has to stand on its own to have been said. A machine is often named
/// after the distribution it runs, and the manifest names that distribution a
/// dozen times over; asked for the letters alone, a machine called `nimbus`
/// running NimbusOS is named by every copy ever built and no copy can be
/// published. Asked for the word, a distribution is a distribution and a
/// machine is a machine.
///
/// A letter, a digit or an underscore on either side means the name is part of
/// a longer word and was not said. Everything else counts, `-` included, so
/// `nimbus-handheld` is still that machine being named while `nimbusos` is not.
///
/// Said with no real name in it, because this file is carried into the copy and
/// checked along with the rest of it. A test written against the machine this
/// is built on would be the one file that fails its own check.
fn says(text: &str, name: &str) -> Says {
    let bytes = text.as_bytes();
    let said = text.match_indices(name).any(|(at, _)| {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + name.len()).copied();
        ![before, after]
            .into_iter()
            .flatten()
            .any(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    });

    match said {
        true => Says::TheName,
        false => Says::JustLetters,
    }
}

/// Whether text says a name, or merely holds its letters inside a longer word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Says {
    /// The name stands on its own, with nothing wordlike either side of it.
    TheName,
    /// Its letters are there inside something else, which is not a mention.
    JustLetters,
}

/// What a command said, or nothing at all.
fn said(argv: &[&str]) -> String {
    match Command::new(argv[0])
        .args(&argv[1..])
        .output()
    {
        Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watching(names: &[&str]) -> Vec<Watched> {
        names.iter().map(|name| Watched { name: name.to_string(), what: "somebody" }).collect()
    }

    #[test]
    fn a_name_that_is_said_is_the_name_that_comes_back() {
        let names = watching(&["ada", "her-laptop"]);
        assert_eq!(leaks("ada on her-laptop", &names).map(|w| w.name.as_str()), Some("ada"));
        assert_eq!(leaks("on her-laptop", &names).map(|w| w.name.as_str()), Some("her-laptop"));
    }

    #[test]
    fn text_saying_none_of_them_says_nothing() {
        assert_eq!(leaks("a handheld belonging to a player", &watching(&["ada"])), None);
    }

    /// The fault this rule exists for, said with a made-up machine: one named
    /// after the distribution it runs, which the manifest names a dozen times
    /// over. Matching letters rather than words made every copy ever built
    /// unpublishable.
    #[test]
    fn a_name_inside_a_longer_word_was_not_said() {
        let names = watching(&["nimbus"]);
        assert_eq!(leaks("nimbusos-gamescope-autologin.service", &names), None);
        assert_eq!(leaks("because NimbusOS put it there", &names), None);
        // Said, though: standing on its own, and inside a longer name that is
        // the machine's rather than somebody else's word.
        assert!(leaks("root@nimbus", &names).is_some());
        assert!(leaks("ssh://root@nimbus-handheld/etc/console", &names).is_some());
        assert!(leaks("the nimbus in question", &names).is_some());
    }

    /// A home directory is the name with a slash either side of it.
    #[test]
    fn a_name_in_a_path_was_said() {
        let names = watching(&["ada"]);
        assert!(leaks("files/home/ada/.config", &names).is_some());
        assert_eq!(leaks("files/home/adam/.config", &names), None);
        assert_eq!(leaks("the adage about it", &names), None);
    }

    /// This machine answers to something, and the check is worth nothing if it
    /// does not: a list of no names finds no names in anything.
    #[test]
    fn this_machine_can_be_asked_who_it_is() {
        let (names, _) = watched();
        assert!(!names.is_empty(), "nothing could be asked of this machine");
    }

    /// The guard against the mistake that would make this silently useless.
    /// `contains("")` is true of every file there is.
    #[test]
    fn a_name_too_short_to_mean_anything_is_not_watched_for() {
        let (names, _) = watched();
        assert!(names.iter().all(|watched| watched.name.len() > 2), "{names:?}");
    }
}
