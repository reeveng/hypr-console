#![forbid(unsafe_code)]
use crate::include::common::intops::{apply_sign, apply_sign64, iclip};
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::src::levels::Mv;
use console_core_geometry::{Point, Rectangle};
use console_core_never::Never;
use console_core_number_conversion::{checked_index, wrapped};
use core::cmp::Ordering;

const ONE: i32 = 0x10000;

static DIV_LUT: [u16; 257] = [
  16384, 16320, 16257, 16194, 16132, 16070, 16009, 15948, 15888, 15828, 15768, 15709, 15650,
  15592, 15534, 15477, 15420, 15364, 15308, 15252, 15197, 15142, 15087, 15033, 14980, 14926,
  14873, 14821, 14769, 14717, 14665, 14614, 14564, 14513, 14463, 14413, 14364, 14315, 14266,
  14218, 14170, 14122, 14075, 14028, 13981, 13935, 13888, 13843, 13797, 13752, 13707, 13662,
  13618, 13574, 13530, 13487, 13443, 13400, 13358, 13315, 13273, 13231, 13190, 13148, 13107,
  13066, 13026, 12985, 12945, 12906, 12866, 12827, 12788, 12749, 12710, 12672, 12633, 12596,
  12558, 12520, 12483, 12446, 12409, 12373, 12336, 12300, 12264, 12228, 12193, 12157, 12122,
  12087, 12053, 12018, 11984, 11950, 11916, 11882, 11848, 11815, 11782, 11749, 11716, 11683,
  11651, 11619, 11586, 11555, 11523, 11491, 11460, 11429, 11398, 11367, 11336, 11305, 11275,
  11245, 11215, 11185, 11155, 11125, 11096, 11067, 11038, 11009, 10980, 10951, 10923, 10894,
  10866, 10838, 10810, 10782, 10755, 10727, 10700, 10673, 10645, 10618, 10592, 10565, 10538,
  10512, 10486, 10460, 10434, 10408, 10382, 10356, 10331, 10305, 10280, 10255, 10230, 10205,
  10180, 10156, 10131, 10107, 10082, 10058, 10034, 10010, 9986, 9963, 9939, 9916, 9892, 9869,
  9846, 9823, 9800, 9777, 9754, 9732, 9709, 9687, 9664, 9642, 9620, 9598, 9576, 9554, 9533, 9511,
  9489, 9468, 9447, 9425, 9404, 9383, 9362, 9341, 9321, 9300, 9279, 9259, 9239, 9218, 9198, 9178,
  9158, 9138, 9118, 9098, 9079, 9059, 9039, 9020, 9001, 8981, 8962, 8943, 8924, 8905, 8886, 8867,
  8849, 8830, 8812, 8793, 8775, 8756, 8738, 8720, 8702, 8684, 8666, 8648, 8630, 8613, 8595, 8577,
  8560, 8542, 8525, 8508, 8490, 8473, 8456, 8439, 8422, 8405, 8389, 8372, 8355, 8339, 8322, 8306,
  8289, 8273, 8257, 8240, 8224, 8208, 8192,
];

#[inline]
fn round_shear(value: i32) -> Result<i16, Never> {
  let clipped = iclip(value, i32::from(i16::MIN), i32::from(i16::MAX));
  let magnitude = clipped.abs().wrapping_add(32).wrapping_shr(6);
  wrapped(apply_sign(magnitude, clipped).wrapping_shl(6))
}

struct Divisor {
  shift: u32,
  factor: i32,
}

impl Divisor {
  fn normalized(value: u64) -> Result<Option<Self>, Never> {
    let shift = match value.checked_ilog2() {
      Some(nonzero_divisor) => nonzero_divisor,
      None => return Ok(None),
    };

    let excess = value.wrapping_sub(1_u64.wrapping_shl(shift));
    let mantissa = match shift {
      0..=8 => excess.wrapping_shl(8_u32.wrapping_sub(shift)),
      9.. => excess
        .wrapping_add(1_u64.wrapping_shl(shift.wrapping_sub(9)))
        .wrapping_shr(shift.wrapping_sub(8)),
    };
    let factor = match DIV_LUT.get(match checked_index(mantissa) {
      Ok(position) => position,
      Err(_outside_table) => return Ok(None),
    }) {
      Some(&table_factor) => table_factor,
      None => return Ok(None),
    };

    Ok(Some(Self {
      shift: shift.wrapping_add(14),
      factor: i32::from(factor),
    }))
  }

  fn scaled_for_affine(self, determinant: i64) -> Result<Self, Never> {
    let factor = apply_sign64(self.factor, determinant);

    Ok(match self.shift {
      0..=15 => Self {
        shift: 0,
        factor: factor.wrapping_shl(16_u32.wrapping_sub(self.shift)),
      },
      16.. => Self {
        shift: self.shift.wrapping_sub(16),
        factor,
      },
    })
  }

