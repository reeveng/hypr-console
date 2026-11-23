//! The radix-2 Cooley-Tukey FFT, from the bit-reversal permutation up.
//!
//! The permutation is applied once as a gather, the blocks of 32 are finished
//! by unrolled kernels that call each other down to two points, and every
//! stage above that is a pass of butterflies over the twiddle factors for its
//! size. That is the order the recursive original did its arithmetic in, taken
//! bottom-up, so every output is the same number. The twiddle factors belong to
//! the transform that needs them rather than to the process, and a transform
//! of 65536 points is not one: the permutation table is 16-bit, which made the
//! largest size the original advertised an empty one.

use console_core_never::Never;
use console_core_number_conversion::{index, narrow_f32};
use core::f32::consts::FRAC_1_SQRT_2;
use core::f64::consts::PI;

use crate::complex::Complex;
use crate::{BufferLength, TransformError, TransformSize};

pub(crate) struct Fft {
  permutation: Box<[u16]>,
  stages: Box<[Box<[Complex]>]>,
}

fn twiddles(log: u32) -> Result<Box<[Complex]>, Never> {
  let count = 1u32.wrapping_shl(log.saturating_sub(1));
  let theta = PI / f64::from(count);

  Ok(
    (0..count)
      .map(|step| {
        let angle = theta * f64::from(step);
        let Ok(re) = narrow_f32(angle.cos());
        let Ok(im) = narrow_f32(-angle.sin());

        Complex { re, im }
      })
      .collect(),
  )
}

fn butterflies(
  even: &mut [Complex],
  odd: &mut [Complex],
  table: &[Complex],
) -> Result<(), Never> {
  let evens = even.as_chunks_mut::<2>().0.iter_mut();
  let odds = odd.as_chunks_mut::<2>().0.iter_mut();

  for ((e, o), w) in evens.zip(odds).zip(table.as_chunks::<2>().0.iter()) {
    let [e0, e1] = e;
    let [o0, o1] = o;
    let [w0, w1] = w;
    let (p0, q0) = (*e0, *o0 * *w0);
    let (p1, q1) = (*e1, *o1 * *w1);

    *e0 = p0 + q0;
    *o0 = p0 - q0;
    *e1 = p1 + q1;
    *o1 = p1 - q1;
  }

  Ok(())
}

impl Fft {
  pub(crate) fn buffer(&self) -> Result<Box<[Complex]>, Never> {
    Ok(vec![Complex::default(); self.permutation.len()].into_boxed_slice())
  }

  pub(crate) fn new(size: u32) -> Result<Fft, TransformError> {
    let count = match u16::try_from(size) {
      Ok(count) => count,
      Err(_too_large) => return Err(TransformError::Size),
    };

    match TransformSize::from(size.is_power_of_two() && size >= 2) {
      TransformSize::Supported => {
        let shift = count.leading_zeros().saturating_add(1);
        let permutation = (0..count)
          .map(|at| at.reverse_bits().wrapping_shr(shift))
          .collect();
        let stages = (6..=size.trailing_zeros())
          .map(|log| {
            let Ok(table) = twiddles(log);

            table
          })
          .collect();

        Ok(Fft {
          permutation,
          stages,
        })
      }
      TransformSize::Unsupported => Err(TransformError::Size),
    }
  }

  pub(crate) fn fft(&self, x: &[Complex], y: &mut [Complex]) -> Result<(), TransformError> {
    match (
      BufferLength::from(x.len() == self.permutation.len()),
      BufferLength::from(y.len() == self.permutation.len()),
    ) {
      (BufferLength::Exact, BufferLength::Exact) => {}
      (BufferLength::Mismatched, BufferLength::Exact | BufferLength::Mismatched) => {
        return Err(TransformError::Spectrum);
      }
      (BufferLength::Exact, BufferLength::Mismatched) => return Err(TransformError::Output),
    }

    for (slot, source) in y.iter_mut().zip(self.permutation.iter()) {
      let Ok(at) = index(*source);

      match x.get(at) {
        Some(value) => *slot = *value,
        None => return Err(TransformError::Spectrum),
      }
    }

    match y.len() {
      2 => {
        for block in y.as_chunks_mut::<2>().0 {
          let Ok(done) = fft2(*block);

          *block = done;
        }
      }
      4 => {
        for block in y.as_chunks_mut::<4>().0 {
          let Ok(done) = fft4(*block);

          *block = done;
        }
      }
      8 => {
        for block in y.as_chunks_mut::<8>().0 {
          let Ok(done) = fft8(*block);

          *block = done;
        }
      }
      16 => {
        for block in y.as_chunks_mut::<16>().0 {
          let Ok(done) = fft16(*block);

          *block = done;
        }
      }
      _ => {
        for block in y.as_chunks_mut::<16>().0.as_chunks_mut::<2>().0 {
          let Ok(()) = fft32(block);
        }

        for table in self.stages.iter() {
          for block in y.chunks_exact_mut(table.len().saturating_mul(2).max(1)) {
            let (even, odd) = block.split_at_mut(table.len());
            let Ok(()) = butterflies(even, odd, table);
          }
        }
      }
    }

    Ok(())
  }
}

fn fft2([x0, x1]: [Complex; 2]) -> Result<[Complex; 2], Never> {
  Ok([x0 + x1, x0 - x1])
}

fn fft4([x0, x1, x2, x3]: [Complex; 4]) -> Result<[Complex; 4], Never> {
  let (e0, e1) = (x0 + x1, x0 - x1);
  let (o0, o1) = (x2 + x3, x2 - x3);
  let turned = Complex {
    re: o1.im,
    im: -o1.re,
  };

  Ok([e0 + o0, e1 + turned, e0 - o0, e1 - turned])
}

