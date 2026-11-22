#![forbid(unsafe_code)]
use crate::src::align::Align16;
use crate::src::align::Align32;
use crate::src::align::Align64;
use crate::src::align::aligned_inner;
use crate::src::const_fn::const_for;
use crate::src::const_fn::const_min;
use crate::src::levels::BlockSize;
use crate::src::levels::InterIntraPredMode;
use aligned::Aligned;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use strum::EnumCount;

#[derive(Clone, Copy)]
enum WedgeDirectionType {
  Horizontal,
  Vertical,
  Oblique27,
  Oblique63,
  Oblique117,
  Oblique153,
}

struct WedgeCodeType {
  direction: WedgeDirectionType,
  position: Point<u8>,
}

struct WedgeCodeBook {
  tall: [WedgeCodeType; 16],
  wide: [WedgeCodeType; 16],
  square: [WedgeCodeType; 16],
}

const WEDGE_CODEBOOK: WedgeCodeBook = {
  use WedgeDirectionType::*;
  WedgeCodeBook {
    tall: [
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Horizontal, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Horizontal, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Horizontal, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Vertical, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 6, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 6, y: 4 } },
    ],
    wide: [
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Vertical, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Vertical, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Vertical, position: Point { x: 6, y: 4 } },
      WedgeCodeType { direction: Horizontal, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 6, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 6, y: 4 } },
    ],
    square: [
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 4 } },
      WedgeCodeType { direction: Horizontal, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Horizontal, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Vertical, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Vertical, position: Point { x: 6, y: 4 } },
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Oblique27, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 2 } },
      WedgeCodeType { direction: Oblique153, position: Point { x: 4, y: 6 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Oblique63, position: Point { x: 6, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 2, y: 4 } },
      WedgeCodeType { direction: Oblique117, position: Point { x: 6, y: 4 } },
    ],
  }
};

impl WedgeCodeBook {
  const fn get(&self, size: Size<u32>) -> Result<&[WedgeCodeType; 16], Never> {
    Ok(match size.height.checked_sub(size.width) {
      None => &self.wide,
      Some(0) => &self.square,
      Some(_) => &self.tall,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "constant evaluation needs a native array index and every generated mask slot is below 4096"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "this is the native indexing boundary for the constant-size Rust mask arrays"
  )
)]
const fn native_slot(value: u32) -> Result<usize, Never> {
  Ok(value as usize)
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the array initializer consumes the native index at the same const-evaluated access site"
  )
)]
macro_rules! slot {
  ($position:expr) => {{
    let Ok(index) = native_slot($position);
    index
  }};
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "the wedge table is indexed by the explicit u8 block-size discriminants from the coded bitstream"
  )
)]
const fn block_slot(block: BlockSize) -> Result<u32, Never> {
  Ok(::console_core_number_conversion::const_wrapped_integer!(unsigned u8 => u32, block as u8))
}

const fn border_row(src: &[u8; 8], center: u32) -> Result<[u8; 64], Never> {
  let mut row = [0; 64];
  let dst = center.saturating_sub(4);
  let source = 4_u32.saturating_sub(center);
  let count = const_min!(64_u32.wrapping_sub(center), 8);
  const_for!(offset in 0_u32..count => {
    row[slot!(dst.wrapping_add(offset))] = src[slot!(source.wrapping_add(offset))];
  });

  let outside = center.wrapping_add(4);
  const_for!(x in outside..64 => {
    row[slot!(x)] = 64;
  });

  Ok(row)
}

const fn transposed<const N: usize, const M: usize>(
  src: &[[u8; N]; M],
) -> core::result::Result<[[u8; M]; N], console_core_never::Never> {
  Ok({
    let mut dst = [[0; M]; N];

    const_for!(y in 0..M => {
        const_for!(x in 0..N => {
            dst[x][y] = src[y][x];
        });
    });

    dst
  })
}

const fn hflip(
  src: &[[u8; 64]; 64],
) -> core::result::Result<[[u8; 64]; 64], console_core_never::Never> {
  Ok({
    let mut dst = [[0; 64]; 64];
    const_for!(y in 0..dst.len() => {
        const_for!(x in 0..dst[y].len() => {
            dst[y][dst[y].len() - 1 - x] = src[y][x];
        });
    });
    dst
  })
}

