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
use console_core_video_planes::Plane;

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
      }
      Codec::Hevc(record) => {
        let decoder = hevc::Decoder::from_hvcc(record).map_err(DecodeFault::Hevc)?;

        Ok(Decoder::Hevc(Box::new(decoder)))
      }
      Codec::Vp9 => Ok(Decoder::Vp9(Box::new({
        let Ok(explicit_value) = vp9::Decoder::new();
        explicit_value
      }))),
      Codec::Av1 => {
        let mut settings = av1::Settings::default();

        settings.threads = 0;
        settings.max_frame_delay = 0;

        let decoder = av1::Decoder::with_workers(settings, &console_concurrency::HostWorkers)
          .map_err(|fault| DecodeFault::Av1(fault.decompose().0))?;

        Ok(Decoder::Av1(decoder))
      }
    }
  }

  pub(crate) fn fed(&mut self, unit: &[u8], pts: i64) -> Result<Vec<Decoded>, DecodeFault> {
    match self {
      Decoder::Avc(decoder) => {
        let pictures = decoder.decode(unit, pts).map_err(DecodeFault::Avc)?;

        Ok(pictures.into_iter().flat_map(from_avc).collect())
      }
      Decoder::Hevc(decoder) => {
        let pictures = decoder.decode(unit, pts).map_err(DecodeFault::Hevc)?;

        Ok(pictures.into_iter().flat_map(from_hevc).collect())
      }
      Decoder::Vp9(decoder) => {
        let pictures = decoder.decode(unit, pts).map_err(DecodeFault::Vp9)?;

        Ok(pictures.into_iter().flat_map(from_vp9).collect())
      }
      Decoder::Av1(decoder) => av1_fed(decoder, unit, pts),
    }
  }

  pub(crate) fn drained(&mut self) -> Result<Vec<Decoded>, DecodeFault> {
    Ok(match self {
      Decoder::Avc(decoder) => {
        let Ok(flush) = decoder.flush();

        flush.into_iter().flat_map(from_avc).collect()
      }
      Decoder::Hevc(decoder) => {
        let Ok(flush) = decoder.flush();

        flush.into_iter().flat_map(from_hevc).collect()
      }
      Decoder::Vp9(decoder) => {
        let Ok(flush) = decoder.flush();

        flush.into_iter().flat_map(from_vp9).collect()
      }
      Decoder::Av1(decoder) => {
        let flushed = decoder
          .flush()
          .map_err(|fault| DecodeFault::Av1(fault.decompose().0))?;

        let decoded = flushed.iter().map(from_av1).collect::<Result<_, _>>()?;
        decoded
      }
    })
  }
}

fn h273((matrix, full_range): (u8, bool)) -> Result<Colour, Never> {
  let matrix_2 = match matrix {
    0 => Matrix::Rgb,
    1 => Matrix::Bt709,
    5..=7 => Matrix::Bt601,
    9 | 10 => Matrix::Bt2020,
    _unsaid_or_rare => Matrix::Unsaid,
  };

  let range = match ::core::primitive::bool::then_some(full_range, ()) {
    ::core::option::Option::Some(()) => Range::Full,
    ::core::option::Option::None => Range::Studio,
  };

  Ok(Colour {
    matrix: matrix_2,
    range,
  })
}

fn from_avc(picture: avc::Picture) -> Result<Decoded, Never> {
  let Ok(colour) = h273((picture.color.matrix, picture.color.full_range));
  let Ok(luma) = fitted::<_, u32>(picture.y_stride);
  let Ok(chroma) = fitted::<_, u32>(picture.uv_stride);

  Ok(Decoded {
    pts: picture.pts,
    stored: Size {
      width: picture.width,
      height: picture.height,
    },
    chroma: Size {
      width: picture.chroma_width,
      height: picture.chroma_height,
    },
    planes: Planes::Eight([picture.y, picture.u, picture.v]),
    strides: Strides { luma, chroma },
    colour,
  })
}

