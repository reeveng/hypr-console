//! Graphics state for the TrueType interpreter.

use super::{
  F26Dot6, Point, Target,
  round::RoundState,
  zone::{Zone, ZonePointer},
};
use core::ops::{Deref, DerefMut};

use crate::{BackwardCompatibility, GlyphKind, LinearMetrics};

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum CoordAxis {
  #[default]
  Both,
  X,
  Y,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropoutControl {
  Enabled,
  Disabled,
}

#[derive(Debug)]
pub struct GraphicsState<'a> {
  pub retained: RetainedGraphicsState,

  pub proj_vector: Point<i32>,

  pub proj_axis: CoordAxis,

  pub dual_proj_vector: Point<i32>,

  pub dual_proj_axis: CoordAxis,

  pub freedom_vector: Point<i32>,

  pub freedom_axis: CoordAxis,

  pub fdotp: i32,

  pub round_state: RoundState,

  pub rp0: u64,

  pub rp1: u64,

  pub rp2: u64,

  pub loop_counter: u32,

  pub zp0: ZonePointer,

  pub zp1: ZonePointer,

  pub zp2: ZonePointer,

  pub zones: [Zone<'a>; 2],

  pub glyph_kind: GlyphKind,

  pub backward_compatibility: crate::BackwardCompatibility,

  pub is_pedantic: crate::Pedantry,

  pub did_iup_x: crate::InterpolationProgress,

  pub did_iup_y: crate::InterpolationProgress,
}

impl GraphicsState<'_> {
  pub fn unscaled_to_pixels(&self) -> Result<i32, console_core_never::Never> {
    Ok(match self.glyph_kind {
      GlyphKind::Composite => i32::wrapping_shl(1, 16_u32),
      GlyphKind::Simple => self.scale,
    })
  }

  pub fn reset(&mut self) -> Result<(), console_core_never::Never> {
    let GraphicsState {
      retained,
      zones,
      glyph_kind,
      ..
    } = core::mem::take(self);
    *self = GraphicsState {
      retained,
      zones,
      glyph_kind,
      ..Default::default()
    };

    {
      let Ok(value) = self.update_projection_state();
      value
    };

    Ok(())
  }

  pub fn reset_retained(&mut self) -> Result<(), console_core_never::Never> {
    let _: () = {
      let scale = self.scale;
      let ppem = self.ppem;
      let mode = self.target;
      self.retained = RetainedGraphicsState {
        scale,
        ppem,
        target: mode,
        ..Default::default()
      }
    };
    Ok(())
  }
}

impl Default for GraphicsState<'_> {
  fn default() -> Self {
    let vector = Point::new(0x4000, 0);
    Self {
      retained: RetainedGraphicsState::default(),
      proj_vector: {
        let Ok(value) = vector;
        value
      },
      proj_axis: CoordAxis::Both,
      dual_proj_vector: {
        let Ok(value) = vector;
        value
      },
      dual_proj_axis: CoordAxis::Both,
      freedom_vector: {
        let Ok(value) = vector;
        value
      },
      freedom_axis: CoordAxis::Both,
      fdotp: 0x4000,
      round_state: RoundState::default(),
      rp0: 0,
      rp1: 0,
      rp2: 0,
      loop_counter: 1,
      zp0: ZonePointer::default(),
      zp1: ZonePointer::default(),
      zp2: ZonePointer::default(),
      zones: [Zone::default(), Zone::default()],
      glyph_kind: GlyphKind::Simple,
      backward_compatibility: crate::BackwardCompatibility::Enabled,
      is_pedantic: crate::Pedantry::Lenient,
      did_iup_x: crate::InterpolationProgress::Pending,
      did_iup_y: crate::InterpolationProgress::Pending,
    }
  }
}

#[derive(Copy, Clone, Debug)]
pub struct RetainedGraphicsState {
  pub auto_flip: crate::AutomaticFlip,

  pub control_value_cutin: F26Dot6,

  pub delta_base: u16,

  pub delta_shift: u16,

  pub instruct_control: u8,

  pub min_distance: F26Dot6,

  pub scan_control: DropoutControl,

  pub scan_type: i32,

  pub single_width_cutin: F26Dot6,

  pub single_width: F26Dot6,

  pub target: Target,

  pub scale: i32,

  pub ppem: i32,

  pub is_rotated: crate::Rotation,

  pub is_stretched: crate::Stretch,
}

impl RetainedGraphicsState {
  pub fn initial_backward_compatibility(
    &self,
  ) -> Result<BackwardCompatibility, console_core_never::Never> {
    Ok(match self.target {
      Target::Mono => BackwardCompatibility::Disabled,
      Target::Smooth {
        linear_metrics: LinearMetrics::Preserve,
        ..
      } => BackwardCompatibility::Enabled,
      Target::Smooth {
        linear_metrics: LinearMetrics::Hint,
        ..
      } => BackwardCompatibility::from(self.instruct_control & 0x4 == 0),
    })
  }
}

impl Default for RetainedGraphicsState {
  fn default() -> Self {
    Self {
      auto_flip: crate::AutomaticFlip::Enabled,

      control_value_cutin: {
        let Ok(value) = F26Dot6::from_bits(68);
        value
      },
      delta_base: 9,
      delta_shift: 3,
      instruct_control: 0,

      min_distance: {
        let Ok(value) = F26Dot6::from_bits(64);
        value
      },
      scan_control: DropoutControl::Disabled,
      scan_type: 0,
      single_width_cutin: F26Dot6::ZERO,
      single_width: F26Dot6::ZERO,
      target: Default::default(),
      scale: 0,
      ppem: 0,
      is_rotated: crate::Rotation::Unrotated,
      is_stretched: crate::Stretch::Unstretched,
    }
  }
}

impl Deref for GraphicsState<'_> {
  type Target = RetainedGraphicsState;

  fn deref(&self) -> &Self::Target {
    &self.retained
  }
}

impl DerefMut for GraphicsState<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.retained
  }
}

#[derive(Clone, Copy)]
pub(crate) enum Touch {
  Mark,
  Keep,
}
