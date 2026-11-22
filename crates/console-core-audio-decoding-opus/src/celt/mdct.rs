//! Inverse MDCT with the low-overlap window and in-place TDAC (RFC 6716 section 4.3.7).
//!
//! The IMDCT is computed through an N/4-point complex FFT (pre-rotation, FFT, post-rotation),
//! producing the N/2 folded time samples; the window overlap is then unfolded against the
//! folded tail of the previous block, which the caller keeps in place in the output buffer.
//!
//! CELT's sizes are 60, 120, 240 and 480 points, which are not powers of two, so the FFT is
//! the mixed-radix one of radices 4, 2, 3 and 5 rather than the radix-2 transform the
//! workspace shares. It is the decimation in time the recursive form describes, walked as a
//! loop: the inputs are put in digit-reversed order once, then every stage's butterflies run
//! from the smallest segments up, each segment's after the ones it is made of, which is the
//! order the recursion ran them in and so the same arithmetic in the same order. The
//! butterfly multiplies by each twiddle and sums its terms one radix at a time, exactly as
//! before; only the walk to them changed. The sizes are fixed and every one of them factors
//! completely, so the factoring loop is bounded by the bits in a size.

use std::num::NonZeroU32;

use console_core_never::Never;
use console_core_number_conversion::{index, narrow_f32};

const RADICES: [NonZeroU32; 4] = [
  match NonZeroU32::new(4) {
    Some(four) => four,
    None => NonZeroU32::MIN,
  },
  match NonZeroU32::new(2) {
    Some(two) => two,
    None => NonZeroU32::MIN,
  },
  match NonZeroU32::new(3) {
    Some(three) => three,
    None => NonZeroU32::MIN,
  },
  match NonZeroU32::new(5) {
    Some(five) => five,
    None => NonZeroU32::MIN,
  },
];

#[derive(Clone, Copy, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the fields are the real and imaginary coordinates in the transform’s input unit"
  )
)]
pub(crate) struct Cpx {
  re: f32,
  im: f32,
}

impl Cpx {
  fn mul(self, other: Cpx) -> Result<Cpx, Never> {
    Ok(Cpx {
      re: self.re * other.re - self.im * other.im,
      im: self.re * other.im + self.im * other.re,
    })
  }

  fn add(self, other: Cpx) -> Result<Cpx, Never> {
    Ok(Cpx {
      re: self.re + other.re,
      im: self.im + other.im,
    })
  }
}

#[derive(Clone, Copy)]
struct Stage {
  radix: NonZeroU32,
  span: u32,
  stride: u32,
}

pub(crate) struct Fft {
  size: NonZeroU32,
  stages: Vec<Stage>,
  order: Vec<u32>,
  twiddle: Vec<Cpx>,
}

fn factored(size: u32) -> Result<Vec<NonZeroU32>, Never> {
  let mut factors = Vec::new();
  let mut left = size;

  for radix in RADICES {
    'radix_factors: for _bit in 0..u32::BITS {
      match left % radix {
        0 => {
          factors.push(radix);
          left /= radix;
        }
        _remainder => break 'radix_factors,
      }
    }
  }

  Ok(factors)
}

fn staged(size: u32, factors: &[NonZeroU32]) -> Result<Vec<Stage>, Never> {
  let mut stages = Vec::new();
  let mut span = size;
  let mut stride = 1u32;

  for radix in factors {
    stages.push(Stage {
      radix: *radix,
      span,
      stride,
    });
    span /= *radix;
    stride = stride.wrapping_mul(radix.get());
  }

  Ok(stages)
}

fn reversed(size: u32, stages: &[Stage]) -> Result<Vec<u32>, Never> {
  let mut order = Vec::new();

  for place in 0..size {
    let mut from = 0u32;

    for stage in stages {
      let digits = stage.span / stage.radix;
      let digit = match place.checked_div(digits) {
        Some(above) => above % stage.radix,
        None => 0,
      };

      from = from.wrapping_add(digit.wrapping_mul(stage.stride));
    }

    order.push(from);
  }

  Ok(order)
}

fn turned(size: u32) -> Result<Vec<Cpx>, Never> {
  let mut twiddle = Vec::new();

  for step in 0..size {
    let phase = -2.0 * std::f64::consts::PI * f64::from(step) / f64::from(size);
    let Ok(re) = narrow_f32(phase.cos());
    let Ok(im) = narrow_f32(phase.sin());

    twiddle.push(Cpx { re, im });
  }

  Ok(twiddle)
}

