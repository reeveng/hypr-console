//! The keyboard half of the table, handed to the compositor.
//!
//! A press on the pad is this daemon's to act on. A press on a keyboard is not,
//! and the reason is the one the lua binds were written under: a chord has to
//! be swallowed, or the key lands in whatever window has focus as well as doing
//! what it was bound to. The compositor is what can swallow one -- it owns
//! modifier state and it decides what reaches a window -- so what is here turns
//! rows of the table into `hl.bind` and hands them over.
//!
//! NoOne writes a bind. The table is the whole of the answer, and this is a
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
//! survived because the one thing no one asked was the compositor itself.
//! `console-compositor` knows that sentence for a refusal now, and the checks
//! ask what is held rather than what was sent.
//!
//! So a bind is lua: `hl.bind`, with `hl.dsp.exec_cmd` for the effect and the
//! flags in a table beside it. `repeating` rather than `repeat`, because the
//! word a person would use is a keyword in that language.
//!
//! ## What a bind carries
//!
//! `exec_cmd`, always, and never a dispatcher. Every job's effect is already a
//! `Effect::Run` or it is not a job a keyboard can reach at all, so rendering
//! one thing rather than two means there is no second vocabulary to keep
//! agreeing with the first. It costs a process on the workspace binds, which
//! run `hyprctl dispatch` back at the compositor that just called them. That
//! is a real cost and it buys the property worth having: what a key does and
//! what a button does are the same sentence, read out of the same row.
//!
//! `locked` where the job answers with the screen locked and `repeating` where
//! holding it goes on stepping -- both read off `Action` rather than written
//! here, so the volume keys repeat because volume repeats and not because
//! someone remembered to say so.
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
//! rather than being told by someone whose screen would not come back on.
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
//! one: a number of seconds is an assumption about a fault no one has timed.
//! The power key is the row with the most riding on this -- a screen that will
//! not come back is the state a person cannot see their way out of -- and the
//! honest answer for it is not a shorter interval, it is asking at the moment
//! the compositor says the thing that causes it.
//!
//! The slug alone would not do it. A job can be on two keys at once -- the
//! on-screen keyboard is on Super and K and on the key a board marks
//! calculator -- and two binds a compositor cannot tell apart are two binds
//! this cannot put back one at a time.
//!
//! ## What the machine holds is the only baseline
//!
//! This asked what it had handed over rather than what the machine has, and at
//! startup it had handed over nothing -- so a daemon coming up beside a
//! compositor that was already holding every one of these pushed the whole
//! table on top of itself. A bind does not replace a bind: Hyprland keeps a
//! vector of them and fires every entry a press matches, so the second copy is
//! the volume rocker stepping twice and raising two cards, and the hundred and
//! twenty-fourth is a press no one can use.
//!
//! Nothing said this either. The daemon was up, the table was right, the
//! rendering was right, and every key did the right thing more times than it
//! was asked -- the same shape of fault as the refusal above, arrived at from
//! the other end: the one thing no one asked was how many.
//!
//! What restarts the daemon is the ordinary business of the device. An apply
//! stops and starts it, and so does every device run of the checks, while the
//! compositor stays up across all of them -- so this is not a rare state, it is
//! the state the device is in by the end of an afternoon.
//!
//! So the count is what is read. A key held once is left alone, a key held not
//! at all is bound, and a key held more than once is unbound and bound again.
//! `hl.unbind` erases every entry on the key rather than the first of them,
//! which is what makes that last one converge on a machine already holding a
//! hundred copies rather than merely stopping the next one arriving.

use console_compositor::events::CompositorEvent;
use console_core_never::Never;
use console_input_bindings::bound::{Binding, Input, Played};
use console_input_bindings::keys;

use crate::effect::Effect;
use crate::actions::{Task, LockBehavior, ButtonPress, RepeatMode, Table};

const BIND: &str = "hl.bind";

const UNBIND: &str = "hl.unbind";

const RUNS: &str = "hl.dsp.exec_cmd";

const WHILE_LOCKED: &str = "locked = true";

const WHILE_HELD: &str = "repeating = true";

const ABOUT: &str = "description";

const ON: &str = "on";

const PLAIN: &[char] = &['.', '_', '-', '/', '+'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub keys: String,
    pub held: u64,
    pub runs: String,
    pub about: String,
    pub locked: LockBehavior,
    pub repeats: RepeatMode,
}

