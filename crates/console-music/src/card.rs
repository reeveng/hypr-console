//! The music, drawn.
//!
//! Two tabs: what is playing, and what there is to play. The player itself is
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
//! already been asked to read itself are `crate::pressing`, which is a
//! `console_program_contract::Program`.


use console_external_programs::Program;
use console_never::Never;
use console_number_conversion::{Float, fitted};
use std::path::Path;
use std::sync::Arc;

use console_panel::icons::Icon;
use gtk4::glib;
use crate::ascii;
use crate::library::{self, Kind, Thing};
use crate::looking::{self, Song};
use crate::player::{self, Order, Over, Playing};
use crate::pressing::{Closes, Heard, Its, Music, Standing, closes};
use crate::library::folder;
use console_panel::actor::{self, Addr, Answer};
use console_program_contract::{Argv, Doing, Named, Program as _, Turn, Word as Said};
use console_panel::page::{Bar, Does, Heading, InEffect, Level, Page, Picture, Press, Row, Rows, Showing, Watch};
use console_panel::card::{Card, Door};
use console_panel::running;

const TALL: usize = 8;

const ABOUT: &str = "Type a song, whose it is, or anything it says";

const MANY: usize = 120;

const SCRUB: i64 = 5_000_000;

enum Msg {
    Heard(Heard, Answer<Vec<Doing<Its>>>),
    At(Answer<Standing>),
}

struct Held(Standing);

impl actor::Machine for Held {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Heard(heard, answer) => {
                let Turn { now, doings } = Music::heard(&self.0, &Said::Its(heard));
                let _ = answer.say(doings);

                Held(now)
            }
            Msg::At(answer) => {
                let _ = answer.say(self.0.clone());

                self
            },
        }
    }
}

type Panel = Addr<Msg>;

fn standing(held: &Panel) -> Result<Standing, Never> {
    Ok(match held.ask(Msg::At) {
        Ok(standing) => standing,
        Err(_) => {
            eprintln!("music-panel: the panel's own state has gone, so it drew as it opened");

            Standing::default()
        }
    })
}

fn decided(held: &Panel, heard: Heard) -> Result<Vec<Doing<Its>>, Never> {
    Ok(match held.ask(|answer| Msg::Heard(heard, answer)) {
        Ok(doings) => doings,
        Err(_) => {
            eprintln!("music-panel: the panel's own state has gone, so the press did nothing");

            Vec::new()
        }
    })
}

fn press(held: &Panel, heard: Heard, showing: &dyn Showing) -> Result<(), Never> {
    let doings = decided(held, heard)?;

    for doing in doings {
        carry(&doing, showing)?;
    }

    Ok(())
}

fn carry(doing: &Doing<Its>, showing: &dyn Showing) -> Result<(), Never> {
    match doing {
        Doing::Its(Its::Replace(row)) => showing.replace(*row),
        Doing::Its(Its::Note(said)) => showing.note(said),
        Doing::Its(Its::ForgetTyping) => showing.forget_typing(),
        Doing::Its(Its::Shuffle(order)) => {
            player::shuffle(*order)?;
            showing.refresh();
        }
        Doing::Its(Its::Repeat(over)) => {
            player::repeat(*over)?;
            showing.refresh();
        }

        Doing::Ask(runs) => {
            let whole = whole(runs)?;

            showing.later(whole);
        }
        Doing::Start(runs) => {
            let whole = whole(runs)?;

            showing.leave_running(whole);
        }

        Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Write(_)
        | Doing::Say(_)
        | Doing::Print(_)
        | Doing::Stop(_) => {},
    }

    Ok(())
}

fn started(doing: &Doing<Its>) -> Result<Option<&console_program_contract::Runs>, Never> {
    Ok(match doing {
        Doing::Start(runs) => Some(runs),

        Doing::Its(_)
        | Doing::Ask(_)
        | Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Write(_)
        | Doing::Say(_)
        | Doing::Print(_)
        | Doing::Stop(_) => None,
    })
}

fn whole(runs: &console_program_contract::Runs) -> Result<Vec<String>, Never> {
    let mut argv = vec![
        match runs.program {
            Named::Theirs(program) => {
                let Ok(name) = program.name();

                name.to_string()
            }
            Named::Ours(name) => name.to_string(),
        },
    ];

    argv.extend(runs.argv.clone());

    Ok(argv)
}

fn playing_rows(held: &Panel) -> Result<Vec<Row>, Never> {
    let asked = player::playing()?;

    match asked.as_ref() {
        Some(playing) if !playing.stopped => playing_card(held, playing),
        Some(_) | None => {
            let Ok(row) = Row::nothing("Nothing is playing");

            Ok(vec![row])
        }
    }
}

