//! The four decoders, and what each hands back made into one shape.
//!
//! Each is handed the bytes of one picture from the container, in the order
//! the container keeps them, with the moment it is shown, and hands back
//! whichever pictures that made ready -- in the order they are shown, which
//! for H.264 and HEVC is not the order they arrived in, and for AV1 is a few
//! pictures later, because it is decoding several at once. The moment travels
//! with each picture through all of that, so nothing here counts frames.
//!
//! An AV1 decoder decoding several frames at once has a queue, and a full
//! queue refuses the next picture rather than taking it. A refusal is not a
//! fault: the pictures already finished are taken out and the same bytes are
//! handed over again, which is the order the decoder asks for.

use console_core_geometry::Size;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_video_decoding_advanced_video_coding as avc;
use console_core_video_decoding_av1 as av1;
use console_core_video_decoding_high_efficiency_video_coding as hevc;
use console_core_video_decoding_vp9 as vp9;

use crate::colour::{Colour, Decoded, Matrix, Planes, Range, Strides};
use crate::tracks::Codec;

#[derive(Debug)]
pub enum DecodeFault {
    Avc(avc::Error),
    Hevc(hevc::Error),
    Vp9(vp9::Error),
    Av1(av1::Error),
}

impl std::fmt::Display for DecodeFault {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeFault::Avc(fault) => write!(to, "H.264: {fault}"),
            DecodeFault::Hevc(fault) => write!(to, "HEVC: {fault}"),
            DecodeFault::Vp9(fault) => write!(to, "VP9: {fault}"),
            DecodeFault::Av1(fault) => write!(to, "AV1: {fault}"),
        }
    }
}

impl std::error::Error for DecodeFault {}

pub(crate) enum Decoder {
    Avc(Box<avc::Decoder>),
    Hevc(Box<hevc::Decoder>),
    Vp9(Box<vp9::Decoder>),
    Av1(av1::Decoder),
}

impl Decoder {
    pub(crate) fn for_codec(codec: &Codec) -> Result<Decoder, DecodeFault> {
        match codec {
            Codec::Avc(record) => {
                let decoder = avc::Decoder::from_avcc(record).map_err(DecodeFault::Avc)?;

                Ok(Decoder::Avc(Box::new(decoder)))
            },
            Codec::Hevc(record) => {
                let decoder = hevc::Decoder::from_hvcc(record).map_err(DecodeFault::Hevc)?;

                Ok(Decoder::Hevc(Box::new(decoder)))
            },
            Codec::Vp9 => Ok(Decoder::Vp9(Box::new(vp9::Decoder::new()))),
            Codec::Av1 => {
                let mut settings = av1::Settings::default();

                settings.threads = 0;
                settings.max_frame_delay = 0;

                let decoder = av1::Decoder::with_settings(settings).map_err(|fault| DecodeFault::Av1(fault.decompose().0))?;

                Ok(Decoder::Av1(decoder))
            },
        }
    }

    pub(crate) fn fed(&mut self, unit: &[u8], pts: i64) -> Result<Vec<Decoded>, DecodeFault> {
        match self {
            Decoder::Avc(decoder) => {
                let pictures = decoder.decode(unit, pts).map_err(DecodeFault::Avc)?;

                Ok(pictures.into_iter().flat_map(from_avc).collect())
            },
            Decoder::Hevc(decoder) => {
                let pictures = decoder.decode(unit, pts).map_err(DecodeFault::Hevc)?;

                Ok(pictures.into_iter().flat_map(from_hevc).collect())
            },
            Decoder::Vp9(decoder) => {
                let pictures = decoder.decode(unit, pts).map_err(DecodeFault::Vp9)?;

                Ok(pictures.into_iter().flat_map(from_vp9).collect())
            },
            Decoder::Av1(decoder) => av1_fed(decoder, unit, pts),
        }
    }

    pub(crate) fn drained(&mut self) -> Result<Vec<Decoded>, DecodeFault> {
        Ok(match self {
            Decoder::Avc(decoder) => decoder.flush().into_iter().flat_map(from_avc).collect(),
            Decoder::Hevc(decoder) => decoder.flush().into_iter().flat_map(from_hevc).collect(),
            Decoder::Vp9(decoder) => decoder.flush().into_iter().flat_map(from_vp9).collect(),
            Decoder::Av1(decoder) => {
                let flushed = decoder.flush().map_err(|fault| DecodeFault::Av1(fault.decompose().0))?;

                flushed.iter().flat_map(from_av1).collect()
            },
        })
    }
}

