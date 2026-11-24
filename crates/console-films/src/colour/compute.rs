//! The shared compute kernel converts decoded planes to upright RGBA and can
//! export a display-sized frame for Wayland. The Vulkan preference falls
//! back to ordinary CPU conversion if dispatch or plane packing fails.
//! Scaling uses the ordinary playback path's chroma filtering and rounding.
//! Packing checks the products the shader uses for 32-bit chroma coordinates.
//! The color matrix uses bounded fixed-point coefficients for 8-bit samples.

use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;

use console_concurrency::Cores;
use console_core_never::Never;
use console_operating_system_linux::compute_map;
use console_operating_system_linux::program_runtime::compute::{
  self, Map, MapProgram, Target, Vulkan,
};

use super::{
  Bitmap, Colour, Layout, Matrix, Picture, Planes, Sample, Size, Strides, Turn, rows_of, weights,
};

pub struct DmaBufFrame {
  pub fd: OwnedFd,
  pub size: Size<u32>,
  pub stride: u32,
}

impl Picture {
  pub fn export_pixels_at(
    &self,
    shown: Size<u32>,
    cores: Cores,
    device: &Vulkan,
    shader: &Path,
  ) -> io::Result<DmaBufFrame> {
    let input = self.scaled_input(shown, cores)?;
    let pitch = shown
      .width
      .checked_next_multiple_of(64)
      .ok_or(InvalidPlane)?;
    let stride = pitch.checked_mul(4).ok_or(InvalidPlane)?;
    let fd = compute::map_export(
      device,
      Map {
        program: MapProgram {
          kernel: &RGBA,
          spirv: Some(shader),
        },
        input: &input,
        dimensions: [pitch, shown.height, 1],
        parameters: [0; 2],
      },
    )?;
    Ok(DmaBufFrame {
      fd,
      size: shown,
      stride,
    })
  }

  fn scaled_input(&self, shown: Size<u32>, cores: Cores) -> io::Result<Vec<u32>> {
    let Ok(made) = self.unturned_size(shown);

    match (made.width, made.height) {
      (0, _) | (_, 0) => return Err(io::Error::from(InvalidPlane)),
      _ => {}
    }

    let Ok(scaled) = self.scaled_planes(made, cores);
    let layout = Layout {
      stored: made,
      chroma: made,
      depth: 8,
      strides: Strides {
        luma: made.width,
        chroma: made.width,
      },
    };
    packed(
      &scaled,
      self.colour,
      &layout,
      Presentation {
        original_height: self.stored.height,
        turn: self.turn,
        shown_width: shown.width,
      },
    )
  }

  pub fn color_pixels_with_compute(
    &self,
    target: Target<'_>,
    shader: Option<&Path>,
  ) -> io::Result<Bitmap> {
    match target {
      Target::PreferVulkan(device) => compute::run(
        Some(device),
        |device_2| self.mapped(Target::RequireVulkan(device_2), shader),
        || {
          let Ok(bitmap) = self.color_pixels();
          Ok(bitmap)
        },
      ),
      Target::Cpu | Target::SerialCpu | Target::RequireVulkan(_) => self.mapped(target, shader),
    }
  }

  fn mapped(&self, target: Target<'_>, shader: Option<&Path>) -> io::Result<Bitmap> {
    let Ok(layout) = self.colour_layout();
    let rgba = match &self.planes {
      Planes::Eight(planes) => calculate(planes, self.colour, &layout, target, shader),
      Planes::Deep(planes, _) => calculate(planes, self.colour, &layout, target, shader),
    }?;
    let Ok(bitmap) = self.coloured_bitmap(rgba);
    Ok(bitmap)
  }
}

pub fn colour_glsl() -> io::Result<String> {
  RGBA.glsl()
}

struct InvalidPlane;

impl From<InvalidPlane> for io::Error {
  fn from(_: InvalidPlane) -> Self {
    Self::new(
      io::ErrorKind::InvalidInput,
      "invalid video compute plane layout",
    )
  }
}