fn typed_in(held: &Panel) -> Result<String, Never> {
    let standing = standing(held)?;

    Ok(standing.typed.trim().to_string())
}

fn press_at(held: &Panel) -> Result<usize, Never> {
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
    let head = head_row(&markup, &playing.title, under.trim())?;
    let scrub = scrub_row()?;
    let at = press_at(held)?;
    let transport = transport_row(held, playing, at)?;
    let walking = walking(held, transport)?;
    let Ok(chief) = walking.chief();
    let rows = vec![head, scrub, chief];

    about_the_song(rows, playing.path.as_deref(), &|path| {
        let Ok(offered) = shown_in_the_files(held, path);

        Arc::new(offered)
    })
}

type Offered = Arc<dyn Fn(&dyn Showing) -> bool + Send + Sync>;

fn about_the_song(
    rows: Vec<Row>,
    path: Option<&Path>,
    offers: &dyn Fn(&Path) -> Offered,
) -> Result<Vec<Row>, Never> {
    let Some(path) = path else { return Ok(rows) };

    Ok(rows
        .into_iter()
        .map(|row| {
            let offered = offers(path);
            let Ok(carrying) = row.offering(move |showing| offered(showing));
            let Ok(heading) = carrying.heading();

            match heading {
                Heading::Yes => carrying,
                Heading::No => {
                    let Ok(ended) = carrying.ended("", "");

                    ended
                }
            }
        })
        .collect())
}

fn walking(held: &Panel, row: Row) -> Result<Row, Never> {
    let of = row.across.as_ref().map_or(0, |across| across.presses.len());
    let held = held.clone();

    row.levelled(Arc::new(move |by| {
        let _ = decided(&held, Heard::Along { by, of });
    }))
}

fn head_row(sleeve: &str, title: &str, under: &str) -> Result<Row, Never> {
    Row::stacked(Picture::Written(sleeve.to_string()), title, under)
}

fn scrub_row() -> Result<Row, Never> {
    let pos = player::position()?;
    let total = player::length()?;
    let done = clock(pos)?;

    let whole = match total > 0 {
        true => clock(total)?,
        false => String::new(),
    };

    let bar = scrub_bar(pos, total)?;
    let step = scrub_step(pos, total)?;

    let Ok(nothing) = Does::and_stay(|_| {});
    let Ok(row) = Row::new(&done, &whole, nothing);
    let Ok(row) = row.picturing(Picture::Bar(bar));
    let Ok(row) = row.levelled(step);

    row.seeking(|showing, frac| {
        let Ok(()) = player::seek(frac);

        showing.refresh();
    })
}

fn scrub_bar(pos: i64, total: i64) -> Result<Bar, Never> {
    let Ok(at) = fitted(pos.max(0));
    let Ok(of) = fitted(total.max(0));

    Ok(Bar { at, of })
}

fn clock(micros: i64) -> Result<String, Never> {
    let seconds = micros.max(0).saturating_div(1_000_000);
    let (hours, minutes, seconds) = (
        seconds.saturating_div(3600),
        seconds.saturating_div(60).wrapping_rem(60),
        seconds.wrapping_rem(60),
    );

    Ok(match hours > 0 {
        true => format!("{hours}:{minutes:02}:{seconds:02}"),
        false => format!("{minutes}:{seconds:02}"),
    })
}

fn scrub_step(pos: i64, total: i64) -> Result<Level, Never> {
    Ok(Arc::new(move |dir| {
        let Ok(step) = stepped(pos, total, dir);

        let Ok(()) = player::seek(step);
    }))
}

fn stepped(pos: i64, total: i64, dir: i32) -> Result<f64, Never> {
    let target = pos.saturating_add(i64::from(dir).saturating_mul(SCRUB)).clamp(0, total);

    let Ok(along) = target.float();
    let Ok(whole) = total.max(1).float();

    Ok(along / whole)
}

