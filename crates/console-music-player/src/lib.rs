//! The player this desktop owns, in place of the one it forked.
//!
//! Everything the Music panel's buttons do was kew's: someone else's C,
//! carried on the device as a compiled binary no one here could read, linked
//! against libopus, libvorbis and libfaad because miniaudio brings decoders for
//! flac, mp3 and wav and for nothing else. The formats it could not get from
//! the audio backend, ravachol had to bridge by hand. That is a sound
//! arrangement for a terminal player that may assume nothing is installed,
//! and it was the wrong one here.
//!
//! The first answer linked no codec at all: ffmpeg wrote one song out as plain
//! samples and ffprobe read its tags, both of them in `[packages]` already.
//! That was a start of ffmpeg's libraries on every press of next and every
//! seek, and somebody else's program in the middle of the one thing a music
//! player does. So the codecs are in the tree now, carried in `forks/`:
//! symphonia's readers and decoders for every container and codec but Opus,
//! and filmcraft's clean-room decoder for Opus. [`decoding`] reads a song into
//! samples, [`sounding`] carries them, and pw-cat plays what it is handed.
//! Standing between the two is the whole point: pausing is not writing,
//! position is what has been written, and stopping is a pipe that closes.
//!
//! GStreamer was the other road and it is worth saying why it was not taken.
//! This desktop linked it once, in `console-media-viewer`, and only because GTK
//! there was built with no media backend at all -- a film needed
//! `gtk4paintablesink` to hand a widget something it could draw. A song needs
//! nothing drawn, and the pipeline would have bought a scheduler, a bus and a
//! plugin tree in order to reach decoders that a few crates reach on their own.
//!
//! What the panel talks to does not change. kew answered on MPRIS and so does
//! this: the surface is already what `console_music::player` asks for and what
//! the checks press, and it is what media keys and the rest of the machine
//! speak. The one member the panel used to ask for and will not find here is
//! `xesam:url`, which said which file was playing so the now-playing card could
//! offer to show it in Files. That offer goes with the fork.

pub mod answers;
pub mod art;
pub mod bus;
pub mod library;
pub mod playlist;
pub mod bookmark;
pub mod decoding;
pub mod sounding;
pub mod stretching;
pub mod tags;