const fn invert<const N: usize>(src: &[u8; N]) -> Result<[u8; N], Never> {
  let mut dst = [0; N];

  const_for!(offset in 0..N => {
    dst[offset] = 64_u8.wrapping_sub(src[offset]);
  });

  Ok(dst)
}

const fn copy2d<const N: usize>(
  src: &[[u8; 64]; 64],
  bounds: Rectangle<u32>,
) -> Result<[u8; N], Never> {
  let mut dst = [0; N];

  const_for!(y in 0_u32..bounds.size.height => {
    let dst_row = y.wrapping_mul(bounds.size.width);
    let source_row = &src[slot!(bounds.origin.y.wrapping_add(y))];
    const_for!(x in 0_u32..bounds.size.width => {
      dst[slot!(dst_row.wrapping_add(x))] = source_row[slot!(bounds.origin.x.wrapping_add(x))];
    });
  });

  Ok(dst)
}

#[derive(Clone, Copy)]
enum MaskPolarity {
  Original,
  Complement,
}

impl MaskPolarity {
  const fn for_mask(signs: u16, code: u32) -> Result<Self, Never> {
    Ok(match signs.wrapping_shr(code) & 1 {
      0 => Self::Original,
      _ => Self::Complement,
    })
  }

  const fn slots(self) -> Result<(u32, u32), Never> {
    Ok(match self {
      Self::Original => (0, 1),
      Self::Complement => (1, 0),
    })
  }
}

#[derive(Clone, Copy)]
enum ChromaRows {
  FullResolution,
  Subsampled,
}

const fn init_chroma<const LEN_LUMA: usize, const LEN_CHROMA: usize>(
  luma: &[u8; LEN_LUMA],
  rounding_polarity: MaskPolarity,
  size: Size<u32>,
  rows: ChromaRows,
) -> Result<[u8; LEN_CHROMA], Never> {
  let rounding_offset = match rounding_polarity {
    MaskPolarity::Original => 0,
    MaskPolarity::Complement => 1,
  };
  let row_shift = match rows {
    ChromaRows::FullResolution => 0,
    ChromaRows::Subsampled => 1,
  };
  let step = 1_u32.wrapping_add(row_shift);
  let mut chroma = [0; LEN_CHROMA];
  let mut luma_row = 0_u32;
  let mut chroma_row = 0_u32;

  const_for!(_y in 0_u32..size.height, step_by step => {
    const_for!(x in 0_u32..size.width, step_by 2 => {
      let luma_start = luma_row.wrapping_add(x);
      let a = u16::from_le_bytes([luma[slot!(luma_start)], 0]);
      let b = u16::from_le_bytes([luma[slot!(luma_start.wrapping_add(1))], 0]);
      let mut sum = a.wrapping_add(b).wrapping_add(1);

      match rows {
        ChromaRows::Subsampled => {
          let next_row = luma_start.wrapping_add(size.width);
          let c = u16::from_le_bytes([luma[slot!(next_row)], 0]);
          let d = u16::from_le_bytes([luma[slot!(next_row.wrapping_add(1))], 0]);
          sum = sum.wrapping_add(c).wrapping_add(d).wrapping_add(1);
        }
        ChromaRows::FullResolution => {}
      }

      let average = sum.wrapping_sub(rounding_offset).wrapping_shr(step);
      chroma[slot!(chroma_row.wrapping_add(x.wrapping_shr(1)))] =
        { let [weight, _] = average.to_le_bytes(); weight };
    });
    luma_row = luma_row.wrapping_add(size.width.wrapping_shl(row_shift));
    chroma_row = chroma_row.wrapping_add(size.width.wrapping_shr(1));
  });

  Ok(chroma)
}

struct WedgeMasks<const LEN_444: usize, const LEN_422: usize, const LEN_420: usize> {
  masks_444: Align64<[[[u8; LEN_444]; 16]; 2]>,
  masks_422: Align64<[[[u8; LEN_422]; 16]; 2]>,
  masks_420: Align64<[[[u8; LEN_420]; 16]; 2]>,
  signs: u16,
}

