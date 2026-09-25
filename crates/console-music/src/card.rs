//! The music, drawn.
//!
//! Two tabs: what is playing, and what there is to play -- and two ways in.
//! The panel the bar's note opens is the first tab alone, because what a hand
//! reaching for the bar wants is the song on now, and a glance at it. The app
//! is both, the library first: somewhere a person stays and walks a shelf of
//! nine hundred songs, which a panel that went away whenever the settings
//! came up was the wrong shape for. The player itself is
//! kew, running headless behind this, and every button here is one MPRIS call.
//! Nothing about a song is worked out in this program: the title, the artist
//! and the cover are what the player says they are.
//!
//! The Playing tab is one card about one song, and it is three rows: the
//! sleeve with the title and whose it is beside it, the bar, and the row of
//! buttons. It was five, one thing under another up the middle, which is a
//! card taller than the screen it opens on -- the buttons a hand came for were
//! off the bottom of it. The sleeve is written in characters off kew's ramp,
//! which is the same picture the player draws in a terminal and the one
//! alphabet the rest of the screen is in. Its square is held whether there is
//! a cover for it yet or not, because the player says what the song is a
//! moment before it says where its picture is. The bar says where in the song
//! you are, the width of the card, with the two clocks under its ends: a tap
//! lands on the moment, and the d-pad moves it a few seconds a press. Under it
//! are the five buttons, said five times so each is its own press for the
//! d-pad: shuffle on the left, repeat on the right, and the three that move
//! between songs in the middle. The card opens with the thumb on play, which
//! is what a hand came to it for.
//!
//! The head of the card is a thing to read rather than a thing to choose, so
//! the d-pad walks past it and nothing on the card draws as though it could be
//! pressed and then cannot.
//!
//! The Music tab stands under letters -- `console_panel::page::lettered` --
//! and that is why an album is no longer above the songs beside it. Folders
//! first was the player's order and it read as a shelf only while the folder
//! was short; a heading over each letter cannot be right about a list ordered
//! twice, so the whole folder is one alphabet now and what is an album is said
//! beside its name rather than by where it stands. The viewer's Media page
//! asks the same question of the same place, which is what keeps a file called
//! `_draft` under the same heading in both.
//!
//! Y is the files panel, standing on the song: renaming a song, copying it to
//! a stick and throwing it away all live there already, behind the same
//! button, and none of it is worth teaching this panel twice. It is offered on
//! the song playing now as well, so the one you are listening to does not have
//! to be found in a list of nine hundred first -- from any row of the card a
//! thumb can stand on, because the whole card is about the one song. The mark
//! that says so is drawn once, in the corner of the head, rather than on each
//! of the rows a thumb stands on: three of the same offer down one card about
//! one song reads as three different offers.
//!
//! The Music tab is the folder, and the line at the top of it is not a filter
//! on the folder. It looks at every song under the music folder and at what
//! each of them says about itself, which is what `music-index` reads and writes
//! down; the ordering of what it finds is `looking`'s.
//!
//! What is here is the machine: MPRIS, the folder, and the cover on the disk.
//! Where the thumb is standing, what is typed, and whether the library has
//! already been asked to read itself are `crate::update`, which is a
//! `console_program_contract::Program`.


use console_core_localization::positional;
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use console_panel::icons::Icon;
use crate::ascii;
use crate::library::{self, Kind, Thing};
use crate::looking::{self, Song};
use crate::player::{self, Order, Over, Playing, Sound};
use crate::update::{Closes, MusicEvent, MusicEffect, Music, Standing, closes};
use crate::library::folder;
use console_panel::actor::{self, Address, Answer};
use console_program_contract::{Arguments, Effect, Executable, Program as _, Topic, Update, Event};
use console_panel::page::{Aside, Bar, Handler, Active, Level, Page, Picture, ButtonPress, Row, Rows, Showing};
use console_panel::card::{Card, Door};
use console_panel::running;

const TALL: u32 = 8;

const ABOUT: &str = "Search songs, artists or albums";

const MANY: u32 = 120;

const SCRUB: i64 = 5_000_000;

enum Message {
    Event(MusicEvent, Answer<Vec<Effect<MusicEffect>>>),
    At(Answer<Standing>),
}

struct Actor(Standing);

impl actor::Machine for Actor {
    type Message = Message;

    fn step(self, message: Message) -> Self {
        match message {
            Message::Event(heard, answer) => {
                let Update { state, effects } = Music::update(&self.0, &Event::Custom(heard));
                let _ = answer.say(effects);

                Actor(state)
            }
            Message::At(answer) => {
                let _ = answer.say(self.0.clone());

                self
            },
        }
    }
}

