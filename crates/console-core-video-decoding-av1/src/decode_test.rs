//! Decode parity test — decodes AV1 bitstreams and produces a hash
//! of the pixel output for parity verification between asm and safe-simd.
//!
//! Usage:
//!   RAV1D_TEST_IVF=/path/to/file.ivf cargo test --lib --release -- decode_test_ivf --nocapture
//!
//! Set RAV1D_TEST_EXPECTED_HASH to verify against a known-good hash.
//!
//! Note: Requires feature "c-ffi" to access the dav1d_* API, OR uses the
//! internal rav1d_* API when c-ffi is not enabled.

#[cfg(test)]
mod tests {
  use core::{cmp::Ordering, fmt};
  use std::{env, fs};

  use console_core_number_conversion::{fixed_index, fixed_offset, index, indices};

  use crate::include::common::bitdepth::BitDepth8;
  use crate::include::dav1d::data::Rav1dData;
  use crate::include::dav1d::dav1d::Rav1dSettings;
  use crate::include::dav1d::headers::Rav1dPixelLayout;
  use crate::include::dav1d::picture::{Rav1dPicture, Rav1dPictureDataComponent};
  use crate::src::c_arc::CArc;
  use crate::src::c_box::CBox;
  use crate::src::error::Rav1dError;
  use crate::src::internal::Rav1dContext;
  use crate::src::lib::{rav1d_close, rav1d_get_picture, rav1d_open, rav1d_send_data};

  #[derive(Debug)]
  enum DecoderStage {
    AllocatePacket,
    Open,
    ReadPicture,
    SendPacket,
    Flush,
  }

  #[derive(Debug)]
  enum FixtureError {
    Read(std::io::Error),
    InvalidHeader,
    InvalidPacket,
    Decoder(DecoderStage, Rav1dError),
    PictureBudgetExceeded,
    InputStalled,
    InvalidDimensions,
    MissingPixels,
    InvalidPlane,
  }