impl<const LEN_444: usize, const LEN_422: usize, const LEN_420: usize>
  WedgeMasks<LEN_444, LEN_422, LEN_420>
{
  const fn fill2d_16x2(
    size: Size<u32>,
    master: &WedgeMaster,
    cb: &[WedgeCodeType; 16],
    signs: u16,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let pixels = size.width.wrapping_mul(size.height);
      assert!(LEN_444 == slot!(pixels));
      assert!(LEN_422 == slot!(pixels.wrapping_shr(1)));
      assert!(LEN_420 == slot!(pixels.wrapping_shr(2)));

      let mut masks_444 = [[[0; LEN_444]; 16]; 2];
      let mut masks_422 = [[[0; LEN_422]; 16]; 2];
      let mut masks_420 = [[[0; LEN_420]; 16]; 2];

      const_for!(code in 0_u32..16 => {
        let entry = &cb[slot!(code)];
        let Ok(source) = master.direction(entry.direction);
        let x = u32::from_le_bytes([entry.position.x, 0, 0, 0]);
        let y = u32::from_le_bytes([entry.position.y, 0, 0, 0]);
        let bounds = Rectangle {
          origin: Point {
            x: 32_u32.wrapping_sub(size.width.wrapping_mul(x).wrapping_shr(3)),
            y: 32_u32.wrapping_sub(size.height.wrapping_mul(y).wrapping_shr(3)),
          },
          size,
        };
        let Ok(original) = copy2d(source, bounds);
        let Ok(inverted) = invert(&original);
        masks_444[0][slot!(code)] = original;
        masks_444[1][slot!(code)] = inverted;
      });

      const_for!(code in 0_u32..16 => {
        let Ok(polarity) = MaskPolarity::for_mask(signs, code);
        let Ok((same, opposite)) = polarity.slots();
        let luma = &masks_444[slot!(same)][slot!(code)];
        let Ok(original_422) = init_chroma(luma, MaskPolarity::Original, size, ChromaRows::FullResolution);
        let Ok(complement_422) = init_chroma(luma, MaskPolarity::Complement, size, ChromaRows::FullResolution);
        masks_422[slot!(same)][slot!(code)] = original_422;
        masks_422[slot!(opposite)][slot!(code)] = complement_422;

        let Ok(original_420) = init_chroma(luma, MaskPolarity::Original, size, ChromaRows::Subsampled);
        let Ok(complement_420) = init_chroma(luma, MaskPolarity::Complement, size, ChromaRows::Subsampled);
        masks_420[slot!(same)][slot!(code)] = original_420;
        masks_420[slot!(opposite)][slot!(code)] = complement_420;
      });

      Self {
        masks_444: Aligned(masks_444),
        masks_422: Aligned(masks_422),
        masks_420: Aligned(masks_420),
        signs,
      }
    })
  }

  const fn slice(
    &self,
  ) -> core::result::Result<[[[&[u8]; 16]; 2]; 3], console_core_never::Never> {
    Ok({
      let luma = aligned_inner(&self.masks_444);
      let chroma_422 = aligned_inner(&self.masks_422);
      let chroma_420 = aligned_inner(&self.masks_420);
      let mut masks: [[[&[u8]; 16]; 2]; 3] = [[[&[]; 16]; 2]; 3];

      const_for!(code in 0_u32..16 => {
        let Ok(polarity) = MaskPolarity::for_mask(self.signs, code);
        let Ok((same, opposite)) = polarity.slots();
        masks[0][0][slot!(code)] = &luma[slot!(same)][slot!(code)];
        masks[0][1][slot!(code)] = &luma[slot!(same)][slot!(code)];
        masks[1][0][slot!(code)] = &chroma_422[slot!(same)][slot!(code)];
        masks[1][1][slot!(code)] = &chroma_422[slot!(opposite)][slot!(code)];
        masks[2][0][slot!(code)] = &chroma_420[slot!(same)][slot!(code)];
        masks[2][1][slot!(code)] = &chroma_420[slot!(opposite)][slot!(code)];
      });

      masks
    })
  }
}

struct WedgeMaster {
  horizontal: [[u8; 64]; 64],
  vertical: [[u8; 64]; 64],
  oblique_27: [[u8; 64]; 64],
  oblique_63: [[u8; 64]; 64],
  oblique_117: [[u8; 64]; 64],
  oblique_153: [[u8; 64]; 64],
}