impl KeyBinding {
    pub fn said(&self) -> Result<String, Never> {
        let Ok(keys) = console_compositor::quoted(&self.keys);
        let Ok(runs) = console_compositor::quoted(&self.runs);
        let Ok(about) = console_compositor::quoted(&self.about);

        let mut opts: Vec<String> = vec![format!("{ABOUT} = {about}")];

        match self.locked {
            LockBehavior::EvenThen => opts.push(WHILE_LOCKED.to_string()),
            LockBehavior::Unlocked => {},
        }

        match self.repeats {
            RepeatMode::WhileHeld => opts.push(WHILE_HELD.to_string()),
            RepeatMode::Once => {},
        }

        Ok(format!("{BIND}({keys}, {RUNS}({runs}), {{ {} }})", opts.join(", ")))
    }

    pub fn unsaid(&self) -> Result<String, Never> {
        let Ok(keys) = console_compositor::quoted(&self.keys);

        Ok(format!("{UNBIND}({keys})"))
    }
}

pub fn wanted(table: &Table) -> Result<Vec<KeyBinding>, Never> {
    let Ok(every) = table.every();
    let mut wanted: Vec<KeyBinding> = Vec::new();

    for (job, bound) in every {
        'over_binds: for one in bound.iter().filter(|one| one.on == Input::Keyboard) {
            let Ok(played) = one.played();

            match played {
                Played::ByNothing => continue 'over_binds,
                Played::ByAPress => {},
            }

            let Ok(keys) = keyed(one);

            let keys = match keys {
                Some(keys) => keys,
                None => continue 'over_binds,
            };

            let Ok(held) = keys::mask(&one.held);

            let held = match held {
                Some(held) => held,
                None => continue 'over_binds,
            };

            let Ok(does) = job.action.does(ButtonPress::Down);

            let arguments = match does {
                Some(Effect::Run(arguments)) => arguments,
                Some(Effect::Frame(_) | Effect::Tell(_) | Effect::Using(_)) | None => continue 'over_binds,
            };

            let Ok(runs) = quoted(&arguments);
            let Ok(locked) = job.action.locked();
            let Ok(repeats) = job.action.repeats();

            let Ok(about) = named(job, &keys);

            wanted.push(KeyBinding { keys, held, runs, about, locked, repeats });
        }
    }

    Ok(wanted)
}

fn named(job: &Task, keys: &str) -> Result<String, Never> {
    Ok(format!("{} {ON} {keys}", job.slug))
}

fn keyed(binding: &Binding) -> Result<Option<String>, Never> {
    keys::bind(&binding.held, &binding.pressed)
}