fn transport_row(held: &Panel, playing: &Playing, at: usize) -> Result<Row, Never> {
    let shuffling = player::shuffling()?;
    let over = player::over()?;
    let shuffle = held.clone();
    let repeat = held.clone();

    let Ok(shuffles) = Press::new(
            Icon::Shuffle,
            match shuffling {
                Order::Any => InEffect::Yes,
                Order::AsListed => InEffect::No,
            },
            move |showing| {
                let Ok(shuffling) = player::shuffling();

                let Ok(()) = press(&shuffle, Heard::Shuffling(shuffling), showing);
            },
    );
    let Ok(previous) = Press::new(Icon::Previous, InEffect::No, |showing| {
        let Ok(()) = player::previous();

        showing.refresh();
    });
    let Ok(playing) = Press::new(
            match playing.paused {
                true => Icon::Play,
                false => Icon::Pause,
            },
            InEffect::No,
            |showing| {
                let Ok(()) = player::play_pause();

                showing.refresh();
            },
    );
    let Ok(playing) = playing.chief();
    let Ok(next) = Press::new(Icon::Next, InEffect::No, |showing| {
        let Ok(()) = player::next();

        showing.refresh();
    });
    let Ok(repeats) = Press::new(
            match over {
                Over::Again => Icon::RepeatSong,
                Over::On | Over::Round => Icon::Repeat,
            },
            match over {
                Over::On => InEffect::No,
                Over::Again | Over::Round => InEffect::Yes,
            },
            move |showing| {
                let Ok(over) = player::over();

                let Ok(()) = press(&repeat, Heard::Repeating(over), showing);
            },
    );
    let presses = vec![shuffles, previous, playing, next, repeats];

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
    let things = library::things(folder)?;

    match things.is_empty() {
        true => {
            let Ok(row) = Row::nothing(&format!("Nothing in {}", folder.display()));

            return Ok(vec![row]);
        }
        false => {},
    }

    let mut rows: Vec<Row> = Vec::new();

    for thing in &things {
        let row = chosen(held, thing)?;

        rows.push(row);
    }

    Ok(rows)
}

fn answering(held: &Panel, folder: &Path, word: &str) -> Result<Vec<Row>, Never> {
    let songs = songs(folder)?;
    let found = looking::ranked(&songs, word)?;

    match found.is_empty() {
        true => {
            let Ok(row) = Row::nothing(&format!("Nothing here answers to {word}"));

            return Ok(vec![row]);
        }
        false => {},
    }

    let mut rows: Vec<Row> = Vec::new();

    for song in found.iter().take(MANY) {
        let row = played(held, song, folder)?;

        rows.push(row);
    }

    Ok(rows)
}