fn h273(matrix: u8) -> Result<Matrix, Never> {
    Ok(match matrix {
        0 => Matrix::Rgb,
        1 => Matrix::Bt709,
        5..=7 => Matrix::Bt601,
        9 | 10 => Matrix::Bt2020,
        _unsaid_or_rare => Matrix::Unsaid,
    })
}


fn from_avc(picture: avc::Picture) -> Result<Decoded, Never> {
    let Ok(matrix) = h273(picture.color.matrix);
    let Ok(luma) = fitted::<_, u32>(picture.y_stride);
    let Ok(chroma) = fitted::<_, u32>(picture.uv_stride);

    let range = match picture.color.full_range {
        true => Range::Full,
        false => Range::Studio,
    };

    Ok(Decoded {
        pts: picture.pts,
        stored: Size { width: picture.width, height: picture.height },
        chroma: Size { width: picture.chroma_width, height: picture.chroma_height },
        planes: Planes::Eight([picture.y, picture.u, picture.v]),
        strides: Strides { luma, chroma },
        colour: Colour { matrix, range },
    })
}

fn planes_of<P>(planes: [P; 3], depth: u32, split: impl Fn(P) -> Result<Vec<u8>, Vec<u16>>) -> Result<Planes, Never> {
    let [luma, blue, red] = planes;

    Ok(match (split(luma), split(blue), split(red)) {
        (Ok(luma), Ok(blue), Ok(red)) => Planes::Eight([luma, blue, red]),
        (Err(luma), Err(blue), Err(red)) => Planes::Deep([luma, blue, red], depth),
        (_, _, _) => Planes::Eight([Vec::new(), Vec::new(), Vec::new()]),
    })
}

fn from_hevc(picture: hevc::Picture) -> Result<Decoded, Never> {
    let Ok(matrix) = h273(picture.color.matrix);
    let Ok(luma) = fitted::<_, u32>(picture.y_stride);
    let Ok(chroma) = fitted::<_, u32>(picture.uv_stride);

    let range = match picture.color.full_range {
        true => Range::Full,
        false => Range::Studio,
    };

    let Ok(planes) = planes_of([picture.y, picture.u, picture.v], picture.bit_depth, |plane| match plane {
        hevc::Plane::U8(samples) => Ok(samples),
        hevc::Plane::U16(samples) => Err(samples),
    });

    Ok(Decoded {
        pts: picture.pts,
        stored: Size { width: picture.width, height: picture.height },
        chroma: Size { width: picture.chroma_width, height: picture.chroma_height },
        planes,
        strides: Strides { luma, chroma },
        colour: Colour { matrix, range },
    })
}

fn from_vp9(picture: vp9::Picture) -> Result<Decoded, Never> {
    let matrix = match picture.color.color_space {
        2 => Matrix::Bt709,
        1 | 3 | 4 => Matrix::Bt601,
        5 => Matrix::Bt2020,
        7 => Matrix::Rgb,
        _unsaid => Matrix::Unsaid,
    };

    let Ok(luma) = fitted::<_, u32>(picture.y_stride);
    let Ok(chroma) = fitted::<_, u32>(picture.uv_stride);

    let range = match picture.color.full_range {
        true => Range::Full,
        false => Range::Studio,
    };

    let Ok(planes) = planes_of([picture.y, picture.u, picture.v], picture.bit_depth, |plane| match plane {
        vp9::Plane::U8(samples) => Ok(samples),
        vp9::Plane::U16(samples) => Err(samples),
    });

    Ok(Decoded {
        pts: picture.pts,
        stored: Size { width: picture.width, height: picture.height },
        chroma: Size { width: picture.chroma_width, height: picture.chroma_height },
        planes,
        strides: Strides { luma, chroma },
        colour: Colour { matrix, range },
    })
}

fn av1_matrix(matrix: av1::MatrixCoefficients) -> Result<Matrix, Never> {
    Ok(match matrix {
        av1::MatrixCoefficients::Identity => Matrix::Rgb,
        av1::MatrixCoefficients::BT709 => Matrix::Bt709,
        av1::MatrixCoefficients::BT470BG | av1::MatrixCoefficients::BT601 | av1::MatrixCoefficients::SMPTE240 | av1::MatrixCoefficients::FCC => Matrix::Bt601,
        av1::MatrixCoefficients::BT2020NCL | av1::MatrixCoefficients::BT2020CL => Matrix::Bt2020,
        av1::MatrixCoefficients::Unspecified
        | av1::MatrixCoefficients::Reserved
        | av1::MatrixCoefficients::YCgCo
        | av1::MatrixCoefficients::SMPTE2085
        | av1::MatrixCoefficients::ChromaDerivedNCL
        | av1::MatrixCoefficients::ChromaDerivedCL
        | av1::MatrixCoefficients::ICtCp => Matrix::Unsaid,
    })
}