  impl fmt::Display for FixtureError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
      match self {
        Self::Read(error) => write!(to, "Reading the IVF fixture failed ({error})"),
        Self::InvalidHeader => to.write_str("Incomplete IVF header"),
        Self::InvalidPacket => to.write_str("Incomplete IVF packet"),
        Self::Decoder(stage, error) => write!(
          to,
          "The fixture decoder failed during {stage:?} ({error:?})"
        ),
        Self::PictureBudgetExceeded => {
          to.write_str("The decoder exceeded the fixture picture budget")
        }
        Self::InputStalled => {
          to.write_str("The decoder could not accept the remaining fixture packet")
        }
        Self::InvalidDimensions => to.write_str("Invalid fixture picture dimensions"),
        Self::MissingPixels => to.write_str("Missing fixture picture data"),
        Self::InvalidPlane => to.write_str("Invalid fixture plane geometry"),
      }
    }
  }

  type FixtureResult<T> = Result<T, FixtureError>;

  fn parse_ivf(data: &[u8]) -> FixtureResult<Vec<&[u8]>> {
    let (header, mut unread) = data
      .split_at_checked(32)
      .ok_or(FixtureError::InvalidHeader)?;
    let marker = header.get(..4).ok_or(FixtureError::InvalidHeader)?;
    let codec = header.get(8..12).ok_or(FixtureError::InvalidHeader)?;
    assert_eq!(marker, b"DKIF", "Not an IVF file");
    assert_eq!(codec, b"AV01", "Not an AV1 IVF file");

    let Ok(maximum_headers) = fixed_index(unread.len().wrapping_div(12));
    let mut packets = Vec::new();

    for _ in 0..maximum_headers {
      let (record, payload) = match unread.split_at_checked(12) {
        Some(record_and_payload) => record_and_payload,
        None => break,
      };
      let size = record
        .first_chunk::<4>()
        .ok_or(FixtureError::InvalidPacket)?;
      let Ok(packet_size) = index(u32::from_le_bytes(*size));

      match payload.split_at_checked(packet_size) {
        Some((packet, remainder)) => {
          packets.push(packet);
          unread = remainder;
        }
        None => break,
      }
    }

    Ok(packets)
  }

  #[derive(Default)]
  struct DecodedOutput {
    pixels: Vec<u8>,
    frames: u64,
  }

  enum PicturePoll {
    Ready,
    Pending,
  }

  fn read_picture(ctx: &Rav1dContext, output: &mut DecodedOutput) -> FixtureResult<PicturePoll> {
    let mut picture = Rav1dPicture::default();

    match rav1d_get_picture(ctx, &mut picture) {
      Ok(()) => {
        extract_pixels(&picture, &mut output.pixels)?;
        output.frames = output.frames.wrapping_add(1);
        Ok(PicturePoll::Ready)
      }
      Err(Rav1dError::EAGAIN) => Ok(PicturePoll::Pending),
      Err(error) => Err(FixtureError::Decoder(DecoderStage::ReadPicture, error)),
    }
  }

  fn drain_pictures(
    ctx: &Rav1dContext,
    output: &mut DecodedOutput,
    maximum_pictures: u64,
  ) -> FixtureResult<()> {
    for _ in 0..=maximum_pictures {
      let poll = read_picture(ctx, output)?;

      match poll {
        PicturePoll::Pending => return Ok(()),
        PicturePoll::Ready => {}
      }
    }

    Err(FixtureError::PictureBudgetExceeded)
  }

  fn send_packet(
    ctx: &Rav1dContext,
    packet: &[u8],
    output: &mut DecodedOutput,
    maximum_pictures: u64,
  ) -> FixtureResult<()> {
    let Ok(boxed) = CBox::from_box(packet.to_vec().into_boxed_slice());
    let storage = CArc::wrap(boxed)
      .map_err(|error| FixtureError::Decoder(DecoderStage::AllocatePacket, error))?;
    let mut input = Rav1dData::from(storage);

    for _ in 0..=maximum_pictures {
      match rav1d_send_data(ctx, &mut input) {
        Ok(()) => return drain_pictures(ctx, output, maximum_pictures),
        Err(Rav1dError::EAGAIN) => {
          let _retry_after_picture_poll = read_picture(ctx, output)?;
        }
        Err(error) => return Err(FixtureError::Decoder(DecoderStage::SendPacket, error)),
      }
    }

    Err(FixtureError::InputStalled)
  }

  fn decode_packets(
    ctx: &Rav1dContext,
    packets: Vec<&[u8]>,
    maximum_pictures: u64,
  ) -> FixtureResult<DecodedOutput> {
    let mut output = DecodedOutput::default();

    for packet in packets {
      send_packet(ctx, packet, &mut output, maximum_pictures)?;
    }

    match rav1d_send_data(ctx, &mut Rav1dData::default()) {
      Ok(()) | Err(Rav1dError::EAGAIN) => {
        drain_pictures(ctx, &mut output, maximum_pictures)?;
      }
      Err(error) => return Err(FixtureError::Decoder(DecoderStage::Flush, error)),
    }

    Ok(output)
  }

  fn decode_ivf_to_hash(ivf_path: &str) -> FixtureResult<(String, u64)> {
    let ivf_data = fs::read(ivf_path).map_err(FixtureError::Read)?;
    let packets = parse_ivf(&ivf_data)?;
    assert!(!packets.is_empty(), "No frames found in IVF file");

    let settings = Rav1dSettings {
      n_threads: 1,
      max_frame_delay: 1,
      apply_grain: false,
      ..Default::default()
    };
    let (ctx, _handles) = rav1d_open(&settings, &console_core_parallel_execution::Serial)
      .map_err(|error| FixtureError::Decoder(DecoderStage::Open, error))?;

    let Ok(maximum_pictures) = fixed_index(ivf_data.len());
    let decoded = decode_packets(&ctx, packets, maximum_pictures);
    let Ok(()) = rav1d_close(ctx);
    let output = decoded?;
    let Ok([h1, h2]) = console_core_checksums::fnv1a::legacy_av1(&output.pixels);
    Ok((format!("{h1:016x}{h2:016x}"), output.frames))
  }

  struct PlaneRows {
    row_bytes: u64,
    rows: u32,
  }

  fn append_plane(
    output: &mut Vec<u8>,
    plane: &Rav1dPictureDataComponent,
    shape: PlaneRows,
    stride: i64,
  ) -> FixtureResult<()> {
    let Ok(byte_length) = fixed_index(plane.byte_len());
    let Ok(native_length) = index(byte_length);
    let guard = plane.slice::<BitDepth8, _>((0.., ..native_length));
    let bytes: &[u8] = &guard;
    let step = stride.unsigned_abs();
    let direction = stride.cmp(&0);
    let origin = match direction {
      Ordering::Less => byte_length
        .checked_sub(step)
        .ok_or(FixtureError::InvalidPlane)?,
      Ordering::Equal | Ordering::Greater => 0,
    };

    for row in 0..shape.rows {
      let distance = u64::from(row)
        .checked_mul(step)
        .ok_or(FixtureError::InvalidPlane)?;
      let row_start = match direction {
        Ordering::Less => origin.checked_sub(distance),
        Ordering::Equal | Ordering::Greater => origin.checked_add(distance),
      };
      let start = row_start.ok_or(FixtureError::InvalidPlane)?;
      let end = start
        .checked_add(shape.row_bytes)
        .ok_or(FixtureError::InvalidPlane)?;
      let Ok(row_bounds) = indices(start..end);
      let pixels = bytes.get(row_bounds).ok_or(FixtureError::InvalidPlane)?;
      output.extend_from_slice(pixels);
    }

    Ok(())
  }

  fn extract_pixels(pic: &Rav1dPicture, output: &mut Vec<u8>) -> FixtureResult<()> {
    let width =
      u32::try_from(pic.p.w).map_err(|_negative_width| FixtureError::InvalidDimensions)?;
    let height =
      u32::try_from(pic.p.h).map_err(|_negative_height| FixtureError::InvalidDimensions)?;
    let data = pic.data.as_ref().ok_or(FixtureError::MissingPixels)?;
    let [luma, chroma_u, chroma_v] = &data.data;
    let [luma_stride, chroma_stride] = pic.stride.map(|pitch| {
      let Ok(distance) = fixed_offset(pitch);
      distance
    });
    let bytes_per_sample = match pic.p.bpc.cmp(&8) {
      Ordering::Greater => 2u64,
      Ordering::Less | Ordering::Equal => 1,
    };

    append_plane(
      output,
      luma,
      PlaneRows {
        row_bytes: u64::from(width).wrapping_mul(bytes_per_sample),
        rows: height,
      },
      luma_stride,
    )?;

    let (chroma_width, chroma_height) = match pic.p.layout {
      Rav1dPixelLayout::I400 => return Ok(()),
      Rav1dPixelLayout::I420 => (width.div_ceil(2), height.div_ceil(2)),
      Rav1dPixelLayout::I422 => (width.div_ceil(2), height),
      Rav1dPixelLayout::I444 => (width, height),
    };

    for plane in [chroma_u, chroma_v] {
      append_plane(
        output,
        plane,
        PlaneRows {
          row_bytes: u64::from(chroma_width).wrapping_mul(bytes_per_sample),
          rows: chroma_height,
        },
        chroma_stride,
      )?;
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    expect(
      explicit026_env_read_once,
      reason = "the fixture path and its optional golden hash are configuration from the test runner"
    )
  )]
  #[test]
  fn decode_test_ivf() -> FixtureResult<()> {
    let ivf_path = match env::var("RAV1D_TEST_IVF") {
      Ok(path) => path,
      Err(env::VarError::NotPresent | env::VarError::NotUnicode(_)) => {
        eprintln!("RAV1D_TEST_IVF not set, skipping decode test");
        return Ok(());
      }
    };
    let (hash, frame_count) = decode_ivf_to_hash(&ivf_path)?;
    let mode = "safe-simd";

    eprintln!("Decoded {} frames in {} mode", frame_count, mode);
    eprintln!("Pixel hash: {}", hash);

    let expected_hash = match env::var("RAV1D_TEST_EXPECTED_HASH") {
      Ok(expected) => Some(expected),
      Err(env::VarError::NotPresent | env::VarError::NotUnicode(_)) => None,
    };

    match expected_hash {
      Some(expected) => {
        assert_eq!(
          hash, expected,
          "Parity mismatch! {} mode produced different output.\n\n\
                 Expected: {}\n\n\
                 Got:      {}",
          mode, expected, hash
        );
        eprintln!("Parity verified: {} matches expected hash", mode);
      }
      None => {}
    }

    Ok(())
  }
}
