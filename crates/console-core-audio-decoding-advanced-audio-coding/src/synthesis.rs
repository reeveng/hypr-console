//! From spectrum to samples: the inverse transform of a window, the window
//! function that shapes it, and the overlap-add with the tail of the last one.
//!
//! A long window is 2048 samples and overlaps its neighbours by half. A block
//! of eight short windows is laid out inside the same span, so the start and
//! stop windows either side of it are long windows with a short slope on the
//! side that meets it, which is what the cut points below are.

use console_core_audio_decoding_interfaces::errors::Result;
use console_core_audio_windows::{Extent, Gain, KaiserAlpha, WindowType, generate_window};
use console_core_frequency_transforms::mdct::{Imdct, Scale};

use crate::ics::{IcsInfo, WindowSequence, WindowShape};
use crate::lane::{Lane, Sure, invalid, transformed};
use crate::subbands::Spectrum;

const SHORT_WIN_POINT0: u32 = 512 - 64;

const SHORT_WIN_POINT1: u32 = 512 + 64;

struct Windows {
  long: [f32; 1024],
  short: [f32; 128],
}

pub(crate) struct Dsp {
  kaiser_bessel: Windows,
  sine: Windows,
  imdct_long: Imdct,
  imdct_short: Imdct,
  pcm_long: [f32; 2048],
  pcm_short: [f32; 1152],
}

impl Dsp {
  pub(crate) fn new() -> Result<Self> {
    let mut kaiser_bessel = Windows {
      long: [0.0; 1024],
      short: [0.0; 128],
    };
    let mut sine = Windows {
      long: [0.0; 1024],
      short: [0.0; 128],
    };

    let Ok(()) = generate_window(
      WindowType::KaiserBessel(KaiserAlpha(4.0)),
      Gain(1.0),
      1024,
      Extent::Half,
      &mut kaiser_bessel.long,
    );
    let Ok(()) = generate_window(
      WindowType::KaiserBessel(KaiserAlpha(6.0)),
      Gain(1.0),
      128,
      Extent::Half,
      &mut kaiser_bessel.short,
    );
    let Ok(()) = generate_window(
      WindowType::Sine,
      Gain(1.0),
      1024,
      Extent::Half,
      &mut sine.long,
    );
    let Ok(()) = generate_window(
      WindowType::Sine,
      Gain(1.0),
      128,
      Extent::Half,
      &mut sine.short,
    );

    let imdct_long = transformed(Imdct::new_scaled(1024, Scale(1.0 / 2048.0)))?;
    let imdct_short = transformed(Imdct::new_scaled(128, Scale(1.0 / 256.0)))?;

    Ok(Self {
      kaiser_bessel,
      sine,
      imdct_long,
      imdct_short,
      pcm_long: [0.0; 2048],
      pcm_short: [0.0; 1152],
    })
  }

  pub(crate) fn synth(
    &mut self,
    coeffs: &Spectrum,
    delay: &mut Spectrum,
    info: &IcsInfo,
    dst: &mut [f32],
  ) -> Result<()> {
    let sequence = info.window_sequence;
    let Ok((current_long, current_short)) = self.windows(info.window_shape);
    let (long_win, short_win) = (*current_long, *current_short);
    let Ok((previous_long, previous_short)) = self.windows(info.prev_window_shape);
    let (prev_long_win, prev_short_win) = (*previous_long, *previous_short);

    self.transform(coeffs, sequence, &short_win, &prev_short_win)?;

    let dst_2 = dst.between_mut(0..1024)?;

    self.output(sequence, delay, dst_2, &prev_long_win, &prev_short_win)?;
    self.save_delay(sequence, delay, &long_win, &short_win)
  }

  fn windows(&self, shape: WindowShape) -> Sure<(&[f32; 1024], &[f32; 128])> {
    Ok(match shape {
      WindowShape::KaiserBessel => (&self.kaiser_bessel.long, &self.kaiser_bessel.short),
      WindowShape::Sine => (&self.sine.long, &self.sine.short),
    })
  }

  fn transform(
    &mut self,
    coeffs: &Spectrum,
    sequence: WindowSequence,
    short_win: &[f32; 128],
    prev_short_win: &[f32; 128],
  ) -> Result<()> {
    match sequence {
      WindowSequence::EightShort => self.transform_short(coeffs, short_win, prev_short_win),
      WindowSequence::OnlyLong | WindowSequence::LongStart | WindowSequence::LongStop => {
        transformed(self.imdct_long.imdct(coeffs, &mut self.pcm_long))
      }
    }
  }

  fn transform_short(
    &mut self,
    coeffs: &Spectrum,
    short_win: &[f32; 128],
    prev_short_win: &[f32; 128],
  ) -> Result<()> {
    let (inputs, _) = coeffs.as_chunks::<128>();
    let (outputs, _) = self.pcm_long.as_chunks_mut::<256>();

    for (input, output) in inputs.iter().zip(outputs) {
      transformed(self.imdct_short.imdct(input, output))?;
    }

    self.pcm_short.fill(0.0);

    let (sources, _) = self.pcm_long.as_chunks::<256>();

    for (window, source) in (0u32..).zip(sources) {
      let base = window.saturating_mul(128);
      let target = self.pcm_short.between_mut(base..base.saturating_add(256))?;

      match window {
        0 => first_window(target, source, prev_short_win, short_win)?,
        _ => add_window(target, source, (short_win, short_win))?,
      }
    }

    Ok(())
  }

