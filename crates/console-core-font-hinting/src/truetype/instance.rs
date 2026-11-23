//! Instance state for TrueType hinting.

use super::{
  FontProgram, HintOutline, PointFlags, Target,
  cow_slice::CowSlice,
  definition::{Definition, DefinitionMap, DefinitionState},
  engine::{Engine, EngineInputs},
  error::{HintError, HintErrorKind},
  graphics::RetainedGraphicsState,
  program::{Program, ProgramBytes, ProgramState},
  value_stack::ValueStack,
  zone::Zone,
};
use alloc::vec::Vec;
use console_core_never::Never;
use raw::{
  TableProvider,
  types::{F2Dot14, F26Dot6, Fixed, Point},
};

#[derive(Clone, Default)]
pub struct HintInstance {
  functions: Vec<Definition>,
  instructions: Vec<Definition>,
  cvt: Vec<i32>,
  storage: Vec<i32>,
  graphics: RetainedGraphicsState,
  twilight_scaled: Vec<Point<F26Dot6>>,
  twilight_original_scaled: Vec<Point<F26Dot6>>,
  twilight_flags: Vec<PointFlags>,
  axis_count: u16,
  max_stack: u16,
}

impl HintInstance {
  pub fn reconfigure(
    &mut self,
    font_program: &FontProgram,
    scale: crate::Scale,
    target: Target,
    coords: &[F2Dot14],
  ) -> Result<(), HintError> {
    let Ok(scale_factor) = scale.factor.to_bits();
    let Ok(()) = self.setup(font_program, scale_factor, coords);

    let twilight_contours = [{
      let Ok(value) =
        console_core_number_conversion::wrapped::<_, u16>(self.twilight_scaled.len());
      value
    }];
    let Ok(twilight) = Zone::new(
      &[],
      &mut self.twilight_original_scaled,
      &mut self.twilight_scaled,
      &mut self.twilight_flags,
      &twilight_contours,
    );
    let glyph = Zone::default();
    let mut stack_buf: Vec<_> = (0..self.max_stack).map(|_| 0).collect();
    let Ok(value_stack) = ValueStack::new(&mut stack_buf, crate::Validation::Compatible);
    let graphics = RetainedGraphicsState {
      scale: {
        let Ok(value) = scale.factor.to_bits();
        value
      },
      ppem: scale.ppem,
      target,
      ..Default::default()
    };
    let Ok(program) = ProgramState::new(
      ProgramBytes {
        font: font_program.fpgm,
        control_value: font_program.prep,
        glyph: &[],
      },
      Program::Font,
    );
    let Ok(mut engine) = Engine::new(EngineInputs {
      font_program,
      program,
      graphics,
      definitions: DefinitionState {
        functions: DefinitionMap::Mut(&mut self.functions),
        instructions: DefinitionMap::Mut(&mut self.instructions),
      },
      cvt: super::cvt::Cvt::from(CowSlice::from(self.cvt.as_mut_slice())),
      storage: super::storage::Storage::from(CowSlice::from(self.storage.as_mut_slice())),
      value_stack,
      twilight,
      glyph,
      axis_count: self.axis_count,
      coords,
      glyph_kind: crate::GlyphKind::Simple,
    });

    engine.run_program(Program::Font, crate::Validation::Compatible)?;

    engine.run_program(Program::ControlValue, crate::Validation::Compatible)?;

    let Ok(retained) = engine.retained_graphics_state();
    self.graphics = *retained;
    Ok(())
  }