impl Fft {
  fn new(size: NonZeroU32) -> Result<Fft, Never> {
    let Ok(factors) = factored(size.get());
    let Ok(stages) = staged(size.get(), &factors);
    let Ok(order) = reversed(size.get(), &stages);
    let Ok(twiddle) = turned(size.get());

    Ok(Fft {
      size,
      stages,
      order,
      twiddle,
    })
  }

  fn twiddle(&self, at: u32) -> Result<Cpx, Never> {
    let Ok(place) = index(at % self.size);

    Ok(match self.twiddle.get(place) {
      Some(turn) => *turn,
      None => Cpx::default(),
    })
  }

  fn process(&self, data: &mut [Cpx], scratch: &mut Vec<Cpx>) -> Result<(), Never> {
    let Ok(size) = index(self.size.get());

    scratch.clear();
    scratch.extend(data.iter().take(size));

    for (slot, from) in data.iter_mut().zip(&self.order) {
      let Ok(place) = index(*from);

      *slot = match scratch.get(place) {
        Some(value) => *value,
        None => Cpx::default(),
      };
    }

    for stage in self.stages.iter().rev() {
      let Ok(span) = index(stage.span);

      for segment in data.chunks_exact_mut(span.max(1)) {
        let Ok(()) = self.butterfly(segment, *stage);
      }
    }

    Ok(())
  }

  fn butterfly(&self, segment: &mut [Cpx], stage: Stage) -> Result<(), Never> {
    let quarter = stage.span / stage.radix;
    let Ok(apart) = index(quarter);
    let Ok(radix) = index(stage.radix.get());
    let unit = self.size.get() / stage.radix;

    for (offset, column) in (0..quarter).zip(0usize..) {
      let mut terms = [Cpx::default(); 5];

      for ((term, value), row) in terms
        .iter_mut()
        .zip(segment.iter().skip(column).step_by(apart.max(1)))
        .zip(0u32..)
        .take(radix)
      {
        let Ok(turn) = self.twiddle(row.wrapping_mul(offset).wrapping_mul(stage.stride));
        let Ok(product) = value.mul(turn);

        *term = product;
      }

      for (slot, output) in segment
        .iter_mut()
        .skip(column)
        .step_by(apart.max(1))
        .zip(0u32..)
        .take(radix)
      {
        let mut sum = Cpx::default();

        for (term, row) in terms.iter().zip(0u32..).take(radix) {
          let Ok(turn) =
            self.twiddle((row.wrapping_mul(output) % stage.radix).wrapping_mul(unit));
          let Ok(product) = term.mul(turn);
          let Ok(added) = sum.add(product);

          sum = added;
        }

        *slot = sum;
      }
    }

    Ok(())
  }
}

pub(crate) struct Imdct {
  size: u32,
  trig: Vec<f32>,
  fft: Fft,
}

#[derive(Clone, Copy)]
pub(crate) struct Lapping<'a> {
  pub(crate) window: &'a [f32],
  pub(crate) overlap: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct Spectrum<'a> {
  pub(crate) bins: &'a [f32],
  pub(crate) stride: u32,
}

impl Imdct {
  pub(crate) fn new(size: NonZeroU32) -> Result<Imdct, Never> {
    let mut trig = Vec::new();

    for place in 0..size.get().wrapping_shr(1) {
      let angle = 2.0 * std::f64::consts::PI * (f64::from(place) + 0.125) / f64::from(size.get());
      let Ok(cosine) = narrow_f32(angle.cos());

      trig.push(cosine);
    }

    let quarter = match NonZeroU32::new(size.get().wrapping_shr(2)) {
      Some(quarter) => quarter,
      None => NonZeroU32::MIN,
    };
    let Ok(fft) = Fft::new(quarter);

    Ok(Imdct {
      size: size.get(),
      trig,
      fft,
    })
  }

