//! What layers one and two have in common: the table of scale factors both
//! read an index into, the placing of a sample into its subband's row, and
//! the frame around them, which is a checksum to skip, the bits, a layer's own
//! reading of them into a sheet of samples per channel, and the hand-over to
//! the polyphase filterbank, [`crate::synthesis`], that layer three uses too.
//! A layer says only how it reads, as a [`Coding`], and is a [`Layer`] of it.
//!
//! Both layers deal a frame out as thirty-two subbands of a number of time
//! slots, twelve for layer one and thirty-six for layer two, and both keep
//! them as one row of slots to a subband, so the filterbank reads a layer's
//! rows flattened and strides across them by the number of slots.
//!
//! A joint stereo frame in these layers codes everything from the subband the
//! header names upward as one signal, which each channel scales for itself.
//! The scale factor table has a sixty-fourth entry the standard does not: some
//! streams use index 63, and silence is the answer that decodes them.

use std::marker::PhantomData;

use console_core_audio_decoding_interfaces::audio::{AudioBuffer, Signal};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::{BitReaderLtr, BufReader, ReadBytes};
use console_core_number_conversion::{fitted, index};

use crate::common::{ChannelMode, FrameHeader, Infallible, Protection};
use crate::synthesis::{self, SynthesisState};

pub const BANDS: u32 = 32;

#[allow(
  clippy::excessive_precision,
  reason = "the table is the standard's, with the digits it prints"
)]
const SCALE_FACTORS: [f32; 64] = [
  2.00000000000000,
  1.58740105196820,
  1.25992104989487,
  1.00000000000000,
  0.79370052598410,
  0.62996052494744,
  0.50000000000000,
  0.39685026299205,
  0.31498026247372,
  0.25000000000000,
  0.19842513149602,
  0.15749013123686,
  0.12500000000000,
  0.09921256574801,
  0.07874506561843,
  0.06250000000000,
  0.04960628287401,
  0.03937253280921,
  0.03125000000000,
  0.02480314143700,
  0.01968626640461,
  0.01562500000000,
  0.01240157071850,
  0.00984313320230,
  0.00781250000000,
  0.00620078535925,
  0.00492156660115,
  0.00390625000000,
  0.00310039267963,
  0.00246078330058,
  0.00195312500000,
  0.00155019633981,
  0.00123039165029,
  0.00097656250000,
  0.00077509816991,
  0.00061519582514,
  0.00048828125000,
  0.00038754908495,
  0.00030759791257,
  0.00024414062500,
  0.00019377454248,
  0.00015379895629,
  0.00012207031250,
  0.00009688727124,
  0.00007689947814,
  0.00006103515625,
  0.00004844363562,
  0.00003844973907,
  0.00003051757813,
  0.00002422181781,
  0.00001922486954,
  0.00001525878906,
  0.00001211090890,
  0.00000961243477,
  0.00000762939453,
  0.00000605545445,
  0.00000480621738,
  0.00000381469727,
  0.00000302772723,
  0.00000240310869,
  0.00000190734863,
  0.00000151386361,
  0.00000120155435,
  0.00000000000000,
];

pub trait Coding {
  const SLOTS: u32;

  fn read(bs: &mut BitReaderLtr<'_>, header: &FrameHeader, sheets: [&mut [f32]; 2])
  -> Result<()>;
}

pub struct Layer<C> {
  synthesis: [SynthesisState; 2],
  coding: PhantomData<C>,
}

struct Block<'a> {
  slots: u32,
  lanes: [&'a [f32]; 2],
}

pub fn scale_factor(code: u32) -> Result<f32> {
  let Ok(at) = index(code);

  match SCALE_FACTORS.get(at) {
    Some(&factor) => Ok(factor),
    None => decode_error("mpa: invalid scale factor index"),
  }
}

pub fn bound(header: &FrameHeader) -> Infallible<u32> {
  Ok(match header.channel_mode {
    ChannelMode::JointStereo { .. } => header.intensity_bound,
    ChannelMode::Mono | ChannelMode::DualMono | ChannelMode::Stereo => BANDS,
  })
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the normalized subband sample is a dimensionless signal amplitude"
  )
)]
pub fn put(row: &mut [f32], at: u32, value: f32) -> Infallible<()> {
  let Ok(at_2) = index(at);

  match row.get_mut(at_2) {
    Some(cell) => *cell = value,
    None => {}
  }

  Ok(())
}

pub fn centered(sample: [u32; 2]) -> Infallible<i16> {
  let [raw, bits] = sample;
  let Ok(raw_2) = fitted::<u32, i32>(raw);
  let Ok(held) =
    fitted::<i32, i16>(raw_2.saturating_sub(1i32.wrapping_shl(bits.saturating_sub(1))));

  Ok(held)
}

fn render(
  synthesis: &mut [SynthesisState; 2],
  header: &FrameHeader,
  block: Block<'_>,
  out: &mut AudioBuffer<f32>,
) -> Result<()> {
  let Block { slots, lanes } = block;
  let Ok(channels) = index(header.channels);

  out.render_reserved(Some(slots.saturating_mul(32)))?;

  for (lane, (state, samples)) in (0u32..).zip(synthesis.iter_mut().zip(lanes)).take(channels) {
    let Ok(value_1) = out.chan_mut(lane);

    let Ok(()) = synthesis::synthesis(state, slots, samples, value_1);
  }

  Ok(())
}

impl<C: Coding> Layer<C> {
  pub fn new() -> Infallible<Self> {
    Ok(Layer {
      synthesis: Default::default(),
      coding: PhantomData,
    })
  }

  pub fn reset(&mut self) -> Infallible<()> {
    self.synthesis = Default::default();

    Ok(())
  }

  pub fn decode(
    &mut self,
    reader: &mut BufReader<'_>,
    header: &FrameHeader,
    out: &mut AudioBuffer<f32>,
  ) -> Result<()> {
    let crc = match header.protection {
      Protection::Crc => reader.read_be_u16(),
      Protection::None => Ok(0),
    };

    crc?;

    let Ok(available) = reader.read_buf_bytes_available_ref();
    let Ok(mut bs) = BitReaderLtr::new(available);
    let mut sheets = [[0f32; 1152]; 2];
    let [left, right] = &mut sheets;

    C::read(&mut bs, header, [left, right])?;

    let [left_2, right_2] = &sheets;

    render(
      &mut self.synthesis,
      header,
      Block {
        slots: C::SLOTS,
        lanes: [left_2, right_2],
      },
      out,
    )
  }
}