impl WedgeMaster {
  const fn direction(&self, direction: WedgeDirectionType) -> Result<&[[u8; 64]; 64], Never> {
    Ok(match direction {
      WedgeDirectionType::Horizontal => &self.horizontal,
      WedgeDirectionType::Vertical => &self.vertical,
      WedgeDirectionType::Oblique27 => &self.oblique_27,
      WedgeDirectionType::Oblique63 => &self.oblique_63,
      WedgeDirectionType::Oblique117 => &self.oblique_117,
      WedgeDirectionType::Oblique153 => &self.oblique_153,
    })
  }

  const fn build() -> Result<Self, Never> {
    const VERTICAL: [u8; 8] = [0, 2, 7, 21, 43, 57, 62, 64];
    const ODD: [u8; 8] = [1, 2, 6, 18, 37, 53, 60, 63];
    const EVEN: [u8; 8] = [1, 4, 11, 27, 46, 58, 62, 63];
    let Ok(vertical_row) = border_row(&VERTICAL, 32);
    let vertical = [vertical_row; 64];
    let mut oblique_63 = [[0; 64]; 64];

    const_for!(row in 0_u32..64, step_by 2 => {
      let center = 48_u32.wrapping_sub(row.wrapping_shr(1));
      let Ok(even) = border_row(&EVEN, center);
      let Ok(odd) = border_row(&ODD, center.wrapping_sub(1));
      oblique_63[slot!(row)] = even;
      oblique_63[slot!(row.wrapping_add(1))] = odd;
    });

    let Ok(horizontal) = transposed(&vertical);
    let Ok(oblique_27) = transposed(&oblique_63);
    let Ok(oblique_117) = hflip(&oblique_63);
    let Ok(oblique_153) = hflip(&oblique_27);

    Ok(Self { horizontal, vertical, oblique_27, oblique_63, oblique_117, oblique_153 })
  }
}

pub static dav1d_wedge_masks: [[[[&[u8]; 16]; 2]; 3]; BlockSize::COUNT] = {
  use BlockSize::*;

  static MASTER: WedgeMaster = {
    let Ok(master) = WedgeMaster::build();
    master
  };

  let mut masks: [[[[&[u8]; 16]; 2]; 3]; BlockSize::COUNT] = [[[[&[]; 16]; 2]; 3]; BlockSize::COUNT];

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "each literal block size determines both its three const array lengths and the fixed mask lookup entry"
    )
  )]
  macro_rules! fill {
    ($block:ident, $w:literal x $h:literal, $signs:expr) => {{
      static MASKS: WedgeMasks<{ $w * $h }, { ($w / 2) * $h }, { ($w / 2) * ($h / 2) }> = {
        let size = Size { width: $w, height: $h };
        let Ok(codebook) = WEDGE_CODEBOOK.get(size);
        let Ok(block_masks) = WedgeMasks::fill2d_16x2(size, &MASTER, codebook, $signs);
        block_masks
      };
      let Ok(block_masks) = MASKS.slice();
      let Ok(block_index) = block_slot($block);
      masks[slot!(block_index)] = block_masks;
    }};
  }

  fill!(Bs32x32, 32 x 32, 0x7bfb);
  fill!(Bs32x16, 32 x 16, 0x7beb);
  fill!(Bs32x8, 32 x 8, 0x6beb);
  fill!(Bs16x32, 16 x 32, 0x7beb);
  fill!(Bs16x16, 16 x 16, 0x7bfb);
  fill!(Bs16x8, 16 x 8, 0x7beb);
  fill!(Bs8x32, 8 x 32, 0x7aeb);
  fill!(Bs8x16, 8 x 16, 0x7beb);
  fill!(Bs8x8, 8 x 8, 0x7bfb);

  masks
};

static ii_dc_mask: Align64<[u8; 32 * 32]> = Aligned([32; 32 * 32]);

