//! The keyboard half of the table, handed to the compositor.
//!
//! A press on the pad is this daemon's to act on. A press on a keyboard is not,
//! and the reason is the one the lua binds were written under: a chord has to
//! be swallowed, or the key lands in whatever window has focus as well as doing
//! what it was bound to. The compositor is what can swallow one -- it owns
//! modifier state and it decides what reaches a window -- so what is here turns
//! rows of the table into `hl.bind` and hands them over.
//!
//! Nobody writes a bind. The table is the whole of the answer, and this is a
//! rendering of it, pushed at startup and again whenever `buttons.toml`
//! changes. That is what keeps a move immediate: a job moved onto Super and J
//! is on Super and J before the thumb is off the row, with no apply and no
//! reload.
//!
//! ## Through eval, because the config is lua
//!
//! This was `hyprctl keyword bind` and it never once worked. A lua config
//! swaps the parser, and the swapped parser answers *keyword can't work with
//! non-legacy parsers* and exits zero, so every bind this desktop rendered was
//! refused by a compositor that then said it had taken them. The daemon was
//! up, the table was right, the rendering was right, and the keys did nothing
//! -- which is the shape of fault this whole crate is arranged against, and it
//! survived because the one thing nobody asked was the compositor itself.
//! `console-compositor` knows that sentence for a refusal now, and the checks
//! ask what is held rather than what was sent.
//!
//! So a bind is lua: `hl.bind`, with `hl.dsp.exec_cmd` for the doing and the
//! flags in a table beside it. `repeating` rather than `repeat`, because the
//! word a person would use is a keyword in that language.
//!
//! ## What a bind carries
//!
//! `exec_cmd`, always, and never a dispatcher. Every job's doing is already a
//! `Doing::Run` or it is not a job a keyboard can reach at all, so rendering
//! one thing rather than two means there is no second vocabulary to keep
//! agreeing with the first. It costs a process on the workspace binds, which
//! run `hyprctl dispatch` back at the compositor that just called them. That
//! is a real cost and it buys the property worth having: what a key does and
//! what a button does are the same sentence, read out of the same row.
//!
//! `locked` where the job answers with the screen locked and `repeating` where
//! holding it goes on stepping -- both read off `What` rather than written
//! here, so the volume keys repeat because volume repeats and not because
//! somebody remembered to say so.
//!
//! ## And what it carries for us
//!
//! A `description`, which is the row and the keys it is on. It is the only
//! part of a bind that survives the round trip: asked what it is holding, a
//! compositor with a lua config answers `__lua` for every dispatcher and a
//! handle for every argument, and a bind named by keycode comes back with no
//! key on it at all. The modifiers and the description are what is left, so
//! the description carries what the modifiers cannot -- which key -- and
//! [`kept`] is how the daemon learns that a reload threw the binds away,
//! rather than being told by somebody whose screen would not come back on.
//!
//! ## When it is asked again
//!
//! A reload keeps only what a file holds, and none of these are in one, so
//! `hyprctl reload` -- or a compositor restarted under a daemon that was not
//! -- leaves the table saying one thing and the machine holding nothing.
//! [`worth_asking_after`] is the line that says so: the compositor announces
//! `configreloaded` on its own socket, and getting onto that socket counts as
//! one too, because a connection that has just been made is a stretch this
//! daemon was not watching. What follows is not a blind push. The compositor
//! is asked what it is holding and only what is missing is sent, so an event
//! that meant nothing costs one question and changes nothing, and an event
//! that never arrives is the only way to stay broken.
//!
//! It was going to be a poll, and the argument against a cadence is the usual
//! one: a number of seconds is an assumption about a fault nobody has timed.
//! The power key is the row with the most riding on this -- a screen that will
//! not come back is the state a person cannot see their way out of -- and the
//! honest answer for it is not a shorter interval, it is asking at the moment
//! the compositor says the thing that causes it.
//!
//! The slug alone would not do it. A job can be on two keys at once -- the
//! on-screen keyboard is on Super and K and on the key a board marks
//! calculator -- and two binds a compositor cannot tell apart are two binds
//! this cannot put back one at a time.