fn songs(folder: &Path) -> Result<Vec<Song>, Never> {
    let at = looking::at(&glib::user_cache_dir())?;
    let said = std::fs::read_to_string(at);

    let known = match said {
        Ok(said) => looking::kept(&said)?,
        Err(_) => Vec::new(),
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
    let offering = shown_in_the_files(held, &thing.path)?;

    let Ok(row) = Row::new(&thing.name, said, plays);

    row.offering(offering)
}

fn played(held: &Panel, song: &Song, folder: &Path) -> Result<Row, Never> {
    let says = song.says()?;
    let aside = song.aside(folder)?;
    let plays = plays(held, &song.path, Kind::ASong)?;
    let offering = shown_in_the_files(held, &song.path)?;

    let Ok(row) = Row::new(says, &aside, plays);

    row.offering(offering)
}

fn shown_in_the_files(
    held: &Panel,
    path: &Path,
) -> Result<impl Fn(&dyn Showing) -> bool + Send + Sync + 'static, Never> {
    let held = held.clone();
    let path = path.to_path_buf();

    Ok(move |_: &dyn Showing| {
        let Ok(doings) = decided(&held, Heard::Shown(path.clone()));

        for doing in &doings {
            let Ok(runs) = started(doing);

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

fn plays(held: &Panel, path: &Path, folder: Kind) -> Result<Does, Never> {
    let held = held.clone();
    let path = path.to_path_buf();

    Does::and_stay(move |showing| {
        let Ok(()) = press(&held, Heard::Chose { path: path.clone(), kind: folder }, showing);
    })
}

fn read_the_library(held: &Panel, showing: &dyn Showing) -> Result<(), Never> {
    let folder = folder()?;

    library::tell_kew(&folder)?;

    let songs = songs(&folder)?;
    let unread = looking::unread(&songs)?;

    press(held, Heard::Arrived { unread }, showing)
}

fn pages(held: &Panel) -> Result<Vec<Page>, Never> {
    let showing = held.clone();
    let music = music_page(held)?;
    let Ok(shell) = Program::Sh.name();

    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = playing_rows(&showing);

        rows
    });
    let Ok(page) = Page::new("Playing", asked);
    let Ok(page) = page.in_the_middle();
    let Ok(watch) = Watch::on(
        &[shell, "-c", "while true; do echo tick; sleep 1; done"],
        "tick",
    );
    let Ok(playing) = page.watching(watch);

    Ok(vec![playing, music])
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
    let Ok(page) = page.on_arriving(move |showing| {
        let Ok(()) = read_the_library(&arriving, showing);
    });
    let Ok(page) = page.on_back(move |showing| {
        let Ok(was) = standing(&backing);

        let Ok(()) = press(&backing, Heard::Back, showing);

        let Ok(closes) = closes(&was);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });

    page.searching(ABOUT, move |showing, word| {
        let Ok(()) = press(&typing, Heard::Typed(word.to_string()), showing);
    })
}


pub const WHO: &str = "music-panel";

const DOOR: &str = "music";

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
    let opening = Music::opening(&Argv::default());
    let Ok(holding) = actor::supervise(move || Held(opening.state.clone()));
    let held = holding.addr.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }));

    card.shutting(Box::new(move || holding.shutdown()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_panel::page::Same;

    fn stub(_path: &Path) -> Offered {
        Arc::new(|_showing: &dyn Showing| true)
    }

    fn card() -> Vec<Row> {
        let Ok(room) = ascii::room(TALL);

        let Ok(markup) = room.markup();

        let Ok(head) =
            head_row(&markup, "Blue Monday", "New Order \u{2014} Power, Corruption & Lies");

        let Ok(scrub) = scrub_row();

        let Ok(play) = Press::new(Icon::Play, InEffect::No, |_| ());

        let Ok(pressing) = Row::pressing(vec![play], 0);

        vec![head, scrub, pressing]
    }

    fn heading(row: &Row) -> Heading {
        let Ok(heading) = row.heading();

        heading
    }

    fn about(rows: Vec<Row>, path: Option<&Path>) -> Vec<Row> {
        let Ok(rows) = about_the_song(rows, path, &stub);

        rows
    }

    #[test]
    fn y_reaches_the_song_from_every_row_the_d_pad_can_stand_on() {
        let rows = about(card(), Some(Path::new("/music/blue-monday.flac")));
        let standing: Vec<bool> = rows
            .iter()
            .filter(|row| heading(row) == Heading::No)
            .map(|row| row.more.is_some())
            .collect();

        assert_eq!(standing, [true, true], "a row a thumb stands on with nothing behind Y");
    }

    #[test]
    fn a_player_that_will_not_say_which_file_it_is_offers_nothing() {
        let rows = about(card(), None);

        assert!(rows.iter().all(|row| row.more.is_none()));
    }

    #[test]
    fn nothing_on_the_card_that_cannot_be_pressed_is_stood_on() {
        let rows = card();

        assert_eq!(
            rows.first().map(heading),
            Some(Heading::Yes),
            "the head is stood on for nothing",
        );
        assert_eq!(
            rows.iter().filter(|row| heading(row) == Heading::Yes).count(),
            1,
            "a card about one song reads more than its head",
        );
    }

    #[test]
    fn the_song_is_offered_once_and_from_everywhere_on_the_card() {
        let rows = about(card(), Some(Path::new("/music/blue-monday.flac")));
        let marked: Vec<bool> = rows.iter().map(|row| row.ends.is_none()).collect();

        assert!(rows.iter().all(|row| row.more.is_some()), "a row of the card without Y");
        assert_eq!(marked, [true, false, false], "the offer is drawn other than in the corner");
    }

    #[test]
    fn the_sleeve_keeps_its_room_before_there_is_a_cover_for_it() {
        let Ok(room) = ascii::room(TALL);

        let Ok(markup) = room.markup();

        let Ok(empty) = head_row(&markup, "Blue Monday", "New Order");

        let Ok(plain) = room.plain();

        assert_eq!(plain.lines().count(), TALL);
        assert!(plain.lines().all(|line| line.chars().count() == room.cols));
        let Ok(bare) = Row::stacked(Picture::None, "Blue Monday", "New Order");

        assert_eq!(empty.looks_like(&bare), Ok(Same::No));
    }

    #[test]
    fn the_bar_steps_by_the_same_few_seconds_whatever_the_song_is() {
        let single = 3 * 60 * 1_000_000;
        let mix = 73 * 60 * 1_000_000;
        let moved = |total: i64| {
            let Ok(stepped) = stepped(30_000_000, total, 1);
            let Ok(whole) = total.float();

            (stepped * whole - 30_000_000.0) / 1_000_000.0
        };

        assert!((moved(single) - 5.0).abs() < 0.001, "a single moved {}", moved(single));
        assert!((moved(mix) - 5.0).abs() < 0.001, "a mix moved {}", moved(mix));
    }

    #[test]
    fn the_bar_stops_at_both_ends_of_the_song() {
        let song = 3 * 60 * 1_000_000;

        let Ok(back) = stepped(1_000_000, song, -1);

        let Ok(on) = stepped(song - 1_000_000, song, 1);

        assert!(back < f64::EPSILON);
        assert!((on - 1.0).abs() < f64::EPSILON);
    }
}
