//! Step every keyboard along to another alphabet.
//!
//! ```text
//! language-switch            the next alphabet, on every keyboard
//! language-switch --back     the one before it instead
//! language-switch --settle   put each one back on what it was last switched to
//! ```
//!
//! The setting is asked first either way, so a keyboard is offered whatever
//! this machine is set to type at the moment somebody reaches for the key
//! rather than whatever it was set to when the session started. That is one
//! `hyprctl keyword` per keyboard and it happens on a keypress, which is
//! cheaper than the alternative: a daemon watching the setting so it could
//! push the list the moment it changed, for a list that changes twice a year.
//!
//! Every keyboard, rather than the one in front. The compositor has no notion
//! of which board somebody's hands are on, and a person with two of them who
//! pressed the key on one would be as surprised by the other staying behind as
//! by it coming along -- but only one of those two can be undone by pressing
//! the key again. What each keeps is still its own: they are stepped from
//! wherever each of them was, so two boards on different alphabets stay a step
//! apart.

use console_core_walking::Step;
use console_input_alphabets::wearing;
use console_input_language::{BACK, SETTLE, along, at, layouts};

fn main() -> std::process::ExitCode {
    let words: Vec<String> = std::env::args().collect();
    let settling = words.iter().any(|word| word == SETTLE);
    let way = match words.iter().any(|word| word == BACK) {
        true => Step::Back,
        false => Step::Forward,
    };

    let devices = match console_compositor::asked(console_compositor::Asked::Devices) {
        Ok(devices) => devices,
        Err(fault) => {
            eprintln!("language-switch: {fault}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let Ok(keyboards) = console_compositor::keyboards(&devices);
    let Ok(walk) = console_input_alphabets::chosen();
    let Ok(said) = layouts(&walk);
    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => {
            eprintln!("language-switch: nobody's home is known, so nothing can be remembered");
            return std::process::ExitCode::FAILURE;
        }
    };

    let Ok(every) = wearing::every(&home);

    for keyboard in &keyboards {
        let Ok(worn) = wearing::among(&every, &keyboard.name, &walk);

        let Ok(wants) = match settling {
            true => Ok(worn),
            false => along(&walk, worn, way),
        };

        let Ok(told) = console_compositor::offers(&keyboard.name, console_compositor::Layouts(&said));

        match told {
            console_compositor::Done::Taken => {},
            console_compositor::Done::Refused(why) => {
                eprintln!("language-switch: {} would not take {said:?}: {why}", keyboard.name);
                continue;
            }
        }

        let Ok(which) = at(&walk, wants);
        let Ok(switched) = console_compositor::wears(&keyboard.name, which);

        match switched {
            console_compositor::Done::Taken => {},
            console_compositor::Done::Refused(why) => {
                eprintln!("language-switch: {} would not wear {}: {why}", keyboard.name, wants.key);
                continue;
            }
        }

        match wearing::remember(&home, &keyboard.name, wants) {
            Ok(()) => {},
            Err(fault) => eprintln!("language-switch: {fault}"),
        }

        println!("{} is typing {}", keyboard.name, wants.says);
    }

    match keyboards.is_empty() {
        true => eprintln!("language-switch: the compositor lists no keyboard at all"),
        false => {},
    }

    std::process::ExitCode::SUCCESS
}