type Panel = Address<Message>;

fn standing(held: &Panel) -> Result<Standing, Never> {
    Ok(match held.ask(Message::At) {
        Ok(standing) => standing,
        Err(_the_actor_has_gone) => {
            eprintln!("music-panel: the panel's own state is missing, so it drew as it opened");

            Standing::default()
        }
    })
}

fn decided(held: &Panel, heard: MusicEvent) -> Result<Vec<Effect<MusicEffect>>, Never> {
    Ok(match held.ask(|answer| Message::Event(heard, answer)) {
        Ok(effects) => effects,
        Err(_the_actor_has_gone) => {
            eprintln!("music-panel: the panel's own state is missing, so the press did nothing");

            Vec::new()
        }
    })
}

fn press(held: &Panel, heard: MusicEvent, showing: &dyn Showing) -> Result<(), Never> {
    let effects = decided(held, heard)?;

    for effect in effects {
        carry(&effect, showing)?;
    }

    Ok(())
}

fn carry(effect: &Effect<MusicEffect>, showing: &dyn Showing) -> Result<(), Never> {
    match effect {
        Effect::Custom(MusicEffect::Replace(row)) => {
            let Ok(row) = fitted(*row);

            showing.replace(row)
        }
        Effect::Custom(MusicEffect::Note(said)) => showing.note(said),
        Effect::Custom(MusicEffect::ForgetTyping) => showing.forget_typing(),
        Effect::Custom(MusicEffect::Shuffle(order)) => {
            player::shuffle(*order)?;
            showing.refresh();
        }
        Effect::Custom(MusicEffect::Repeat(over)) => {
            player::repeat(*over)?;
            showing.refresh();
        }

        Effect::Run(runs) => {
            let whole = whole(runs)?;

            showing.later(whole);
        }
        Effect::Spawn(runs) => {
            let whole = whole(runs)?;

            showing.leave_running(whole);
        }

        Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_) => {},
    }

    Ok(())
}

fn whole(runs: &console_program_contract::Command) -> Result<Vec<String>, Never> {
    let mut arguments = vec![
        match runs.program {
            Executable::External(program) => {
                let Ok(name) = program.name();

                name.to_string()
            }
            Executable::Internal(name) => name.to_string(),
        },
    ];

    arguments.extend(runs.arguments.clone());

    Ok(arguments)
}

fn playing_rows(held: &Panel) -> Result<Vec<Row>, Never> {
    let asked = player::playing()?;

    match asked.as_ref() {
        Some(playing) => match playing.sound != Sound::Stopped {
            true => playing_card(held, playing),
            false => {
                let Ok(row) = Row::nothing("Not Playing");

                Ok(vec![row])
            }
        },
        None => {
            let Ok(row) = Row::nothing("Not Playing");

            Ok(vec![row])
        }
    }
}

fn typed_in(held: &Panel) -> Result<String, Never> {
    let standing = standing(held)?;

    Ok(standing.typed.trim().to_string())
}

fn press_at(held: &Panel) -> Result<u32, Never> {
    let standing = standing(held)?;

    Ok(standing.press)
}

fn playing_card(held: &Panel, playing: &Playing) -> Result<Vec<Row>, Never> {
    let cover = match playing.art.as_deref() {
        Some(art) => ascii::read(art, TALL)?,
        None => None,
    };

    let drawn = match cover {
        Some(cover) => cover,
        None => ascii::room(TALL)?,
    };

    let under = match playing.album.trim().is_empty() {
        true => playing.artist.clone(),
        false => format!("{} \u{2014} {}", playing.artist, playing.album),
    };

    let markup = drawn.markup()?;
    let head = head_row(Head { sleeve: &markup, title: &playing.title, under: under.trim() })?;
    let scrub = scrub_row()?;
    let at = press_at(held)?;
    let transport = transport_row(held, playing, at)?;
    let walking = walking(held, transport)?;
    let Ok(chief) = walking.chief();
    let rows = vec![head, scrub, chief];

    Ok(rows)
}