  fn output(
    &self,
    sequence: WindowSequence,
    delay: &Spectrum,
    dst: &mut [f32],
    prev_long_win: &[f32; 1024],
    prev_short_win: &[f32; 128],
  ) -> Result<()> {
    match sequence {
      WindowSequence::OnlyLong | WindowSequence::LongStart => {
        let source = self.pcm_long.between(0..1024)?;

        for ((out, held), (pcm, win)) in dst
          .iter_mut()
          .zip(delay)
          .zip(source.iter().zip(prev_long_win))
        {
          *out = *held + (*pcm * *win);
        }

        Ok(())
      }
      WindowSequence::EightShort => {
        let head = dst.between_mut(0..SHORT_WIN_POINT0)?;
        let kept = delay.between(0..SHORT_WIN_POINT0)?;

        copy(head, kept)?;

        let tail = delay.between(SHORT_WIN_POINT0..1024)?;
        let pcm = self.pcm_short.between(0..576)?;
        let out = dst.between_mut(SHORT_WIN_POINT0..1024)?;

        for ((out_2, held), pcm_2) in out.iter_mut().zip(tail).zip(pcm) {
          *out_2 = *held + *pcm_2;
        }

        Ok(())
      }
      WindowSequence::LongStop => self.output_stop(delay, dst, prev_short_win),
    }
  }

  fn output_stop(
    &self,
    delay: &Spectrum,
    dst: &mut [f32],
    prev_short_win: &[f32; 128],
  ) -> Result<()> {
    let head = dst.between_mut(0..SHORT_WIN_POINT0)?;
    let kept = delay.between(0..SHORT_WIN_POINT0)?;

    copy(head, kept)?;

    let sloped = dst.between_mut(SHORT_WIN_POINT0..SHORT_WIN_POINT1)?;
    let held = delay.between(SHORT_WIN_POINT0..SHORT_WIN_POINT1)?;
    let pcm = self.pcm_long.between(SHORT_WIN_POINT0..SHORT_WIN_POINT1)?;

    for ((out, held_2), (pcm_2, win)) in sloped
      .iter_mut()
      .zip(held)
      .zip(pcm.iter().zip(prev_short_win))
    {
      *out = *held_2 + *pcm_2 * *win;
    }

    let flat = dst.between_mut(SHORT_WIN_POINT1..1024)?;
    let held_3 = delay.between(SHORT_WIN_POINT1..1024)?;
    let pcm_3 = self.pcm_long.between(SHORT_WIN_POINT1..1024)?;

    for ((out, held_4), pcm_4) in flat.iter_mut().zip(held_3).zip(pcm_3) {
      *out = *held_4 + *pcm_4;
    }

    Ok(())
  }

  fn save_delay(
    &self,
    sequence: WindowSequence,
    delay: &mut Spectrum,
    long_win: &[f32; 1024],
    short_win: &[f32; 128],
  ) -> Result<()> {
    match sequence {
      WindowSequence::OnlyLong | WindowSequence::LongStop => {
        let pcm = self.pcm_long.between(1024..2048)?;

        for (held, (pcm_2, win)) in delay.iter_mut().zip(pcm.iter().zip(long_win.iter().rev())) {
          *held = *pcm_2 * *win;
        }

        Ok(())
      }
      WindowSequence::EightShort => {
        let head = delay.between_mut(0..SHORT_WIN_POINT1)?;
        let pcm = self.pcm_short.between(SHORT_WIN_POINT1..1152)?;

        copy(head, pcm)?;

        let rest = delay.between_mut(SHORT_WIN_POINT1..1024)?;

        rest.fill(0.0);

        Ok(())
      }
      WindowSequence::LongStart => {
        let head = delay.between_mut(0..SHORT_WIN_POINT0)?;
        let pcm = self.pcm_long.between(1024..1472)?;

        copy(head, pcm)?;

        let sloped = delay.between_mut(SHORT_WIN_POINT0..SHORT_WIN_POINT1)?;
        let pcm_3 = self.pcm_long.between(1472..1600)?;

        for (held, (pcm_4, win)) in sloped
          .iter_mut()
          .zip(pcm_3.iter().zip(short_win.iter().rev()))
        {
          *held = *pcm_4 * *win;
        }

        let rest = delay.between_mut(SHORT_WIN_POINT1..1024)?;

        rest.fill(0.0);

        Ok(())
      }
    }
  }
}

fn copy(dst: &mut [f32], src: &[f32]) -> Result<()> {
  for (out, held) in dst.iter_mut().zip(src) {
    *out = *held;
  }

  Ok(())
}

fn first_window(
  target: &mut [f32],
  source: &[f32; 256],
  prev_short_win: &[f32; 128],
  short_win: &[f32; 128],
) -> Result<()> {
  target.fill(0.0);

  add_window(target, source, (prev_short_win, short_win))
}

fn add_window(
  target: &mut [f32],
  source: &[f32; 256],
  wins: (&[f32; 128], &[f32; 128]),
) -> Result<()> {
  let (low_win, high_win) = wins;
  let (halves, _) = target.as_chunks_mut::<128>();
  let (parts, _) = source.as_chunks::<128>();

  match (halves, parts) {
    ([low, high], [first, second]) => {
      for ((out, src), win) in low.iter_mut().zip(first).zip(low_win) {
        *out += *src * *win;
      }

      for ((out, src), win) in high.iter_mut().zip(second).zip(high_win.iter().rev()) {
        *out += *src * *win;
      }

      Ok(())
    }
    _ => invalid(),
  }
}
