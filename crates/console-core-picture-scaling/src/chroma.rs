//! Bilinear interpolation of centred chroma at twice its original size.
//!
//! [`chroma_2x_rows`] repeats the closest row at the edges of a plane.
//! [`upsample_chroma_2x_bilinear`] can write a prefix of the doubled row,
//! including an odd width.
//! WebP rounds halfway samples up. JPEG alternates the direction by column.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromaRounding {
  HalfUp,
  AlternateColumns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MismatchedChromaRows;

impl core::fmt::Display for MismatchedChromaRows {
  fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    formatter.write_str("incompatible chroma row lengths")
  }
}

impl core::error::Error for MismatchedChromaRows {}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "JPEG and WebP callers destructure the existing pair of row indices"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "the existing chroma API accepts the output row followed by the source height"
  )
)]
#[inline]
#[must_use]
pub fn chroma_2x_rows(row: u32, source_height: u32) -> (u32, u32) {
  let last = source_height.saturating_sub(1);
  let near = row.wrapping_shr(1);
  let far = match row & 1 {
    0 => near.saturating_sub(1),
    _odd_row => near.saturating_add(1),
  };
  (near.min(last), far.min(last))
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the existing chroma API reports incompatible row lengths as None"
  )
)]
#[inline]
pub fn upsample_chroma_2x_bilinear(near: &[u8], far: &[u8], output: &mut [u8]) -> Option<()> {
  match ChromaRounding::HalfUp.upsample_2x(near, far, output) {
    Ok(()) => Some(()),
    Err(MismatchedChromaRows) => None,
  }
}

impl ChromaRounding {
  #[inline]
  pub fn upsample_2x(
    self,
    near: &[u8],
    far: &[u8],
    output: &mut [u8],
  ) -> Result<(), MismatchedChromaRows> {
    use core::cmp::Ordering;

    match (
      near.len().cmp(&far.len()),
      output.len().div_ceil(2).cmp(&near.len()),
    ) {
      (Ordering::Equal, Ordering::Less | Ordering::Equal) => {}
      _mismatch => return Err(MismatchedChromaRows),
    }

    let mut vertical = near.iter().zip(far).map(|(&near_2, &far_2)| {
      u16::from(near_2)
        .wrapping_mul(3)
        .wrapping_add(u16::from(far_2))
    });
    let mut current = match vertical.next() {
      Some(first) => first,
      None => return Ok(()),
    };
    let mut previous = current;
    let (pairs, tail) = output.as_chunks_mut::<2>();
    let right_bias = match self {
      Self::HalfUp => 8,
      Self::AlternateColumns => 7,
    };

    for pair in pairs {
      let next = match vertical.next() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => current,
      };
      let Ok(left) = interpolated(InterpolatedArguments {
        near: current,
        far: previous,
        bias: 8,
      });
      let Ok(right) = interpolated(InterpolatedArguments {
        near: current,
        far: next,
        bias: right_bias,
      });

      *pair = [left, right];
      previous = current;
      current = next;
    }

    match tail.first_mut() {
      Some(last) => {
        let Ok(sample) = interpolated(InterpolatedArguments {
          near: current,
          far: previous,
          bias: 8,
        });
        *last = sample;
      }
      None => {}
    }

    Ok(())
  }
}

#[inline]
fn interpolated(
  InterpolatedArguments { near, far, bias }: InterpolatedArguments,
) -> Result<u8, Never> {
  let value = near
    .wrapping_mul(3)
    .wrapping_add(far)
    .wrapping_add(bias)
    .wrapping_shr(4);
  let [sample, _] = value.to_le_bytes();
  Ok(sample)
}

struct InterpolatedArguments {
  near: u16,
  far: u16,
  bias: u16,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn jpeg_rounds_exact_halves_down_in_odd_columns_and_webp_rounds_them_up() {
    let samples = [4, 6, 4];
    let mut row = [0; 6];
    assert_eq!(
      ChromaRounding::AlternateColumns.upsample_2x(&samples, &samples, &mut row),
      Ok(())
    );
    assert_eq!(row, [4, 4, 6, 5, 5, 4]);
    assert_eq!(
      upsample_chroma_2x_bilinear(&samples, &samples, &mut row),
      Some(())
    );
    assert_eq!(row, [4, 5, 6, 6, 5, 4]);
  }

  #[test]
  fn centred_weights_preserve_edges_and_odd_widths() {
    let near = [16, 48, 80];
    let far = [0, 64, 112];
    let mut row = [0; 6];
    assert_eq!(upsample_chroma_2x_bilinear(&near, &far, &mut row), Some(()));
    assert_eq!(row, [12, 22, 42, 61, 79, 88]);
    let mut odd = [0; 5];
    assert_eq!(upsample_chroma_2x_bilinear(&near, &far, &mut odd), Some(()));
    assert_eq!(odd, [12, 22, 42, 61, 79]);
    let mut cropped = [0; 4];
    assert_eq!(
      upsample_chroma_2x_bilinear(&near, &far, &mut cropped),
      Some(())
    );
    assert_eq!(cropped, [12, 22, 42, 61]);

    assert_eq!(chroma_2x_rows(0, 3), (0, 0));
    assert_eq!(chroma_2x_rows(2, 3), (1, 0));
    assert_eq!(chroma_2x_rows(3, 3), (1, 2));
    assert_eq!(chroma_2x_rows(5, 3), (2, 2));
  }
}
