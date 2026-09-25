//! A song is described by two programs, and they are asked at once.
//!
//! Each of ffprobe and ffmpeg spends most of what it costs on the device
//! loading ffmpeg's libraries, so asking one and then the other was every
//! press of next paying that twice before a sound was made. The two stand-ins
//! here each wait for the other to have started before they answer, which is
//! a pair that can only both answer when they are running together: asked one
//! after the other, the first gives up waiting and the song says nothing.

use console_music_player::bus::{Described, described};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const PROBE: &str = r#"#!/bin/sh
touch "$MEETING/probe"
for _ in $(seq 100); do
    [ -e "$MEETING/copy" ] && break
    sleep 0.05
done
[ -e "$MEETING/copy" ] || exit 1
echo '{"format":{"duration":"61.0","tags":{"title":"Sweetness","artist":"Reay"}},"streams":[{"codec_type":"audio"}]}'
"#;

const COPY: &str = r#"#!/bin/sh
touch "$MEETING/copy"
for _ in $(seq 100); do
    [ -e "$MEETING/probe" ] && break
    sleep 0.05
done
[ -e "$MEETING/probe" ] || exit 1
for last; do :; done
echo cover > "$last"
"#;

fn stand_in(bin: &Path, name: &str, script: &str) {
    let at = bin.join(name);

    fs::write(&at, script).expect("the stand-in is written");
    fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("the stand-in runs");
}

fn room() -> PathBuf {
    let here = std::env::temp_dir().join(format!("the-describing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&here);

    for folder in ["bin", "meeting", "cache"] {
        fs::create_dir_all(here.join(folder)).expect("the room is made");
    }

    stand_in(&here.join("bin"), "ffprobe", PROBE);
    stand_in(&here.join("bin"), "ffmpeg", COPY);

    here
}

#[test]
fn the_tags_and_the_cover_are_asked_for_together() {
    let here = room();
    let path = format!("{}:{}", here.join("bin").display(), std::env::var("PATH").expect("a PATH"));

    // SAFETY: this file is its own test binary with this one test in it, and
    // the environment is written before anything it starts could read it.
    unsafe {
        std::env::set_var("PATH", path);
        std::env::set_var("MEETING", here.join("meeting"));
        std::env::set_var("XDG_CACHE_HOME", here.join("cache"));
    }

    let Ok(Described { said, art }) = described(&here.join("song.opus"), 1);

    let _ = fs::remove_dir_all(&here);

    assert_eq!(said.artist, "Reay", "the tags were not read while the cover was being copied");
    assert!(art.is_some(), "the cover was not copied while the tags were being read");
}