  fn affine_coefficient(&self, product: i64, term: AffineTerm) -> Result<i32, Never> {
    let scaled = product.wrapping_mul(i64::from(self.factor));
    let Ok(value) = round_product(scaled, self.shift, RoundingWidth::Affine);

    Ok(match term {
      AffineTerm::Diagonal => iclip(value, 0xe001, 0x11fff),
      AffineTerm::OffDiagonal => iclip(value, -0x1fff, 0x1fff),
    })
  }
}

enum AffineTerm {
  Diagonal,
  OffDiagonal,
}

enum RoundingWidth {
  Shear,
  Affine,
}

fn round_product(product: i64, shift: u32, width: RoundingWidth) -> Result<i32, Never> {
  let round = match width {
    RoundingWidth::Shear => i64::from(1_i32.wrapping_shl(shift).wrapping_shr(1)),
    RoundingWidth::Affine => 1_i64.wrapping_shl(shift).wrapping_shr(1),
  };
  let Ok(magnitude) = wrapped(
    product
      .wrapping_abs()
      .wrapping_add(round)
      .wrapping_shr(shift),
  );

  Ok(apply_sign64(magnitude, product))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WarpValidity {
  Usable,
  Rejected,
}

pub(crate) fn rav1d_get_shear_params(
  wm: &Rav1dWarpedMotionParams,
) -> Result<WarpValidity, Never> {
  let [_, _, xx, xy, yx, yy] = wm.matrix;

  match xx {
    i32::MIN..=0 => return Ok(WarpValidity::Rejected),
    1.. => {}
  }

  let divisor = match Divisor::normalized(u64::from(xx.unsigned_abs())) {
    Ok(Some(reciprocal)) => reciprocal,
    Ok(None) => return Ok(WarpValidity::Rejected),
  };

  let y_factor = i64::from(yx).wrapping_mul(i64::from(divisor.factor));
  let Ok(vertical) = round_product(
    y_factor.wrapping_mul(i64::from(ONE)),
    divisor.shift,
    RoundingWidth::Shear,
  );
  let Ok(cross) = round_product(
    y_factor.wrapping_mul(i64::from(xy)),
    divisor.shift,
    RoundingWidth::Shear,
  );
  let Ok(alpha) = round_shear(xx.wrapping_sub(ONE));
  let Ok(beta) = round_shear(xy);
  let Ok(gamma) = round_shear(vertical);
  let Ok(delta) = round_shear(yy.wrapping_sub(cross).wrapping_sub(ONE));
  let Ok(()) = wm.abcd.set([alpha, beta, gamma, delta]);

  let x_warp = u32::from(alpha.unsigned_abs())
    .wrapping_mul(4)
    .wrapping_add(u32::from(beta.unsigned_abs()).wrapping_mul(7));
  let y_warp = u32::from(gamma.unsigned_abs())
    .wrapping_add(u32::from(delta.unsigned_abs()))
    .wrapping_mul(4);

  Ok(match (x_warp.cmp(&0x10000), y_warp.cmp(&0x10000)) {
    (Ordering::Less, Ordering::Less) => WarpValidity::Usable,
    _ => WarpValidity::Rejected,
  })
}

fn block_center(block: Rectangle<i32>) -> Result<Point<i32>, Never> {
  Ok(Point {
    x: block.size.width.wrapping_mul(2).wrapping_sub(1),
    y: block.size.height.wrapping_mul(2).wrapping_sub(1),
  })
}

pub(crate) fn rav1d_set_affine_mv2d(
  block: Rectangle<i32>,
  mv: Mv,
  wm: &mut Rav1dWarpedMotionParams,
) -> Result<(), Never> {
  let Ok(center) = block_center(block);
  let x = block.origin.x.wrapping_mul(4).wrapping_add(center.x);
  let y = block.origin.y.wrapping_mul(4).wrapping_add(center.y);
  let [horizontal, vertical, xx, xy, yx, yy] = &mut wm.matrix;
  let drift_x = x
    .wrapping_mul(xx.wrapping_sub(ONE))
    .wrapping_add(y.wrapping_mul(*xy));
  let drift_y = x
    .wrapping_mul(*yx)
    .wrapping_add(y.wrapping_mul(yy.wrapping_sub(ONE)));
  *horizontal = iclip(
    i32::from(mv.x).wrapping_mul(0x2000).wrapping_sub(drift_x),
    -0x800000,
    0x7fffff,
  );
  *vertical = iclip(
    i32::from(mv.y).wrapping_mul(0x2000).wrapping_sub(drift_y),
    -0x800000,
    0x7fffff,
  );

  Ok(())
}

#[derive(Default)]
struct NormalEquations {
  xx: i32,
  xy: i32,
  yy: i32,
  ref_x: Point<i32>,
  ref_y: Point<i32>,
}

impl NormalEquations {
  fn add(&mut self, sample: Point<i32>, reference: Point<i32>) -> Result<(), Never> {
    let Point { x: sx, y: sy } = sample;
    let Point { x: dx, y: dy } = reference;

    match (
      sx.wrapping_sub(dx).wrapping_abs().cmp(&256),
      sy.wrapping_sub(dy).wrapping_abs().cmp(&256),
    ) {
      (Ordering::Less, Ordering::Less) => {}
      _ => return Ok(()),
    }

    let Ok(xx) = normal_product([sx, sx], AffineTerm::Diagonal);
    let Ok(xy) = normal_product([sx, sy], AffineTerm::OffDiagonal);
    let Ok(yy) = normal_product([sy, sy], AffineTerm::Diagonal);
    let Ok(ref_xx) = normal_product([sx, dx], AffineTerm::Diagonal);
    let Ok(ref_xy) = normal_product([sy, dx], AffineTerm::OffDiagonal);
    let Ok(ref_yx) = normal_product([sx, dy], AffineTerm::OffDiagonal);
    let Ok(ref_yy) = normal_product([sy, dy], AffineTerm::Diagonal);

    self.xx = self.xx.wrapping_add(xx);
    self.xy = self.xy.wrapping_add(xy);
    self.yy = self.yy.wrapping_add(yy);
    self.ref_x.x = self.ref_x.x.wrapping_add(ref_xx);
    self.ref_x.y = self.ref_x.y.wrapping_add(ref_xy);
    self.ref_y.x = self.ref_y.x.wrapping_add(ref_yx);
    self.ref_y.y = self.ref_y.y.wrapping_add(ref_yy);

    Ok(())
  }

  fn fit(self, wm: &mut Rav1dWarpedMotionParams) -> Result<WarpValidity, Never> {
    let Ok(det) = determinant([self.xx, self.xy], [self.xy, self.yy]);
    let reciprocal = match Divisor::normalized(det.unsigned_abs()) {
      Ok(Some(divisor)) => divisor,
      Ok(None) => return Ok(WarpValidity::Rejected),
    };
    let Ok(divisor) = reciprocal.scaled_for_affine(det);
    let [_, _, xx, xy, yx, yy] = &mut wm.matrix;
    let Ok(xx_product) = self.x_product(self.ref_x);
    let Ok(xy_product) = self.y_product(self.ref_x);
    let Ok(yx_product) = self.x_product(self.ref_y);
    let Ok(yy_product) = self.y_product(self.ref_y);
    let Ok(xx_value) = divisor.affine_coefficient(xx_product, AffineTerm::Diagonal);
    let Ok(xy_value) = divisor.affine_coefficient(xy_product, AffineTerm::OffDiagonal);
    let Ok(yx_value) = divisor.affine_coefficient(yx_product, AffineTerm::OffDiagonal);
    let Ok(yy_value) = divisor.affine_coefficient(yy_product, AffineTerm::Diagonal);
    *xx = xx_value;
    *xy = xy_value;
    *yx = yx_value;
    *yy = yy_value;

    Ok(WarpValidity::Usable)
  }

  fn x_product(&self, reference: Point<i32>) -> Result<i64, Never> {
    determinant([reference.x, reference.y], [self.xy, self.yy])
  }

  fn y_product(&self, reference: Point<i32>) -> Result<i64, Never> {
    determinant([self.xx, self.xy], [reference.x, reference.y])
  }
}

fn determinant([xx, xy]: [i32; 2], [yx, yy]: [i32; 2]) -> Result<i64, Never> {
  Ok(
    i64::from(xx)
      .wrapping_mul(i64::from(yy))
      .wrapping_sub(i64::from(xy).wrapping_mul(i64::from(yx))),
  )
}

fn normal_product([first, second]: [i32; 2], term: AffineTerm) -> Result<i32, Never> {
  let bias = match term {
    AffineTerm::Diagonal => 8,
    AffineTerm::OffDiagonal => 4,
  };
  Ok(
    first
      .wrapping_mul(second)
      .wrapping_shr(2)
      .wrapping_add(first)
      .wrapping_add(second)
      .wrapping_add(bias),
  )
}

pub(crate) fn rav1d_find_affine_int(
  samples: &[[[i32; 2]; 2]],
  block: Rectangle<i32>,
  mv: Mv,
  wm: &mut Rav1dWarpedMotionParams,
) -> Result<WarpValidity, Never> {
  let Ok(center) = block_center(block);
  let source = Point {
    x: center.x.wrapping_mul(8),
    y: center.y.wrapping_mul(8),
  };
  let destination = Point {
    x: source.x.wrapping_add(i32::from(mv.x)),
    y: source.y.wrapping_add(i32::from(mv.y)),
  };
  let mut equations = NormalEquations::default();

  for &[[sx, sy], [dx, dy]] in samples {
    let Ok(()) = equations.add(
      Point {
        x: sx.wrapping_sub(source.x),
        y: sy.wrapping_sub(source.y),
      },
      Point {
        x: dx.wrapping_sub(destination.x),
        y: dy.wrapping_sub(destination.y),
      },
    );
  }

  let Ok(usable) = equations.fit(wm);

  match usable {
    WarpValidity::Usable => {
      let Ok(()) = rav1d_set_affine_mv2d(block, mv, wm);
    }
    WarpValidity::Rejected => {}
  }

  Ok(usable)
}