fn append_plane<T: Sample>(
  input: &mut Vec<u32>,
  plane: &[T],
  size: Size<u32>,
  storage: PlaneStorage,
) -> io::Result<u32> {
  let offset = u32::try_from(input.len()).map_err(|_layout| InvalidPlane)?;
  let Ok(candidate) = rows_of(size, storage.stride);
  let layout = candidate.ok_or(InvalidPlane)?;
  let rows = layout.rows(plane).map_err(|_layout| InvalidPlane)?;
  let mut word = 0;
  let mut shift = 0;

  for sample in rows.flatten() {
    word |= u32::from(sample.eight(storage.depth)).wrapping_shl(shift);
    shift = shift.saturating_add(8);

    match shift {
      32 => {
        input.push(word);
        word = 0;
        shift = 0;
      }
      _ => {}
    }
  }

  match shift {
    0 => {}
    _ => {
      input.push(word);
    }
  }

  Ok(offset)
}

fn calculate<T: Sample>(
  planes: &[Vec<T>; 3],
  colour: Colour,
  layout: &Layout,
  target: Target<'_>,
  shader: Option<&Path>,
) -> io::Result<Vec<u8>> {
  let stored = layout.stored;
  let input = packed(
    planes,
    colour,
    layout,
    Presentation {
      original_height: stored.height,
      turn: Turn::Upright,
      shown_width: stored.width,
    },
  )?;
  let output = compute::map(
    target,
    Map {
      program: MapProgram {
        kernel: &RGBA,
        spirv: shader,
      },
      input: &input,
      dimensions: [stored.width, stored.height, 1],
      parameters: [0; 2],
    },
  )?;
  Ok(output.into_iter().flat_map(u32::to_le_bytes).collect())
}

struct Presentation {
  original_height: u32,
  turn: Turn,
  shown_width: u32,
}

#[derive(Clone, Copy)]
struct PlaneStorage {
  stride: u32,
  depth: u32,
}

fn packed<T: Sample>(
  [luma, blue, red]: &[Vec<T>; 3],
  colour: Colour,
  layout: &Layout,
  presentation: Presentation,
) -> io::Result<Vec<u32>> {
  let Layout {
    stored,
    chroma,
    strides,
    depth,
  } = *layout;
  stored
    .width
    .checked_mul(stored.height)
    .ok_or(InvalidPlane)?;
  chroma
    .width
    .checked_mul(chroma.height)
    .ok_or(InvalidPlane)?;
  stored.width.checked_mul(chroma.width).ok_or(InvalidPlane)?;
  stored
    .height
    .checked_mul(chroma.height)
    .ok_or(InvalidPlane)?;

  let Ok(weights) = weights(colour, presentation.original_height);
  let orientation = match presentation.turn {
    Turn::Upright => 0,
    Turn::Quarter => 1,
    Turn::Half => 2,
    Turn::ThreeQuarters => 3,
  };
  let mut input = vec![
    chroma.width,
    chroma.height,
    u32::from(colour.matrix == Matrix::Rgb),
    weights.luma.cast_unsigned(),
    weights.red_from_red.cast_unsigned(),
    weights.green_from_blue.cast_unsigned(),
    weights.green_from_red.cast_unsigned(),
    weights.blue_from_blue.cast_unsigned(),
    weights.black.cast_unsigned(),
    0,
    0,
    0,
    stored.width,
    stored.height,
    orientation,
    presentation.shown_width,
  ];
  let luma_storage = PlaneStorage {
    stride: strides.luma,
    depth,
  };
  let chroma_storage = PlaneStorage {
    stride: strides.chroma,
    depth,
  };
  let luma_offset = append_plane(&mut input, luma, stored, luma_storage)?;
  let blue_offset = append_plane(&mut input, blue, chroma, chroma_storage)?;
  let red_offset = append_plane(&mut input, red, chroma, chroma_storage)?;

  match input.get_mut(9..12) {
    Some([luma_slot, blue_slot, red_slot]) => {
      *luma_slot = luma_offset;
      *blue_slot = blue_offset;
      *red_slot = red_offset;
    }
    Some(_) | None => return Err(io::Error::from(InvalidPlane)),
  }

  Ok(input)
}