  fn rotated(&self, spectrum: Spectrum<'_>, z: &mut Vec<Cpx>) -> Result<(), Never> {
    let Ok(quarter) = index(self.size.wrapping_shr(2));
    let Ok(stride) = index(spectrum.stride);
    let Ok(pair) = index(spectrum.stride.saturating_mul(2));
    let evens = spectrum.bins.iter().step_by(pair.max(1)).take(quarter);
    let odds = spectrum
      .bins
      .iter()
      .skip(stride)
      .step_by(pair.max(1))
      .take(quarter)
      .rev();
    let cosines = self.trig.iter().zip(self.trig.iter().skip(quarter));

    z.clear();

    for ((x1, x2), (t0, t1)) in evens.zip(odds).zip(cosines) {
      let yr = *x2 * *t0 + *x1 * *t1;
      let yi = *x1 * *t0 - *x2 * *t1;

      z.push(Cpx { re: yi, im: yr });
    }

    Ok(())
  }

  fn unrotated(&self, z: &[Cpx], y: &mut [f32]) -> Result<(), Never> {
    let Ok(quarter) = index(self.size.wrapping_shr(2));
    let Ok(half) = index(self.size.wrapping_shr(1));
    let Ok(pairs) = index(self.size.wrapping_shr(3));
    let (low, high) = match <[_]>::split_at_checked(&self.trig, quarter) {
      ::core::option::Option::Some(split_halves) => split_halves,
      ::core::option::Option::None => {
        let (split_whole, split_empty) = <[_]>::as_chunks::<1>(&self.trig);
        (split_whole.as_flattened(), split_empty)
      }
    };
    let (front, back) = y.split_at_mut(quarter.min(y.len()));
    let fronts = z
      .iter()
      .zip(low.iter().zip(high.iter()))
      .zip(front.as_chunks_mut::<2>().0.iter_mut())
      .take(pairs);
    let backs = z
      .iter()
      .rev()
      .zip(
        low
          .iter()
          .rev()
          .zip(high.iter().take(half.saturating_sub(quarter)).rev()),
      )
      .zip(back.as_chunks_mut::<2>().0.iter_mut().rev());

    for (((from_front, (t0, t1)), [yr0, yi1]), ((from_back, (u0, u1)), [yr1, yi0])) in
      fronts.zip(backs)
    {
      let (re, im) = (from_front.im, from_front.re);

      *yr0 = re * *t0 + im * *t1;

      let first_imaginary = re * *t1 - im * *t0;
      let (re_2, im_2) = (from_back.im, from_back.re);

      *yr1 = re_2 * *u0 + im_2 * *u1;
      *yi1 = re_2 * *u1 - im_2 * *u0;
      *yi0 = first_imaginary;
    }

    Ok(())
  }

  pub(crate) fn backward(
    &self,
    spectrum: Spectrum<'_>,
    out: &mut [f32],
    lapping: Lapping<'_>,
    work: &mut ImdctScratch,
  ) -> Result<(), Never> {
    let Ok(()) = self.rotated(spectrum, &mut work.z);
    let Ok(()) = self.fft.process(&mut work.z, &mut work.scratch);
    let Ok(base) = index(lapping.overlap.wrapping_shr(1));
    let Ok(half) = index(self.size.wrapping_shr(1));

    match out.get_mut(base..base.saturating_add(half)) {
      Some(y) => {
        let Ok(()) = self.unrotated(&work.z, y);
      }
      None => {}
    }

    let Ok(overlap) = index(lapping.overlap);
    let Ok(folded) = index(lapping.overlap.wrapping_shr(1));
    let (window_front, window_back) = match <[_]>::split_at_checked(lapping.window, folded) {
      ::core::option::Option::Some(split_halves) => split_halves,
      ::core::option::Option::None => {
        let (split_whole, split_empty) = <[_]>::as_chunks::<1>(lapping.window);
        (split_whole.as_flattened(), split_empty)
      }
    };
    let lapped = match out.get_mut(..overlap) {
      Some(lapped) => lapped,
      None => return Ok(()),
    };
    let (head, tail) = lapped.split_at_mut(folded.min(overlap));
    let windows = window_front.iter().zip(
      window_back
        .iter()
        .take(overlap.saturating_sub(folded))
        .rev(),
    );

    for ((x2, x1), (w1, w2)) in head.iter_mut().zip(tail.iter_mut().rev()).zip(windows) {
      let (earlier, later) = (*x2, *x1);

      *x2 = earlier * *w2 - later * *w1;
      *x1 = earlier * *w1 + later * *w2;
    }

    Ok(())
  }
}

#[derive(Default)]
pub(crate) struct ImdctScratch {
  z: Vec<Cpx>,
  scratch: Vec<Cpx>,
}