const fn build_nondc_ii_masks<const N: usize>(
  size: Size<u32>,
  step: u32,
) -> Result<[[u8; N]; 3], Never> {
  const WEIGHTS: [u8; 32] = [
    60, 52, 45, 39, 34, 30, 26, 22, 19, 17, 15, 13, 11, 10, 8, 7, 6, 6, 5, 4, 4, 3, 3, 2, 2, 2,
    2, 1, 1, 1, 1, 1,
  ];
  let mut masks = [[0; N]; 3];
  let [vertical, horizontal, smooth] = &mut masks;

  const_for!(y in 0_u32..size.height => {
    let row = y.wrapping_mul(size.width);
    const_for!(x in 0_u32..size.width => {
      let index = row.wrapping_add(x);
      vertical[slot!(index)] = WEIGHTS[slot!(y.wrapping_mul(step))];
      horizontal[slot!(index)] = WEIGHTS[slot!(x.wrapping_mul(step))];
      smooth[slot!(index)] = WEIGHTS[slot!(const_min!(x, y).wrapping_mul(step))];
    });
  });

  Ok(masks)
}

static ii_nondc_mask_32x32: Align64<[[u8; 32 * 32]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 32, height: 32 }, 1);
  explicit_value
});
static ii_nondc_mask_16x32: Align64<[[u8; 16 * 32]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 16, height: 32 }, 1);
  explicit_value
});
static ii_nondc_mask_16x16: Align64<[[u8; 16 * 16]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 16, height: 16 }, 2);
  explicit_value
});
static ii_nondc_mask_8x32: Align64<[[u8; 8 * 32]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 8, height: 32 }, 1);
  explicit_value
});
static ii_nondc_mask_8x16: Align64<[[u8; 8 * 16]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 8, height: 16 }, 2);
  explicit_value
});
static ii_nondc_mask_8x8: Align64<[[u8; 8 * 8]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 8, height: 8 }, 4);
  explicit_value
});
static ii_nondc_mask_4x16: Align64<[[u8; 4 * 16]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 4, height: 16 }, 2);
  explicit_value
});
static ii_nondc_mask_4x8: Align32<[[u8; 4 * 8]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 4, height: 8 }, 4);
  explicit_value
});
static ii_nondc_mask_4x4: Align16<[[u8; 4 * 4]; 3]> = Aligned({
  let Ok(explicit_value) = build_nondc_ii_masks(Size { width: 4, height: 4 }, 8);
  explicit_value
});

pub static dav1d_ii_masks: [[[&[u8]; InterIntraPredMode::COUNT]; 3]; BlockSize::COUNT] = {
  use BlockSize::*;

  let mut masks: [[[&[u8]; InterIntraPredMode::COUNT]; 3]; BlockSize::COUNT] =
    [[[&[]; InterIntraPredMode::COUNT]; 3]; BlockSize::COUNT];

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "each plane borrows a static weight table of its own dimensions and alignment"
    )
  )]
  macro_rules! set {
    ($($non_dc:ident),+) => {
      [$({
        let dc: &[u8] = aligned_inner(&ii_dc_mask);
        let [vertical, horizontal, smooth] = aligned_inner(&$non_dc);
        [dc, vertical, horizontal, smooth]
      }),+]
    };
  }

  masks[slot!(match block_slot(Bs8x8) { Ok(slot) => slot })] = set!(ii_nondc_mask_8x8, ii_nondc_mask_4x8, ii_nondc_mask_4x4);
  masks[slot!(match block_slot(Bs8x16) { Ok(slot) => slot })] = set!(ii_nondc_mask_8x16, ii_nondc_mask_4x16, ii_nondc_mask_4x8);
  masks[slot!(match block_slot(Bs16x8) { Ok(slot) => slot })] = set!(ii_nondc_mask_16x16, ii_nondc_mask_8x8, ii_nondc_mask_8x8);
  masks[slot!(match block_slot(Bs16x16) { Ok(slot) => slot })] = set!(ii_nondc_mask_16x16, ii_nondc_mask_8x16, ii_nondc_mask_8x8);
  masks[slot!(match block_slot(Bs16x32) { Ok(slot) => slot })] = set!(ii_nondc_mask_16x32, ii_nondc_mask_8x32, ii_nondc_mask_8x16);
  masks[slot!(match block_slot(Bs32x16) { Ok(slot) => slot })] = set!(
    ii_nondc_mask_32x32,
    ii_nondc_mask_16x16,
    ii_nondc_mask_16x16
  );
  masks[slot!(match block_slot(Bs32x32) { Ok(slot) => slot })] = set!(
    ii_nondc_mask_32x32,
    ii_nondc_mask_16x32,
    ii_nondc_mask_16x16
  );

  masks
};