fn walking(held: &Panel, row: Row) -> Result<Row, Never> {
    let Ok(of) =
        console_core_number_conversion::fitted::<_, u32>(row.buttons.as_ref().map_or(0, |across| across.presses.len()));
    let held = held.clone();

    row.leveled(Arc::new(move |by| {
        let _ = decided(&held, MusicEvent::Along { by, of });
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Head<'a> {
    sleeve: &'a str,
    title: &'a str,
    under: &'a str,
}

fn head_row(head: Head<'_>) -> Result<Row, Never> {
    let Head { sleeve, title, under } = head;

    Row::stacked(Picture::Written(sleeve.to_string()), title, Aside(under))
}

fn scrub_row() -> Result<Row, Never> {
    let position = player::position()?;
    let total = player::length()?;
    let done = clock(position)?;

    let whole = match total > 0 {
        true => clock(total)?,
        false => String::new(),
    };

    let scrub = Scrub { at: position, of: total };
    let bar = scrub_bar(scrub)?;
    let step = scrub_step(scrub)?;

    let Ok(nothing) = Handler::and_stay(|_| {});
    let Ok(row) = Row::new(&done, Aside(&whole), nothing);
    let Ok(row) = row.picturing(Picture::Bar(bar));
    let Ok(row) = row.leveled(step);

    row.seeking(|showing, frac| {
        let Ok(()) = player::seek(frac);

        showing.refresh();
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scrub {
    at: i64,
    of: i64,
}

fn scrub_bar(scrub: Scrub) -> Result<Bar, Never> {
    let Ok(at) = fitted(scrub.at.max(0));
    let Ok(of) = fitted(scrub.of.max(0));

    Ok(Bar { at, of })
}

fn clock(micros: i64) -> Result<String, Never> {
    let Ok(micros) = fitted::<i64, u64>(micros);

    positional(Duration::from_micros(micros))
}

fn scrub_step(scrub: Scrub) -> Result<Level, Never> {
    Ok(Arc::new(move |directory| {
        let Ok(step) = stepped(scrub, directory);

        let Ok(()) = player::seek(step);
    }))
}

fn stepped(scrub: Scrub, directory: i32) -> Result<f64, Never> {
    let Scrub { at, of } = scrub;
    let target = at.saturating_add(i64::from(directory).saturating_mul(SCRUB)).clamp(0, of);

    let Ok(along) = target.float();
    let Ok(whole) = of.max(1).float();

    Ok(along / whole)
}

fn transport_row(held: &Panel, playing: &Playing, at: u32) -> Result<Row, Never> {
    let shuffling = player::shuffling()?;
    let over = player::over()?;
    let shuffle = held.clone();
    let repeat = held.clone();

    let Ok(shuffles) = ButtonPress::new(
            Icon::Shuffle,
            match shuffling {
                Order::Any => Active::Yes,
                Order::AsListed => Active::No,
            },
            move |showing| {
                let Ok(shuffling) = player::shuffling();

                let Ok(()) = press(&shuffle, MusicEvent::Shuffling(shuffling), showing);
            },
    );
    let Ok(previous) = ButtonPress::new(Icon::Previous, Active::No, |showing| {
        let Ok(()) = player::previous();

        showing.refresh();
    });
    let Ok(playing) = ButtonPress::new(
            match playing.sound {
                Sound::Paused | Sound::Stopped => Icon::Play,
                Sound::Playing => Icon::Pause,
            },
            Active::No,
            |showing| {
                let Ok(()) = player::play_pause();

                showing.refresh();
            },
    );
    let Ok(playing) = playing.chief();
    let Ok(next) = ButtonPress::new(Icon::Next, Active::No, |showing| {
        let Ok(()) = player::next();

        showing.refresh();
    });
    let Ok(repeats) = ButtonPress::new(
            match over {
                Over::Again => Icon::RepeatSong,
                Over::On | Over::Round => Icon::Repeat,
            },
            match over {
                Over::On => Active::No,
                Over::Again | Over::Round => Active::Yes,
            },
            move |showing| {
                let Ok(over) = player::over();

                let Ok(()) = press(&repeat, MusicEvent::Repeating(over), showing);
            },
    );
    let presses = vec![shuffles, previous, playing, next, repeats];
    let Ok(at) = fitted(at);

    Row::pressing(presses, at)
}

fn music_rows(held: &Panel) -> Result<Vec<Row>, Never> {
    let typed = typed_in(held)?;
    let folder = folder()?;

    match typed.is_empty() {
        true => in_the_folder(held, &folder),
        false => answering(held, &folder, &typed),
    }
}

fn in_the_folder(held: &Panel, folder: &Path) -> Result<Vec<Row>, Never> {
    let mut things = library::things(folder)?;

    match things.is_empty() {
        true => {
            let Ok(row) = Row::nothing(&format!("No Music in {}", folder.display()));

            return Ok(vec![row]);
        }
        false => {},
    }

    things.sort_by(|one, other| {
        let Ok(first) = console_panel::page::standing(&one.name);
        let Ok(second) = console_panel::page::standing(&other.name);

        first
            .cmp(&second)
            .then_with(|| one.name.to_lowercase().cmp(&other.name.to_lowercase()))
    });

    let mut rows: Vec<Row> = Vec::new();

    for thing in &things {
        let row = chosen(held, thing)?;

        rows.push(row);
    }

    console_panel::page::lettered(rows)
}

fn answering(held: &Panel, folder: &Path, word: &str) -> Result<Vec<Row>, Never> {
    let songs = songs(folder)?;
    let found = looking::ranked(&songs, word)?;

    match found.is_empty() {
        true => {
            let Ok(row) = Row::nothing(&format!("No Results for \u{201c}{word}\u{201d}"));

            return Ok(vec![row]);
        }
        false => {},
    }

    let mut rows: Vec<Row> = Vec::new();
    let Ok(most) = console_core_number_conversion::index(MANY);

    for song in found.iter().take(most) {
        let row = played(held, song, folder)?;

        rows.push(row);
    }

    Ok(rows)
}

fn songs(folder: &Path) -> Result<Vec<Song>, Never> {
    let Ok(cache) = console_core_places::Base::Cache.hers();

    let cache = match cache {
        Some(cache) => cache,
        None => return looking::songs(folder, &library::things, &[]),
    };

    let at = looking::at(&cache)?;
    let said = std::fs::read_to_string(at);

    let known = match said {
        Ok(said) => looking::kept(&said)?,
        Err(_unreadable) => Vec::new(),
    };

    looking::songs(folder, &library::things, &known)
}

fn chosen(held: &Panel, thing: &Thing) -> Result<Row, Never> {
    let said = match thing.folder {
        true => "album",
        false => "",
    };
    let kind = match thing.folder {
        true => Kind::AFolder,
        false => Kind::ASong,
    };
    let plays = plays(held, &thing.path, kind)?;
    let shows = shown_in_the_files(held, &thing.path)?;
    let Ok(offering) = console_panel::page::shown_or_selected(&thing.name, &thing.path, move |showing| {
        let _ = shows(showing);
    });

    let Ok(row) = Row::new(&thing.name, Aside(said), plays);
    let Ok(row) = row.selectable(&thing.path.to_string_lossy());

    row.offering(offering)
}

fn played(held: &Panel, song: &Song, folder: &Path) -> Result<Row, Never> {
    let says = song.says()?;
    let aside = song.aside(folder)?;
    let plays = plays(held, &song.path, Kind::ASong)?;
    let shows = shown_in_the_files(held, &song.path)?;
    let Ok(offering) = console_panel::page::shown_or_selected(says, &song.path, move |showing| {
        let _ = shows(showing);
    });

    let Ok(row) = Row::new(says, Aside(&aside), plays);
    let Ok(row) = row.selectable(&song.path.to_string_lossy());

    row.offering(offering)
}

fn shown_in_the_files(
    held: &Panel,
    path: &Path,
) -> Result<impl Fn(&dyn Showing) -> bool + Send + Sync + 'static, Never> {
    let held = held.clone();
    let path = path.to_path_buf();

    Ok(move |_: &dyn Showing| {
        let Ok(effects) = decided(&held, MusicEvent::Shown(path.clone()));

        for effect in &effects {
            let Ok(runs) = effect.spawned();

            match runs {
                Some(runs) => {
                    let Ok(whole) = whole(runs);
                    let Ok(()) = running::left_running(&whole);
                }
                None => {},
            }
        }

        true
    })
}

fn plays(held: &Panel, path: &Path, folder: Kind) -> Result<Handler, Never> {
    let held = held.clone();
    let path = path.to_path_buf();

    Handler::and_stay(move |showing| {
        let Ok(()) = press(&held, MusicEvent::Chose { path: path.clone(), kind: folder }, showing);
    })
}

fn read_the_library(held: &Panel, showing: &dyn Showing) -> Result<(), Never> {
    let folder = folder()?;

    let songs = songs(&folder)?;
    let unread = looking::unread(&songs)?;

    press(held, MusicEvent::Arrived { unread }, showing)
}

fn playing_page(held: &Panel) -> Result<Page, Never> {
    let showing = held.clone();

    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = playing_rows(&showing);

        rows
    });
    let Ok(page) = Page::new("Now Playing", asked);
    let Ok(page) = page.in_the_middle();

    page.listening(Topic::Player, player::worth_moving_the_clock)
}

fn music_page(held: &Panel) -> Result<Page, Never> {
    let reading = held.clone();
    let arriving = held.clone();
    let backing = held.clone();
    let typing = held.clone();

    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = music_rows(&reading);

        rows
    });
    let Ok(page) = Page::new("Music", asked);
    let Ok(page) = page.trashing_selected();
    let Ok(page) = page.on_arriving(move |showing| {
        let Ok(()) = read_the_library(&arriving, showing);
    });
    let Ok(page) = page.on_back(move |showing| {
        let Ok(was) = standing(&backing);

        let Ok(()) = press(&backing, MusicEvent::Back, showing);

        let Ok(closes) = closes(&was);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });

    page.searching(ABOUT, move |showing, word| {
        let Ok(()) = press(&typing, MusicEvent::Typed(word.to_string()), showing);
    })
}


pub const WHO: &str = "music-panel";

pub const APP: &str = "music";

const DOOR: &str = "music";

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holds {
    Playing,
    Library,
}

fn held_as(holds: Holds) -> Result<Card, Never> {
    let initial = Music::init(&Arguments::default());
    let Ok(holding) = actor::supervise(move || Actor(initial.state.clone()));
    let held = holding.address.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(playing) = playing_page(&held);

        match holds {
            Holds::Playing => vec![playing],
            Holds::Library => {
                let Ok(music) = music_page(&held);

                vec![music, playing]
            },
        }
    }));

    card.shutting(Box::new(move || holding.shutdown()))
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
    held_as(Holds::Playing)
}