fn tight<T: Copy>(rows: impl Iterator<Item = impl AsRef<[T]>>) -> Result<Vec<T>, Never> {
    Ok(rows.flat_map(|row| row.as_ref().to_vec()).collect())
}

fn from_av1(frame: &av1::Frame) -> Result<Decoded, Never> {
    let colour = frame.color_info();
    let Ok(matrix) = av1_matrix(colour.matrix_coefficients);

    let range = match colour.color_range {
        av1::ColorRange::Full => Range::Full,
        av1::ColorRange::Limited => Range::Studio,
    };

    let stored = Size { width: frame.width(), height: frame.height() };

    let planes = match frame.planes() {
        av1::Planes::Depth8(planes) => {
            let Ok(luma) = tight(planes.y().rows());
            let Ok(blue) = tight(planes.u().iter().flat_map(|plane| plane.rows().map(<[u8]>::to_vec).collect::<Vec<_>>()));
            let Ok(red) = tight(planes.v().iter().flat_map(|plane| plane.rows().map(<[u8]>::to_vec).collect::<Vec<_>>()));

            Planes::Eight([luma, blue, red])
        },
        av1::Planes::Depth16(planes) => {
            let Ok(luma) = tight(planes.y().rows());
            let Ok(blue) = tight(planes.u().iter().flat_map(|plane| plane.rows().map(<[u16]>::to_vec).collect::<Vec<_>>()));
            let Ok(red) = tight(planes.v().iter().flat_map(|plane| plane.rows().map(<[u16]>::to_vec).collect::<Vec<_>>()));

            Planes::Deep([luma, blue, red], u32::from(frame.bit_depth()))
        },
    };

    let chroma = match frame.pixel_layout() {
        av1::PixelLayout::I420 => Size { width: stored.width.div_ceil(2), height: stored.height.div_ceil(2) },
        av1::PixelLayout::I422 => Size { width: stored.width.div_ceil(2), height: stored.height },
        av1::PixelLayout::I444 => stored,
        av1::PixelLayout::I400 => Size { width: 0, height: 0 },
    };

    Ok(Decoded { pts: frame.timestamp(), stored, chroma, planes, strides: Strides { luma: stored.width, chroma: chroma.width }, colour: Colour { matrix, range } })
}

fn av1_ready(decoder: &mut av1::Decoder, into: &mut Vec<Decoded>) -> Result<(), DecodeFault> {
    let taken = console_core_iteration::iterate((&mut *decoder, &mut *into), |(decoder, into)| {
        Ok(match decoder.get_frame() {
            Ok(Some(frame)) => {
                let Ok(decoded) = from_av1(&frame);

                into.push(decoded);

                Step::Again((decoder, into))
            },
            Ok(None) => Step::Halt(Ok(())),
            Err(fault) => Step::Halt(Err(DecodeFault::Av1(fault.decompose().0))),
        })
    });

    match taken {
        Ok(answer) => answer,
        Err(_endless) => Ok(()),
    }
}

fn av1_fed(decoder: &mut av1::Decoder, unit: &[u8], pts: i64) -> Result<Vec<Decoded>, DecodeFault> {
    let mut ready = Vec::new();

    let sent = console_core_iteration::iterate((&mut *decoder, &mut ready), |(decoder, ready)| {
        Ok(match decoder.decode_stamped(unit, pts) {
            Ok(Some(frame)) => {
                let Ok(decoded) = from_av1(&frame);

                ready.push(decoded);

                Step::Halt(Ok(()))
            },
            Ok(None) => Step::Halt(Ok(())),
            Err(fault) => match fault.decompose().0 {
                av1::Error::NeedMoreData => match av1_ready(decoder, ready) {
                    Ok(()) => Step::Again((decoder, ready)),
                    Err(fault) => Step::Halt(Err(fault)),
                },
                fault @ (av1::Error::InvalidSettings(_)
                | av1::Error::InitFailed
                | av1::Error::OutOfMemory
                | av1::Error::InvalidData
                | av1::Error::Cancelled
                | av1::Error::Other(_)) => Step::Halt(Err(DecodeFault::Av1(fault))),
            },
        })
    });

    match sent {
        Ok(Ok(())) => {},
        Ok(Err(fault)) => return Err(fault),
        Err(_endless) => {},
    }

    av1_ready(decoder, &mut ready)?;

    Ok(ready)
}