use console_compositor::stirred::Stirred;
use console_core_never::Never;
use console_input_bindings::bound::{Binding, Input, Played};
use console_input_bindings::keys;

use crate::doing::Doing;
use crate::means::{Job, Locked, Press, Repeats, Table};

const BIND: &str = "hl.bind";

const UNBIND: &str = "hl.unbind";

const RUNS: &str = "hl.dsp.exec_cmd";

const WHILE_LOCKED: &str = "locked = true";

const WHILE_HELD: &str = "repeating = true";

const ABOUT: &str = "description";

const ON: &str = "on";

const PLAIN: &[char] = &['.', '_', '-', '/', '+'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind {
    pub keys: String,
    pub held: u64,
    pub runs: String,
    pub about: String,
    pub locked: Locked,
    pub repeats: Repeats,
}

impl Bind {
    pub fn said(&self) -> Result<String, Never> {
        let Ok(keys) = console_compositor::quoted(&self.keys);
        let Ok(runs) = console_compositor::quoted(&self.runs);
        let Ok(about) = console_compositor::quoted(&self.about);

        let mut opts: Vec<String> = vec![format!("{ABOUT} = {about}")];

        match self.locked {
            Locked::EvenThen => opts.push(WHILE_LOCKED.to_string()),
            Locked::Awake => {},
        }

        match self.repeats {
            Repeats::WhileHeld => opts.push(WHILE_HELD.to_string()),
            Repeats::Once => {},
        }

        Ok(format!("{BIND}({keys}, {RUNS}({runs}), {{ {} }})", opts.join(", ")))
    }

    pub fn unsaid(&self) -> Result<String, Never> {
        let Ok(keys) = console_compositor::quoted(&self.keys);

        Ok(format!("{UNBIND}({keys})"))
    }
}

pub fn wanted(table: &Table) -> Result<Vec<Bind>, Never> {
    let Ok(every) = table.every();
    let mut wanted: Vec<Bind> = Vec::new();

    for (job, bound) in every {
        for one in bound.iter().filter(|one| one.on == Input::Keyboard) {
            let Ok(played) = one.played();

            match played {
                Played::ByNothing => continue,
                Played::ByAPress => {},
            }

            let Ok(keys) = keyed(one);

            let keys = match keys {
                Some(keys) => keys,
                None => continue,
            };

            let Ok(held) = keys::mask(&one.held);

            let held = match held {
                Some(held) => held,
                None => continue,
            };

            let Ok(does) = job.what.does(Press::Down);

            let argv = match does {
                Some(Doing::Run(argv)) => argv,
                Some(Doing::Frame(_) | Doing::Tell(_) | Doing::Using(_)) | None => continue,
            };

            let Ok(runs) = quoted(&argv);
            let Ok(locked) = job.what.locked();
            let Ok(repeats) = job.what.repeats();

            let Ok(about) = named(job, &keys);

            wanted.push(Bind { keys, held, runs, about, locked, repeats });
        }
    }

    Ok(wanted)
}

fn named(job: &Job, keys: &str) -> Result<String, Never> {
    Ok(format!("{} {ON} {keys}", job.slug))
}

fn keyed(binding: &Binding) -> Result<Option<String>, Never> {
    keys::bind(&binding.held, &binding.pressed)
}

fn quoted(argv: &[String]) -> Result<String, Never> {
    let mut said: Vec<String> = Vec::new();

    for word in argv {
        let plain = !word.is_empty()
            && word.chars().all(|letter| letter.is_ascii_alphanumeric() || PLAIN.contains(&letter));

        said.push(match plain {
            true => word.clone(),
            false => format!("'{}'", word.replace('\'', "'\\''")),
        });
    }

    Ok(said.join(" "))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Asking,
    Ignoring,
}

pub fn worth_asking_after(line: &str) -> Result<Worth, Never> {
    let Ok(stirred) = console_compositor::stirred::read(line);

    Ok(match stirred {
        Stirred::ConfigReloaded => Worth::Asking,
        Stirred::WindowOpened(_)
        | Stirred::WindowClosed(_)
        | Stirred::WindowRenamed(_)
        | Stirred::WindowMoved
        | Stirred::WindowFloated
        | Stirred::WindowPinned
        | Stirred::WindowFilled
        | Stirred::LayerOpened
        | Stirred::LayerClosed
        | Stirred::WorkspaceChanged
        | Stirred::ScreenFocused
        | Stirred::Nothing => Worth::Ignoring,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    Through,
    Nowhere,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holding {
    These(Vec<console_compositor::Bind>),
    Unanswered(String),
}

pub fn holding() -> Result<Holding, Never> {
    Ok(match console_compositor::asked(console_compositor::Asked::Binds) {
        Ok(said) => {
            let Ok(these) = console_compositor::binds(&said);

            Holding::These(these)
        }
        Err(fault) => Holding::Unanswered(fault),
    })
}

pub fn kept(handed: &[Bind], held: &[console_compositor::Bind]) -> Result<Vec<Bind>, Never> {
    Ok(handed
        .iter()
        .filter(|one| held.iter().any(|there| there.held == one.held && there.about == one.about))
        .cloned()
        .collect())
}

pub fn told(wanted: &[Bind], before: &[Bind]) -> Result<Went, Never> {
    let mut went = Went::Through;

    for gone in before.iter().filter(|one| !wanted.contains(one)) {
        let Ok(said) = gone.unsaid();
        let Ok(done) = console_compositor::told(console_compositor::Told::Eval, &said);

        let Ok(through) = complained(done, &said);

        went = match (went, through) {
            (Went::Through, Went::Through) => Went::Through,
            (_, _) => Went::Nowhere,
        };
    }

    for one in wanted.iter().filter(|one| !before.contains(one)) {
        let Ok(said) = one.said();
        let Ok(done) = console_compositor::told(console_compositor::Told::Eval, &said);

        let Ok(through) = complained(done, &said);

        went = match (went, through) {
            (Went::Through, Went::Through) => Went::Through,
            (_, _) => Went::Nowhere,
        };
    }

    Ok(went)
}

fn complained(said: console_compositor::Done, about: &str) -> Result<Went, Never> {
    Ok(match said {
        console_compositor::Done::Taken => Went::Through,
        console_compositor::Done::Refused(why) => {
            eprintln!("stick-scroll: the compositor would not take {about:?}: {why}");

            Went::Nowhere
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use console_input_bindings::moved::Jobs;

    fn ours() -> Vec<Bind> {
        let Ok(table) = Table::ours();
        let Ok(wanted) = wanted(&table);

        wanted
    }

    fn one<'a>(every: &'a [Bind], keys: &str) -> &'a Bind {
        every.iter().find(|bind| bind.keys == keys).expect("a bind")
    }

    fn keys_of(word: &str, pressed: &str) -> String {
        let held = vec![word.to_string()];
        let Ok(said) = keys::bind(&held, pressed);

        said.expect("a key")
    }

    fn held(bind: &Bind) -> console_compositor::Bind {
        console_compositor::Bind {
            held: bind.held,
            named: String::new(),
            about: bind.about.clone(),
        }
    }

    #[test]
    fn a_job_on_a_key_is_a_bind_that_runs_what_the_job_runs() {
        let every = ours();
        let settings = one(&every, &keys_of("super", "i"));

        assert_eq!(settings.runs, "settings-panel");
        assert_eq!(settings.about, "settings on SUPER+code:31");
    }

    #[test]
    fn a_job_on_no_key_is_no_bind() {
        let every = ours();

        assert!(
            !every.iter().any(|bind| bind.runs == "put-away"),
            "put-away is on the pad alone and has nothing to bind"
        );
    }

    #[test]
    fn a_word_with_room_in_it_is_handed_over_whole() {
        let every = ours();
        let sound = one(&every, &format!("SUPER+CTRL+{}", tail("a")));

        assert_eq!(sound.runs, "settings-panel Sound");

        let left = every.iter().find(|bind| bind.runs.contains("direction")).expect("a bind");

        assert!(left.runs.contains('\''), "a dispatcher's braces are held together: {}", left.runs);
    }

    fn tail(pressed: &str) -> String {
        let Ok(said) = keys::bind(&[], pressed);

        said.expect("a key")
    }

    #[test]
    fn what_answers_in_the_dark_says_so_and_what_repeats_says_that() {
        let every = ours();
        let Ok(volume) = one(&every, &tail("volume-up")).said();
        let Ok(mute) = one(&every, &tail("mute")).said();
        let Ok(browser) = one(&every, &keys_of("super", "b")).said();

        assert!(volume.contains(WHILE_LOCKED) && volume.contains(WHILE_HELD), "{volume}");
        assert!(mute.contains(WHILE_LOCKED) && !mute.contains(WHILE_HELD), "{mute}");
        assert!(!browser.contains(WHILE_LOCKED) && !browser.contains(WHILE_HELD), "{browser}");
    }

    #[test]
    fn a_bind_is_lua_the_compositor_will_take() {
        let every = ours();
        let Ok(said) = one(&every, &keys_of("super", "i")).said();

        assert_eq!(
            said,
            "hl.bind(\"SUPER+code:31\", hl.dsp.exec_cmd(\"settings-panel\"), \
             { description = \"settings on SUPER+code:31\" })"
        );

        let Ok(unsaid) = one(&every, &keys_of("super", "i")).unsaid();

        assert_eq!(unsaid, "hl.unbind(\"SUPER+code:31\")");
    }

    #[test]
    fn moving_a_job_moves_the_bind_and_leaves_the_rest_alone() {
        let said = Jobs::read("[jobs]\nsettings = \"keyboard: super + j\"\n").expect("a table");
        let Ok(table) = Table::of(&said);
        let Ok(now) = wanted(&table);

        assert!(now.iter().any(|bind| bind.keys == keys_of("super", "j")));
        assert!(!now.iter().any(|bind| bind.keys == keys_of("super", "i")));
        assert!(
            now.iter().any(|bind| bind.keys == keys_of("super", "b")),
            "the browser is where it was"
        );
    }

    #[test]
    fn nothing_that_only_sends_a_key_is_handed_over() {
        let every = ours();

        assert!(
            !every.iter().any(|bind| bind.runs.is_empty()),
            "a bind that runs nothing is a key that does nothing"
        );
    }

    #[test]
    fn no_two_binds_look_the_same_to_the_compositor() {
        let every = ours();

        for bind in &every {
            let same = every.iter().filter(|other| other.about == bind.about).count();

            assert_eq!(same, 1, "{:?} names two binds, and neither can be put back alone", bind.about);
        }

        let keyboard: Vec<&Bind> =
            every.iter().filter(|bind| bind.about.starts_with("keyboard ")).collect();

        assert_eq!(keyboard.len(), 2, "the keyboard is the job that is on two keys at once");
    }

    #[test]
    fn a_compositor_that_threw_them_away_is_holding_none_of_them() {
        let every = ours();
        let Ok(still) = kept(&every, &[]);

        assert!(still.is_empty(), "nothing held is nothing kept");

        let all: Vec<console_compositor::Bind> = every.iter().map(held).collect();
        let Ok(still) = kept(&every, &all);

        assert_eq!(still.len(), every.len(), "what it says it holds is what it kept");
    }

    #[test]
    fn a_bind_moved_onto_another_key_is_not_the_one_that_is_held() {
        let every = ours();
        let settings = one(&every, &keys_of("super", "i")).clone();

        let elsewhere = console_compositor::Bind {
            held: 0,
            named: String::new(),
            about: settings.about.clone(),
        };

        let Ok(still) = kept(&[settings], &[elsewhere]);

        assert!(still.is_empty(), "the same job on a different chord is a different bind");
    }
}
