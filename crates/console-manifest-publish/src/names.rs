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
//!
//! It asks for every one of them in any case and in any file. A name is the
//! same name written with a capital, and the builder's own account name and
//! the author git signs commits with differ in exactly that. A file that is not
//! text is still bytes, and a picture or an archive made in somebody's home can
//! carry that home in it, so nothing is skipped for failing to be UTF-8. A
//! question that could not be answered -- a local program that would not run, a
//! device that said only half of what it was asked -- is said as a name that
//! was not checked for, rather than being left out quietly and read as a clean
//! copy.

use console_core_external_programs::Program;
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    pub name: String,
    pub what: &'static str,
}

fn push(names: &mut Vec<Watched>, found: Watched) -> Result<(), Never> {
    let Watched { name, what } = found;
    let name = name.trim().to_ascii_lowercase();

    let worth = name.len() > 2 && !names.iter().any(|held: &Watched| held.name == name);

    match worth {
        true => names.push(Watched { name, what }),
        false => {}
    }

    Ok(())
}

fn asked_here(names: &mut Vec<Watched>, missing: &mut Vec<String>, question: Question) -> Result<(), Never> {
    let Question { program, rest, what } = question;
    let Ok(answer) = said(program, rest);

    match answer {
        Some(answer) => push(names, Watched { name: answer, what }),
        None => {
            missing.push(format!("this machine would not say {what}, so it was not checked for."));

            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Question<'a> {
    program: Program,
    rest: &'a [&'a str],
    what: &'static str,
}

const ON_THE_DEVICE: [&str; 2] = ["the device", "whoever the device belongs to"];

pub fn watched() -> Result<(Vec<Watched>, Vec<String>), Never> {
    let mut names = Vec::new();
    let mut missing = Vec::new();

    let Ok(()) = asked_here(&mut names, &mut missing, Question {
        program: Program::Id,
        rest: &["-un"],
        what: "whoever is building this",
    });
    let Ok(()) = asked_here(&mut names, &mut missing, Question {
        program: Program::Hostname,
        rest: &[],
        what: "what this machine calls itself",
    });

    let told = console_device_name::device();

    let at = match told {
        Err(fault) => {
            missing.push(format!(
                "{fault}, so the device's own name and the name of whoever it \
                 belongs to were not checked for."
            ));

            return Ok((names, missing));
        }
        Ok(at) => at,
    };

    match at.trim().is_empty() {
        true => {
            missing.push(
                "CONSOLE_HOST is not set, so the device's own name and the name of \
                 whoever it belongs to were not checked for."
                    .to_string(),
            );

            return Ok((names, missing));
        }
        false => {}
    }

    let named = match at.rsplit('@').next() {
        Some(named) => named,
        None => at.as_str(),
    };

    let Ok(()) = push(&mut names, Watched { name: named.to_string(), what: "the device" });

    let Ok(asked) = said(Program::Ssh, &[
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        &at,
        "hostname; set -- $(ls -1 /home 2>/dev/null); \
         if [ $# -eq 1 ]; then echo \"$1\"; else id -nu 1000; fi",
    ]);

    let answered: Vec<String> = match asked {
        Some(asked) => asked.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect(),
        None => Vec::new(),
    };

    for (which, what) in ON_THE_DEVICE.iter().enumerate() {
        match answered.get(which) {
            Some(said) => {
                let Ok(()) = push(&mut names, Watched { name: said.clone(), what });
            }
            None => missing.push(format!("{at} did not say {what}, so it was not checked for.")),
        }
    }

    Ok((names, missing))
}

pub fn leaks<'a>(text: &[u8], names: &'a [Watched]) -> Result<Option<&'a Watched>, Never> {
    Ok(names.iter().find(|watched| {
        let Ok(says) = says(text, Name(watched.name.as_bytes()));

        says == Spelling::TheName
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Name<'a>(&'a [u8]);

fn says(text: &[u8], name: Name<'_>) -> Result<Spelling, Never> {
    let name = name.0;

    let said = match name.len() {
        0 => false,
        long => text.windows(long).enumerate().any(|(at, window)| {
            let before = at.checked_sub(1).and_then(|before| text.get(before).copied());
            let after = text.get(at.saturating_add(long)).copied();
            let alone = ![before, after]
                .into_iter()
                .flatten()
                .any(|byte| byte.is_ascii_alphanumeric() || byte == b'_');

            alone && window.eq_ignore_ascii_case(name)
        }),
    };

    Ok(match said {
        true => Spelling::TheName,
        false => Spelling::JustLetters,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spelling {
    TheName,
    JustLetters,
}

fn said(program: Program, rest: &[&str]) -> Result<Option<String>, Never> {
    let Ok(mut asking) = program.command();

    let done = match asking.args(rest).output() {
        Ok(done) => done,
        Err(_would_not_run) => return Ok(None),
    };

    let answer = String::from_utf8_lossy(&done.stdout).trim().to_string();

    Ok(match (done.status.success(), answer.is_empty()) {
        (true, false) => Some(answer),
        (true, true) | (false, true) | (false, false) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watching(names: &[&str]) -> Vec<Watched> {
        let mut watched = Vec::new();

        for name in names {
            let Ok(()) = push(&mut watched, Watched { name: name.to_string(), what: "someone" });
        }

        watched
    }

    fn leaked<'a>(text: &str, names: &'a [Watched]) -> Option<&'a Watched> {
        let Ok(leaks) = leaks(text.as_bytes(), names);

        leaks
    }

    #[test]
    fn a_name_that_is_said_is_the_name_that_comes_back() {
        let names = watching(&["ada", "her-laptop"]);
        let both = leaked("ada on her-laptop", &names);
        let one = leaked("on her-laptop", &names);

        assert_eq!(both.map(|w| w.name.as_str()), Some("ada"));
        assert_eq!(one.map(|w| w.name.as_str()), Some("her-laptop"));
    }

    #[test]
    fn text_saying_none_of_them_says_nothing() {
        let names = watching(&["ada"]);

        assert_eq!(leaked("a handheld belonging to a player", &names), None);
    }

    #[test]
    fn a_name_inside_a_longer_word_was_not_said() {
        let names = watching(&["nimbus"]);

        assert_eq!(leaked("nimbusos-gamescope-autologin.service", &names), None);
        assert_eq!(leaked("because NimbusOS put it there", &names), None);
        assert!(leaked("root@nimbus", &names).is_some());
        assert!(leaked("ssh://root@nimbus-handheld/etc/console", &names).is_some());
        assert!(leaked("the nimbus in question", &names).is_some());
    }

    #[test]
    fn a_name_in_a_path_was_said() {
        let names = watching(&["ada"]);

        assert!(leaked("files/home/ada/.config", &names).is_some());
        assert_eq!(leaked("files/home/adam/.config", &names), None);
        assert_eq!(leaked("the adage about it", &names), None);
    }

    #[test]
    fn a_name_with_a_capital_is_the_same_name() {
        let names = watching(&["ada"]);

        assert!(leaked("Signed-off-by: ADA", &names).is_some());
        assert!(leaked("files/home/Ada/.config", &names).is_some());
    }

    #[test]
    fn a_name_watched_for_with_a_capital_is_found_without_one() {
        let names = watching(&["Nimbus"]);

        assert!(leaked("root@nimbus", &names).is_some());
    }

    #[test]
    fn a_file_that_is_not_text_is_still_read() {
        let names = watching(&["ada"]);
        let mut picture = vec![0x89, b'P', b'N', b'G', 0xff, 0xfe, 0x00];

        picture.extend_from_slice(b"/home/ada/Pictures");
        picture.extend_from_slice(&[0xc3, 0x28, 0x00]);

        let Ok(found) = leaks(&picture, &names);

        assert!(found.is_some());
    }

    #[test]
    fn a_name_too_short_to_mean_anything_is_not_watched_for() {
        let names = watching(&["", "  ", "al", "ada"]);

        assert_eq!(names.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), vec!["ada"]);
    }

    #[test]
    fn a_question_that_could_not_be_answered_is_said_rather_than_skipped() {
        let mut names = Vec::new();
        let mut missing = Vec::new();

        let Ok(()) = asked_here(&mut names, &mut missing, Question {
            program: Program::Id,
            rest: &["-un", "nobody-by-this-name-at-all"],
            what: "whoever is building this",
        });

        assert!(names.is_empty(), "{names:?}");
        assert_eq!(missing.len(), 1, "{missing:?}");
    }

    #[test]
    fn this_machine_can_be_asked_who_it_is() {
        let Ok((names, _)) = watched();

        assert!(!names.is_empty(), "nothing could be asked of this machine");
    }
}
