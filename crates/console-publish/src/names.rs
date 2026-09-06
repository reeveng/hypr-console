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


use console_external_programs::Program;
use console_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    pub name: String,
    pub what: &'static str,
}

pub fn watched() -> Result<(Vec<Watched>, Option<String>), Never> {
    let mut names = Vec::new();
    let mut push = |name: String, what| {
        let name = name.trim().to_string();

        let worth = name.len() > 2 && !names.iter().any(|held: &Watched| held.name == name);

        match worth {
            true => names.push(Watched { name, what }),
            false => {}
        }
    };

    let Ok(building) = said(Program::Id, &["-un"]);
    let Ok(machine) = said(Program::Hostname, &[]);

    push(building, "whoever is building this");
    push(machine, "what this machine calls itself");

    let missing = match std::env::var("CONSOLE_HOST") {
        Ok(at) if !at.trim().is_empty() => {
            push(at.rsplit('@').next().unwrap_or_default().to_string(), "the device");
            let Ok(asked) = said(Program::Ssh, &[
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
        Ok(_) | Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => Some(
            "CONSOLE_HOST is not set, so the device's own name and the name of \
             whoever it belongs to were not checked for."
                .to_string(),
        ),
    };

    Ok((names, missing))
}

pub fn leaks<'a>(text: &str, names: &'a [Watched]) -> Result<Option<&'a Watched>, Never> {
    Ok(names.iter().find(|watched| {
        let Ok(says) = says(text, &watched.name);

        says == Says::TheName
    }))
}

fn says(text: &str, name: &str) -> Result<Says, Never> {
    let bytes = text.as_bytes();
    let said = text.match_indices(name).any(|(at, _)| {
        let before = at.checked_sub(1).and_then(|before| bytes.get(before).copied());
        let after = bytes.get(at.saturating_add(name.len())).copied();
        ![before, after]
            .into_iter()
            .flatten()
            .any(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    });

    Ok(match said {
        true => Says::TheName,
        false => Says::JustLetters,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Says {
    TheName,
    JustLetters,
}

fn said(program: Program, rest: &[&str]) -> Result<String, Never> {
    let Ok(mut asking) = program.command();

    Ok(match asking.args(rest).output() {
        Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),
        Err(_) => String::new(),
    })
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
        let Ok(both) = leaks("ada on her-laptop", &names);
        let Ok(one) = leaks("on her-laptop", &names);

        assert_eq!(both.map(|w| w.name.as_str()), Some("ada"));
        assert_eq!(one.map(|w| w.name.as_str()), Some("her-laptop"));
    }

    #[test]
    fn text_saying_none_of_them_says_nothing() {
        let names = watching(&["ada"]);
        let Ok(said) = leaks("a handheld belonging to a player", &names);

        assert_eq!(said, None);
    }

    #[test]
    fn a_name_inside_a_longer_word_was_not_said() {
        let names = watching(&["nimbus"]);
        let Ok(unit) = leaks("nimbusos-gamescope-autologin.service", &names);
        let Ok(prose) = leaks("because NimbusOS put it there", &names);
        let Ok(address) = leaks("root@nimbus", &names);
        let Ok(url) = leaks("ssh://root@nimbus-handheld/etc/console", &names);
        let Ok(sentence) = leaks("the nimbus in question", &names);

        assert_eq!(unit, None);
        assert_eq!(prose, None);
        assert!(address.is_some());
        assert!(url.is_some());
        assert!(sentence.is_some());
    }

    #[test]
    fn a_name_in_a_path_was_said() {
        let names = watching(&["ada"]);
        let Ok(hers) = leaks("files/home/ada/.config", &names);
        let Ok(his) = leaks("files/home/adam/.config", &names);
        let Ok(word) = leaks("the adage about it", &names);

        assert!(hers.is_some());
        assert_eq!(his, None);
        assert_eq!(word, None);
    }

    #[test]
    fn this_machine_can_be_asked_who_it_is() {
        let Ok((names, _)) = watched();

        assert!(!names.is_empty(), "nothing could be asked of this machine");
    }

    #[test]
    fn a_name_too_short_to_mean_anything_is_not_watched_for() {
        let Ok((names, _)) = watched();

        assert!(names.iter().all(|watched| watched.name.len() > 2), "{names:?}");
    }
}
