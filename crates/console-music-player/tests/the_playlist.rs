//! The three faults kew had here, asked of the thing that replaces it.
//!
//! Each of these was watched on the device against a library of six, and each
//! cost a fork of someone else's C to fix. They are written down as presses
//! rather than as a claim about a permutation, because what a person does is
//! press next and see whether the song changes.

use std::error::Error;
use std::path::PathBuf;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_music_player::playlist::{Moved, Order, Over, Playlist};

const SONGS: u32 = 6;

fn library() -> Result<Vec<PathBuf>, Never> {
    Ok(["a", "b", "c", "d", "e", "f"]
        .iter()
        .map(|name| PathBuf::from(format!("/music/{name}.opus")))
        .collect())
}

fn names() -> Result<Vec<String>, Never> {
    let Ok(library) = library();
    let mut held: Vec<String> = library.iter().map(|path| path.to_string_lossy().to_string()).collect();

    held.sort();

    Ok(held)
}

fn song(list: &Playlist) -> Result<String, Box<dyn Error>> {
    let Ok(song) = list.song();
    let path = song.ok_or("nothing is playing")?;

    Ok(path.to_string_lossy().to_string())
}

fn walked(list: &mut Playlist) -> Result<Vec<String>, Box<dyn Error>> {
    let first = song(list)?;
    let mut seen = vec![first];

    for _ in 1..SONGS {
        let Ok(_the_walk_is_what_is_asserted) = list.onward();
        let now = song(list)?;

        seen.push(now);
    }

    seen.sort();

    Ok(seen)
}

fn walked_through_every_song(list: &mut Playlist) -> Result<(), Box<dyn Error>> {
    let walked = walked(list)?;
    let Ok(names) = names();

    assert_eq!(walked, names);
    Ok(())
}

#[test]
fn next_plays_each_song_once_and_then_comes_round() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let first = song(&list)?;
    let mut played = vec![first.clone()];

    for _ in 1..SONGS {
        let Ok(moved) = list.onward();
        let now = song(&list)?;

        assert_eq!(moved, Moved::Yes);

        played.push(now);
    }

    let mut once = played.clone();

    once.sort();
    once.dedup();

    assert_eq!(fitted::<_, u32>(once.len()), Ok(SONGS));

    let Ok(moved) = list.onward();
    let now = song(&list)?;

    assert_eq!(moved, Moved::Yes);
    assert_eq!(now, first);
    Ok(())
}

#[test]
fn shuffle_off_puts_the_library_back_rather_than_nothing() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let Ok(()) = list.shuffling(Order::Any, 7);
    let Ok(()) = list.shuffling(Order::AsListed, 7);

    walked_through_every_song(&mut list)
}

#[test]
fn shuffling_keeps_every_song_and_keeps_it_once() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let Ok(()) = list.shuffling(Order::Any, 99);

    walked_through_every_song(&mut list)
}

#[test]
fn the_press_after_the_shuffle_button_moves() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let before = song(&list)?;
    let Ok(()) = list.shuffling(Order::Any, 11);
    let Ok(moved) = list.onward();
    let now = song(&list)?;

    assert_eq!(moved, Moved::Yes);
    assert_ne!(now, before);
    Ok(())
}

#[test]
fn a_toggle_keeps_the_song_that_is_playing() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let Ok(_walking_to_somewhere_that_is_not_the_first) = list.onward();
    let Ok(_walking_to_somewhere_that_is_not_the_first) = list.onward();
    let playing = song(&list)?;
    let Ok(()) = list.shuffling(Order::Any, 3);
    let shuffled = song(&list)?;

    assert_eq!(shuffled, playing);

    let Ok(()) = list.shuffling(Order::AsListed, 3);
    let listed = song(&list)?;

    assert_eq!(listed, playing);
    Ok(())
}

#[test]
fn a_list_with_no_repeat_stops_at_its_end() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let Ok(()) = list.repeat(Over::On);

    for _ in 1..SONGS {
        let Ok(moved) = list.onward();

        assert_eq!(moved, Moved::Yes);
    }

    let last = song(&list)?;
    let Ok(moved) = list.onward();
    let now = song(&list)?;

    assert_eq!(moved, Moved::No);
    assert_eq!(now, last);
    Ok(())
}

#[test]
fn repeating_one_track_is_the_ending_and_not_the_button() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let Ok(()) = list.repeat(Over::Again);
    let playing = song(&list)?;
    let Ok(moved) = list.finished();
    let again = song(&list)?;

    assert_eq!(moved, Moved::Yes);
    assert_eq!(again, playing);

    let Ok(moved) = list.onward();
    let next = song(&list)?;

    assert_eq!(moved, Moved::Yes);
    assert_ne!(next, playing);
    Ok(())
}

#[test]
fn a_song_opened_is_the_one_that_plays_and_the_library_follows_it() -> Result<(), Box<dyn Error>> {
    let Ok(songs) = library();
    let asked = songs.get(3).cloned().ok_or("the library has no fourth song")?;
    let Ok(mut list) = Playlist::opened(songs, &asked);
    let playing = song(&list)?;

    assert_eq!(playing, asked.to_string_lossy().to_string());

    walked_through_every_song(&mut list)
}

#[test]
fn an_empty_library_moves_nowhere_rather_than_saying_it_did() {
    let Ok(mut list) = Playlist::of(Vec::new());
    let Ok(onward) = list.onward();
    let Ok(back) = list.back();
    let Ok(finished) = list.finished();
    let Ok(song) = list.song();

    assert_eq!((onward, back, finished), (Moved::No, Moved::No, Moved::No));
    assert_eq!(song, None);
}

#[test]
fn previous_at_the_start_comes_round_to_the_end() -> Result<(), Box<dyn Error>> {
    let Ok(library) = library();
    let Ok(mut list) = Playlist::of(library);
    let Ok(moved) = list.back();

    assert_eq!(moved, Moved::Yes);

    let Ok(()) = list.repeat(Over::On);
    let Ok(library) = self::library();
    let Ok(mut standing) = Playlist::of(library);
    let Ok(()) = standing.repeat(Over::On);
    let first = song(&standing)?;
    let Ok(moved) = standing.back();
    let now = song(&standing)?;

    assert_eq!(moved, Moved::No);
    assert_eq!(now, first);
    Ok(())
}