fn fft8([x0, x1, x2, x3, x4, x5, x6, x7]: [Complex; 8]) -> Result<[Complex; 8], Never> {
  let Ok([e0, e1, e2, e3]) = fft4([x0, x1, x2, x3]);
  let Ok([o0, o1, o2, o3]) = fft4([x4, x5, x6, x7]);
  let (a1, b1) = (FRAC_1_SQRT_2 * o1.re, FRAC_1_SQRT_2 * o1.im);
  let (a3, b3) = (-FRAC_1_SQRT_2 * o3.re, -FRAC_1_SQRT_2 * o3.im);
  let [p0, p1, p2, p3] = [
    o0,
    Complex {
      re: a1 + b1,
      im: b1 - a1,
    },
    Complex {
      re: o2.im,
      im: -o2.re,
    },
    Complex {
      re: a3 - b3,
      im: a3 + b3,
    },
  ];

  Ok([
    e0 + p0,
    e1 + p1,
    e2 + p2,
    e3 + p3,
    e0 - p0,
    e1 - p1,
    e2 - p2,
    e3 - p3,
  ])
}

fn fft16(
  [
    x0,
    x1,
    x2,
    x3,
    x4,
    x5,
    x6,
    x7,
    x8,
    x9,
    x10,
    x11,
    x12,
    x13,
    x14,
    x15,
  ]: [Complex; 16],
) -> Result<[Complex; 16], Never> {
  let Ok([e0, e1, e2, e3, e4, e5, e6, e7]) = fft8([x0, x1, x2, x3, x4, x5, x6, x7]);
  let Ok([o0, o1, o2, o3, o4, o5, o6, o7]) = fft8([x8, x9, x10, x11, x12, x13, x14, x15]);
  let (a2, b2) = (FRAC_1_SQRT_2 * o2.re, FRAC_1_SQRT_2 * o2.im);
  let (a6, b6) = (-FRAC_1_SQRT_2 * o6.re, -FRAC_1_SQRT_2 * o6.im);
  let [p0, p1, p2, p3, p4, p5, p6, p7] = [
    o0,
    Complex {
      re: 0.9238795,
      im: -0.38268343,
    } * o1,
    Complex {
      re: a2 + b2,
      im: b2 - a2,
    },
    Complex {
      re: 0.38268343,
      im: -0.9238795,
    } * o3,
    Complex {
      re: o4.im,
      im: -o4.re,
    },
    Complex {
      re: -0.38268343,
      im: -0.9238795,
    } * o5,
    Complex {
      re: a6 - b6,
      im: a6 + b6,
    },
    Complex {
      re: -0.9238795,
      im: -0.38268343,
    } * o7,
  ];

  Ok([
    e0 + p0,
    e1 + p1,
    e2 + p2,
    e3 + p3,
    e4 + p4,
    e5 + p5,
    e6 + p6,
    e7 + p7,
    e0 - p0,
    e1 - p1,
    e2 - p2,
    e3 - p3,
    e4 - p4,
    e5 - p5,
    e6 - p6,
    e7 - p7,
  ])
}

fn fft32([low, high]: &mut [[Complex; 16]; 2]) -> Result<(), Never> {
  let Ok(even) = fft16(*low);
  let Ok(odd) = fft16(*high);
  let Ok(turned) = rotate_fft32(odd);

  for (((sum, difference), even_bin), odd_bin) in low.iter_mut().zip(high).zip(even).zip(turned) {
    *sum = even_bin + odd_bin;
    *difference = even_bin - odd_bin;
  }

  Ok(())
}

fn rotate_fft32(
  [
    o0,
    o1,
    o2,
    o3,
    o4,
    o5,
    o6,
    o7,
    o8,
    o9,
    o10,
    o11,
    o12,
    o13,
    o14,
    o15,
  ]: [Complex; 16],
) -> Result<[Complex; 16], Never> {
  let (a4, b4) = (FRAC_1_SQRT_2 * o4.re, FRAC_1_SQRT_2 * o4.im);
  let (a12, b12) = (-FRAC_1_SQRT_2 * o12.re, -FRAC_1_SQRT_2 * o12.im);

  Ok([
    o0,
    Complex {
      re: 0.98078525,
      im: -0.19509032,
    } * o1,
    Complex {
      re: 0.9238795,
      im: -0.38268343,
    } * o2,
    Complex {
      re: 0.8314696,
      im: -0.55557024,
    } * o3,
    Complex {
      re: a4 + b4,
      im: b4 - a4,
    },
    Complex {
      re: 0.55557024,
      im: -0.8314696,
    } * o5,
    Complex {
      re: 0.38268343,
      im: -0.9238795,
    } * o6,
    Complex {
      re: 0.19509032,
      im: -0.98078525,
    } * o7,
    Complex {
      re: o8.im,
      im: -o8.re,
    },
    Complex {
      re: -0.19509032,
      im: -0.98078525,
    } * o9,
    Complex {
      re: -0.38268343,
      im: -0.9238795,
    } * o10,
    Complex {
      re: -0.55557024,
      im: -0.8314696,
    } * o11,
    Complex {
      re: a12 - b12,
      im: a12 + b12,
    },
    Complex {
      re: -0.8314696,
      im: -0.55557024,
    } * o13,
    Complex {
      re: -0.9238795,
      im: -0.38268343,
    } * o14,
    Complex {
      re: -0.98078525,
      im: -0.19509032,
    } * o15,
  ])
}
