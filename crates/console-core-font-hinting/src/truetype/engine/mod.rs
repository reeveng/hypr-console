//! TrueType bytecode interpreter.

mod arith;
mod control_flow;
mod cvt;
mod data;
mod definition;
mod delta;
mod dispatch;
mod graphics;
mod logical;
mod misc;
mod outline;
mod round;
mod stack;
mod storage;

use console_core_never::Never;
use crate::GlyphKind;

use read_fonts::{
  tables::glyf::bytecode::Instruction,
  types::{F2Dot14, F26Dot6, Point},
};

use super::{
  FontProgram,
  cvt::Cvt,
  definition::DefinitionState,
  error::{HintError, HintErrorKind},
  graphics::{GraphicsState, RetainedGraphicsState},
  math,
  program::ProgramState,
  storage::Storage,
  value_stack::ValueStack,
  zone::Zone,
};

pub type OpResult = Result<(), HintErrorKind>;

pub struct Engine<'a> {
  program: ProgramState<'a>,
  graphics: GraphicsState<'a>,
  definitions: DefinitionState<'a>,
  cvt: Cvt<'a>,
  storage: Storage<'a>,
  value_stack: ValueStack<'a>,
  loop_budget: LoopBudget,
  axis_count: u16,
  coords: &'a [F2Dot14],
}

pub struct EngineInputs<'a, 'f, 'p> {
  pub font_program: &'f FontProgram<'p>,
  pub program: ProgramState<'a>,
  pub graphics: RetainedGraphicsState,
  pub definitions: DefinitionState<'a>,
  pub cvt: Cvt<'a>,
  pub storage: Storage<'a>,
  pub value_stack: ValueStack<'a>,
  pub twilight: Zone<'a>,
  pub glyph: Zone<'a>,
  pub axis_count: u16,
  pub coords: &'a [F2Dot14],
  pub glyph_kind: GlyphKind,
}

impl<'a> Engine<'a> {
  pub fn new(inputs: EngineInputs<'a, '_, '_>) -> Result<Self, Never> {
    let point_count = match ::core::primitive::bool::then_some(inputs.glyph.points.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => {
        let Ok(count) =
          console_core_number_conversion::fitted::<_, u32>(inputs.glyph.points.len());
        Some(count)
      }
    };
    let graphics = GraphicsState {
      retained: inputs.graphics,
      zones: [inputs.twilight, inputs.glyph],
      glyph_kind: inputs.glyph_kind,
      ..Default::default()
    };
    let Ok(loop_budget) = LoopBudget::new(inputs.font_program, point_count);
    Ok(Self {
      program: inputs.program,
      graphics,
      definitions: inputs.definitions,
      cvt: inputs.cvt,
      storage: inputs.storage,
      value_stack: inputs.value_stack,
      loop_budget,
      axis_count: inputs.axis_count,
      coords: inputs.coords,
    })
  }

  pub fn backward_compatibility(&self) -> Result<crate::BackwardCompatibility, Never> {
    Ok(match self.graphics.backward_compatibility {
      crate::BackwardCompatibility::Enabled => crate::BackwardCompatibility::Enabled,
      crate::BackwardCompatibility::Disabled => crate::BackwardCompatibility::Disabled,
    })
  }

  pub fn retained_graphics_state(&self) -> Result<&RetainedGraphicsState, Never> {
    Ok(&self.graphics.retained)
  }
}

struct LoopBudget {
  limit: u64,
  backward_jumps: u64,
  loop_calls: u64,
}

impl LoopBudget {
  fn new(font_program: &FontProgram, point_count: Option<u32>) -> Result<Self, Never> {
    let cvt_len = u64::from(font_program.cvt_len);
    let limit = match point_count {
      Some(points) => u64::from(points)
        .saturating_mul(10)
        .max(50)
        .saturating_add(cvt_len.wrapping_div(10).max(50)),
      None => 300u64.saturating_add(cvt_len.saturating_mul(22)),
    };
    Ok(Self {
      limit,
      backward_jumps: 0,
      loop_calls: 0,
    })
  }

  fn reset(&mut self) -> Result<(), Never> {
    self.backward_jumps = 0;
    self.loop_calls = 0;
    Ok(())
  }

  fn doing_backward_jump(&mut self) -> Result<(), HintErrorKind> {
    self.backward_jumps = self.backward_jumps.saturating_add(1);

    match ::core::primitive::bool::then_some(self.backward_jumps > self.limit, ()) {
      ::core::option::Option::Some(()) => Err(HintErrorKind::ExceededExecutionBudget),
      ::core::option::Option::None => Ok(()),
    }
  }