compute_map! {
  static RGBA = [8, 8, 1] |item| {
    let Ok(across): Result<u32, Never> = item.id(0);
    let Ok(down): Result<u32, Never> = item.id(1);
    let Ok(displayed_width): Result<u32, Never> = item.word(15);

    match (across >= displayed_width).then_some(()) {
      Some(()) => { return 0xff00_0000u32; }
      None => {}
    }

    let Ok(width): Result<u32, Never> = item.word(12);
    let Ok(height): Result<u32, Never> = item.word(13);
    let Ok(turn): Result<u32, Never> = item.word(14);
    let mut x: u32 = across;
    let mut y: u32 = down;

    match (turn == 1).then_some(()) {
      Some(()) => {
        x = down;
        y = u32::wrapping_sub(u32::wrapping_sub(height, 1), across);
      }
      None => {}
    }

    match (turn == 2).then_some(()) {
      Some(()) => {
        x = u32::wrapping_sub(u32::wrapping_sub(width, 1), across);
        y = u32::wrapping_sub(u32::wrapping_sub(height, 1), down);
      }
      None => {}
    }

    match (turn == 3).then_some(()) {
      Some(()) => {
        x = u32::wrapping_sub(u32::wrapping_sub(width, 1), down);
        y = across;
      }
      None => {}
    }

    let index: u32 = u32::wrapping_add(u32::wrapping_mul(y, width), x);
    let Ok(blue_width): Result<u32, Never> = item.word(0);
    let Ok(blue_height): Result<u32, Never> = item.word(1);
    let chroma_row: u32 = u32::wrapping_div(u32::wrapping_mul(y, blue_height), u32::max(height, 1));
    let chroma_column: u32 = u32::wrapping_div(u32::wrapping_mul(x, blue_width), u32::max(width, 1));
    let chroma: u32 = u32::wrapping_add(u32::wrapping_mul(chroma_row, blue_width), chroma_column);
    let luma_shift: u32 = u32::wrapping_mul(u32::wrapping_rem(index, 4), 8);
    let chroma_shift: u32 = u32::wrapping_mul(u32::wrapping_rem(chroma, 4), 8);
    let luma_word: u32 = u32::wrapping_div(index, 4);
    let chroma_word: u32 = u32::wrapping_div(chroma, 4);
    let Ok(luma_base): Result<u32, Never> = item.word(9);
    let Ok(luma_samples): Result<u32, Never> = item.word(u32::wrapping_add(luma_base, luma_word));
    let luma: u32 = u32::wrapping_shr(luma_samples, luma_shift) & 255;
    let Ok(blue_base): Result<u32, Never> = item.word(10);
    let Ok(blue_samples): Result<u32, Never> = item.word(u32::wrapping_add(blue_base, chroma_word));
    let blue: u32 = u32::wrapping_shr(blue_samples, chroma_shift) & 255;
    let Ok(red_base): Result<u32, Never> = item.word(11);
    let Ok(red_samples): Result<u32, Never> = item.word(u32::wrapping_add(red_base, chroma_word));
    let red: u32 = u32::wrapping_shr(red_samples, chroma_shift) & 255;
    let Ok(packed_rgb): Result<u32, Never> = item.word(2);

    match (packed_rgb != 0).then_some(()) {
      Some(()) => {
        return red | u32::wrapping_shl(luma, 8) | u32::wrapping_shl(blue, 16) | 0xff00_0000u32;
      }
      None => {}
    }

    let Ok(luma_offset): Result<u32, Never> = item.word(8);
    let luminous: i32 = i32::wrapping_sub(u32::cast_signed(luma), u32::cast_signed(luma_offset));
    let Ok(luma_scale): Result<u32, Never> = item.word(3);
    let bright: i32 = i32::wrapping_mul(luminous, u32::cast_signed(luma_scale));
    let cb: i32 = i32::wrapping_sub(u32::cast_signed(blue), 128);
    let cr: i32 = i32::wrapping_sub(u32::cast_signed(red), 128);
    let Ok(red_scale): Result<u32, Never> = item.word(4);
    let red_from_cr: i32 = i32::wrapping_mul(cr, u32::cast_signed(red_scale));
    let Ok(green_cb_scale): Result<u32, Never> = item.word(5);
    let green_from_cb: i32 = i32::wrapping_mul(cb, u32::cast_signed(green_cb_scale));
    let Ok(green_cr_scale): Result<u32, Never> = item.word(6);
    let green_from_cr: i32 = i32::wrapping_mul(cr, u32::cast_signed(green_cr_scale));
    let Ok(blue_scale): Result<u32, Never> = item.word(7);
    let blue_from_cb: i32 = i32::wrapping_mul(cb, u32::cast_signed(blue_scale));
    let r: i32 = i32::wrapping_add(bright, red_from_cr);
    let g: i32 = i32::wrapping_sub(i32::wrapping_sub(bright, green_from_cb), green_from_cr);
    let b: i32 = i32::wrapping_add(bright, blue_from_cb);
    let red_display: i32 = i32::clamp(i32::wrapping_shr(i32::wrapping_add(r, 8192), 14), 0, 255);
    let green_display: i32 = i32::clamp(i32::wrapping_shr(i32::wrapping_add(g, 8192), 14), 0, 255);
    let blue_display: i32 = i32::clamp(i32::wrapping_shr(i32::wrapping_add(b, 8192), 14), 0, 255);

    i32::cast_unsigned(red_display)
      | u32::wrapping_shl(i32::cast_unsigned(green_display), 8)
      | u32::wrapping_shl(i32::cast_unsigned(blue_display), 16)
      | 0xff00_0000u32
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Seconds;
  use crate::colour::{Range, Strides, Turn};

  #[test]
  fn padded_output_matches_playback_at_each_orientation() -> io::Result<()> {
    let shader = colour_glsl()?;
    assert!(shader.contains("int("));
    let mut picture = Picture {
      at: Seconds(0.0),
      stored: Size {
        width: 9,
        height: 7,
      },
      chroma: Size {
        width: 5,
        height: 4,
      },
      planes: Planes::Eight([
        (0u8..63).map(|value| value.wrapping_mul(11)).collect(),
        (0u8..20).map(|value| value.wrapping_mul(13)).collect(),
        (0u8..20).map(|value| value.wrapping_mul(17)).collect(),
      ]),
      strides: Strides { luma: 9, chroma: 5 },
      colour: Colour {
        matrix: Matrix::Bt709,
        range: Range::Studio,
      },
      turn: Turn::Upright,
    };
    let shown = Size {
      width: 7,
      height: 5,
    };
    let Ok(cores) = Cores::counted();

    for turn in [
      Turn::Upright,
      Turn::Quarter,
      Turn::Half,
      Turn::ThreeQuarters,
    ] {
      picture.turn = turn;
      let input = picture.scaled_input(shown, cores)?;
      let output = compute::map(
        Target::Cpu,
        Map {
          program: MapProgram {
            kernel: &RGBA,
            spirv: None,
          },
          input: &input,
          dimensions: [64, shown.height, 1],
          parameters: [0; 2],
        },
      )?;
      let actual: Vec<_> = output
        .chunks_exact(64)
        .flat_map(|row| row.iter().take(7))
        .flat_map(|word| word.to_le_bytes())
        .collect();
      let Ok(expected) = picture.color_pixels_at(shown, cores);
      assert_eq!(actual, expected.bytes);
    }

    Ok(())
  }

  #[test]
  fn mapped_color_preserves_chroma_layout_depth_and_rotation() -> io::Result<()> {
    let luma: Vec<_> = (0u8..27).map(|n| n.wrapping_mul(59)).collect();
    let blue: Vec<_> = (0u8..8).map(|n| n.wrapping_mul(37)).collect();
    let red: Vec<_> = (0u8..8).map(|n| n.wrapping_mul(79)).collect();
    let deep = [&luma, &blue, &red].map(|plane| {
      plane
        .iter()
        .map(|&sample| u16::from(sample).wrapping_shl(2))
        .collect()
    });

    for planes in [Planes::Eight([luma, blue, red]), Planes::Deep(deep, 10)] {
      for matrix in [
        Matrix::Bt601,
        Matrix::Bt709,
        Matrix::Bt2020,
        Matrix::Rgb,
        Matrix::Unsaid,
      ] {
        for range in [Range::Studio, Range::Full] {
          let picture = Picture {
            at: Seconds(0.0),
            stored: Size {
              width: 7,
              height: 3,
            },
            chroma: Size {
              width: 3,
              height: 2,
            },
            planes: planes.clone(),
            strides: Strides { luma: 9, chroma: 4 },
            colour: Colour { matrix, range },
            turn: Turn::Quarter,
          };
          let Ok(expected) = picture.color_pixels();
          let mapped = picture.color_pixels_with_compute(Target::Cpu, None)?;
          assert_eq!(mapped.size, expected.size);
          assert_eq!(mapped.bytes, expected.bytes, "{matrix:?} {range:?}");
        }
      }
    }

    Ok(())
  }
}