pub fn library(_argv: &[String]) -> Result<Card, Never> {
    held_as(Holds::Library)
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_panel::page::Same;

    fn card() -> Vec<Row> {
        let Ok(room) = ascii::room(TALL);

        let Ok(markup) = room.markup();

        let Ok(head) = head_row(Head {
            sleeve: &markup,
            title: "Blue Monday",
            under: "New Order \u{2014} Power, Corruption & Lies",
        });

        let Ok(scrub) = scrub_row();

        let Ok(play) = ButtonPress::new(Icon::Play, Active::No, |_| ());

        let Ok(pressing) = Row::pressing(vec![play], 0);

        vec![head, scrub, pressing]
    }

    fn heading(row: &Row) -> console_panel::page::Heading {
        let Ok(heading) = row.heading();

        heading
    }

    #[test]
    fn nothing_on_the_card_that_cannot_be_pressed_is_stood_on() {
        let rows = card();

        assert_eq!(
            rows.first().map(heading),
            Some(console_panel::page::Heading::Yes),
            "the head is stood on for nothing",
        );
        assert_eq!(
            rows.iter().filter(|row| heading(row) == console_panel::page::Heading::Yes).count(),
            1,
            "a card about one song reads more than its head",
        );
    }

    #[test]
    fn the_sleeve_keeps_its_room_before_there_is_a_cover_for_it() {
        let Ok(room) = ascii::room(TALL);

        let Ok(markup) = room.markup();

        let Ok(empty) =
            head_row(Head { sleeve: &markup, title: "Blue Monday", under: "New Order" });

        let Ok(plain) = room.plain();

        assert_eq!(u32::try_from(plain.lines().count()).unwrap(), TALL);
        assert!(plain.lines().all(|line| u32::try_from(line.chars().count()).unwrap() == room.columns));
        let Ok(bare) = Row::stacked(Picture::None, "Blue Monday", Aside("New Order"));

        assert_eq!(empty.looks_like(&bare), Ok(Same::No));
    }

    #[test]
    fn the_bar_steps_by_the_same_few_seconds_whatever_the_song_is() {
        let single = 3 * 60 * 1_000_000;
        let mix = 73 * 60 * 1_000_000;
        let moved = |total: i64| {
            let Ok(stepped) = stepped(Scrub { at: 30_000_000, of: total }, 1);
            let Ok(whole) = total.float();

            (stepped * whole - 30_000_000.0) / 1_000_000.0
        };

        assert!((moved(single) - 5.0).abs() < 0.001, "a single moved {}", moved(single));
        assert!((moved(mix) - 5.0).abs() < 0.001, "a mix moved {}", moved(mix));
    }

    #[test]
    fn the_bar_stops_at_both_ends_of_the_song() {
        let song = 3 * 60 * 1_000_000;

        let Ok(back) = stepped(Scrub { at: 1_000_000, of: song }, -1);

        let Ok(on) = stepped(Scrub { at: song - 1_000_000, of: song }, 1);

        assert!(back < f64::EPSILON);
        assert!((on - 1.0).abs() < f64::EPSILON);
    }
}