fn planes_of(planes: [Plane; 3], depth: u32) -> Result<Planes, Never> {
  let [luma, blue, red] = planes;

  Ok(match (luma, blue, red) {
    (Plane::U8(luma_2), Plane::U8(blue_2), Plane::U8(red_2)) => {
      Planes::Eight([luma_2, blue_2, red_2])
    }
    (Plane::U16(luma_3), Plane::U16(blue_3), Plane::U16(red_3)) => {
      Planes::Deep([luma_3, blue_3, red_3], depth)
    }
    (_, _, _) => Planes::Eight([Vec::new(), Vec::new(), Vec::new()]),
  })
}

fn from_hevc(picture: hevc::Picture) -> Result<Decoded, Never> {
  let Ok(colour) = h273((picture.color.matrix, picture.color.full_range));
  let Ok(luma) = fitted::<_, u32>(picture.y_stride);
  let Ok(chroma) = fitted::<_, u32>(picture.uv_stride);

  let Ok(planes) = planes_of([picture.y, picture.u, picture.v], picture.bit_depth);

  Ok(Decoded {
    pts: picture.pts,
    stored: Size {
      width: picture.width,
      height: picture.height,
    },
    chroma: Size {
      width: picture.chroma_width,
      height: picture.chroma_height,
    },
    planes,
    strides: Strides { luma, chroma },
    colour,
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

  let range = match ::core::primitive::bool::then_some(picture.color.full_range, ()) {
    ::core::option::Option::Some(()) => Range::Full,
    ::core::option::Option::None => Range::Studio,
  };

  let Ok(planes) = planes_of([picture.y, picture.u, picture.v], picture.bit_depth);

  Ok(Decoded {
    pts: picture.pts,
    stored: Size {
      width: picture.width,
      height: picture.height,
    },
    chroma: Size {
      width: picture.chroma_width,
      height: picture.chroma_height,
    },
    planes,
    strides: Strides { luma, chroma },
    colour: Colour { matrix, range },
  })
}

fn av1_matrix(matrix: av1::MatrixCoefficients) -> Result<Matrix, Never> {
  Ok(match matrix {
    av1::MatrixCoefficients::Identity => Matrix::Rgb,
    av1::MatrixCoefficients::BT709 => Matrix::Bt709,
    av1::MatrixCoefficients::BT470BG
    | av1::MatrixCoefficients::BT601
    | av1::MatrixCoefficients::SMPTE240
    | av1::MatrixCoefficients::FCC => Matrix::Bt601,
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
  let mut samples = Vec::new();

  for row in rows {
    samples.extend_from_slice(row.as_ref());
  }

  Ok(samples)
}

fn av1_samples<Pixel: Copy>(
  planes: [Option<av1::PlaneView<'_, Pixel>>; 3],
) -> Result<[Vec<Pixel>; 3], Never> {
  Ok(planes.map(|plane| match plane {
    Some(view) => {
      let Ok(rows) = view.rows();
      let Ok(samples) = tight(rows);
      samples
    }
    None => Vec::new(),
  }))
}

fn from_av1(frame: &av1::Frame) -> Result<Decoded, DecodeFault> {
  let colour = frame.color_info().map_err(DecodeFault::Av1)?;
  let Ok(matrix) = av1_matrix(colour.matrix_coefficients);

  let range = match colour.color_range {
    av1::ColorRange::Full => Range::Full,
    av1::ColorRange::Limited => Range::Studio,
  };

  let Ok(width) = frame.width();
  let Ok(height) = frame.height();
  let stored = Size { width, height };

  let frame_planes = frame.planes().map_err(DecodeFault::Av1)?;
  let planes = match frame_planes {
    av1::Planes::Depth8(planes) => {
      let Ok(luma) = planes.y();
      let Ok(blue) = planes.u();
      let Ok(red) = planes.v();
      let Ok(samples) = av1_samples([Some(luma), blue, red]);

      Planes::Eight(samples)
    }
    av1::Planes::Depth16(planes) => {
      let Ok(luma) = planes.y();
      let Ok(blue) = planes.u();
      let Ok(red) = planes.v();
      let Ok(samples) = av1_samples([Some(luma), blue, red]);
      let Ok(depth) = frame.bit_depth();

      Planes::Deep(samples, u32::from(depth))
    }
  };

  let Ok(chroma) = frame.chroma_dimensions();

  let Ok(pts) = frame.timestamp();

  Ok(Decoded {
    pts,
    stored,
    chroma,
    planes,
    strides: Strides {
      luma: stored.width,
      chroma: chroma.width,
    },
    colour: Colour { matrix, range },
  })
}

fn av1_ready(decoder: &mut av1::Decoder, into: &mut Vec<Decoded>) -> Result<(), DecodeFault> {
  let taken =
    console_core_iteration::iterate((&mut *decoder, &mut *into), |(decoder_2, into_2)| {
      Ok(match decoder_2.get_frame() {
        Ok(Some(frame)) => match from_av1(&frame) {
          Ok(decoded) => {
            into_2.push(decoded);
            Step::Again((decoder_2, into_2))
          }
          Err(fault) => Step::Halt(Err(fault)),
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

fn av1_fed(
  decoder: &mut av1::Decoder,
  unit: &[u8],
  pts: i64,
) -> Result<Vec<Decoded>, DecodeFault> {
  let mut ready = Vec::new();

  let sent =
    console_core_iteration::iterate((&mut *decoder, &mut ready), |(decoder_2, ready_2)| {
      Ok(match decoder_2.decode_stamped(unit, pts) {
        Ok(Some(frame)) => match from_av1(&frame) {
          Ok(decoded) => {
            ready_2.push(decoded);
            Step::Halt(Ok(()))
          }
          Err(fault) => Step::Halt(Err(fault)),
        },
        Ok(None) => Step::Halt(Ok(())),
        Err(fault) => match fault.decompose().0 {
          av1::Error::NeedMoreData => match av1_ready(decoder_2, ready_2) {
            Ok(()) => Step::Again((decoder_2, ready_2)),
            Err(fault_2) => Step::Halt(Err(fault_2)),
          },
          fault_3 @ (av1::Error::InvalidSettings(_)
          | av1::Error::InitFailed
          | av1::Error::OutOfMemory
          | av1::Error::InvalidData
          | av1::Error::Cancelled
          | av1::Error::Other(_)) => Step::Halt(Err(DecodeFault::Av1(fault_3))),
        },
      })
    });

  match sent {
    Ok(Ok(())) => {}
    Ok(Err(fault)) => return Err(fault),
    Err(_endless) => return Ok(ready),
  }

  av1_ready(decoder, &mut ready)?;

  Ok(ready)
}

#[cfg(test)]
mod plane_tests {
  use super::{Planes, hevc, planes_of, tight, vp9};

  #[test]
  fn both_decoders_hand_back_the_same_sample_storage() {
    let luma: hevc::Plane = vp9::Plane::U16(vec![1023, 512]);
    let Ok(planes) = planes_of(
      [
        luma,
        vp9::Plane::U16(vec![256]),
        hevc::Plane::U16(vec![768]),
      ],
      10,
    );

    assert_eq!(
      planes,
      Planes::Deep([vec![1023, 512], vec![256], vec![768]], 10)
    );
  }

  #[test]
  fn packing_visible_rows_preserves_samples_without_row_padding()
  -> ::core::result::Result<(), impl ::core::fmt::Debug> {
    use console_core_geometry::Size;
    use console_core_pixel_buffers::RowLayout;

    let rows = [[1023u16, 512, 9999], [256, 768, 9999]];
    let layout = match RowLayout::new(
      Size {
        width: 2,
        height: 2,
      },
      0,
      3,
    ) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(read_error),
    };
    let Ok(packed) = tight(match layout.rows(rows.as_flattened()) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(read_error),
    });

    assert_eq!(packed, [1023, 512, 256, 768]);
    ::core::result::Result::Ok(())
  }
}
