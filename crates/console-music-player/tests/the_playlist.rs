//! The three faults kew had here, asked of the thing that replaces it.
//!
//! Each of these was watched on the device against a library of six, and each
//! cost a fork of somebody else's C to fix. They are written down as presses
//! rather than as a claim about a permutation, because what a person does is
//! press next and see whether the song changes.

use console_music_player::playlist::{Moved, Order, Over, Playlist};
use std::path::PathBuf;

const SONGS: usize = 6;

fn library() -> Vec<PathBuf> {
    ["a", "b", "c", "d", "e", "f"]
        .iter()
        .map(|name| PathBuf::from(format!("/music/{name}.opus")))
        .collect()
}

fn names() -> Vec<String> {
    let mut held: Vec<String> =
        library().iter().map(|path| path.to_string_lossy().to_string()).collect();

    held.sort();

    held
}

fn song(list: &Playlist) -> String {
    let Ok(song) = list.song();

    song.map(|path| path.to_string_lossy().to_string()).unwrap_or_default()
}

fn walked(list: &mut Playlist) -> Vec<String> {
    let mut seen = vec![song(list)];

    for _ in 1..SONGS {
        let Ok(_the_walk_is_what_is_asserted) = list.onward();

        seen.push(song(list));
    }

    seen.sort();

    seen
}

#[test]
fn next_plays_each_song_once_and_then_comes_round() {
    let Ok(mut list) = Playlist::of(library());
    let first = song(&list);
    let mut played = vec![first.clone()];

    for _ in 1..SONGS {
        let Ok(moved) = list.onward();

        assert_eq!(moved, Moved::Yes);
        played.push(song(&list));
    }

    let mut once = played.clone();

    once.sort();
    once.dedup();

    assert_eq!(once.len(), SONGS);

    let Ok(moved) = list.onward();

    assert_eq!(moved, Moved::Yes);
    assert_eq!(song(&list), first);
}

#[test]
fn shuffle_off_puts_the_library_back_rather_than_nothing() {
    let Ok(mut list) = Playlist::of(library());
    let Ok(()) = list.shuffling(Order::Any, 7);
    let Ok(()) = list.shuffling(Order::AsListed, 7);

    assert_eq!(walked(&mut list), names());
}

#[test]
fn shuffling_keeps_every_song_and_keeps_it_once() {
    let Ok(mut list) = Playlist::of(library());
    let Ok(()) = list.shuffling(Order::Any, 99);

    assert_eq!(walked(&mut list), names());
}

#[test]
fn the_press_after_the_shuffle_button_moves() {
    let Ok(mut list) = Playlist::of(library());
    let before = song(&list);
    let Ok(()) = list.shuffling(Order::Any, 11);
    let Ok(moved) = list.onward();

    assert_eq!(moved, Moved::Yes);
    assert_ne!(song(&list), before);
}

#[test]
fn a_toggle_keeps_the_song_that_is_playing() {
    let Ok(mut list) = Playlist::of(library());
    let Ok(_walking_to_somewhere_that_is_not_the_first) = list.onward();
    let Ok(_walking_to_somewhere_that_is_not_the_first) = list.onward();

    let playing = song(&list);
    let Ok(()) = list.shuffling(Order::Any, 3);

    assert_eq!(song(&list), playing);

    let Ok(()) = list.shuffling(Order::AsListed, 3);

    assert_eq!(song(&list), playing);
}

#[test]
fn a_list_with_no_repeat_stops_at_its_end() {
    let Ok(mut list) = Playlist::of(library());
    let Ok(()) = list.repeat(Over::On);

    for _ in 1..SONGS {
        let Ok(moved) = list.onward();

        assert_eq!(moved, Moved::Yes);
    }

    let last = song(&list);
    let Ok(moved) = list.onward();

    assert_eq!(moved, Moved::No);
    assert_eq!(song(&list), last);
}

#[test]
fn repeating_one_track_is_the_ending_and_not_the_button() {
    let Ok(mut list) = Playlist::of(library());
    let Ok(()) = list.repeat(Over::Again);

    let playing = song(&list);
    let Ok(moved) = list.finished();

    assert_eq!(moved, Moved::Yes);
    assert_eq!(song(&list), playing);

    let Ok(moved) = list.onward();

    assert_eq!(moved, Moved::Yes);
    assert_ne!(song(&list), playing);
}

#[test]
fn a_song_opened_is_the_one_that_plays_and_the_library_follows_it() {
    let songs = library();
    let asked = songs.get(3).cloned().unwrap_or_default();
    let Ok(mut list) = Playlist::opened(songs, &asked);

    assert_eq!(song(&list), asked.to_string_lossy().to_string());
    assert_eq!(walked(&mut list), names());
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
fn previous_at_the_start_comes_round_to_the_end() {
    let Ok(mut list) = Playlist::of(library());
    let Ok(moved) = list.back();

    assert_eq!(moved, Moved::Yes);

    let Ok(()) = list.repeat(Over::On);
    let Ok(mut standing) = Playlist::of(library());
    let Ok(()) = standing.repeat(Over::On);

    let first = song(&standing);
    let Ok(moved) = standing.back();

    assert_eq!(moved, Moved::No);
    assert_eq!(song(&standing), first);
}
