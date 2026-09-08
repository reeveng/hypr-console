//! The player this desktop owns, in place of the one it forked.
//!
//! Everything the Music panel's buttons do was kew's: somebody else's C,
//! carried on the device as a compiled binary nobody here could read, linked
//! against libopus, libvorbis and libfaad because miniaudio brings decoders for
//! flac, mp3 and wav and for nothing else. The formats it could not get from
//! the audio backend, ravachol had to bridge by hand. That is a sound
//! arrangement for a terminal player that may assume nothing is installed, and
//! it is the wrong one here: ffmpeg is in `[packages]` already, and every tag
//! in the library is read by asking ffprobe. The seam that cost kew its codec
//! bridges is one this desktop is allowed to skip.
//!
//! So no codec is linked and none is written. ffmpeg is told to write one song
//! out as plain samples, [`sounding`] carries them, and pw-cat plays what it is
//! handed. Standing between the two is the whole point: pausing is not writing,
//! position is what has been written, and stopping is a pipe that closes.
//!
//! GStreamer was the other road and it is worth saying why it was not taken.
//! This desktop already links it, but only in `console-media-viewer`, and only
//! because GTK here is built with no media backend at all -- a film needs
//! `gtk4paintablesink` to hand a widget something it can draw. A song needs
//! nothing drawn. And the decoders behind gst-libav are ffmpeg's own, so the
//! pipeline would have bought a scheduler, a bus and a plugin tree in order to
//! reach exactly the decoders a pipe reaches. What it would have bought that is
//! real is seeking without opening the file again, and that is the one place
//! this is worse: a seek here is ffmpeg started again at an offset.
//!
//! What the panel talks to does not change. kew answered on MPRIS and so does
//! this: the surface is already what `console_music_panel::player` asks for and what
//! the checks press, and it is what media keys and the rest of the machine
//! speak. The one member the panel used to ask for and will not find here is
//! `xesam:url`, which said which file was playing so the now-playing card could
//! offer to show it in Files. That offer goes with the fork.

pub mod answers;
pub mod art;
pub mod bus;
pub mod library;
pub mod playlist;
pub mod sounding;
pub mod tags;
