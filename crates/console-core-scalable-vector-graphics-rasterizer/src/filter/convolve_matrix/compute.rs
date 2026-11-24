//! The compute kernel sums each convolution channel. It reads the same
//! normalized byte values as the scalar renderer, which also performs the
//! final division and rounding so both backends produce the same bytes.

use std::io;
use std::path::Path;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

use console_operating_system_linux::compute_map;
use console_operating_system_linux::program_runtime::compute::{self, Map, MapProgram, Target};

use super::{ConvolveMatrix, EdgeMode, ImageRefMut, Kernel, PreserveAlphaAnswer};

pub(crate) fn mapped(
  matrix: &ConvolveMatrix,
  mut source: ImageRefMut<'_>,
  target: Target<'_>,
  shader: Option<&Path>,
) -> io::Result<()> {
  match target {
    Target::PreferVulkan(device) => {
      match calculate(matrix, &mut source, Target::RequireVulkan(device), shader) {
        Ok(()) => Ok(()),
        Err(error) => {
          console_core_logging::debug!("using CPU convolution: {error}");
          super::apply(matrix, source).map_err(|_invalid| io::Error::from(ComputeFault::Filter))
        }
      }
    }
    Target::RequireVulkan(_) | Target::Cpu | Target::SerialCpu => {
      calculate(matrix, &mut source, target, shader)
    }
  }
}

enum ComputeFault {
  Filter,
  Output,
}

impl From<ComputeFault> for io::Error {
  fn from(fault: ComputeFault) -> Self {
    match fault {
      ComputeFault::Filter => {
        Self::new(io::ErrorKind::InvalidInput, "invalid convolution filter")
      }
      ComputeFault::Output => Self::new(
        io::ErrorKind::InvalidData,
        "incomplete convolution output planes",
      ),
    }
  }
}

fn calculate(
  matrix: &ConvolveMatrix,
  source: &mut ImageRefMut<'_>,
  target: Target<'_>,
  shader: Option<&Path>,
) -> io::Result<()> {
  let size = source
    .dimensions()
    .map_err(|_invalid| ComputeFault::Filter)?;

  match (size.width, size.height) {
    (0, _) | (_, 0) => return Ok(()),
    _ => {}
  }

  let _ = i32::try_from(size.width).map_err(io::Error::other)?;
  let _ = i32::try_from(size.height).map_err(io::Error::other)?;
  let kernel = Kernel::from_matrix(matrix).map_err(|_invalid| ComputeFault::Filter)?;
  let edge = match kernel.edge {
    EdgeMode::None => 0,
    EdgeMode::Duplicate => 1,
    EdgeMode::Wrap => 2,
  };
  let input: Vec<u32> = [
    kernel.dimensions.width,
    kernel.dimensions.height,
    kernel.anchor.x,
    kernel.anchor.y,
    edge,
  ]
  .into_iter()
  .chain((u8::MIN..=u8::MAX).map(|sample| (f32::from(sample) / 255.0).to_bits()))
  .chain(kernel.weights.iter().map(|weight| weight.to_bits()))
  .chain(source.data.iter().copied().map(u32::from_le_bytes))
  .collect();

  let channels = match kernel.alpha {
    PreserveAlphaAnswer::Yes => 3,
    PreserveAlphaAnswer::No => 4,
  };
  let sums = compute::map(
    target,
    Map {
      program: MapProgram {
        kernel: &CONVOLUTION,
        spirv: shader,
      },
      input: &input,
      dimensions: [size.width, size.height, channels],
      parameters: [0; 2],
    },
  )?;

  finish_image(&kernel, source, &sums)
}

fn finish_image(
  kernel: &Kernel<'_>,
  source: &mut ImageRefMut<'_>,
  sums: &[u32],
) -> io::Result<()> {
  let Ok(pixel_count) = fixed_index(source.data.len());

  match pixel_count {
    0 => return Err(io::Error::from(ComputeFault::Output)),
    _ => {}
  }

  let Ok(samples_per_plane) = index(pixel_count);
  let mut planes = sums.chunks_exact(samples_per_plane.max(1));
  let (red_plane, green_plane, blue_plane) = match (planes.next(), planes.next(), planes.next()) {
    (Some(red), Some(green), Some(blue)) => (red, green, blue),
    _ => return Err(io::Error::from(ComputeFault::Output)),
  };
  let alpha_plane: &[u32] = match (kernel.alpha, planes.next()) {
    (PreserveAlphaAnswer::Yes, None) => &[],
    (PreserveAlphaAnswer::No, Some(alpha)) => alpha,
    (PreserveAlphaAnswer::Yes, Some(_)) | (PreserveAlphaAnswer::No, None) => {
      return Err(io::Error::from(ComputeFault::Output));
    }
  };

  match (planes.next(), planes.remainder()) {
    (None, []) => {}
    _ => return Err(io::Error::from(ComputeFault::Output)),
  }

  let alpha_or_zero = alpha_plane
    .iter()
    .copied()
    .chain(std::iter::repeat_n(0, samples_per_plane));
  let channel_words = red_plane
    .iter()
    .zip(green_plane)
    .zip(blue_plane)
    .zip(alpha_or_zero);

  for (pixel, (((&red, &green), &blue), alpha)) in source.data.iter_mut().zip(channel_words) {
    let [_, _, _, center_alpha] = *pixel;
    let totals = [red, green, blue, alpha].map(f32::from_bits);
    let Ok(output) = kernel.finish(totals, i16::from(center_alpha));
    *pixel = output;
  }

  Ok(())
}

