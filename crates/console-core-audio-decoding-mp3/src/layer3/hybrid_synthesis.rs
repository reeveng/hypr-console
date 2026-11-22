//! The steps of a granule between the stereo decoding and the filterbank: the
//! short block lines put back in subband order, the butterflies across each
//! subband boundary, the inverse transform itself and the inversion of every
//! second sample of every second subband that the filterbank's own sign
//! pattern asks for.

use console_core_never::Never;
use console_core_number_conversion::index;

use super::computed::Computed;
use super::frame::{BlockType, GranuleChannel};
use super::imdct::{imdct12_win, imdct36};
use crate::common::Bands;

type Spectrum = [f32; 576];

pub fn reorder(
  scale_bands: &Bands,
  channel: &mut GranuleChannel,
  buf: &mut Spectrum,
) -> Result<(), Never> {
  let Ok(switch) = index(scale_bands.mixed_switch);

  let bands: &[u32] = match channel.block_type {
    BlockType::Short => scale_bands.short,
    BlockType::Mixed => match scale_bands.mixed.get(switch..) {
      Some(bands) => bands,
      None => return Ok(()),
    },
    BlockType::Long | BlockType::Start | BlockType::End => return Ok(()),
  };

  let start = match bands.first() {
    Some(&start) => start,
    None => return Ok(()),
  };

  let rzero = channel.rzero;
  let triples = bands
    .iter()
    .zip(bands.iter().skip(1))
    .zip(bands.iter().skip(2))
    .zip(bands.iter().skip(3))
    .step_by(3)
    .take_while(|&(((&s0, _), _), _)| s0 < rzero);

  let windows = |from: u32, to: u32| {
    let Ok(from_2) = index(from);
    let Ok(to_2) = index(to);

    buf
      .get(from_2..to_2)
      .into_iter()
      .flat_map(|window| window.iter().copied())
  };

  let reordered: Vec<f32> = triples
    .flat_map(|(((&s0, &s1), &s2), &s3)| {
      windows(s0, s1)
        .zip(windows(s1, s2))
        .zip(windows(s2, s3))
        .flat_map(|((w0, w1), w2)| [w0, w1, w2])
        .collect::<Vec<f32>>()
    })
    .collect();

  let Ok(from) = index(start);

  for (slot, value) in buf.iter_mut().skip(from).zip(reordered.iter()) {
    *slot = *value;
  }

  let written = reordered
    .iter()
    .fold(0u32, |count, _| count.saturating_add(1));

  channel.rzero = channel.rzero.max(start.saturating_add(written));

  Ok(())
}

pub fn antialias(
  channel: &mut GranuleChannel,
  samples: &mut Spectrum,
  computed: &Computed,
) -> Result<(), Never> {
  let sb_limit: u32 = match channel.block_type {
    BlockType::Short => return Ok(()),
    BlockType::Mixed => 2,
    BlockType::Long | BlockType::Start | BlockType::End => 32,
  };

  let sb_rzero = channel.rzero.div_euclid(18);

  channel.rzero = 18u32.saturating_mul(sb_limit.min(sb_rzero.saturating_add(2)).min(32));

  let Ok(antialias) = computed.antialias();

  for boundary in (18..channel.rzero).step_by(18) {
    let Ok(at) = index(boundary);

    for (lower_half, upper_half) in samples.split_at_mut_checked(at).into_iter() {
      let lower = lower_half.iter_mut().rev();
      let upper = upper_half.iter_mut();

      for ((l, u), (cs, ca)) in lower.zip(upper).zip(antialias.iter()) {
        let (lower_value, upper_value) = (*l, *u);

        *l = lower_value * cs - upper_value * ca;
        *u = upper_value * cs + lower_value * ca;
      }
    }
  }

  Ok(())
}

pub fn hybrid_synthesis(
  channel: &GranuleChannel,
  overlap: &mut [[f32; 18]; 32],
  samples: &mut Spectrum,
  computed: &Computed,
) -> Result<(), Never> {
  let sb_limit = channel.rzero.saturating_add(17).div_euclid(18);

  let sb_split: u32 = match channel.block_type {
    BlockType::Short => 0,
    BlockType::Mixed => 2,
    BlockType::Long | BlockType::Start | BlockType::End => 32,
  };

  let Ok([normal, start, short, end]) = computed.windows();
  let Ok(cos12) = computed.cos12();

  let long = match channel.block_type {
    BlockType::Start => start,
    BlockType::End => end,
    BlockType::Long | BlockType::Short | BlockType::Mixed => normal,
  };

  let subbands = samples.as_chunks_mut::<18>().0.iter_mut();

  for (sb, (subband, overlap_2)) in (0u32..).zip(subbands.zip(overlap.iter_mut())) {
    match (sb < sb_limit, sb < sb_split) {
      (true, true) => imdct36(subband, long, overlap_2)?,
      (true, false) => imdct12_win(subband, short, overlap_2, cos12)?,
      (false, _) => {
        *subband = *overlap_2;
        *overlap_2 = [0.0; 18];
      }
    }
  }

  Ok(())
}

pub fn frequency_inversion(samples: &mut Spectrum) -> Result<(), Never> {
  for subband in samples
    .as_chunks_mut::<18>()
    .0
    .iter_mut()
    .skip(1)
    .step_by(2)
  {
    subband
      .iter_mut()
      .skip(1)
      .step_by(2)
      .for_each(|sample| *sample = -*sample);
  }

  Ok(())
}
