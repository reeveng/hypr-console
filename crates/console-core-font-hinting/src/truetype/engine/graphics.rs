//! Managing the graphics state.
//!
//! Implements 45 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-the-graphics-state>

use super::{
  super::{
    graphics::DropoutControl,
    math,
    program::Program,
    round::{RoundMode, SuperGrid},
  },
  Engine, F26Dot6, HintErrorKind, OpResult, Point,
};

impl Engine<'_> {
  pub(super) fn op_svtca(&mut self, opcode: u8) -> OpResult {
    let Ok(opcode_2) = console_core_number_conversion::wrapped::<_, i32>(opcode);

    let x = i32::wrapping_shl(opcode_2 & 1, 14_u32);
    let y = x ^ 0x4000;

    match ::core::primitive::bool::then_some(opcode_2 < 4, ()) {
      ::core::option::Option::Some(()) => {
        self.graphics.proj_vector.x = x;
        self.graphics.proj_vector.y = y;
        self.graphics.dual_proj_vector.x = x;
        self.graphics.dual_proj_vector.y = y;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(opcode_2 & 2 == 0, ()) {
      ::core::option::Option::Some(()) => {
        self.graphics.freedom_vector.x = x;
        self.graphics.freedom_vector.y = y;
      }
      ::core::option::Option::None => {}
    }

    {
      let Ok(value) = self.graphics.update_projection_state();
      value
    };

    Ok(())
  }

  pub(super) fn op_svtl(&mut self, opcode: u8) -> OpResult {
    let index1 = self.value_stack.pop_index()?;
    let index2 = self.value_stack.pop_index()?;
    let is_parallel = opcode & 1 == 0;
    let p1 = {
      let Ok(value) = self.graphics.zp1();
      value
    }
    .point(index2)?;
    let p2 = {
      let Ok(value) = self.graphics.zp2();
      value
    }
    .point(index1)?;
    let Ok(vector) = line_vector(
      p1,
      p2,
      match ::core::primitive::bool::then_some(is_parallel, ()) {
        ::core::option::Option::Some(()) => LineOrientation::Parallel,
        ::core::option::Option::None => LineOrientation::Perpendicular,
      },
    );

    match ::core::primitive::bool::then_some(opcode < 8, ()) {
      ::core::option::Option::Some(()) => {
        self.graphics.proj_vector = vector;
        self.graphics.dual_proj_vector = vector;
      }
      ::core::option::Option::None => {
        self.graphics.freedom_vector = vector;
      }
    }

    {
      let Ok(value) = self.graphics.update_projection_state();
      value
    };

    Ok(())
  }

  pub(super) fn op_sfvtpv(&mut self) -> OpResult {
    self.graphics.freedom_vector = self.graphics.proj_vector;

    {
      let Ok(value) = self.graphics.update_projection_state();
      value
    };

    Ok(())
  }

  pub(super) fn op_sdpvtl(&mut self, opcode: u8) -> OpResult {
    let index1 = self.value_stack.pop_index()?;
    let index2 = self.value_stack.pop_index()?;
    let is_parallel = opcode & 1 == 0;

    let p1 = {
      let Ok(value) = self.graphics.zp1();
      value
    }
    .original(index2)?;
    let p2 = {
      let Ok(value) = self.graphics.zp2();
      value
    }
    .original(index1)?;
    self.graphics.dual_proj_vector = {
      let Ok(value) = line_vector(
        p1,
        p2,
        match ::core::primitive::bool::then_some(is_parallel, ()) {
          ::core::option::Option::Some(()) => LineOrientation::Parallel,
          ::core::option::Option::None => LineOrientation::Perpendicular,
        },
      );
      value
    };

    let p1_2 = {
      let Ok(value) = self.graphics.zp1();
      value
    }
    .point(index2)?;
    let p2_2 = {
      let Ok(value) = self.graphics.zp2();
      value
    }
    .point(index1)?;
    self.graphics.proj_vector = {
      let Ok(value) = line_vector(
        p1_2,
        p2_2,
        match ::core::primitive::bool::then_some(is_parallel, ()) {
          ::core::option::Option::Some(()) => LineOrientation::Parallel,
          ::core::option::Option::None => LineOrientation::Perpendicular,
        },
      );
      value
    };

    {
      let Ok(value) = self.graphics.update_projection_state();
      value
    };

    Ok(())
  }

  pub(super) fn op_spvfs(&mut self) -> OpResult {
    let Ok(y) = console_core_number_conversion::wrapped::<_, i32>({
      let Ok(value) = console_core_number_conversion::wrapped::<_, i16>({
        let answer = self.value_stack.pop()?;
        answer
      });
      value
    });
    let Ok(x) = console_core_number_conversion::wrapped::<_, i32>({
      let Ok(value) = console_core_number_conversion::wrapped::<_, i16>({
        let answer = self.value_stack.pop()?;
        answer
      });
      value
    });
    let vector = match ::core::primitive::bool::then_some((x, y) == (0, 0), ()) {
      ::core::option::Option::Some(()) => self.graphics.proj_vector,
      ::core::option::Option::None => {
        let Ok(math_result_1) = math::normalize14(read_fonts::types::Point { x, y });
        math_result_1
      }
    };
    self.graphics.proj_vector = vector;
    self.graphics.dual_proj_vector = vector;

    {
      let Ok(value) = self.graphics.update_projection_state();
      value
    };

    Ok(())
  }

  pub(super) fn op_sfvfs(&mut self) -> OpResult {
    let Ok(y) = console_core_number_conversion::wrapped::<_, i32>({
      let Ok(value) = console_core_number_conversion::wrapped::<_, i16>({
        let answer = self.value_stack.pop()?;
        answer
      });
      value
    });
    let Ok(x) = console_core_number_conversion::wrapped::<_, i32>({
      let Ok(value) = console_core_number_conversion::wrapped::<_, i16>({
        let answer = self.value_stack.pop()?;
        answer
      });
      value
    });
    let vector = match ::core::primitive::bool::then_some((x, y) == (0, 0), ()) {
      ::core::option::Option::Some(()) => self.graphics.freedom_vector,
      ::core::option::Option::None => {
        let Ok(math_result_2) = math::normalize14(read_fonts::types::Point { x, y });
        math_result_2
      }
    };
    self.graphics.freedom_vector = vector;

    {
      let Ok(value) = self.graphics.update_projection_state();
      value
    };

    Ok(())
  }

  pub(super) fn op_gpv(&mut self) -> OpResult {
    let vector = self.graphics.proj_vector;
    self.value_stack.push(vector.x)?;
    self.value_stack.push(vector.y)
  }

  pub(super) fn op_gfv(&mut self) -> OpResult {
    let vector = self.graphics.freedom_vector;
    self.value_stack.push(vector.x)?;
    self.value_stack.push(vector.y)
  }

  pub(super) fn op_srp0(&mut self) -> OpResult {
    let p = self.value_stack.pop_index()?;
    self.graphics.rp0 = p;
    Ok(())
  }

  pub(super) fn op_srp1(&mut self) -> OpResult {
    let p = self.value_stack.pop_index()?;
    self.graphics.rp1 = p;
    Ok(())
  }

  pub(super) fn op_srp2(&mut self) -> OpResult {
    let p = self.value_stack.pop_index()?;
    self.graphics.rp2 = p;
    Ok(())
  }

  pub(super) fn op_szp0(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;
    self.graphics.zp0 = {
      let answer = n.try_into()?;
      answer
    };
    Ok(())
  }

  pub(super) fn op_szp1(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;
    self.graphics.zp1 = {
      let answer = n.try_into()?;
      answer
    };
    Ok(())
  }

  pub(super) fn op_szp2(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;
    self.graphics.zp2 = {
      let answer = n.try_into()?;
      answer
    };
    Ok(())
  }

  pub(super) fn op_szps(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;
    let zp = n.try_into()?;
    self.graphics.zp0 = zp;
    self.graphics.zp1 = zp;
    self.graphics.zp2 = zp;
    Ok(())
  }

  pub(super) fn op_rthg(&mut self) -> OpResult {
    let Ok(()) = self.graphics.round_state.select(RoundMode::HalfGrid);
    Ok(())
  }

  pub(super) fn op_rtg(&mut self) -> OpResult {
    let Ok(()) = self.graphics.round_state.select(RoundMode::Grid);
    Ok(())
  }

  pub(super) fn op_rtdg(&mut self) -> OpResult {
    let Ok(()) = self.graphics.round_state.select(RoundMode::DoubleGrid);
    Ok(())
  }

  pub(super) fn op_rdtg(&mut self) -> OpResult {
    let Ok(()) = self.graphics.round_state.select(RoundMode::DownToGrid);
    Ok(())
  }

  pub(super) fn op_rutg(&mut self) -> OpResult {
    let Ok(()) = self.graphics.round_state.select(RoundMode::UpToGrid);
    Ok(())
  }

  pub(super) fn op_roff(&mut self) -> OpResult {
    let Ok(()) = self.graphics.round_state.select(RoundMode::Off);
    Ok(())
  }

  pub(super) fn op_sround(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;

    let Ok(()) = self
      .graphics
      .round_state
      .configure_super(SuperGrid::AxisAligned, n);
    Ok(())
  }

  pub(super) fn op_s45round(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;

    let Ok(()) = self
      .graphics
      .round_state
      .configure_super(SuperGrid::Diagonal, n);
    Ok(())
  }

  pub(super) fn op_sloop(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;

    match ::core::primitive::bool::then_some(n < 0, ()) {
      ::core::option::Option::Some(()) => {
        return Err(HintErrorKind::NegativeLoopCounter);
      }
      ::core::option::Option::None => {}
    }

    self.graphics.loop_counter = {
      let Ok(value) = console_core_number_conversion::wrapped::<_, u32>(n);
      value
    }
    .min(0xFFFF);
    Ok(())
  }

  pub(super) fn op_smd(&mut self) -> OpResult {
    let distance = self.value_stack.pop_f26dot6()?;
    self.graphics.min_distance = distance;
    Ok(())
  }

  pub(super) fn op_instctrl(&mut self) -> OpResult {
    let Ok(selector) = console_core_number_conversion::wrapped::<_, u32>({
      let answer = self.value_stack.pop()?;
      answer
    });
    let Ok(value) = console_core_number_conversion::wrapped::<_, u32>({
      let answer = self.value_stack.pop()?;
      answer
    });

    match ::core::primitive::bool::then_some(!(1..=3).contains(&selector), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let selector_flag = u32::wrapping_shl(1, {
      let Ok(value_2) =
        console_core_number_conversion::wrapped::<_, u32>(u32::wrapping_sub(selector, 1));
      value_2
    });

    match ::core::primitive::bool::then_some(value != 0 && value != selector_flag, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match (selector, self.graphics.target) {
      (
        3,
        crate::Target::Smooth {
          linear_metrics: crate::LinearMetrics::Preserve,
          ..
        },
      ) => {
        return Ok(());
      }
      (_, _) => {}
    }

    match (self.program.initial, selector) {
      (Program::ControlValue, _) => {
        self.graphics.instruct_control &= !{
          let Ok(value_3) = console_core_number_conversion::wrapped::<_, u8>(selector_flag);
          value_3
        };
        self.graphics.instruct_control |= {
          let Ok(value_4) = console_core_number_conversion::wrapped::<_, u8>(value);
          value_4
        };
      }

      (Program::Glyph, 3) => {
        self.graphics.backward_compatibility = crate::BackwardCompatibility::from(value != 4);
      }
      _ => {}
    }

    Ok(())
  }

  pub(super) fn op_scanctrl(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;

    let threshold = n & 0xFF;

    match threshold {
      0xFF => self.graphics.scan_control = DropoutControl::Enabled,

      0 => self.graphics.scan_control = DropoutControl::Disabled,
      _ => {
        let ppem = self.graphics.ppem;
        let is_rotated = self.graphics.is_rotated;
        let is_stretched = self.graphics.is_stretched;
        let scan_control = &mut self.graphics.scan_control;

        match ::core::primitive::bool::then_some((n & 0x100) != 0 && ppem <= threshold, ()) {
          ::core::option::Option::Some(()) => {
            *scan_control = DropoutControl::Enabled;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(
          (n & 0x200) != 0 && matches!(is_rotated, crate::Rotation::Rotated),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            *scan_control = DropoutControl::Enabled;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(
          (n & 0x400) != 0 && matches!(is_stretched, crate::Stretch::Stretched),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            *scan_control = DropoutControl::Enabled;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some((n & 0x800) != 0 && ppem > threshold, ()) {
          ::core::option::Option::Some(()) => {
            *scan_control = DropoutControl::Disabled;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(
          (n & 0x1000) != 0 && matches!(is_rotated, crate::Rotation::Rotated),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            *scan_control = DropoutControl::Disabled;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(
          (n & 0x2000) != 0 && matches!(is_stretched, crate::Stretch::Stretched),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            *scan_control = DropoutControl::Disabled;
          }
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(())
  }

  pub(super) fn op_scantype(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;
    self.graphics.scan_type = n & 0xFFFF;
    Ok(())
  }

  pub(super) fn op_scvtci(&mut self) -> OpResult {
    let n = self.value_stack.pop_f26dot6()?;
    self.graphics.control_value_cutin = n;
    Ok(())
  }

  pub(super) fn op_sswci(&mut self) -> OpResult {
    let n = self.value_stack.pop_f26dot6()?;
    self.graphics.single_width_cutin = n;
    Ok(())
  }

  pub(super) fn op_ssw(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;
    self.graphics.single_width = {
      let Ok(value) = F26Dot6::from_bits({
        let Ok(math_result_3) = math::mul(math::Product {
          left: n,
          right: self.graphics.scale,
        });
        math_result_3
      });
      value
    };
    Ok(())
  }

  pub(super) fn op_flipon(&mut self) -> OpResult {
    self.graphics.auto_flip = crate::AutomaticFlip::Enabled;
    Ok(())
  }

  pub(super) fn op_flipoff(&mut self) -> OpResult {
    self.graphics.auto_flip = crate::AutomaticFlip::Disabled;
    Ok(())
  }

  pub(super) fn op_sangw(&mut self) -> OpResult {
    let _weight = self.value_stack.pop()?;
    Ok(())
  }

  pub(super) fn op_sdb(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;
    self.graphics.delta_base = {
      let Ok(value) = console_core_number_conversion::wrapped::<_, u16>(n);
      value
    };
    Ok(())
  }

  pub(super) fn op_sds(&mut self) -> OpResult {
    let n = self.value_stack.pop()?;

    match ::core::primitive::bool::then_some(
      {
        let Ok(value) = console_core_number_conversion::wrapped::<_, u32>(n);
        value
      } > 6,
      (),
    ) {
      ::core::option::Option::Some(()) => Err(HintErrorKind::InvalidStackValue(n)),
      ::core::option::Option::None => {
        self.graphics.delta_shift = {
          let Ok(value) = console_core_number_conversion::wrapped::<_, u16>(n);
          value
        };
        Ok(())
      }
    }
  }
}

fn line_vector(
  p1: Point<F26Dot6>,
  p2: Point<F26Dot6>,
  orientation: LineOrientation,
) -> Result<Point<i32>, console_core_never::Never> {
  let Ok(mut a) = (p1.x - p2.x).to_bits();
  let Ok(mut b) = (p1.y - p2.y).to_bits();

  match (a, b, orientation) {
    (0, 0, _) => {
      a = 0x4000;
    }
    (_, _, LineOrientation::Perpendicular) => {
      let c = b;
      b = a;
      a = c.wrapping_neg();
    }
    (_, _, LineOrientation::Parallel) => {}
  }

  Ok({
    let Ok(math_result_4) = math::normalize14(read_fonts::types::Point { x: a, y: b });
    math_result_4
  })
}

#[derive(Clone, Copy)]

enum LineOrientation {
  Parallel,
  Perpendicular,
}

#[cfg(test)]
mod tests {
  use super::{
    super::{
      super::zone::{Zone, ZonePointer},
      F2Dot14, MockEngine, math,
    },
    DropoutControl, F26Dot6, HintErrorKind, Point, Program, RoundMode,
  };

  const ONE: i32 = ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i16 => ::core::primitive::i32, ({
      let Ok(value) = F2Dot14::ONE.to_bits();
      value
  }));

  const X_AXIS: Point<i32> = {
    let Ok(value) = Point::new(ONE, 0);
    value
  };
  const Y_AXIS: Point<i32> = {
    let Ok(value) = Point::new(0, ONE);
    value
  };

  #[test]
  fn set_vectors_to_coord_axis() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.op_svtca(0x00).required()?;
    assert_eq!(engine.graphics.freedom_vector, Y_AXIS);
    assert_eq!(engine.graphics.proj_vector, Y_AXIS);

    engine.op_svtca(0x01).required()?;
    assert_eq!(engine.graphics.freedom_vector, X_AXIS);
    assert_eq!(engine.graphics.proj_vector, X_AXIS);

    engine.op_svtca(0x02).required()?;
    assert_eq!(engine.graphics.proj_vector, Y_AXIS);

    engine.op_svtca(0x03).required()?;
    assert_eq!(engine.graphics.proj_vector, X_AXIS);

    engine.op_svtca(0x04).required()?;
    assert_eq!(engine.graphics.freedom_vector, Y_AXIS);

    engine.op_svtca(0x05).required()?;
    assert_eq!(engine.graphics.freedom_vector, X_AXIS);

    Ok(())
  }

  #[test]
  fn set_get_vectors_from_stack() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.value_stack.push(X_AXIS.x).required()?;
    engine.value_stack.push(X_AXIS.y).required()?;
    engine.op_spvfs().required()?;
    assert_eq!(engine.graphics.proj_vector, X_AXIS);
    engine.op_gpv().required()?;
    let y = engine.value_stack.pop().required()?;
    let x = engine.value_stack.pop().required()?;
    assert_eq!(
      ({
        let Ok(value) = Point::new(x, y);
        value
      }),
      X_AXIS
    );

    engine.value_stack.push(Y_AXIS.x).required()?;
    engine.value_stack.push(Y_AXIS.y).required()?;
    engine.op_sfvfs().required()?;
    assert_eq!(engine.graphics.freedom_vector, Y_AXIS);
    engine.op_gfv().required()?;
    let y_2 = engine.value_stack.pop().required()?;
    let x_2 = engine.value_stack.pop().required()?;
    assert_eq!(
      ({
        let Ok(value) = Point::new(x_2, y_2);
        value
      }),
      Y_AXIS
    );

    Ok(())
  }

  #[test]
  fn set_vectors_to_line() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    let Ok(positive) = F26Dot6::from_bits(64);
    let Ok(negative) = F26Dot6::from_bits(-64);
    let mut points = [
      Point::default(),
      Point {
        x: positive,
        y: F26Dot6::ZERO,
      },
    ];
    let mut original = [
      Point {
        x: F26Dot6::ZERO,
        y: positive,
      },
      Point {
        x: F26Dot6::ZERO,
        y: negative,
      },
    ];
    let Ok(glyph) = engine.graphics.zone_mut(ZonePointer::Glyph);
    *glyph = Zone {
      points: &mut points,
      original: &mut original,
      unscaled: &mut [],
      flags: &mut [],
      contours: &[],
    };
    engine.value_stack.push(1).required()?;
    engine.op_szps().required()?;

    for _ in 0..6 {
      engine.value_stack.push(1).required()?;
      engine.value_stack.push(0).required()?;
    }

    engine.op_svtl(0x6).required()?;
    assert_eq!(engine.graphics.proj_vector, X_AXIS);

    engine.op_svtl(0x7).required()?;
    assert_eq!(engine.graphics.proj_vector, Y_AXIS);

    engine.op_svtl(0x8).required()?;
    assert_eq!(engine.graphics.freedom_vector, X_AXIS);

    engine.op_svtl(0x9).required()?;
    assert_eq!(engine.graphics.freedom_vector, Y_AXIS);

    engine.op_sdpvtl(0x86).required()?;
    assert_eq!(
      engine.graphics.dual_proj_vector,
      Point {
        x: 0,
        y: ONE.wrapping_neg()
      }
    );

    engine.op_sdpvtl(0x87).required()?;
    assert_eq!(engine.graphics.dual_proj_vector, X_AXIS);

    Ok(())
  }

  #[test]
  fn set_reference_points_and_zones() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.value_stack.push(111).required()?;
    engine.op_srp0().required()?;
    assert_eq!(engine.graphics.rp0, 111);

    engine.value_stack.push(222).required()?;
    engine.op_srp1().required()?;
    assert_eq!(engine.graphics.rp1, 222);

    engine.value_stack.push(333).required()?;
    engine.op_srp2().required()?;
    assert_eq!(engine.graphics.rp2, 333);

    engine.value_stack.push(1).required()?;
    engine.op_szp0().required()?;
    assert_eq!(engine.graphics.zp0, ZonePointer::Glyph);

    engine.value_stack.push(0).required()?;
    engine.op_szp1().required()?;
    assert_eq!(engine.graphics.zp1, ZonePointer::Twilight);

    engine.value_stack.push(1).required()?;
    engine.op_szp2().required()?;
    assert_eq!(engine.graphics.zp2, ZonePointer::Glyph);

    engine.value_stack.push(0).required()?;
    engine.op_szps().required()?;
    assert_eq!(
      [
        engine.graphics.zp0,
        engine.graphics.zp1,
        engine.graphics.zp2
      ],
      [ZonePointer::Twilight; 3]
    );

    engine.value_stack.push(2).required()?;
    assert!(matches!(
      engine.op_szps(),
      Err(HintErrorKind::InvalidZoneIndex(2))
    ));

    Ok(())
  }

  #[test]
  fn set_rounding_modes() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.op_rtg().required()?;
    assert_eq!(engine.graphics.round_state.mode, RoundMode::Grid);

    engine.op_rtdg().required()?;
    assert_eq!(engine.graphics.round_state.mode, RoundMode::DoubleGrid);

    engine.op_rdtg().required()?;
    assert_eq!(engine.graphics.round_state.mode, RoundMode::DownToGrid);

    engine.op_rutg().required()?;
    assert_eq!(engine.graphics.round_state.mode, RoundMode::UpToGrid);

    engine.op_roff().required()?;
    assert_eq!(engine.graphics.round_state.mode, RoundMode::Off);

    engine.value_stack.push(0).required()?;
    engine.op_sround().required()?;
    assert_eq!(engine.graphics.round_state.mode, RoundMode::Super);

    engine.value_stack.push(0).required()?;
    engine.op_s45round().required()?;
    assert_eq!(engine.graphics.round_state.mode, RoundMode::Super45);

    Ok(())
  }

  #[test]
  fn set_loop_and_dimension_state() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.value_stack.push(10).required()?;
    engine.op_sloop().required()?;
    assert_eq!(engine.graphics.loop_counter, 10);

    engine.value_stack.push(-10).required()?;
    assert!(matches!(
      engine.op_sloop(),
      Err(HintErrorKind::NegativeLoopCounter)
    ));

    engine.value_stack.push(64).required()?;
    engine.op_smd().required()?;
    assert_eq!(
      engine.graphics.min_distance,
      ({
        let Ok(value) = F26Dot6::from_bits(64);
        value
      })
    );

    engine.value_stack.push(50).required()?;
    engine.op_scantype().required()?;
    assert_eq!(engine.graphics.scan_type, 50);

    engine.value_stack.push(128).required()?;
    engine.op_scvtci().required()?;
    assert_eq!(
      engine.graphics.control_value_cutin,
      ({
        let Ok(value) = F26Dot6::from_bits(128);
        value
      })
    );

    engine.value_stack.push(100).required()?;
    engine.op_sswci().required()?;
    assert_eq!(
      engine.graphics.single_width_cutin,
      ({
        let Ok(value) = F26Dot6::from_bits(100);
        value
      })
    );

    engine.graphics.scale = 64;
    engine.value_stack.push(100).required()?;
    engine.op_ssw().required()?;
    let Ok(expected) = math::mul(math::Product {
      left: 100,
      right: engine.graphics.scale,
    });
    assert_eq!(
      engine.graphics.single_width,
      ({
        let Ok(value) = F26Dot6::from_bits(expected);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn set_autoflip_and_delta_state() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.op_flipoff().required()?;
    assert!(!matches!(
      engine.graphics.auto_flip,
      crate::AutomaticFlip::Enabled
    ));

    engine.op_flipon().required()?;
    assert!(matches!(
      engine.graphics.auto_flip,
      crate::AutomaticFlip::Enabled
    ));

    engine.value_stack.push(172).required()?;
    engine.op_sdb().required()?;
    assert_eq!(engine.graphics.delta_base, 172);

    engine.value_stack.push(4).required()?;
    engine.op_sds().required()?;
    assert_eq!(engine.graphics.delta_shift, 4);

    engine.value_stack.push(7).required()?;
    assert!(matches!(
      engine.op_sds(),
      Err(HintErrorKind::InvalidStackValue(7))
    ));

    Ok(())
  }

  #[test]
  fn instctrl() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.program.initial = Program::ControlValue;

    for selector in 1..=3 {
      let enable_mask = {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u8>(i32::wrapping_shl(1, {
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_sub(selector, 1));
            migrated_value
          }));
        migrated_value
      };
      engine
        .value_stack
        .push({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(enable_mask);
          migrated_value
        })
        .required()?;
      engine.value_stack.push(selector).required()?;
      engine.op_instctrl().required()?;
      assert!(engine.graphics.instruct_control & enable_mask != 0);

      engine.value_stack.push(0).required()?;
      engine.value_stack.push(selector).required()?;
      engine.op_instctrl().required()?;
      assert!(engine.graphics.instruct_control & enable_mask == 0);
    }

    engine.program.initial = Program::Glyph;

    engine
      .value_stack
      .push(i32::wrapping_shl(i32::wrapping_sub(3, 1), {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(1);
        migrated_value
      }))
      .required()?;
    engine.value_stack.push(3).required()?;
    engine.op_instctrl().required()?;
    assert!(!matches!(
      engine.graphics.backward_compatibility,
      crate::BackwardCompatibility::Enabled
    ));

    engine.value_stack.push(0).required()?;
    engine.value_stack.push(3).required()?;
    engine.op_instctrl().required()?;
    assert!(matches!(
      engine.graphics.backward_compatibility,
      crate::BackwardCompatibility::Enabled
    ));

    Ok(())
  }

  #[test]
  fn instctrl_avoid_overflow() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.program.initial = Program::ControlValue;
    engine.value_stack.push(0).required()?;
    engine.value_stack.push(0).required()?;
    engine.op_instctrl().required()?;

    Ok(())
  }

  #[test]
  fn scanctrl() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.value_stack.push(0x0000).required()?;
    engine.op_scanctrl().required()?;
    assert_eq!(engine.graphics.scan_control, DropoutControl::Disabled);

    engine.value_stack.push(0x01FF).required()?;
    engine.op_scanctrl().required()?;
    assert_eq!(engine.graphics.scan_control, DropoutControl::Enabled);

    engine.value_stack.push(0x0A10).required()?;
    engine.graphics.is_rotated = crate::Rotation::Rotated;
    engine.graphics.ppem = 12;
    engine.op_scanctrl().required()?;
    assert_eq!(engine.graphics.scan_control, DropoutControl::Enabled);

    Ok(())
  }
}