  fn doing_loop_call(&mut self, count: u32) -> Result<(), HintErrorKind> {
    self.loop_calls = self.loop_calls.saturating_add(u64::from(count));

    match ::core::primitive::bool::then_some(self.loop_calls > self.limit, ()) {
      ::core::option::Option::Some(()) => Err(HintErrorKind::ExceededExecutionBudget),
      ::core::option::Option::None => Ok(()),
    }
  }
}

#[cfg(test)]
use mock::MockEngine;

#[cfg(test)]
mod mock {
  use super::{
    super::{
      Point, PointFlags,
      cow_slice::CowSlice,
      definition::{Definition, DefinitionMap, DefinitionState},
      program::{Program, ProgramBytes, ProgramState},
      zone::Zone,
    },
    Engine, F26Dot6, GraphicsState, LoopBudget, ValueStack,
  };

  pub(super) struct MockEngine {
    cvt_storage: Vec<i32>,
    value_stack: Vec<i32>,
    definitions: Vec<Definition>,
    unscaled: Vec<Point<i32>>,
    points: Vec<Point<F26Dot6>>,
    point_flags: Vec<PointFlags>,
    contours: Vec<u16>,
    twilight: Vec<Point<F26Dot6>>,
    twilight_flags: Vec<PointFlags>,
  }

  impl MockEngine {
    pub fn engine(&mut self) -> Result<Engine<'_>, console_core_never::Never> {
      let font_code = &[];
      let cv_code = &[];
      let glyph_code = &[];
      let (cvt, storage) = self.cvt_storage.split_at_mut(16);
      let (function_defs, instruction_defs) = self.definitions.split_at_mut(5);
      let definition = DefinitionState {
        functions: DefinitionMap::Mut(function_defs),
        instructions: DefinitionMap::Mut(instruction_defs),
      };

      for (i, point) in self.unscaled.iter_mut().enumerate() {
        let Ok(offset) = console_core_number_conversion::fitted::<_, i32>(i);
        point.x = 57i32.saturating_add(offset.saturating_mul(2));
        point.y = point.x.saturating_neg().saturating_mul(3);
      }

      let (points, original) = self.points.split_at_mut(32);
      let Ok(glyph_zone) = Zone::new(
        &self.unscaled,
        original,
        points,
        &mut self.point_flags,
        &self.contours,
      );
      let (points_2, original_2) = self.twilight.split_at_mut(16);
      let Ok(twilight_zone) = Zone::new(&[], original_2, points_2, &mut self.twilight_flags, &[]);
      let mut graphics_state = GraphicsState {
        zones: [twilight_zone, glyph_zone],
        ..Default::default()
      };
      let ::core::result::Result::Ok(()) = graphics_state.update_projection_state();
      let Ok(program) = ProgramState::new(
        ProgramBytes {
          font: font_code,
          control_value: cv_code,
          glyph: glyph_code,
        },
        Program::Font,
      );
      let Ok(value_stack) = ValueStack::new(&mut self.value_stack, crate::Validation::Compatible);
      Ok(Engine {
        graphics: graphics_state,
        cvt: <crate::truetype::cvt::Cvt<'_> as ::core::convert::From<
          crate::truetype::cow_slice::CowSlice<'_>,
        >>::from(CowSlice::from(cvt)),
        storage: <crate::truetype::storage::Storage<'_> as ::core::convert::From<
          crate::truetype::cow_slice::CowSlice<'_>,
        >>::from(CowSlice::from(storage)),
        value_stack,
        program,
        loop_budget: LoopBudget {
          limit: 10,
          backward_jumps: 0,
          loop_calls: 0,
        },
        definitions: definition,
        axis_count: 0,
        coords: &[],
      })
    }
  }

  impl Default for MockEngine {
    fn default() -> Self {
      Self {
        cvt_storage: vec![0; 32],
        value_stack: vec![0; 32],
        definitions: vec![Default::default(); 8],
        unscaled: vec![Default::default(); 32],
        points: vec![Default::default(); 64],
        point_flags: vec![Default::default(); 32],
        contours: vec![31],
        twilight: vec![Default::default(); 32],
        twilight_flags: vec![Default::default(); 32],
      }
    }
  }

  impl Engine<'_> {
    pub(super) fn test_exec(
      &mut self,
      push: &[i32],
      expected_result: impl Into<i32>,
      mut f: impl FnMut(&mut Engine) -> Result<(), crate::TestFailure>,
    ) -> Result<(), crate::TestFailure> {
      use crate::test_support::Required;

      for &operand in push {
        self.value_stack.push(operand).required()?;
      }

      f(self)?;
      let actual = self.value_stack.pop().required()?;
      assert_eq!(
        actual,
        <_ as ::core::convert::Into<::core::primitive::i32>>::into(expected_result)
      );
      Ok(())
    }
  }
}