  pub fn execution(&self) -> Result<crate::Execution, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.graphics.instruct_control & 1 == 0, ()) {
        ::core::option::Option::Some(()) => crate::Execution::Enabled,
        ::core::option::Option::None => crate::Execution::Disabled,
      },
    )
  }

  pub fn backward_compatibility(&self) -> Result<crate::BackwardCompatibility, Never> {
    self.graphics.initial_backward_compatibility()
  }

  fn validate_outline(&self, outline: &HintOutline) -> Result<OutlineValidation, HintError> {
    let buffer_error = HintError {
      program: Program::Glyph,
      glyph_id: Some(outline.glyph_id),
      pc: 0,
      opcode: None,
      kind: HintErrorKind::InvalidOutlineBufferLength,
    };

    match ::core::primitive::bool::then_some(
      outline.twilight_original_scaled.len() != self.twilight_original_scaled.len()
        || outline.twilight_scaled.len() != self.twilight_scaled.len()
        || outline.twilight_flags.len() != self.twilight_flags.len()
        || outline.phantom.len() < 4,
      (),
    ) {
      ::core::option::Option::Some(()) => return Err(buffer_error),
      ::core::option::Option::None => {}
    }

    let Ok(phantom_start) = console_core_number_conversion::fitted::<_, u64>(
      match outline.scaled.len().checked_sub(4) {
        Some(start) => start,
        None => return Err(buffer_error),
      },
    );
    let twilight_count = {
      let Ok(count) =
        console_core_number_conversion::fitted::<_, u64>(outline.twilight_scaled.len());
      count
    };
    let twilight_count_2 = match u16::try_from(twilight_count) {
      Ok(count) => count,
      Err(_too_wide) => return Err(buffer_error),
    };
    Ok(OutlineValidation {
      buffer_error,
      phantom_start,
      twilight_count: twilight_count_2,
    })
  }

  pub fn hint(
    &self,
    font_program: &FontProgram,
    outline: &mut HintOutline,
    validation: crate::Validation,
  ) -> Result<(), HintError> {
    let OutlineValidation {
      buffer_error,
      phantom_start,
      twilight_count,
    } = self.validate_outline(outline)?;
    let twilight_contours = [twilight_count];

    let Ok(()) = self.restore_twilight(outline);
    let Ok(twilight) = Zone::new(
      &[],
      outline.twilight_original_scaled,
      outline.twilight_scaled,
      outline.twilight_flags,
      &twilight_contours,
    );

    let Ok(glyph) = Zone::new(
      outline.unscaled,
      outline.original_scaled,
      outline.scaled,
      outline.flags,
      outline.contours,
    );
    let Ok(value_stack) = ValueStack::new(outline.stack, validation);
    let cvt = match CowSlice::new(&self.cvt, outline.cvt) {
      Ok(cvt) => cvt,
      Err(_length_mismatch) => return Err(buffer_error),
    };
    let storage = match CowSlice::new(&self.storage, outline.storage) {
      Ok(storage) => storage,
      Err(_length_mismatch) => return Err(buffer_error),
    };
    let Ok(program) = ProgramState::new(
      ProgramBytes {
        font: font_program.fpgm,
        control_value: font_program.prep,
        glyph: outline.bytecode,
      },
      Program::Glyph,
    );
    let Ok(mut engine) = Engine::new(EngineInputs {
      font_program,
      program,
      graphics: self.graphics,
      definitions: DefinitionState {
        functions: DefinitionMap::Ref(&self.functions),
        instructions: DefinitionMap::Ref(&self.instructions),
      },
      cvt: super::cvt::Cvt::from(cvt),
      storage: super::storage::Storage::from(storage),
      value_stack,
      twilight,
      glyph,
      axis_count: self.axis_count,
      coords: outline.coords,
      glyph_kind: outline.glyph_kind,
    });
    engine
      .run_program(Program::Glyph, validation)
      .map_err(|mut e| {
        e.glyph_id = Some(outline.glyph_id);
        e
      })?;

    let Ok(compatibility) = engine.backward_compatibility();
    let backward_compatibility = matches!(compatibility, crate::BackwardCompatibility::Enabled);

    match ::core::primitive::bool::then_some(!backward_compatibility, ()) {
      ::core::option::Option::Some(()) => {
        let phantom_points = match outline.scaled.get({
          let Ok(range) = console_core_number_conversion::indices_from(phantom_start..);
          range
        }) {
          Some(points) => points,
          None => return Err(buffer_error),
        };

        for (destination, source) in outline.phantom.iter_mut().zip(phantom_points.iter()) {
          *destination = *source;
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  fn restore_twilight(&self, outline: &mut HintOutline) -> Result<(), Never> {
    for (destination, source) in outline
      .twilight_original_scaled
      .iter_mut()
      .zip(&self.twilight_original_scaled)
    {
      *destination = *source;
    }

    for (destination, source) in outline
      .twilight_scaled
      .iter_mut()
      .zip(&self.twilight_scaled)
    {
      *destination = *source;
    }

    for (destination, source) in outline.twilight_flags.iter_mut().zip(&self.twilight_flags) {
      *destination = *source;
    }

    Ok(())
  }

  fn reset_storage(&mut self, font_program: &FontProgram) -> Result<(), Never> {
    self.storage.clear();
    self
      .storage
      .extend((0..font_program.max_storage).map(|_| 0));

    let twilight_points = 0..font_program.max_twilight_points;
    self.twilight_scaled.clear();
    self
      .twilight_scaled
      .extend(twilight_points.clone().map(|_| Point::default()));
    self.twilight_original_scaled.clear();
    self
      .twilight_original_scaled
      .extend(twilight_points.clone().map(|_| Point::default()));
    self.twilight_flags.clear();
    self
      .twilight_flags
      .extend(twilight_points.map(|_| PointFlags::default()));
    Ok(())
  }

  fn setup(
    &mut self,
    font_program: &FontProgram,
    scale: i32,
    coords: &[F2Dot14],
  ) -> Result<(), console_core_never::Never> {
    let axis_count = font_program.axis_count;
    self.functions.clear();
    self
      .functions
      .extend((0..font_program.max_function_defs).map(|_| Definition::default()));

    let Ok(max_instructions) =
      console_core_number_conversion::index(font_program.max_instruction_defs);
    self.instructions.truncate(max_instructions);
    let Ok(previous_count) =
      console_core_number_conversion::fitted::<_, u64>(self.instructions.len());
    let missing_instructions =
      u64::from(font_program.max_instruction_defs).saturating_sub(previous_count);
    self
      .instructions
      .extend((0..missing_instructions).map(|_| Definition::default()));
    self.cvt.clear();
    let cvt = match font_program.font.cvt() {
      Ok(table) => table,
      Err(_absent_or_malformed_control_values) => Default::default(),
    };

    match font_program.font.cvar() {
      Ok(cvar) => {
        self.cvt.extend(cvt.iter().map(|_| 0));

        match cvar.deltas(axis_count, coords, &mut self.cvt) {
          Ok(()) => {}
          Err(_malformed_variation_data) => self.cvt.fill(0),
        }

        for (value, base_value) in self.cvt.iter_mut().zip(cvt.iter()) {
          let Ok(delta) = {
            let Ok(value_3) = {
              let Ok(value_2) = Fixed::from_bits(*value);
              value_2
            }
            .to_f26dot6();
            value_3
          }
          .to_bits();
          let base_value_2 = i32::wrapping_mul(
            {
              let Ok(value_5) = console_core_number_conversion::wrapped::<_, i32>({
                let Ok(value_4) = base_value.get();
                value_4
              });
              value_5
            },
            64,
          );
          *value = i32::wrapping_add(base_value_2, delta);
        }
      }
      Err(_missing_or_malformed_variation_values) => {
        self.cvt.extend(cvt.iter().map(|value| {
          i32::wrapping_mul(
            {
              let Ok(bits) = value.get();
              i32::from(bits)
            },
            64,
          )
        }));
      }
    }

    let Ok(scale_2) = Fixed::from_bits(i32::wrapping_shr(scale, 6_u32));

    for value in &mut self.cvt {
      *value = {
        let Ok(value_7) = ({
          let Ok(value_6) = Fixed::from_bits(*value);
          value_6
        } * (scale_2))
          .to_bits();
        value_7
      };
    }

    let Ok(()) = self.reset_storage(font_program);
    self.axis_count = axis_count;
    self.max_stack = font_program.max_stack_elements;
    self.graphics = RetainedGraphicsState::default();

    Ok(())
  }
}

#[cfg(any(test, feature = "scaler_test"))]
impl HintInstance {
  pub fn simulate_prep_flag_suppress_hinting(&mut self) -> Result<(), console_core_never::Never> {
    self.graphics.instruct_control |= 1;

    Ok(())
  }
}

struct OutlineValidation {
  buffer_error: HintError,
  phantom_start: u64,
  twilight_count: u16,
}

#[cfg(test)]
mod tests {
  use super::{super::FontProgram, HintInstance};
  use crate::{HintError, HintErrorKind, HintOutline};
  use read_fonts::{
    FontRef,
    types::{F2Dot14, F26Dot6, GlyphId, Point},
  };

  #[test]
  fn invalid_caller_buffers_report_errors() -> Result<(), read_fonts::ReadError> {
    let font = FontRef::new(font_test_data::CVAR)?;
    let font_program = FontProgram::new(&font)?;
    let mut scaled = [Point::<F26Dot6>::default(); 4];
    let mut phantom = [Point::<F26Dot6>::default(); 4];
    let mut outline = HintOutline {
      glyph_id: ({
        let Ok(value) = GlyphId::new(0);
        value
      }),
      unscaled: &[],
      scaled: &mut scaled,
      original_scaled: &mut [],
      flags: &mut [],
      contours: &[],
      phantom: &mut phantom,
      bytecode: &[],
      stack: &mut [],
      cvt: &mut [],
      storage: &mut [],
      twilight_scaled: &mut [],
      twilight_original_scaled: &mut [],
      twilight_flags: &mut [],
      glyph_kind: crate::GlyphKind::Simple,
      coords: &[],
    };

    for instance in [
      HintInstance {
        cvt: vec![0],
        ..Default::default()
      },
      HintInstance {
        storage: vec![0],
        ..Default::default()
      },
      HintInstance {
        twilight_scaled: vec![Point::default()],
        ..Default::default()
      },
    ] {
      assert!(matches!(
        instance.hint(&font_program, &mut outline, crate::Validation::Compatible),
        Err(HintError {
          kind: HintErrorKind::InvalidOutlineBufferLength,
          ..
        })
      ));
    }

    let mut short_outline = HintOutline {
      scaled: &mut [],
      ..outline
    };
    let instance = HintInstance::default();
    assert!(matches!(
      instance.hint(
        &font_program,
        &mut short_outline,
        crate::Validation::Compatible
      ),
      Err(HintError {
        kind: HintErrorKind::InvalidOutlineBufferLength,
        ..
      })
    ));
    Ok(())
  }

  #[test]
  fn scaled_cvar_cvt() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let font = FontRef::new(font_test_data::CVAR).required()?;
    let font_program = FontProgram::new(&font).required()?;
    let mut instance = HintInstance::default();
    let coords = [0.5, -0.5].map(|input| {
      let Ok(value) = F2Dot14::from_f32(input);
      value
    });
    let ppem = 16;

    let scale = 67109;
    instance
      .reconfigure(
        &font_program,
        crate::Scale {
          factor: ({
            let Ok(value) = raw::types::Fixed::from_bits(scale);
            value
          }),
          ppem,
        },
        Default::default(),
        &coords,
      )
      .required()?;
    let expected = [
      778, 10, 731, 0, 731, 10, 549, 10, 0, 0, 0, -10, 0, -10, -256, -10, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 95, 137, 99, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 60, 0, 81, 0, 0, 0, 0, 0, 51, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(&instance.cvt, &expected);

    Ok(())
  }
}
