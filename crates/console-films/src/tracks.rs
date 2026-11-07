//! What a film's container says: where each picture is and when it is shown.
//!
//! The first bytes say which of the two containers a file is: an MPEG-4 or
//! QuickTime file opens with a box whose name is four letters at the fifth
//! byte, and a Matroska or WebM file with the EBML mark. Either is read into
//! the same few things -- the pictures in the order they are decoded, the
//! moment each is shown, which of them a decoder can start at, how the picture
//! is turned to be seen upright, and whether there is sound or anything to
//! read -- so nothing after this asks which container it was.
//!
//! A moment is kept in the container's own ticks and made into seconds once,
//! by the one number each container gives for the length of a tick, so an
//! MPEG-4 edit list and a Matroska timestamp scale are each applied in the
//! place that understands them.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_subtitle_files::{Cue, Written};
use console_core_number_conversion::{Float, fitted, index, toward_zero_i64};
use console_core_video_decoding_matroska_container as matroska;
use console_core_video_decoding_mpeg_4_container as mpeg4;

use crate::Sound;
use console_core_byte_sources::{ByteSource, FileSource};

const EBML: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

const BOXES: [&[u8; 4]; 6] = [b"ftyp", b"moov", b"mdat", b"free", b"wide", b"skip"];

const ORDINARY_RATE: f64 = 30.0;

const NANOSECONDS: f64 = 1_000_000_000.0;

const A_LINE_WITH_NO_END: f64 = 4.0;

#[derive(Debug)]
pub enum ContainerFault {
    Unread(std::io::Error),
    Mpeg4(mpeg4::Error),
    Matroska(matroska::Error),
}

impl std::fmt::Display for ContainerFault {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerFault::Unread(fault) => write!(to, "it would not read: {fault}"),
            ContainerFault::Mpeg4(fault) => write!(to, "its MPEG-4 boxes are not what they should be: {fault}"),
            ContainerFault::Matroska(fault) => write!(to, "its Matroska elements are not what they should be: {fault}"),
        }
    }
}

impl std::error::Error for ContainerFault {}

