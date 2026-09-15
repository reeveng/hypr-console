//! The sky: which wallpaper is up, and why.
//!
//! One picture on the screen all day is a picture nobody sees after the first
//! week. This chooses between several, by the hour and by the weather outside,
//! and hands the answer to the wallpaper daemon.
//!
//! `grade` is how somebody else's picture is brought into this palette.
//!
//! One fault for the crate, because one wallpaper walks all of it: a picture is
//! fetched, decoded frame by frame through ffmpeg, graded into the palette and
//! written back out as a WebP, and a wallpaper that does not appear failed at
//! exactly one of those. `Unpainted` is which one. The sentence each step used
//! to hand back is the `Display` arm, so what reaches the journal is unchanged
//! and a caller that wants to tell a picture that would not come from a picture
//! that came and would not decode can now ask.

use std::fmt;
use std::path::PathBuf;

pub mod choose;
pub mod covered;
pub mod grade;
pub mod here;
pub mod keeping;
pub mod loops;
pub mod moon;
pub mod palette;
pub mod place;
pub mod press;
pub mod source;
pub mod sun;
pub mod weather;
pub mod webp;

#[derive(Debug)]
pub enum Unpainted {
    NoColour(String),
    NoColours,
    NotTheRamp,
    NothingPressed,
    Unreadable(PathBuf, std::io::Error),
    Holding(PathBuf, std::io::Error),
    NoCurl(std::io::Error),
    Unfetched(String, String),
    Unplaced(PathBuf, std::io::Error),
    ChunkTooBig(usize),
    NotASide(i32),
    CutShort,
    ChunkTooLong,
    NoPicture,
    NoFfmpeg(std::io::Error),
    NoPipeOut,
    NoPipeIn,
    Stopped(std::io::Error),
    Unfinished(std::io::Error),
    Refused(PathBuf, String),
    RefusedAFrame(String),
    DecodedToNothing(PathBuf),
    Untaken(std::io::Error),
}

impl fmt::Display for Unpainted {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unpainted::NoColour(name) => write!(to, "the palette names no {name}"),
            Unpainted::NoColours => {
                write!(to, "theme/report.md holds no colours; run `just theme`")
            }
            Unpainted::NotTheRamp => {
                write!(to, "the ramp is not the colours it is made of")
            }
            Unpainted::NothingPressed => write!(to, "nothing was pressed"),
            Unpainted::Unreadable(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Unpainted::Holding(at, fault) => {
                write!(to, "{} could not be made: {fault}", at.display())
            }
            Unpainted::NoCurl(fault) => write!(to, "curl would not run: {fault}"),
            Unpainted::Unfetched(from, said) => write!(to, "{from} would not come: {said}"),
            Unpainted::Unplaced(at, fault) => {
                write!(to, "{} could not be put in place: {fault}", at.display())
            }
            Unpainted::ChunkTooBig(bytes) => {
                write!(to, "a chunk of {bytes} bytes is too big for a WebP")
            }
            Unpainted::NotASide(pixels) => {
                write!(to, "{pixels} is not a size a WebP can hold")
            }
            Unpainted::CutShort => write!(to, "that WebP is cut short"),
            Unpainted::ChunkTooLong => write!(
                to,
                "that WebP holds a chunk longer than this machine can address"
            ),
            Unpainted::NoPicture => write!(to, "that WebP holds no picture"),
            Unpainted::NoFfmpeg(fault) => write!(to, "ffmpeg would not run: {fault}"),
            Unpainted::NoPipeOut => write!(to, "ffmpeg gave no pipe"),
            Unpainted::NoPipeIn => write!(to, "ffmpeg took no pipe"),
            Unpainted::Stopped(fault) => write!(to, "ffmpeg stopped talking: {fault}"),
            Unpainted::Unfinished(fault) => write!(to, "ffmpeg would not finish: {fault}"),
            Unpainted::Refused(source, said) => {
                write!(to, "ffmpeg refused {}: {said}", source.display())
            }
            Unpainted::RefusedAFrame(said) => write!(to, "ffmpeg refused a frame: {said}"),
            Unpainted::DecodedToNothing(source) => {
                write!(to, "{} decoded to nothing", source.display())
            }
            Unpainted::Untaken(fault) => {
                write!(to, "ffmpeg would not take the picture: {fault}")
            }
        }
    }
}

impl std::error::Error for Unpainted {}
