//! A song is described by two programs, and they are asked at once.
//!
//! Each of ffprobe and ffmpeg spends most of what it costs on the device
//! loading ffmpeg's libraries, so asking one and then the other was every
//! press of next paying that twice before a sound was made. The two stand-ins
//! here each wait for the other to have started before they answer, which is
//! a pair that can only both answer when they are running together: asked one
//! after the other, the first gives up waiting and the song says nothing.

use console_music_player::bus::{Described, described};
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

struct StandIn {
    name: &'static str,
    script: &'static str,
}

const PROBE: StandIn = StandIn {
    name: "ffprobe",
    script: r#"#!/bin/sh
touch "$MEETING/probe"
for _ in $(seq 100); do
    [ -e "$MEETING/copy" ] && break
    sleep 0.05
done
[ -e "$MEETING/copy" ] || exit 1
echo '{"format":{"duration":"61.0","tags":{"title":"Sweetness","artist":"Reay"}},"streams":[{"codec_type":"audio"}]}'
"#,
};

const COPY: StandIn = StandIn {
    name: "ffmpeg",
    script: r#"#!/bin/sh
touch "$MEETING/copy"
for _ in $(seq 100); do
    [ -e "$MEETING/probe" ] && break
    sleep 0.05
done
[ -e "$MEETING/probe" ] || exit 1
for last; do :; done
echo cover > "$last"
"#,
};

fn stand_in(bin: &Path, stand_in: &StandIn) -> Result<(), Box<dyn Error>> {
    let at = bin.join(stand_in.name);

    console_core_atomic_writes::whole(&at, stand_in.script.as_bytes())?;

    fs::set_permissions(&at, fs::Permissions::from_mode(0o755))?;

    Ok(())
}

fn room() -> Result<PathBuf, Box<dyn Error>> {
    let here = console_core_temporary_directories::fresh("the-describing")?;

    for folder in ["bin", "meeting", "cache"] {
        fs::create_dir_all(here.join(folder))?;
    }

    stand_in(&here.join("bin"), &PROBE)?;

    stand_in(&here.join("bin"), &COPY)?;

    Ok(here)
}

#[test]
#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "the stand-ins go in front of the PATH this test binary was started with, so that PATH is read here to be handed on whole"
    )
)]
#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the player finds ffprobe and ffmpeg on PATH and its cache under XDG_CACHE_HOME, and this binary is its own machine: the one test in it points those at a room of its own before anything reads them"
    )
)]
fn the_tags_and_the_cover_are_asked_for_together() -> Result<(), Box<dyn Error>> {
    let here = room()?;
    let inherited = std::env::var("PATH")?;
    let path = format!("{}:{inherited}", here.join("bin").display());

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

    Ok(())
}