pub(crate) enum Container {
    Mpeg4(mpeg4::Mp4File),
    Matroska(Box<matroska::MkvFile>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Codec {
    Avc(Vec<u8>),
    Hevc(Vec<u8>),
    Vp9,
    Av1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sample {
    pub(crate) pts: i64,
    pub(crate) key: Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    Upright,
    Quarter,
    Half,
    ThreeQuarters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtitles {
    pub track: u32,
    pub language: Language,
    pub name: String,
    pub written: Written,
}

pub(crate) struct Video {
    pub(crate) track: u32,
    pub(crate) codec: Codec,
    pub(crate) samples: Vec<Sample>,
    pub(crate) a_tick: f64,
    pub(crate) rate: f64,
    pub(crate) shown: Size<u32>,
    pub(crate) turn: Turn,
}

pub(crate) struct Read {
    pub(crate) container: Container,
    pub(crate) video: Option<Video>,
    pub(crate) sound: Sound,
    pub(crate) subtitles: Vec<Subtitles>,
    pub(crate) seconds: f64,
}

pub(crate) fn read(opened: &FileSource) -> Result<Option<Read>, ContainerFault> {
    let mut head = [0u8; 8];

    let begun = ByteSource::read_at(opened, 0, &mut head);

    match begun {
        Ok(()) => {},
        Err(fault) => {
            return match fault.kind() == std::io::ErrorKind::UnexpectedEof {
                true => Ok(None),
                false => Err(ContainerFault::Unread(fault)),
            };
        },
    }

    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = head;

    match [first, second, third, fourth] == EBML {
        true => {
            let file = matroska::open(opened).map_err(ContainerFault::Matroska)?;

            let read = from_matroska(file)?;

            return Ok(Some(read));
        },
        false => {},
    }

    let named = [fifth, sixth, seventh, eighth];

    match BOXES.contains(&&named) {
        true => {
            let file = mpeg4::open(opened).map_err(ContainerFault::Mpeg4)?;

            let read = from_mpeg4(file)?;

            Ok(Some(read))
        },
        false => Ok(None),
    }
}

fn turned(quarters: Option<u8>) -> Result<Turn, Never> {
    Ok(match quarters {
        Some(1) => Turn::Quarter,
        Some(2) => Turn::Half,
        Some(3) => Turn::ThreeQuarters,
        Some(_) | None => Turn::Upright,
    })
}

fn shown(stored: Size<u32>, turn: Turn) -> Result<Size<u32>, Never> {
    Ok(match turn {
        Turn::Upright | Turn::Half => stored,
        Turn::Quarter | Turn::ThreeQuarters => Size { width: stored.height, height: stored.width },
    })
}

fn rate_of(samples: &[Sample], a_tick: f64) -> Result<f64, Never> {
    let first = samples.iter().map(|sample| sample.pts).min();
    let last = samples.iter().map(|sample| sample.pts).max();
    let Ok(between) = fitted::<_, u64>(samples.len().saturating_sub(1));
    let Ok(between) = between.float();

    Ok(match (first, last) {
        (Some(first), Some(last)) => {
            let Ok(ticks) = last.saturating_sub(first).float();
            let seconds = ticks * a_tick;

            match seconds > 0.0 && between > 0.0 {
                true => between / seconds,
                false => ORDINARY_RATE,
            }
        },
        (_, _) => ORDINARY_RATE,
    })
}

fn mpeg4_codec(track: &mpeg4::Track) -> Result<Option<Codec>, Never> {
    Ok(match track.codec() {
        Some(mpeg4::CodecConfig::Avc(config)) => Some(Codec::Avc(config.to_bytes())),
        Some(mpeg4::CodecConfig::Hevc(config)) => Some(Codec::Hevc(config.to_bytes())),
        Some(mpeg4::CodecConfig::Vp9(_)) => Some(Codec::Vp9),
        Some(mpeg4::CodecConfig::Av1(_)) => Some(Codec::Av1),
        Some(
            mpeg4::CodecConfig::ProRes { .. }
            | mpeg4::CodecConfig::Jpeg { .. }
            | mpeg4::CodecConfig::Dnx { .. }
            | mpeg4::CodecConfig::Apv(_)
            | mpeg4::CodecConfig::Aac(_)
            | mpeg4::CodecConfig::Mp3
            | mpeg4::CodecConfig::Pcm(_)
            | mpeg4::CodecConfig::Alac { .. }
            | mpeg4::CodecConfig::Opus(_)
            | mpeg4::CodecConfig::Flac(_)
            | mpeg4::CodecConfig::Ac3 { .. }
            | mpeg4::CodecConfig::Eac3 { .. }
            | mpeg4::CodecConfig::Timecode(_)
            | mpeg4::CodecConfig::Unknown { .. },
        )
        | None => None,
    })
}

fn mpeg4_video(file: &mpeg4::Mp4File) -> Result<Option<Video>, Never> {
    let found = file.tracks.iter().zip(0u32..).find_map(|(track, number)| match (track.kind, mpeg4_codec(track)) {
        (mpeg4::TrackKind::Video, Ok(Some(codec))) => Some((track, number, codec)),
        (_, _) => None,
    });

    let (track, number, codec) = match found {
        Some(found) => found,
        None => return Ok(None),
    };

    let samples: Vec<Sample> = track.samples.iter().map(|sample| Sample {
        pts: sample.pts.saturating_add(track.edit_offset),
        key: match sample.is_sync {
            true => Key::Yes,
            false => Key::No,
        },
    }).collect();

    let a_tick = 1.0 / f64::from(track.timescale.max(1));
    let Ok(rate) = rate_of(&samples, a_tick);
    let Ok(turn) = turned(track.display_rotation());

    let stored = match (track.width, track.height, track.video()) {
        (0, _, Some(video)) | (_, 0, Some(video)) => Size { width: u32::from(video.width), height: u32::from(video.height) },
        (width, height, _) => Size { width, height },
    };

    let Ok(shown) = shown(stored, turn);

    Ok(Some(Video { track: number, codec, samples, a_tick, rate, shown, turn }))
}

fn from_mpeg4(file: mpeg4::Mp4File) -> Result<Read, ContainerFault> {
    let Ok(video) = mpeg4_video(&file);
    let audible = file.tracks.iter().any(|track| track.kind == mpeg4::TrackKind::Audio);
    let Ok(length) = file.duration.float();
    let seconds = length / f64::from(file.timescale.max(1));

    Ok(Read {
        container: Container::Mpeg4(file),
        video,
        sound: match audible {
            true => Sound::Audible,
            false => Sound::Silent,
        },
        subtitles: Vec::new(),
        seconds,
    })
}

fn matroska_codec(track: &matroska::Track) -> Result<Option<Codec>, Never> {
    Ok(match &track.codec {
        matroska::Codec::Avc { avcc } => Some(Codec::Avc(avcc.clone())),
        matroska::Codec::Hevc { hvcc } => Some(Codec::Hevc(hvcc.clone())),
        matroska::Codec::Vp9 { .. } => Some(Codec::Vp9),
        matroska::Codec::Av1 { .. } => Some(Codec::Av1),
        matroska::Codec::Vp8
        | matroska::Codec::Apv { .. }
        | matroska::Codec::ProRes { .. }
        | matroska::Codec::Mjpeg
        | matroska::Codec::VfwFourcc { .. }
        | matroska::Codec::Aac { .. }
        | matroska::Codec::Opus { .. }
        | matroska::Codec::Vorbis { .. }
        | matroska::Codec::Flac { .. }
        | matroska::Codec::Pcm { .. }
        | matroska::Codec::Ac3
        | matroska::Codec::Eac3
        | matroska::Codec::Mp3
        | matroska::Codec::Mp2
        | matroska::Codec::SubRip
        | matroska::Codec::WebVtt
        | matroska::Codec::Ass { .. }
        | matroska::Codec::Other(_) => None,
    })
}

fn a_tick(track: &matroska::Track) -> Result<f64, Never> {
    let (over, under) = track.timebase;
    let Ok(over) = over.float();
    let Ok(under) = under.float();

    Ok(match under > 0.0 {
        true => over / under,
        false => 0.001,
    })
}

fn matroska_video(file: &matroska::MkvFile) -> Result<Option<Video>, Never> {
    let found = file.tracks.iter().zip(0u32..).find_map(|(track, number)| match (track.kind, matroska_codec(track)) {
        (matroska::TrackKind::Video, Ok(Some(codec))) => Some((track, number, codec)),
        (_, _) => None,
    });

    let (track, number, codec) = match found {
        Some(found) => found,
        None => return Ok(None),
    };

    let samples: Vec<Sample> = track.samples.iter().map(|sample| Sample {
        pts: sample.pts,
        key: match sample.keyframe {
            true => Key::Yes,
            false => Key::No,
        },
    }).collect();

    let Ok(a_tick) = a_tick(track);

    let rate = match track.default_duration_ns {
        Some(nanoseconds) => {
            let Ok(nanoseconds) = nanoseconds.float();

            NANOSECONDS / nanoseconds.max(1.0)
        },
        None => {
            let Ok(rate) = rate_of(&samples, a_tick);

            rate
        },
    };

    let (stored, quarters) = match &track.video {
        Some(video) => {
            let (top, bottom, left, right) = video.crop;
            let width = video.pixel_width.saturating_sub(left.saturating_add(right));
            let height = video.pixel_height.saturating_sub(top.saturating_add(bottom));

            (Size { width, height }, video.display_rotation())
        },
        None => (Size { width: 0, height: 0 }, None),
    };

    let Ok(turn) = turned(quarters);
    let Ok(shown) = shown(stored, turn);

    Ok(Some(Video { track: number, codec, samples, a_tick, rate, shown, turn }))
}

fn from_matroska(file: matroska::MkvFile) -> Result<Read, ContainerFault> {
    let Ok(video) = matroska_video(&file);
    let audible = file.tracks.iter().any(|track| track.kind == matroska::TrackKind::Audio);

    let subtitles = file.tracks.iter().zip(0u32..).filter_map(|(track, number)| {
        let written = match (track.kind, &track.codec) {
            (matroska::TrackKind::Subtitle, matroska::Codec::SubRip) => Written::SubRip,
            (matroska::TrackKind::Subtitle, matroska::Codec::WebVtt) => Written::WebVtt,
            (matroska::TrackKind::Subtitle, matroska::Codec::Ass { .. }) => Written::Ass,
            (_, _) => return None,
        };

        Some(Subtitles {
            track: number,
            language: Language(match &track.language_bcp47 {
                Some(tagged) => tagged.clone(),
                None => track.language.clone(),
            }),
            name: track.name.clone(),
            written,
        })
    }).collect();

    let Ok(scale) = file.info.timestamp_scale.float();

    let seconds = match file.info.duration {
        Some(ticks) => ticks * scale / NANOSECONDS,
        None => match file.duration_ns() {
            Some(nanoseconds) => {
                let Ok(nanoseconds) = nanoseconds.float();

                nanoseconds / NANOSECONDS
            },
            None => 0.0,
        },
    };

    Ok(Read {
        container: Container::Matroska(Box::new(file)),
        video,
        sound: match audible {
            true => Sound::Audible,
            false => Sound::Silent,
        },
        subtitles,
        seconds,
    })
}

impl Video {
    pub(crate) fn start_for(&self, from: f64) -> Result<u32, Never> {
        let Ok(wanted) = toward_zero_i64(from / self.a_tick.max(f64::MIN_POSITIVE));

        let at = self.samples.iter().zip(0u32..).filter(|(sample, _)| sample.key == Key::Yes && sample.pts <= wanted).map(|(_, at)| at).last();

        Ok(match at {
            Some(at) => at,
            None => 0,
        })
    }

    pub(crate) fn half_a_frame(&self) -> Result<f64, Never> {
        Ok(0.5 / self.rate.max(1.0))
    }

    pub(crate) fn seconds(&self, pts: i64) -> Result<f64, Never> {
        let Ok(ticks) = pts.float();

        Ok(ticks * self.a_tick)
    }

    pub(crate) fn unit(&self, container: &Container, opened: &FileSource, at: u32) -> Result<Option<(Vec<u8>, i64)>, ContainerFault> {
        let Ok(position) = index(at);

        let sample = match self.samples.get(position) {
            Some(sample) => *sample,
            None => return Ok(None),
        };

        let Ok(track) = index(self.track);

        let bytes = match container {
            Container::Mpeg4(file) => file.read_sample(opened, track, position).map_err(ContainerFault::Mpeg4)?,
            Container::Matroska(file) => file.read_sample(opened, track, position).map_err(ContainerFault::Matroska)?,
        };

        Ok(Some((bytes, sample.pts)))
    }
}

pub(crate) fn cues(container: &Container, opened: &FileSource, subtitles: &Subtitles) -> Result<Vec<Cue>, ContainerFault> {
    let file = match container {
        Container::Matroska(file) => file,
        Container::Mpeg4(_) => return Ok(Vec::new()),
    };

    let Ok(number) = index(subtitles.track);

    let track = match file.tracks.get(number) {
        Some(track) => track,
        None => return Ok(Vec::new()),
    };

    let Ok(tick) = a_tick(track);
    let mut cues = Vec::new();

    for (sample, position) in track.samples.iter().zip(0u32..) {
        let Ok(position) = index(position);
        let block = file.read_sample(opened, number, position).map_err(ContainerFault::Matroska)?;
        let Ok(lines) = console_core_subtitle_files::said(&String::from_utf8_lossy(&block), subtitles.written);
        let Ok(at) = sample.pts.float();
        let Ok(lasting) = sample.duration.float();
        let from = at * tick;

        let until = match lasting > 0.0 {
            true => from + lasting * tick,
            false => from + A_LINE_WITH_NO_END,
        };

        cues.push(Cue { from, until, lines });
    }

    Ok(cues)
}
