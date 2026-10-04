//! A JPEG, read into the pixels it holds, at the size it is wanted.
//!
//! Every photograph this desktop showed was read by ffmpeg, after ffprobe had
//! been asked how big it was: two programs started for a thumbnail a hundred
//! and twenty-eight pixels across. And most of what a JPEG holds is never
//! needed at that size. The format keeps a picture as blocks of eight by eight
//! frequencies, so a block can be drawn one, two or four pixels a side from its
//! lowest frequencies alone, which is what libvips does and why it is quick: a
//! 48 megapixel photograph drawn into a screen is read at a quarter of its
//! size, and never exists anywhere at the size it was taken.
//!
//! [`decoded`] is asked for the size the picture will be drawn into, and reads
//! it at the smallest of its own sizes that still covers that, upright: the
//! way the camera was held is in the EXIF, and the pixels come back turned to
//! it. [`measured`] says how big that upright picture is without decoding any
//! of it.
//!
//! Baseline and progressive JPEGs are read, in grey, YCbCr or RGB, with any
//! chroma subsampling whose factors divide each other. What a camera does not
//! write -- arithmetic coding, twelve bits a sample, the lossless and the
//! hierarchical processes, CMYK -- is refused by name, so a caller can hand the
//! file to something that does read it.
//!
//! Nothing here opens a file. It is handed bytes, and a stranger's bytes are
//! what it is for: every length the file states is checked against what is
//! there, and the memory a picture would take is counted before it is taken.
//! A file cut short is drawn as far as it goes, the way a browser draws one.
//!
//! [`decoded_spread`] is the same picture drawn by more than one hand. The bits
//! of a scan can only be read one after another, but the blocks they make and
//! the color of every row can be drawn anywhere, so they come back to the
//! caller as [`Task`]s for it to spread however it spreads work, with the
//! reading of the next round of rows as one of them. Nothing in here starts a
//! thread.
//!
//! [`encoded`] is the other way: RGBA written as the baseline JPEG libjpeg
//! writes when it is handed a [`Quality`] and nothing else, which is the JPEG
//! every reader reads. A quality is the one to a hundred every encoder takes,
//! and a number past either end is the end it is past.

mod assembly;
mod bits;
mod entropy;
mod exif;
mod fdct;
mod frame;
mod huffman;
mod idct;
mod kept;
mod scans;
mod segments;
mod strips;
mod writing;

use console_core_geometry::Size;
use console_core_never::Never;

pub use crate::strips::{Spread, Task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegError {
    NotAJpeg,
    Truncated,
    Corrupt,
    TooLarge,
    Unsupported(Unsupported),
    Mismatched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    ArithmeticCoding,
    Lossless,
    Hierarchical,
    TwelveBits,
    Cmyk,
    Components(u8),
    Sampling,
    HeightAfterwards,
}

impl std::fmt::Display for JpegError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JpegError::NotAJpeg => write!(to, "this is not a JPEG"),
            JpegError::Truncated => write!(to, "the file ends before the picture in it begins"),
            JpegError::Corrupt => write!(to, "the picture in here is damaged"),
            JpegError::TooLarge => write!(to, "this picture would take more memory than any photograph"),
            JpegError::Unsupported(how) => write!(to, "this JPEG is written {how}, which is not read here"),
            JpegError::Mismatched => write!(to, "the pixels handed over are not the picture their size says"),
        }
    }
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unsupported::ArithmeticCoding => write!(to, "with arithmetic coding"),
            Unsupported::Lossless => write!(to, "losslessly"),
            Unsupported::Hierarchical => write!(to, "hierarchically"),
            Unsupported::TwelveBits => write!(to, "at twelve bits a sample"),
            Unsupported::Cmyk => write!(to, "in CMYK"),
            Unsupported::Components(count) => write!(to, "with {count} components"),
            Unsupported::Sampling => write!(to, "with sampling factors that do not divide each other"),
            Unsupported::HeightAfterwards => write!(to, "with its height after the first scan"),
        }
    }
}

impl std::error::Error for JpegError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality(u8);

impl Quality {
    pub fn percent(percent: u8) -> Result<Quality, Never> {
        Ok(Quality(percent.clamp(1, 100)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub size: Size<u32>,
    pub rgba: Vec<u8>,
}

pub fn measured(bytes: &[u8]) -> Result<Size<u32>, JpegError> {
    let read = segments::read(bytes, segments::Purpose::Measuring, &strips::one_after_another)?;

    match read.stage {
        segments::Stage::Framed(frame) => {
            let Ok(upright) = read.orientation.upright(frame.size);

            Ok(upright)
        },
        segments::Stage::Opening | segments::Stage::Drawing(_) => Err(JpegError::Truncated),
    }
}

pub fn encoded(picture: &Picture, quality: Quality) -> Result<Vec<u8>, JpegError> {
    writing::encoded(picture, quality)
}

pub fn decoded(bytes: &[u8], covering: Size<u32>) -> Result<Picture, JpegError> {
    decoded_spread(bytes, covering, &strips::one_after_another)
}

pub fn decoded_spread(bytes: &[u8], covering: Size<u32>, spread: Spread<'_>) -> Result<Picture, JpegError> {
    let read = segments::read(bytes, segments::Purpose::Drawing(covering), spread)?;
    let segments::Reading { stage, tables, adobe, .. } = read;

    let image = match stage {
        segments::Stage::Drawing(image) => image,
        segments::Stage::Opening | segments::Stage::Framed(_) => return Err(JpegError::Truncated),
    };

    let colors = assembly::colors(&image.frame, adobe)?;

    scans::finished(image, &tables, colors, spread)
}