compute_map! {
  pub(crate) static CONVOLUTION = [8, 8, 1] |item| {
    let Ok(columns): Result<u32, Never> = item.word(0);
    let Ok(rows): Result<u32, Never> = item.word(1);
    let Ok(target_x): Result<u32, Never> = item.id(0);
    let Ok(origin_x): Result<u32, Never> = item.word(2);
    let base_x: i32 = i32::wrapping_sub(u32::cast_signed(target_x), u32::cast_signed(origin_x));
    let Ok(target_y): Result<u32, Never> = item.id(1);
    let Ok(origin_y): Result<u32, Never> = item.word(3);
    let base_y: i32 = i32::wrapping_sub(u32::cast_signed(target_y), u32::cast_signed(origin_y));
    let Ok(edge): Result<u32, Never> = item.word(4);
    let pixels: u32 = u32::wrapping_add(261, u32::wrapping_mul(columns, rows));
    let Ok(wide): Result<u32, Never> = item.extent(0);
    let Ok(tall): Result<u32, Never> = item.extent(1);
    let width: i32 = u32::cast_signed(wide);
    let height: i32 = u32::cast_signed(tall);
    let Ok(component): Result<u32, Never> = item.id(2);
    let shift: u32 = u32::wrapping_mul(component, 8);
    let mut sum: f32 = 0.0f32;

    for row in 0u32..rows {
      let mut y: i32 = i32::wrapping_add(base_y, u32::cast_signed(row));

      let row_outside: bool = y < 0 || y >= height;

      match (row_outside && edge == 0).then_some(()) {
        Some(()) => { continue; }
        None => {}
      }

      match (row_outside).then_some(()) {
        Some(()) => {
          match (edge == 1).then_some(()) {
            Some(()) => { y = i32::clamp(y, 0, i32::wrapping_sub(height, 1)); }
            None => { y = i32::rem_euclid(y, height); }
          }
        }
        None => {}
      }

      for column in 0u32..columns {
        let mut x: i32 = i32::wrapping_add(base_x, u32::cast_signed(column));
        let weight: u32 = u32::wrapping_add(u32::wrapping_mul(row, columns), column);
        let Ok(coefficient_word): Result<u32, Never> = item.word(u32::wrapping_sub(u32::wrapping_sub(pixels, 1), weight));
        let coefficient: f32 = f32::from_bits(coefficient_word);

        let column_outside: bool = x < 0 || x >= width;

        match (!column_outside || edge != 0).then_some(()) {
          Some(()) => {
            match (column_outside).then_some(()) {
              Some(()) => {
                match (edge == 1).then_some(()) {
                  Some(()) => { x = i32::clamp(x, 0, i32::wrapping_sub(width, 1)); }
                  None => { x = i32::rem_euclid(x, width); }
                }
              }
              None => {}
            }

            let position: u32 = i32::cast_unsigned(i32::wrapping_add(i32::wrapping_mul(y, width), x));
            let Ok(pixel): Result<u32, Never> = item.word(u32::wrapping_add(pixels, position));
            let channel: u32 = u32::wrapping_shr(pixel, shift) & 255;
            let Ok(normalized): Result<u32, Never> = item.word(u32::wrapping_add(5, channel));
            sum += f32::from_bits(normalized) * coefficient;
          }
          None => {}
        }
      }
    }

    f32::to_bits(sum)
  }
}

#[cfg(test)]
mod tests {
  use console_core_path_rasterizer::{Pixmap, Transform};
  use console_core_scalable_vector_graphics::{Options, Tree};

  use super::*;

  #[test]
  fn mapped_convolution_matches_rendered_alpha_and_edge_modes()
  -> Result<(), Box<dyn std::error::Error>> {
    for edge in ["none", "duplicate", "wrap"] {
      for preserve in ["true", "false"] {
        let svg = format!(
          r##"<svg xmlns="http://www.w3.org/2000/svg" width="21" height="13">
          <filter id="conv" x="0" y="0" width="21" height="13" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
            <feConvolveMatrix order="3 2" targetX="0" targetY="1" kernelMatrix="2 -1 3 1 0 2"
              divisor="7" bias="-.13" edgeMode="{edge}" preserveAlpha="{preserve}"/>
          </filter>
          <g filter="url(#conv)"><rect width="21" height="13" fill="#9a6012" opacity=".47"/>
            <rect x="1" y="1" width="11" height="5" fill="#7217e3" opacity=".62"/></g>
        </svg>"##
        );
        let tree = Tree::from_str(&svg, &Options::default())?;
        let Ok(identity) = Transform::identity();
        let Ok(allocated) = Pixmap::new(21, 13);
        let mut original = allocated.ok_or("allocate SVG pixmap")?;
        let mut mapped = original.clone();
        let Ok(mut original_view) = original.as_mut();
        crate::render(&tree, identity, &mut original_view);
        let Ok(mut mapped_view) = mapped.as_mut();
        crate::render_with_compute(&tree, identity, &mut mapped_view, Target::Cpu, None)?;
        let (Ok(expected), Ok(output)) = (original.data(), mapped.data());
        assert!(expected.iter().any(|&sample| sample != 0));
        assert_eq!(output, expected, "{edge} preserveAlpha={preserve}");
      }
    }

    Ok(())
  }
}