fn quoted(arguments: &[String]) -> Result<String, Never> {
    let mut said: Vec<String> = Vec::new();

    for word in arguments {
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
    Querying,
    Ignoring,
}

pub fn worth_asking_after(line: &str) -> Result<Worth, Never> {
    let Ok(stirred) = console_compositor::events::read(line);

    Ok(match stirred {
        CompositorEvent::ConfigReloaded => Worth::Querying,
        CompositorEvent::WindowOpened(_)
        | CompositorEvent::WindowClosed(_)
        | CompositorEvent::WindowRenamed(_)
        | CompositorEvent::WindowMoved
        | CompositorEvent::WindowFloated
        | CompositorEvent::WindowPinned
        | CompositorEvent::WindowFilled
        | CompositorEvent::LayerOpened
        | CompositorEvent::LayerClosed
        | CompositorEvent::WorkspaceChanged
        | CompositorEvent::ScreenFocused
        | CompositorEvent::Ignored => Worth::Ignoring,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    Through,
    Nowhere,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holding {
    These(Vec<console_compositor::BoundKey>),
    HyprctlError(String),
}

pub fn holding() -> Result<Holding, Never> {
    Ok(match console_compositor::query(console_compositor::Query::Binds) {
        Ok(console_compositor::Answer::Binds(these)) => Holding::These(these),
        Ok(other) => Holding::HyprctlError(format!("hyprctl answered {other:?} when asked for binds")),
        Err(fault) => Holding::HyprctlError(fault.to_string()),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holds {
    NotAtAll,
    Once,
    Over,
}

pub fn holds(one: &KeyBinding, held: &[console_compositor::BoundKey]) -> Result<Holds, Never> {
    Ok(match held.iter().filter(|there| there.held == one.held && there.about == one.about).count() {
        0 => Holds::NotAtAll,
        1 => Holds::Once,
        _ => Holds::Over,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindCommand {
    Bind(KeyBinding),
    Unbind(KeyBinding),
}

impl BindCommand {
    pub fn said(&self) -> Result<String, Never> {
        match self {
            BindCommand::Bind(one) => one.said(),
            BindCommand::Unbind(one) => one.unsaid(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub missing: u32,
    pub over: u32,
}

pub fn standing(wanted: &[KeyBinding], holding: &Holding) -> Result<Standing, Never> {
    let held = match holding {
        Holding::These(held) => held,
        Holding::HyprctlError(_) => return Ok(Standing { missing: 0, over: 0 }),
    };

    let mut standing = Standing { missing: 0, over: 0 };

    for one in wanted {
        let Ok(holds) = holds(one, held);

        match holds {
            Holds::Once => {},
            Holds::NotAtAll => standing.missing = standing.missing.saturating_add(1),
            Holds::Over => standing.over = standing.over.saturating_add(1),
        }
    }

    Ok(standing)
}

pub fn sent(wanted: &[KeyBinding], handed: &[KeyBinding], holding: &Holding) -> Result<Vec<BindCommand>, Never> {
    let held = match holding {
        Holding::These(held) => held,
        Holding::HyprctlError(_) => return unasked(wanted, handed),
    };

    let Ok(mut sent) = dropped(wanted, handed);

    for one in wanted {
        let Ok(holds) = holds(one, held);

        match holds {
            Holds::Once => {},
            Holds::NotAtAll => sent.push(BindCommand::Bind(one.clone())),
            Holds::Over => {
                sent.push(BindCommand::Unbind(one.clone()));
                sent.push(BindCommand::Bind(one.clone()));
            },
        }
    }

    Ok(sent)
}

fn unasked(wanted: &[KeyBinding], handed: &[KeyBinding]) -> Result<Vec<BindCommand>, Never> {
    let Ok(mut sent) = dropped(wanted, handed);

    #[cfg_attr(
        dylint_lib = "explicit028_no_search_in_a_loop",
        allow(
            explicit028_no_search_in_a_loop,
            reason = "both lists are the binds of one controller profile, which is tens of them and is written out in a file someone reads"
        )
    )]
    for one in wanted.iter().filter(|one| !handed.contains(one)) {
        sent.push(BindCommand::Bind(one.clone()));
    }

    Ok(sent)
}

#[cfg_attr(
    dylint_lib = "explicit028_no_search_in_a_loop",
    allow(
        explicit028_no_search_in_a_loop,
        reason = "both lists are the binds of one controller profile, which is tens of them and is written out in a file someone reads"
    )
)]
fn dropped(wanted: &[KeyBinding], handed: &[KeyBinding]) -> Result<Vec<BindCommand>, Never> {
    Ok(handed
        .iter()
        .filter(|one| !wanted.contains(one))
        .map(|one| BindCommand::Unbind(one.clone()))
        .collect())
}

pub fn told(sent: &[BindCommand]) -> Result<Went, Never> {
    let mut went = Went::Through;

    for one in sent {
        let Ok(said) = one.said();
        let Ok(done) = console_compositor::request(console_compositor::Request::Eval, &said);

        let Ok(through) = complained(done, &said);

        went = match (went, through) {
            (Went::Through, Went::Through) => Went::Through,
            (_, _) => Went::Nowhere,
        };
    }

    Ok(went)
}

fn complained(said: console_compositor::DispatchResult, about: &str) -> Result<Went, Never> {
    Ok(match said {
        console_compositor::DispatchResult::Success => Went::Through,
        console_compositor::DispatchResult::Failure(why) => {
            eprintln!("controller-desktop: the compositor would not take {about:?}: {why}");

            Went::Nowhere
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use console_input_bindings::moved::Tasks;

    fn ours() -> Vec<KeyBinding> {
        let Ok(table) = Table::ours();
        let Ok(wanted) = wanted(&table);

        wanted
    }

    fn one<'a>(every: &'a [KeyBinding], keys: &str) -> &'a KeyBinding {
        every.iter().find(|bind| bind.keys == keys).expect("a bind")
    }

    fn keys_of(word: &str, pressed: &str) -> String {
        let held = vec![word.to_string()];
        let Ok(said) = keys::bind(&held, pressed);

        said.expect("a key")
    }

    fn held(bind: &KeyBinding) -> console_compositor::BoundKey {
        console_compositor::BoundKey {
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
            !every.iter().any(|bind| bind.runs == "console-put-away"),
            "console-put-away is on the pad alone and has nothing to bind"
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
        let said = Tasks::read("[jobs]\nsettings = \"keyboard: super + j\"\n").expect("a table");
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
            assert_eq!(every.iter().filter(|other| other.about == bind.about).count(), 1, "{:?} names two binds, and neither can be put back alone", bind.about);
        }

        let keyboard: Vec<&KeyBinding> =
            every.iter().filter(|bind| bind.about.starts_with("keyboard ")).collect();

        assert_eq!(keyboard.len(), 2, "the keyboard is the job that is on two keys at once");
    }

    fn bound(sent: &[BindCommand]) -> Vec<String> {
        sent.iter()
            .filter_map(|says| match says {
                BindCommand::Bind(one) => Some(one.about.clone()),
                BindCommand::Unbind(_) => None,
            })
            .collect()
    }

    fn unbound(sent: &[BindCommand]) -> Vec<String> {
        sent.iter()
            .filter_map(|says| match says {
                BindCommand::Unbind(one) => Some(one.about.clone()),
                BindCommand::Bind(_) => None,
            })
            .collect()
    }

    fn these(every: &[KeyBinding]) -> Holding {
        Holding::These(every.iter().map(held).collect())
    }

    #[test]
    fn a_compositor_that_threw_them_away_is_handed_every_one_of_them() {
        let every = ours();
        let Ok(sent) = sent(&every, &[], &Holding::These(Vec::new()));

        assert_eq!(bound(&sent).len(), every.len(), "nothing held is everything sent");
        assert!(unbound(&sent).is_empty(), "there is nothing there to take off");
    }

    #[test]
    fn a_compositor_already_holding_them_is_handed_nothing_at_startup() {
        let every = ours();
        let Ok(sent) = sent(&every, &[], &these(&every));

        assert!(
            sent.is_empty(),
            "a daemon starting beside a compositor that holds these pushed them again: {sent:?}"
        );
    }

    #[test]
    fn a_key_held_more_than_once_is_taken_off_and_put_back_once() {
        let every = ours();
        let twice = one(&every, &tail("volume-up")).clone();

        let mut holding: Vec<console_compositor::BoundKey> = every.iter().map(held).collect();
        holding.push(held(&twice));

        let Ok(sent) = sent(&every, &every, &Holding::These(holding));

        assert_eq!(unbound(&sent), vec![twice.about.clone()]);
        assert_eq!(bound(&sent), vec![twice.about.clone()]);

        let before_given: Vec<&BindCommand> = sent.iter().take_while(|says| **says != BindCommand::Bind(twice.clone())).collect();

        assert!(before_given.contains(&&BindCommand::Unbind(twice.clone())), "it goes back on after it comes off, not before");
    }

    #[test]
    fn how_many_are_missing_and_how_many_are_doubled_is_said_apart() {
        let every = ours();
        let twice = one(&every, &tail("volume-up")).clone();

        let mut holding: Vec<console_compositor::BoundKey> =
            every.iter().filter(|bind| bind.about != twice.about).map(held).collect();

        holding.push(held(&twice));
        holding.push(held(&twice));

        let Ok(doubled) = standing(&every, &Holding::These(holding));

        assert_eq!(doubled.over, 1);
        assert_eq!(doubled.missing, 0);

        let Ok(nothing_held) = standing(&every, &Holding::These(Vec::new()));

        assert_eq!(nothing_held.missing, u32::try_from(every.len()).unwrap());
        assert_eq!(nothing_held.over, 0);
    }

    #[test]
    fn a_bind_moved_onto_another_key_is_taken_off_the_key_it_was_on() {
        let every = ours();
        let was = one(&every, &keys_of("super", "i")).clone();

        let said = Tasks::read("[jobs]\nsettings = \"keyboard: super + j\"\n").expect("a table");
        let Ok(table) = Table::of(&said);
        let Ok(now) = wanted(&table);

        let is = one(&now, &keys_of("super", "j")).clone();
        let Ok(sent) = sent(&now, &every, &these(&every));

        assert!(unbound(&sent).contains(&was.about), "the key it left is not put back");
        assert!(bound(&sent).contains(&is.about), "the key it moved onto is not handed over");
    }

    #[test]
    fn a_compositor_that_will_not_say_is_taken_at_its_last_word() {
        let every = ours();
        let would_not = Holding::HyprctlError("no socket".to_string());
        let Ok(again) = sent(&every, &every, &would_not);

        assert!(again.is_empty(), "what cannot be counted is left where it was: {again:?}");

        let Ok(first) = sent(&every, &[], &would_not);

        assert_eq!(bound(&first).len(), every.len(), "a first push still goes out");
    }
}
