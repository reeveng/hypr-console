//! Parsing and evaluation of charstrings.

use crate::{
  Cursor, FontData, FontRead,
  model::pen::OutlinePen,
  ps::{
    cff::{
      blend::BlendState,
      charset::Charset,
      index::Index,
      stack::{OperandParity, Stack},
    },
    error::Error,
    num,
    string::Sid,
    transform::{FontMatrix, Transform},
  },
  tables::cff::Cff,
  types::{Fixed, Point},
};

pub const NESTING_DEPTH_LIMIT: u32 = 10;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CharstringKind {
  Type1,
  Type2,
}

pub trait CharstringContext {
  fn kind(&self) -> CharstringKind;

  fn seac_components(&self, base_code: i32, accent_code: i32) -> Result<[&[u8]; 2], Error>;

  fn global_subr(&self, index: i32) -> Result<&[u8], Error>;

  fn subr(&self, index: i32) -> Result<&[u8], Error>;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CharstringContext::weight_vector` trait method lets external implementations supply a borrowed value using its existing signature"
    )
  )]
  fn weight_vector(&self) -> &[Fixed] {
    &[]
  }
}

impl<'a> CharstringContext for (&'a [u8], &'a Index<'a>, &'a Index<'a>, &'a Index<'a>) {
  fn kind(&self) -> CharstringKind {
    CharstringKind::Type2
  }

  fn seac_components(&self, base_code: i32, accent_code: i32) -> Result<[&[u8]; 2], Error> {
    let Ok(value_1) = FontData::new(self.0);

    let cff = Cff::read(value_1)?;
    let cff_charset = cff.charset(0)?;
    let charset = cff_charset
      .or_else(
        || match Charset::new(FontData::default(), 0, self.1.count()) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        },
      )
      .ok_or(Error::MissingCharset)?;
    let seac_to_gid = |code: i32| {
      let code_2: u8 = match code.try_into() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      }?;
      let Ok(value_22) = console_core_number_conversion::index({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(code_2);
        migrated_value
      });

      let sid =
        ::core::option::Option::<&_>::copied(super::encoding::STANDARD_ENCODING.get(value_22))?;
      let Ok(sid_2) = console_core_number_conversion::wrapped::<_, u16>(sid);

      match charset.glyph_id(Sid::new(sid_2)) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }
    };
    let accent_gid = seac_to_gid(accent_code).ok_or(Error::InvalidSeacCode(accent_code))?;
    let base_gid = seac_to_gid(base_code).ok_or(Error::InvalidSeacCode(base_code))?;
    let Ok(value_2) = accent_gid.to_u32();

    let accent_charstring = self.1.get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_2);
      migrated_value
    })?;
    let Ok(value_3) = base_gid.to_u32();

    let base_charstring = self.1.get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_3);
      migrated_value
    })?;
    Ok([base_charstring, accent_charstring])
  }

  fn global_subr(&self, index: i32) -> Result<&[u8], Error> {
    self.2.get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        ::core::primitive::i32::wrapping_add(index, self.2.subr_bias()),
      );
      migrated_value
    })
  }

  fn subr(&self, index: i32) -> Result<&[u8], Error> {
    self.3.get({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        ::core::primitive::i32::wrapping_add(index, self.3.subr_bias()),
      );
      migrated_value
    })
  }
}

pub trait CommandSink {
  fn move_to(&mut self, x: Fixed, y: Fixed);

  fn line_to(&mut self, x: Fixed, y: Fixed);

  fn curve_to(&mut self, cx0: Fixed, cy0: Fixed, cx1: Fixed, cy1: Fixed, x: Fixed, y: Fixed);

  fn close(&mut self);

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CommandSink::hstem` trait method lets external implementations supply unit using its existing signature"
    )
  )]
  fn hstem(&mut self, _y: Fixed, _dy: Fixed) {}

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CommandSink::vstem` trait method lets external implementations supply unit using its existing signature"
    )
  )]
  fn vstem(&mut self, _x: Fixed, _dx: Fixed) {}

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CommandSink::hint_mask` trait method lets external implementations supply unit using its existing signature"
    )
  )]
  fn hint_mask(&mut self, _mask: &[u8]) {}

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CommandSink::counter_mask` trait method lets external implementations supply unit using its existing signature"
    )
  )]
  fn counter_mask(&mut self, _mask: &[u8]) {}

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CommandSink::clear_hints` trait method lets external implementations supply unit using its existing signature"
    )
  )]
  fn clear_hints(&mut self) {}

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CommandSink::finish` trait method lets external implementations supply unit using its existing signature"
    )
  )]
  fn finish(&mut self) {}
}

pub fn evaluate<'a>(
  context: &'a impl CharstringContext,
  blend_state: Option<BlendState<'a>>,
  charstring_data: &[u8],
  sink: &'a mut impl CommandSink,
) -> Result<Option<Fixed>, Error> {
  let Ok(mut evaluator) = Evaluator::new(context, blend_state, sink);
  evaluator.evaluate(charstring_data)?;
  let width = match evaluator.width_state {
    WidthState::Explicit | WidthState::RetainedForComponent => Some(evaluator.wx),
    WidthState::AwaitingCommand | WidthState::Implicit => None,
  };
  sink.finish();
  Ok(width)
}

#[derive(PartialEq)]
enum SeacMode {
  Explicit,
  Implicit,
}

enum OperatorFlow {
  Continue,
  Return,
  EndChar,
}

enum CharstringEnd {
  EndChar,
  NoEndChar,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContourStart {
  AwaitingMove,
  EmittedMove,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlexState {
  Inactive,
  GatheringPoints,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompositionState {
  SingleGlyph,
  DrawingComponents,
}

#[derive(Clone, Copy)]
enum WidthState {
  AwaitingCommand,
  Implicit,
  Explicit,
  RetainedForComponent,
}

struct Evaluator<'a, S> {
  context: &'a dyn CharstringContext,
  kind: CharstringKind,
  blend_state: Option<BlendState<'a>>,
  sink: &'a mut S,
  contour_start: ContourStart,
  flex_state: FlexState,
  width_state: WidthState,
  stem_count: u64,
  x: Fixed,
  y: Fixed,
  sbx: Fixed,
  wx: Fixed,
  stack: Stack,
  stack_ix: u64,
  composition: CompositionState,
}

impl<'a, S> Evaluator<'a, S>
where
  S: CommandSink,
{
  fn new(
    context: &'a dyn CharstringContext,
    blend_state: Option<BlendState<'a>>,
    sink: &'a mut S,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      context,
      kind: context.kind(),
      blend_state,
      sink,
      contour_start: ContourStart::AwaitingMove,
      flex_state: FlexState::Inactive,
      width_state: WidthState::AwaitingCommand,
      stem_count: 0,
      stack: Stack::new(),
      x: Fixed::ZERO,
      y: Fixed::ZERO,
      sbx: Fixed::ZERO,
      wx: Fixed::ZERO,
      stack_ix: 0,
      composition: CompositionState::SingleGlyph,
    })
  }

  fn evaluate(&mut self, charstring_data: &[u8]) -> Result<(), Error> {
    let end = self.evaluate_impl(charstring_data, 0)?;

    match (self.kind, end) {
      (CharstringKind::Type2, CharstringEnd::NoEndChar) => {
        let Ok(mut cursor) = crate::FontData::default().cursor();
        let _flow = self.evaluate_operator(Operator::EndChar, &mut cursor, 0)?;
      }
      (CharstringKind::Type1, _) | (_, CharstringEnd::EndChar) => {}
    }

    match ::core::primitive::bool::then_some(self.contour_start == ContourStart::EmittedMove, ())
    {
      ::core::option::Option::Some(()) => {
        self.sink.close();
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "each subroutine or seac component increments the depth, and this entry checks NESTING_DEPTH_LIMIT before interpreting bytes"
    )
  )]
  fn evaluate_impl(
    &mut self,
    charstring_data: &[u8],
    nesting_depth: u32,
  ) -> Result<CharstringEnd, Error> {
    match ::core::primitive::bool::then_some(nesting_depth > NESTING_DEPTH_LIMIT, ()) {
      ::core::option::Option::Some(()) => {
        return Err(Error::CharstringNestingDepthLimitExceeded);
      }
      ::core::option::Option::None => {}
    }

    let Ok(data) = crate::FontData::new(charstring_data);
    let Ok(mut cursor) = data.cursor();
    let Ok(byte_count) = cursor.remaining_bytes();

    for _opcode in 0..byte_count {
      let Ok(remaining) = cursor.remaining_bytes();

      match remaining {
        0 => break,
        _has_data => {}
      }

      let b0 = cursor.read::<u8>()?;

      let flow = match b0 {
        28 | 32..=254 => {
          let operand = num::parse_int(&mut cursor, b0)?;
          self.stack.push(operand)?;
          OperatorFlow::Continue
        }
        255 => {
          let signed_operand = cursor.read::<i32>()?;

          match self.kind {
            CharstringKind::Type1 => self.stack.push(signed_operand)?,
            CharstringKind::Type2 => {
              let Ok(fixed_operand) = Fixed::from_bits(signed_operand);
              self.stack.push(fixed_operand)?;
            }
          };

          OperatorFlow::Continue
        }
        _ => match Operator::read(&mut cursor, b0) {
          Ok(operator) => self.evaluate_operator(operator, &mut cursor, nesting_depth)?,
          Err(_unknown_charstring_operator) => {
            let Ok(()) = self.reset_stack();
            OperatorFlow::Continue
          }
        },
      };

      match flow {
        OperatorFlow::Continue => {}
        OperatorFlow::Return => return Ok(CharstringEnd::NoEndChar),
        OperatorFlow::EndChar => return Ok(CharstringEnd::EndChar),
      }
    }

    Ok(CharstringEnd::NoEndChar)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "subroutine operators enter evaluate_impl with one more level and seac rejects a second active composition"
    )
  )]
  fn evaluate_operator(
    &mut self,
    operator: Operator,
    cursor: &mut Cursor,
    nesting_depth: u32,
  ) -> Result<OperatorFlow, Error> {
    use Operator::*;
    use PointMode::*;

    match operator {
      Flex => {
        self.emit_curves([DxDy; 6])?;
        let Ok(()) = self.reset_stack();
      }

      HFlex => {
        self.emit_curves([DxY, DxDy, DxY, DxY, DxInitialY, DxY])?;
        let Ok(()) = self.reset_stack();
      }

      HFlex1 => {
        self.emit_curves([DxDy, DxDy, DxY, DxY, DxDy, DxInitialY])?;
        let Ok(()) = self.reset_stack();
      }

      Flex1 => {
        self.emit_curves([DxDy, DxDy, DxDy, DxDy, DxDy, DLargerCoordDist])?;
        let Ok(()) = self.reset_stack();
      }

      VariationStoreIndex => match self.kind {
        CharstringKind::Type2 => {
          let blend_state = self.blend_state.as_mut().ok_or(Error::MissingBlendState)?;
          let store_index = {
            let pop_i32 = self.stack.pop_i32()?;
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(pop_i32);
            migrated_value
          };
          blend_state.set_store_index(store_index)?;
        }
        CharstringKind::Type1 => {}
      },

      Blend => match self.kind {
        CharstringKind::Type2 => {
          let blend_state = self.blend_state.as_ref().ok_or(Error::MissingBlendState)?;
          self.stack.apply_blend(blend_state)?;
        }
        CharstringKind::Type1 => {}
      },

      Return => return Ok(OperatorFlow::Return),

      EndChar => {
        let stack_len = self.stack.len();

        match ::core::primitive::bool::then_some(
          (stack_len == 1 || stack_len == 5)
            && matches!(
              self.width_state,
              WidthState::AwaitingCommand | WidthState::RetainedForComponent
            ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            self.read_width()?;
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = self.record_width_command();

        match ::core::primitive::bool::then_some(stack_len > 1, ()) {
          ::core::option::Option::Some(()) => {
            self.handle_seac(SeacMode::Implicit, nesting_depth)?;
          }
          ::core::option::Option::None => {}
        }

        return Ok(OperatorFlow::EndChar);
      }

      HStem | VStem | HStemHm | VStemHm => {
        let mut first_operand = 0;
        let len = match ::core::primitive::bool::then_some(
          matches!(self.stack.operand_parity(), OperandParity::Odd)
            && matches!(
              self.width_state,
              WidthState::AwaitingCommand | WidthState::RetainedForComponent
            ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            self.read_width()?;
            first_operand = 1;
            ::core::primitive::u64::wrapping_sub(self.stack.len(), 1)
          }
          ::core::option::Option::None => self.stack.len(),
        };
        let Ok(()) = self.record_width_command();
        let is_horizontal = matches!(operator, HStem | HStemHm);
        let mut u = Fixed::ZERO;

        for pair_index in (first_operand..self.stack.len()).step_by(2) {
          let args = self.stack.fixed_array::<2>(pair_index)?;
          u += *match &args {
            [element, ..] => element,
          };
          let [_, w, ..] = args;
          let v = u.wrapping_add(w);

          match ::core::primitive::bool::then_some(is_horizontal, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(value_6) = v;

              self.sink.hstem(u, value_6);
            }
            ::core::option::Option::None => {
              let Ok(value_7) = v;

              self.sink.vstem(u, value_7);
            }
          }

          let Ok(value_8) = v;

          u = value_8;
        }

        self.stem_count = self
          .stem_count
          .wrapping_add(::core::primitive::u64::wrapping_div(len, 2));

        let Ok(()) = self.reset_stack();
      }

      HintMask | CntrMask => {
        let mut first_operand = 0;
        let len = match ::core::primitive::bool::then_some(
          matches!(self.stack.operand_parity(), OperandParity::Odd)
            && matches!(
              self.width_state,
              WidthState::AwaitingCommand | WidthState::RetainedForComponent
            ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            self.read_width()?;
            first_operand = 1;
            ::core::primitive::u64::wrapping_sub(self.stack.len(), 1)
          }
          ::core::option::Option::None => self.stack.len(),
        };
        let Ok(()) = self.record_width_command();
        let mut u = Fixed::ZERO;

        for pair_index in (first_operand..self.stack.len()).step_by(2) {
          let args = self.stack.fixed_array::<2>(pair_index)?;
          u += *match &args {
            [element, ..] => element,
          };
          let [_, w, ..] = args;
          let v = u + w;
          self.sink.vstem(u, v);
          u = v;
        }

        self.stem_count = self
          .stem_count
          .wrapping_add(::core::primitive::u64::wrapping_div(len, 2));

        let count = self.stem_count.div_ceil(8);
        let mask = cursor.read_array::<u8>(count)?;

        match ::core::primitive::bool::then_some(operator == HintMask, ()) {
          ::core::option::Option::Some(()) => {
            self.sink.hint_mask(mask);
          }
          ::core::option::Option::None => {
            self.sink.counter_mask(mask);
          }
        }

        let Ok(()) = self.reset_stack();
      }

      RMoveTo => {
        match ::core::primitive::bool::then_some(
          self.stack.len() > 2
            && matches!(
              self.width_state,
              WidthState::AwaitingCommand | WidthState::RetainedForComponent
            ),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            self.read_width()?;
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = self.record_width_command();

        match ::core::primitive::bool::then_some(self.flex_state == FlexState::Inactive, ()) {
          ::core::option::Option::Some(()) => {
            let dy = self.stack.pop_fixed()?;
            let dx = self.stack.pop_fixed()?;
            self.x += dx;
            self.y += dy;

            let Ok(()) = self.begin_contour();
          }
          ::core::option::Option::None => {}
        }
      }

      HMoveTo | VMoveTo => self.move_single_axis(operator)?,

      RLineTo => {
        for i in (0..self.stack.len()).step_by(2) {
          let [dx, dy] = self.stack.fixed_array::<2>(i)?;
          self.x += dx;
          self.y += dy;
          let Ok(()) = self.emit_line(self.x, self.y);
        }

        let Ok(()) = self.reset_stack();
      }

      HLineTo | VLineTo => {
        let mut is_x = operator == HLineTo;

        for delta in self.stack.fixed_values() {
          match ::core::primitive::bool::then_some(is_x, ()) {
            ::core::option::Option::Some(()) => {
              self.x += delta;
            }
            ::core::option::Option::None => {
              self.y += delta;
            }
          }

          is_x = !is_x;
          let Ok(()) = Self::open_contour(&mut self.contour_start, self.sink);
          self.sink.line_to(self.x, self.y);
        }

        let Ok(()) = self.reset_stack();
      }

      HhCurveTo => self.draw_axis_curves(CurveAxis::Horizontal)?,

      HvCurveTo | VhCurveTo => {
        let count1 = self.stack.len();
        let count = count1 & !2;
        let mut is_horizontal = operator == HvCurveTo;
        self.stack_ix = ::core::primitive::u64::wrapping_sub(count1, count);

        for _ in (self.stack_ix..count).step_by(4) {
          match self.stack_ix.cmp(&count) {
            core::cmp::Ordering::Less => {}
            core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
          }

          let do_last_delta = ::core::primitive::u64::wrapping_sub(count, self.stack_ix) == 5;

          match ::core::primitive::bool::then_some(is_horizontal, ()) {
            ::core::option::Option::Some(()) => {
              self.emit_curves([DxY, DxDy, MaybeDxDy(do_last_delta)])?;
            }
            ::core::option::Option::None => {
              self.emit_curves([XDy, DxDy, DxMaybeDy(do_last_delta)])?;
            }
          }

          is_horizontal = !is_horizontal;
        }

        let Ok(()) = self.reset_stack();
      }

      RrCurveTo | RCurveLine => {
        let Ok(remaining) = self.coords_remaining();

        for _ in 0..remaining.wrapping_div(6) {
          self.emit_curves([DxDy; 3])?;
        }

        match ::core::primitive::bool::then_some(operator == RCurveLine, ()) {
          ::core::option::Option::Some(()) => {
            let [dx, dy] = self.stack.fixed_array::<2>(self.stack_ix)?;
            self.x += dx;
            self.y += dy;
            let Ok(()) = self.emit_line(self.x, self.y);
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = self.reset_stack();
      }

      RLineCurve => {
        let Ok(remaining) = self.coords_remaining();
        let line_count = remaining.saturating_sub(6).div_ceil(2);

        for _ in 0..line_count {
          let [dx, dy] = self.stack.fixed_array::<2>(self.stack_ix)?;
          self.x += dx;
          self.y += dy;
          let Ok(()) = self.emit_line(self.x, self.y);

          self.stack_ix = self.stack_ix.wrapping_add(2);
        }

        let Ok(curve_operands) = self.coords_remaining();

        for _ in 0..curve_operands.wrapping_div(6) {
          self.emit_curves([DxDy; 3])?;
        }

        let Ok(()) = self.reset_stack();
      }

      VvCurveTo => self.draw_axis_curves(CurveAxis::Vertical)?,

      CallSubr | CallGsubr => {
        let index = self.stack.pop_i32()?;
        let subr_charstring = match ::core::primitive::bool::then_some(operator == CallSubr, ()) {
          ::core::option::Option::Some(()) => self.context.subr(index)?,
          ::core::option::Option::None => self.context.global_subr(index)?,
        };
        self.evaluate_impl(
          subr_charstring,
          ::core::primitive::u32::wrapping_add(nesting_depth, 1),
        )?;
      }

      Hsbw => match self.kind {
        CharstringKind::Type1 => {
          let [sbx, wx] = self.stack.fixed_array(0)?;
          self.sbx += sbx;
          self.x += sbx;
          self.wx = wx;
          self.width_state = WidthState::Explicit;
          let Ok(()) = self.reset_stack();
        }
        CharstringKind::Type2 => {}
      },

      Seac => {
        self.handle_seac(SeacMode::Explicit, nesting_depth)?;
      }

      Sbw => match self.kind {
        CharstringKind::Type1 => {
          let [x, y, wx, _wy] = self.stack.fixed_array(0)?;
          self.x += x;
          self.y += y;
          self.sbx += x;
          self.wx = wx;
          self.width_state = WidthState::Explicit;
          let Ok(()) = self.reset_stack();
        }
        CharstringKind::Type2 => {}
      },

      DotSection => {}

      HStem3 | VStem3 => {
        let Ok(()) = self.reset_stack();
      }

      Div => {
        self.stack.div(self.kind)?;
      }

      CallOtherSubr => {
        let subr_idx = self.stack.pop_i32()?;
        let num_args = {
          let pop_i32 = self.stack.pop_i32()?;
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(pop_i32);
          migrated_value
        };
        let weight_vector = self.context.weight_vector();

        match (subr_idx, num_args) {
          (0, 3) => {
            self.flex_state = FlexState::Inactive;
            let Ok(()) = self.ensure_open();
            self.handle_flex()?;
          }

          (1, 0) => {
            self.flex_state = FlexState::GatheringPoints;
          }

          (12 | 13, _) => {
            let Ok(()) = self.reset_stack();
          }

          (14..=18, _) => match weight_vector {
            [_, _, ..] => self.handle_mm_blend(subr_idx, num_args)?,
            [] | [_] => self.stack.drop(num_args),
          },
          _ => {
            self.stack.drop(num_args);
          }
        }
      }

      Pop => {}

      SetCurrentPoint => match self.kind {
        CharstringKind::Type1 => {
          let [x, y] = self.stack.fixed_array(0)?;
          self.x = x;
          self.y = y;
          let Ok(()) = self.reset_stack();
        }
        CharstringKind::Type2 => {}
      },
    }

    Ok(OperatorFlow::Continue)
  }

  fn record_width_command(&mut self) -> Result<(), console_core_never::Never> {
    self.width_state = match self.width_state {
      WidthState::AwaitingCommand | WidthState::Implicit => WidthState::Implicit,
      WidthState::Explicit | WidthState::RetainedForComponent => WidthState::Explicit,
    };

    Ok(())
  }

  fn read_width(&mut self) -> Result<(), Error> {
    let fixed = self.stack.get_fixed(0)?;
    self.wx = fixed;
    self.width_state = WidthState::Explicit;
    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "seac prevents nesting another composition and visits two component charstrings through the checked nesting budget"
    )
  )]
  fn handle_seac(&mut self, mode: SeacMode, nesting_depth: u32) -> Result<(), Error> {
    match ::core::primitive::bool::then_some(
      self.composition == CompositionState::DrawingComponents,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(Error::CharstringNestingDepthLimitExceeded);
      }
      ::core::option::Option::None => {}
    }

    self.composition = CompositionState::DrawingComponents;
    let accent_code = self.stack.pop_i32()?;
    let base_code = self.stack.pop_i32()?;
    let [base_charstring, accent_charstring] =
      self.context.seac_components(base_code, accent_code)?;
    let dy = self.stack.pop_fixed()?;
    let dx = self.stack.pop_fixed()?;
    let sb = match self.kind {
      CharstringKind::Type1 => self.stack.pop_fixed()?,
      CharstringKind::Type2 => match ::core::primitive::bool::then_some(
        matches!(self.stack.is_empty(), crate::Emptiness::NonEmpty)
          && matches!(
            self.width_state,
            WidthState::AwaitingCommand | WidthState::RetainedForComponent
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let pop_fixed = self.stack.pop_fixed()?;
          self.wx = pop_fixed;
          let Ok(()) = self.record_width_command();
          Fixed::ZERO
        }
        ::core::option::Option::None => Fixed::ZERO,
      },
    };

    let mut sbx = self.sbx;
    let mut wx = self.wx;
    let saved_width = self.width_state;

    struct Component<'a> {
      charstring: &'a [u8],
      x: Fixed,
      y: Fixed,
      maybe_use_metrics: bool,
    }

    let x = self.x;
    let y = self.y;

    let [bx, by] = match ::core::primitive::bool::then_some(mode == SeacMode::Explicit, ()) {
      ::core::option::Option::Some(()) => [Fixed::ZERO; 2],
      ::core::option::Option::None => [x, y],
    };
    let base = Component {
      charstring: base_charstring,
      x: bx,
      y: by,
      maybe_use_metrics: mode == SeacMode::Explicit,
    };
    let accent = Component {
      charstring: accent_charstring,
      x: dx + self.sbx - sb,
      y: dy,
      maybe_use_metrics: false,
    };
    let components = match mode {
      SeacMode::Implicit => [accent, base],
      SeacMode::Explicit => [base, accent],
    };

    for component in components {
      let Ok(()) = self.reset_stack();
      self.width_state = match self.width_state {
        WidthState::Explicit | WidthState::RetainedForComponent => {
          WidthState::RetainedForComponent
        }
        WidthState::AwaitingCommand | WidthState::Implicit => WidthState::AwaitingCommand,
      };
      self.sink.clear_hints();
      self.stem_count = 0;
      self.x = component.x;
      self.y = component.y;
      self.evaluate_impl(
        component.charstring,
        ::core::primitive::u32::wrapping_add(nesting_depth, 1),
      )?;

      match ::core::primitive::bool::then_some(
        component.maybe_use_metrics
          && matches!(
            saved_width,
            WidthState::AwaitingCommand | WidthState::RetainedForComponent
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          sbx = self.sbx;
          wx = self.wx;
        }
        ::core::option::Option::None => {}
      }
    }

    self.width_state = saved_width;
    self.sbx = sbx;
    self.wx = wx;
    self.composition = CompositionState::SingleGlyph;
    Ok(())
  }

  fn handle_flex(&mut self) -> Result<(), Error> {
    let final_y = self.stack.pop_fixed()?;
    let final_x = self.stack.pop_fixed()?;

    let _ = self.stack.pop_fixed()?;
    let p3y = self.stack.pop_fixed()?;
    let p3x = self.stack.pop_fixed()?;
    let bcp4y = self.stack.pop_fixed()?;
    let bcp4x = self.stack.pop_fixed()?;
    let bcp3y = self.stack.pop_fixed()?;
    let bcp3x = self.stack.pop_fixed()?;
    let p2y = self.stack.pop_fixed()?;
    let p2x = self.stack.pop_fixed()?;
    let bcp2y = self.stack.pop_fixed()?;
    let bcp2x = self.stack.pop_fixed()?;
    let bcp1y = self.stack.pop_fixed()?;
    let bcp1x = self.stack.pop_fixed()?;
    let rpy = self.stack.pop_fixed()?;
    let rpx = self.stack.pop_fixed()?;
    let Ok(()) = self.reset_stack();
    self.stack.push(bcp1x + rpx)?;
    self.stack.push(bcp1y + rpy)?;
    self.stack.push(bcp2x)?;
    self.stack.push(bcp2y)?;
    self.stack.push(p2x)?;
    self.stack.push(p2y)?;
    self.emit_curves([PointMode::DxDy; 3])?;
    let Ok(()) = self.reset_stack();
    self.stack.push(bcp3x)?;
    self.stack.push(bcp3y)?;
    self.stack.push(bcp4x)?;
    self.stack.push(bcp4y)?;
    self.stack.push(p3x)?;
    self.stack.push(p3y)?;
    self.emit_curves([PointMode::DxDy; 3])?;
    let Ok(()) = self.reset_stack();

    self.stack.push(final_x)?;
    self.stack.push(final_y)?;
    Ok(())
  }

  fn handle_mm_blend(&mut self, subr_idx: i32, num_args: u64) -> Result<(), Error> {
    let weight_vector = self.context.weight_vector();
    let num_points = ::core::primitive::u64::wrapping_add(
      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
          ::core::primitive::i32::wrapping_sub(subr_idx, 13),
        );
        migrated_value
      },
      u64::from(subr_idx == 18),
    );
    let Ok(value_24) = console_core_number_conversion::fitted::<_, u64>(weight_vector.len());

    match ::core::primitive::bool::then_some(
      num_args != ::core::primitive::u64::wrapping_mul(num_points, value_24),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(Error::Read(crate::ReadError::MalformedData(
          "incorrect number of multiple masters arguments",
        )));
      }
      ::core::option::Option::None => {}
    }

    let stack_base = self
      .stack
      .len()
      .checked_sub(num_args)
      .ok_or(Error::StackUnderflow)?;
    let mut delta_idx = ::core::primitive::u64::wrapping_add(stack_base, num_points);

    for i in 0..num_points {
      let mut blended_operand = self
        .stack
        .get_fixed(::core::primitive::u64::wrapping_add(stack_base, i))?;

      for &weight in weight_vector.iter().skip(1) {
        blended_operand += {
          let checked_value = self.stack.get_fixed(delta_idx)?;
          checked_value
        } * weight;
        delta_idx = ::core::primitive::u64::wrapping_add(delta_idx, 1);
      }

      self.stack.set(
        ::core::primitive::u64::wrapping_add(stack_base, i),
        blended_operand,
      )?;
    }

    self.stack.drop(num_args.saturating_sub(num_points));
    Ok(())
  }

  #[must_use]
  fn coords_remaining(&self) -> core::result::Result<u64, console_core_never::Never> {
    Ok(self.stack.len().saturating_sub(self.stack_ix))
  }

  fn ensure_open(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(()) = Self::open_contour(&mut self.contour_start, self.sink);
    };
    Ok(())
  }

  fn open_contour(
    contour_start: &mut ContourStart,
    sink: &mut S,
  ) -> Result<(), console_core_never::Never> {
    match contour_start {
      ContourStart::AwaitingMove => {
        sink.move_to(Fixed::ZERO, Fixed::ZERO);
        *contour_start = ContourStart::EmittedMove;
      }
      ContourStart::EmittedMove => {}
    }

    Ok(())
  }

  fn draw_axis_curves(&mut self, axis: CurveAxis) -> Result<(), Error> {
    let operand_count = self.stack.len();
    let complete_count = operand_count & !2;
    self.stack_ix = operand_count.wrapping_sub(complete_count);
    let control = match axis {
      CurveAxis::Horizontal => PointMode::DxY,
      CurveAxis::Vertical => PointMode::XDy,
    };

    for _ in (self.stack_ix..complete_count).step_by(4) {
      match self.stack_ix.cmp(&complete_count) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
      }

      match (complete_count.wrapping_sub(self.stack_ix) & 1).cmp(&0) {
        core::cmp::Ordering::Greater => {
          let perpendicular_delta = self.stack.get_fixed(self.stack_ix)?;

          match axis {
            CurveAxis::Horizontal => self.y += perpendicular_delta,
            CurveAxis::Vertical => self.x += perpendicular_delta,
          }

          self.stack_ix = self.stack_ix.wrapping_add(1);
        }
        core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
      }

      self.emit_curves([control, PointMode::DxDy, control])?;
    }

    let Ok(()) = self.reset_stack();
    Ok(())
  }

  fn move_single_axis(&mut self, operator: Operator) -> Result<(), Error> {
    match ::core::primitive::bool::then_some(
      self.stack.len() > 1
        && matches!(
          self.width_state,
          WidthState::AwaitingCommand | WidthState::RetainedForComponent
        ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        self.read_width()?;
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.record_width_command();

    match ::core::primitive::bool::then_some(self.flex_state == FlexState::GatheringPoints, ()) {
      ::core::option::Option::Some(()) => {
        self.stack.push(0)?;

        match ::core::primitive::bool::then_some(operator == Operator::VMoveTo, ()) {
          ::core::option::Option::Some(()) => {
            self.stack.exch()?;
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {
        let delta = self.stack.pop_fixed()?;

        match ::core::primitive::bool::then_some(operator == Operator::HMoveTo, ()) {
          ::core::option::Option::Some(()) => {
            self.x += delta;
          }
          ::core::option::Option::None => {
            self.y += delta;
          }
        }

        let Ok(()) = self.begin_contour();
      }
    }

    Ok(())
  }

  fn begin_contour(&mut self) -> Result<(), console_core_never::Never> {
    match self.contour_start {
      ContourStart::AwaitingMove => self.contour_start = ContourStart::EmittedMove,
      ContourStart::EmittedMove => self.sink.close(),
    }

    self.sink.move_to(self.x, self.y);
    self.reset_stack()
  }

  fn emit_line(
    &mut self,
    x: Fixed,
    y: Fixed,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.ensure_open();
      self.sink.line_to(x, y);
    };
    Ok(())
  }

  fn emit_curves<const N: usize>(&mut self, modes: [PointMode; N]) -> Result<(), Error> {
    let Ok(initial) = Point::new(self.x, self.y);
    let Ok(()) = self.ensure_open();
    let mut controls = CurveControls::Empty;

    for mode in modes {
      self.apply_curve_delta(mode, initial)?;

      controls = match controls {
        CurveControls::Empty => {
          let Ok(first) = Point::new(self.x, self.y);
          CurveControls::First(first)
        }
        CurveControls::First(first) => {
          let Ok(second) = Point::new(self.x, self.y);
          CurveControls::Both([first, second])
        }
        CurveControls::Both([first, second]) => {
          self
            .sink
            .curve_to(first.x, first.y, second.x, second.y, self.x, self.y);
          CurveControls::Empty
        }
      };
    }

    Ok(())
  }

  fn apply_curve_delta(&mut self, mode: PointMode, initial: Point<Fixed>) -> Result<(), Error> {
    use PointMode::*;
    let delta = self.stack.get_fixed(self.stack_ix)?;
    let operands = match mode {
      DxDy | DxMaybeDy(true) => {
        self.x += delta;
        let dy = self.stack.get_fixed(self.stack_ix.wrapping_add(1))?;
        self.y += dy;
        2
      }
      XDy | MaybeDxDy(false) => {
        self.y += delta;
        1
      }
      DxY | DxMaybeDy(false) => {
        self.x += delta;
        1
      }
      DxInitialY => {
        self.x += delta;
        self.y = initial.y;
        1
      }
      DLargerCoordDist => {
        let Ok(x_distance) = (self.x - initial.x).abs();
        let Ok(y_distance) = (self.y - initial.y).abs();

        match x_distance.cmp(&y_distance) {
          core::cmp::Ordering::Greater => {
            self.x += delta;
            self.y = initial.y;
          }
          core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {
            self.y += delta;
            self.x = initial.x;
          }
        }

        1
      }
      MaybeDxDy(true) => {
        self.y += delta;
        let dx = self.stack.get_fixed(self.stack_ix.wrapping_add(1))?;
        self.x += dx;
        2
      }
    };

    self.stack_ix = self.stack_ix.wrapping_add(operands);
    Ok(())
  }

  fn reset_stack(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      self.stack.clear();
      self.stack_ix = 0;
    };
    Ok(())
  }
}

#[derive(Clone, Copy)]
enum CurveAxis {
  Horizontal,
  Vertical,
}

enum CurveControls {
  Empty,
  First(Point<Fixed>),
  Both([Point<Fixed>; 2]),
}

#[derive(Copy, Clone)]
enum PointMode {
  DxDy,
  XDy,
  DxY,
  DxInitialY,
  DLargerCoordDist,
  DxMaybeDy(bool),
  MaybeDxDy(bool),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Operator {
  HStem,
  VStem,
  VMoveTo,
  RLineTo,
  HLineTo,
  VLineTo,
  RrCurveTo,
  CallSubr,
  Return,
  Hsbw,
  EndChar,
  VariationStoreIndex,
  Blend,
  HStemHm,
  HintMask,
  CntrMask,
  RMoveTo,
  HMoveTo,
  VStemHm,
  RCurveLine,
  RLineCurve,
  VvCurveTo,
  HhCurveTo,
  CallGsubr,
  VhCurveTo,
  HvCurveTo,
  DotSection,
  VStem3,
  HStem3,
  Seac,
  Sbw,
  Div,
  CallOtherSubr,
  Pop,
  SetCurrentPoint,
  HFlex,
  Flex,
  HFlex1,
  Flex1,
}

impl Operator {
  fn read(cursor: &mut Cursor, b0: u8) -> Result<Self, Error> {
    const ESCAPE: u8 = 12;
    let (opcode, operator) = match ::core::primitive::bool::then_some(b0 == ESCAPE, ()) {
      ::core::option::Option::Some(()) => {
        let b1 = cursor.read::<u8>()?;
        (
          b1,
          ({
            let Ok(explicit_value) = Self::from_two_byte_opcode(b1);
            explicit_value
          }),
        )
      }
      ::core::option::Option::None => (
        b0,
        ({
          let Ok(explicit_value) = Self::from_opcode(b0);
          explicit_value
        }),
      ),
    };
    operator.ok_or(Error::InvalidCharstringOperator(opcode))
  }

  #[must_use]
  fn from_opcode(opcode: u8) -> core::result::Result<Option<Self>, console_core_never::Never> {
    use Operator::*;

    Ok(Some(match opcode {
      1 => HStem,
      3 => VStem,
      4 => VMoveTo,
      5 => RLineTo,
      6 => HLineTo,
      7 => VLineTo,
      8 => RrCurveTo,
      10 => CallSubr,
      11 => Return,
      13 => Hsbw,
      14 => EndChar,
      15 => VariationStoreIndex,
      16 => Blend,
      18 => HStemHm,
      19 => HintMask,
      20 => CntrMask,
      21 => RMoveTo,
      22 => HMoveTo,
      23 => VStemHm,
      24 => RCurveLine,
      25 => RLineCurve,
      26 => VvCurveTo,
      27 => HhCurveTo,
      29 => CallGsubr,
      30 => VhCurveTo,
      31 => HvCurveTo,
      _ => return Ok(None),
    }))
  }

  #[must_use]
  pub fn from_two_byte_opcode(
    opcode: u8,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    use Operator::*;

    Ok(Some(match opcode {
      0 => DotSection,
      1 => VStem3,
      2 => HStem3,
      6 => Seac,
      7 => Sbw,
      12 => Div,
      16 => CallOtherSubr,
      17 => Pop,
      33 => SetCurrentPoint,
      34 => HFlex,
      35 => Flex,
      36 => HFlex1,
      37 => Flex1,
      _ => return Ok(None),
    }))
  }
}

const ONE_OVER_64: Fixed = {
  let Ok(value) = Fixed::from_bits(0x400);
  value
};

pub struct TransformSink<'a, S> {
  inner: &'a mut S,
  matrix: Option<FontMatrix>,
  scale: Option<Fixed>,
}

impl<'a, S> TransformSink<'a, S> {
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `TransformSink::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new(sink: &'a mut S, transform: Transform) -> Self {
    Self::from_matrix_scale(sink, transform.matrix, transform.scale)
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `TransformSink::from_matrix_scale` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn from_matrix_scale(sink: &'a mut S, matrix: FontMatrix, scale: Option<Fixed>) -> Self {
    Self {
      inner: sink,
      matrix: (matrix != FontMatrix::IDENTITY).then_some(matrix),
      scale,
    }
  }

  #[must_use]
  fn transform(
    &self,
    x: Fixed,
    y: Fixed,
  ) -> core::result::Result<(Fixed, Fixed), console_core_never::Never> {
    let ax = x * ONE_OVER_64;
    let ay = y * ONE_OVER_64;

    let Ok(value_12) = ax.to_bits();

    let Ok(value_20) = Fixed::from_bits(::core::primitive::i32::wrapping_shr(value_12, 10_u32));

    let bx = value_20;
    let Ok(value_13) = ay.to_bits();

    let Ok(value_21) = Fixed::from_bits(::core::primitive::i32::wrapping_shr(value_13, 10_u32));

    let by = value_21;

    let (cx, cy) = match self.matrix.as_ref().map(|mat| {
      let Ok(value_23) = mat.transform(bx, by);

      value_23
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => (bx, by),
    };

    Ok(match self.scale {
      Some(scale) => {
        let dx = cx * scale;
        let dy = cy * scale;

        let Ok(value_14) = Fixed::from_bits({
          let Ok(bits) = dx.to_bits();
          bits.wrapping_shl(10)
        });
        let Ok(value_15) = Fixed::from_bits({
          let Ok(bits) = dy.to_bits();
          bits.wrapping_shl(10)
        });

        (value_14, value_15)
      }
      None => {
        let Ok(value_16) = Fixed::from_bits({
          let Ok(bits) = cx.to_bits();
          bits.wrapping_shl(16)
        });
        let Ok(value_17) = Fixed::from_bits({
          let Ok(bits) = cy.to_bits();
          bits.wrapping_shl(16)
        });

        (value_16, value_17)
      }
    })
  }
}

impl<S: CommandSink> CommandSink for TransformSink<'_, S> {
  fn hstem(&mut self, y: Fixed, dy: Fixed) {
    self.inner.hstem(y, dy);
  }

  fn vstem(&mut self, x: Fixed, dx: Fixed) {
    self.inner.vstem(x, dx);
  }

  fn hint_mask(&mut self, mask: &[u8]) {
    self.inner.hint_mask(mask);
  }

  fn counter_mask(&mut self, mask: &[u8]) {
    self.inner.counter_mask(mask);
  }

  fn clear_hints(&mut self) {
    self.inner.clear_hints();
  }

  fn move_to(&mut self, x: Fixed, y: Fixed) {
    let Ok((x_2, y_2)) = self.transform(x, y);
    self.inner.move_to(x_2, y_2);
  }

  fn line_to(&mut self, x: Fixed, y: Fixed) {
    let Ok((x_2, y_2)) = self.transform(x, y);
    self.inner.line_to(x_2, y_2);
  }

  fn curve_to(&mut self, cx1: Fixed, cy1: Fixed, cx2: Fixed, cy2: Fixed, x: Fixed, y: Fixed) {
    let Ok((cx1_2, cy1_2)) = self.transform(cx1, cy1);
    let Ok((cx2_2, cy2_2)) = self.transform(cx2, cy2);
    let Ok((x_2, y_2)) = self.transform(x, y);
    self.inner.curve_to(cx1_2, cy1_2, cx2_2, cy2_2, x_2, y_2);
  }

  fn close(&mut self) {
    self.inner.close();
  }

  fn finish(&mut self) {
    self.inner.finish();
  }
}

#[derive(Copy, Clone)]
enum PendingElement {
  Move([Fixed; 2]),
  Line([Fixed; 2]),
  Curve([Fixed; 6]),
}

impl PendingElement {
  #[must_use]
  fn target_point(&self) -> core::result::Result<[Fixed; 2], console_core_never::Never> {
    Ok(match self {
      Self::Move(xy) | Self::Line(xy) => *xy,
      Self::Curve([.., x, y]) => [*x, *y],
    })
  }
}

#[derive(Clone, Copy)]
enum FilteredContour {
  Absent,
  Open([Fixed; 2]),
  Closed([Fixed; 2]),
}

pub struct NopFilterSink<'a, S> {
  contour: FilteredContour,
  pending_element: Option<PendingElement>,
  inner: &'a mut S,
}

enum FlushReason {
  Drawing,
  Closing,
}

impl<'a, S> NopFilterSink<'a, S>
where
  S: CommandSink,
{
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `NopFilterSink::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new(inner: &'a mut S) -> Self {
    Self {
      contour: FilteredContour::Absent,
      pending_element: None,
      inner,
    }
  }

  fn flush_pending(&mut self, reason: FlushReason) -> Result<(), console_core_never::Never> {
    let element = match self.pending_element.take() {
      Some(element) => element,
      None => return Ok(()),
    };

    match element {
      PendingElement::Move([x, y]) => match reason {
        FlushReason::Drawing => {
          self.inner.move_to(x, y);
          self.contour = FilteredContour::Open([x, y]);
        }
        FlushReason::Closing => {}
      },
      PendingElement::Line([x, y]) => {
        let start = match self.contour {
          FilteredContour::Absent => None,
          FilteredContour::Open(start) | FilteredContour::Closed(start) => Some(start),
        };

        match (reason, start == Some([x, y])) {
          (FlushReason::Closing, true) => {}
          (FlushReason::Closing, false) | (FlushReason::Drawing, _) => self.inner.line_to(x, y),
        }
      }
      PendingElement::Curve([cx0, cy0, cx1, cy1, x, y]) => {
        self.inner.curve_to(cx0, cy0, cx1, cy1, x, y);
      }
    }

    Ok(())
  }
}

impl<S> CommandSink for NopFilterSink<'_, S>
where
  S: CommandSink,
{
  fn hstem(&mut self, y: Fixed, dy: Fixed) {
    self.inner.hstem(y, dy);
  }

  fn vstem(&mut self, x: Fixed, dx: Fixed) {
    self.inner.vstem(x, dx);
  }

  fn hint_mask(&mut self, mask: &[u8]) {
    self.inner.hint_mask(mask);
  }

  fn counter_mask(&mut self, mask: &[u8]) {
    self.inner.counter_mask(mask);
  }

  fn clear_hints(&mut self) {
    self.inner.clear_hints();
  }

  fn move_to(&mut self, x: Fixed, y: Fixed) {
    self.pending_element = Some(PendingElement::Move([x, y]));
  }

  fn line_to(&mut self, x: Fixed, y: Fixed) {
    match ::core::primitive::bool::then_some(
      match self.pending_element.map(|element| {
        let Ok(target_point) = element.target_point();

        target_point == [x, y]
      }) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => false,
      },
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return;
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.flush_pending(FlushReason::Drawing);
    self.pending_element = Some(PendingElement::Line([x, y]));
  }

  fn curve_to(&mut self, cx1: Fixed, cy1: Fixed, cx2: Fixed, cy2: Fixed, x: Fixed, y: Fixed) {
    let Ok(()) = self.flush_pending(FlushReason::Drawing);
    self.pending_element = Some(PendingElement::Curve([cx1, cy1, cx2, cy2, x, y]));
  }

  fn close(&mut self) {
    let Ok(()) = self.flush_pending(FlushReason::Closing);

    match self.contour {
      FilteredContour::Open(start) => {
        self.inner.close();
        self.contour = FilteredContour::Closed(start);
      }
      FilteredContour::Absent | FilteredContour::Closed(_) => {}
    }
  }

  fn finish(&mut self) {
    self.close();
    self.inner.finish();
  }
}

impl<P: OutlinePen> CommandSink for P {
  fn move_to(&mut self, x: Fixed, y: Fixed) {
    let Ok(value_25) = x.to_f32();
    let Ok(value_26) = y.to_f32();

    self.move_to(value_25, value_26);
  }

  fn line_to(&mut self, x: Fixed, y: Fixed) {
    let Ok(value_27) = x.to_f32();
    let Ok(value_28) = y.to_f32();

    self.line_to(value_27, value_28);
  }

  fn curve_to(&mut self, cx0: Fixed, cy0: Fixed, cx1: Fixed, cy1: Fixed, x: Fixed, y: Fixed) {
    let Ok(value_29) = cx0.to_f32();
    let Ok(value_30) = cy0.to_f32();
    let Ok(value_31) = cx1.to_f32();
    let Ok(value_32) = cy1.to_f32();
    let Ok(value_33) = x.to_f32();
    let Ok(value_34) = y.to_f32();

    self.curve_to(value_29, value_30, value_31, value_32, value_33, value_34);
  }

  fn close(&mut self) {
    self.close()
  }
}

#[cfg(test)]
pub(crate) mod test_helpers {
  use super::{CommandSink, Fixed};

  #[derive(Copy, Clone, PartialEq, Debug)]
  pub enum Command<T = Fixed> {
    Move(T, T),
    Line(T, T),
    Curve(T, T, T, T, T, T),
  }

  impl Command<i32> {
    pub(super) fn to_fixed(self) -> Result<Command, console_core_never::Never> {
      let fixed = |coordinate| {
        let Ok(value) = Fixed::from_i32(coordinate);
        value
      };
      Ok(match self {
        Self::Move(x, y) => Command::Move(fixed(x), fixed(y)),
        Self::Line(x, y) => Command::Line(fixed(x), fixed(y)),
        Self::Curve(cx0, cy0, cx1, cy1, x, y) => Command::Curve(
          fixed(cx0),
          fixed(cy0),
          fixed(cx1),
          fixed(cy1),
          fixed(x),
          fixed(y),
        ),
      })
    }
  }

  #[derive(PartialEq, Default, Debug)]
  pub struct CaptureCommandSink(pub Vec<Command>);

  impl CommandSink for CaptureCommandSink {
    fn move_to(&mut self, x: Fixed, y: Fixed) {
      self.0.push(Command::Move(x, y))
    }

    fn line_to(&mut self, x: Fixed, y: Fixed) {
      self.0.push(Command::Line(x, y))
    }

    fn curve_to(&mut self, cx0: Fixed, cy0: Fixed, cx1: Fixed, cy1: Fixed, x: Fixed, y: Fixed) {
      self.0.push(Command::Curve(cx0, cy0, cx1, cy1, x, y))
    }

    fn close(&mut self) {
      let mut last_move = [Fixed::ZERO; 2];

      for command in self.0.iter().rev() {
        match command {
          Command::Move(x, y) => {
            last_move = [*x, *y];
            break;
          }
          Command::Line(..) | Command::Curve(..) => {}
        }
      }

      self.0.push(Command::Line(
        match last_move {
          [_explicit_element, ..] => _explicit_element,
        },
        match last_move {
          [_, _explicit_element, ..] => _explicit_element,
        },
      ));
    }
  }

  impl CaptureCommandSink {
    #[must_use]
    pub fn to_svg(&self) -> Result<String, core::fmt::Error> {
      use core::fmt::Write;
      let mut svg = String::new();

      for (ordinal, command) in self.0.iter().enumerate() {
        match ordinal {
          0 => {}
          _subsequent_command => svg.push(' '),
        }

        match *command {
          Command::Move(x, y) => {
            let Ok(x_coord) = x.to_f32();
            let Ok(y_coord) = y.to_f32();
            write!(svg, "M{x_coord},{y_coord}")?;
          }
          Command::Line(x, y) => {
            let Ok(x_coord) = x.to_f32();
            let Ok(y_coord) = y.to_f32();
            write!(svg, "L{x_coord},{y_coord}")?;
          }
          Command::Curve(cx0, cy0, cx1, cy1, x, y) => {
            let Ok(cx0_coord) = cx0.to_f32();
            let Ok(cy0_coord) = cy0.to_f32();
            let Ok(cx1_coord) = cx1.to_f32();
            let Ok(cy1_coord) = cy1.to_f32();
            let Ok(x_coord) = x.to_f32();
            let Ok(y_coord) = y.to_f32();
            write!(
              svg,
              "C{cx0_coord},{cy0_coord} {cx1_coord},{cy1_coord} {x_coord},{y_coord}"
            )?;
          }
        }
      }

      Ok(svg)
    }
  }

  #[derive(Default)]
  pub struct CharstringCommandCounter(pub u64);

  impl CommandSink for CharstringCommandCounter {
    fn move_to(&mut self, _x: Fixed, _y: Fixed) {
      match (1, &mut self.0) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    fn line_to(&mut self, _x: Fixed, _y: Fixed) {
      match (1, &mut self.0) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    fn curve_to(
      &mut self,
      _cx0: Fixed,
      _cy0: Fixed,
      _cx1: Fixed,
      _cy1: Fixed,
      _x: Fixed,
      _y: Fixed,
    ) {
      match (1, &mut self.0) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    fn close(&mut self) {
      match (1, &mut self.0) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{test_helpers::*, *};
  use crate::{FontData, FontRead, tables::variations::ItemVariationStore, types::F2Dot14};

  #[test]
  fn cff2_example_subr() -> ::core::result::Result<(), crate::test_support::Failure> {
    use Command::*;
    let charstring = &*match <[_]>::get(&*font_test_data::cff2::EXAMPLE, 0xc8..=0xe1) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(
          crate::read::ReadError::OutOfBounds,
        ));
      }
    };
    let store = ItemVariationStore::read({
      let Ok(explicit_value) =
        FontData::new(&*match <[_]>::get(&*font_test_data::cff2::EXAMPLE, 18..) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::test_support::Failure::from(
              crate::read::ReadError::OutOfBounds,
            ));
          }
        });
      explicit_value
    })?;
    let coords = &[{
      let Ok(value) = F2Dot14::from_f32(0.0);
      value
    }];
    let blend_state =
      crate::test_support::fixture_value(BlendState::new(store, coords, 0), "fixture value")?;
    let mut commands = CaptureCommandSink::default();
    crate::test_support::fixture_value(
      evaluate(
        &NullContext(CharstringKind::Type2),
        Some(blend_state),
        charstring,
        &mut commands,
      ),
      "fixture value",
    )?;

    let expected = &[
      Move(
        {
          let Ok(explicit_value) = Fixed::from_f64(50.0);
          explicit_value
        },
        Fixed::ZERO,
      ),
      Line(
        {
          let Ok(explicit_value) = Fixed::from_f64(550.0);
          explicit_value
        },
        Fixed::ZERO,
      ),
      Line(
        {
          let Ok(value) = Fixed::from_f64(550.0);
          value
        },
        {
          let Ok(value) = Fixed::from_f64(500.0);
          value
        },
      ),
      Line(
        {
          let Ok(value) = Fixed::from_f64(50.0);
          value
        },
        {
          let Ok(value) = Fixed::from_f64(500.0);
          value
        },
      ),
      Line(
        {
          let Ok(explicit_value) = Fixed::from_f64(50.0);
          explicit_value
        },
        Fixed::ZERO,
      ),
    ];
    assert_eq!(&commands.0, expected);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn all_path_ops() -> ::core::result::Result<(), crate::ps::error::Error> {
    let charstring = &[
      251, 29, 253, 12, 21, 173, 134, 159, 133, 5, 140, 141, 142, 6, 251, 71, 129, 142, 7, 109,
      154, 161, 147, 89, 165, 125, 97, 98, 158, 124, 164, 8, 109, 154, 161, 147, 27, 147, 109,
      154, 161, 147, 27, 163, 159, 154, 180, 181, 119, 153, 115, 114, 120, 125, 97, 98, 158, 124,
      164, 31, 159, 4, 119, 153, 115, 114, 120, 125, 143, 144, 24, 119, 153, 115, 114, 120, 125,
      143, 144, 25, 84, 116, 117, 80, 30, 109, 154, 161, 147, 26, 147, 109, 154, 161, 147, 26,
      163, 159, 154, 180, 181, 119, 153, 115, 114, 120, 125, 97, 162, 12, 35, 163, 159, 154, 180,
      181, 119, 153, 12, 34, 152, 22, 180, 181, 119, 153, 115, 114, 120, 125, 97, 12, 36, 154,
      180, 181, 119, 153, 115, 114, 120, 125, 97, 147, 12, 37, 14,
    ];
    let mut commands = CaptureCommandSink::default();
    evaluate(
      &NullContext(CharstringKind::Type2),
      None,
      charstring,
      &mut commands,
    )?;

    let expected = [
      Command::Move(-137, -632),
      Command::Line(-103, -637),
      Command::Line(-83, -643),
      Command::Line(-82, -643),
      Command::Line(-82, -641),
      Command::Line(-79, -641),
      Command::Line(-79, -820),
      Command::Line(-89, -820),
      Command::Line(-89, -817),
      Command::Curve(-119, -802, -97, -794, -147, -768),
      Command::Curve(-161, -810, -202, -791, -217, -766),
      Command::Curve(-247, -766, -232, -744, -224, -744),
      Command::Curve(-254, -736, -239, -714, -231, -714),
      Command::Curve(-207, -714, -187, -699, -187, -658),
      Command::Curve(-187, -616, -207, -602, -231, -602),
      Command::Curve(-256, -602, -275, -616, -275, -658),
      Command::Curve(-275, -699, -256, -714, -231, -714),
      Command::Line(-137, -632),
      Command::Move(-231, -694),
      Command::Curve(-251, -680, -275, -705, -294, -719),
      Command::Line(-290, -714),
      Command::Line(-310, -700),
      Command::Curve(-334, -725, -353, -739, -349, -734),
      Command::Curve(-349, -789, -372, -811, -431, -811),
      Command::Curve(-431, -841, -416, -819, -416, -811),
      Command::Curve(-408, -841, -393, -819, -393, -811),
      Command::Curve(-369, -791, -354, -750, -312, -770),
      Command::Curve(-298, -794, -323, -813, -337, -855),
      Command::Curve(-313, -855, -293, -840, -252, -840),
      Command::Curve(-210, -840, -230, -855, -216, -855),
      Command::Line(-231, -694),
      Command::Move(-203, -855),
      Command::Curve(-162, -813, -182, -799, -206, -799),
      Command::Curve(-231, -799, -250, -813, -292, -855),
      Command::Curve(-277, -814, -235, -834, -221, -858),
      Command::Curve(-246, -877, -260, -919, -292, -911),
      Command::Line(-203, -855),
    ]
    .map(|command| {
      let Ok(expected_command) = command.to_fixed();
      expected_command
    });
    assert_eq!(commands.0, expected);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn coords_remaining_avoid_overflow() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut commands = CaptureCommandSink::default();
    let Ok(mut evaluator) =
      Evaluator::new(&NullContext(CharstringKind::Type2), None, &mut commands);
    evaluator.stack.push(0)?;
    evaluator.stack.push(0)?;
    let mut cursor = {
      let Ok(value) = ({
        let Ok(explicit_value) = FontData::new(&[]);
        explicit_value
      })
      .cursor();
      value
    };

    let flow = evaluator.evaluate_operator(Operator::HhCurveTo, &mut cursor, 0)?;
    assert!(matches!(flow, OperatorFlow::Continue));
    assert!(commands.0.is_empty());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn ignore_reserved_operators() -> ::core::result::Result<(), crate::ps::error::Error> {
    let charstring = &[0u8, 32, 22, 2];
    let mut commands = CaptureCommandSink::default();
    evaluate(
      &NullContext(CharstringKind::Type2),
      None,
      charstring,
      &mut commands,
    )?;
    let Ok(x) = Fixed::from_i32(-107);
    assert_eq!(
      commands.0,
      [Command::Move(x, Fixed::ZERO), Command::Line(x, Fixed::ZERO)]
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn op_div() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut commands = CaptureCommandSink::default();
    let Ok(mut eval) = Evaluator::new(&NullContext(CharstringKind::Type2), None, &mut commands);
    let mut cursor = {
      let Ok(value) = ({
        let Ok(explicit_value) = FontData::new(&[]);
        explicit_value
      })
      .cursor();
      value
    };
    eval.stack.push({
      let Ok(value) = Fixed::from_f64(512.5);
      value
    })?;
    eval.stack.push(2)?;
    eval.evaluate_operator(Operator::Div, &mut cursor, 0)?;
    assert_eq!(
      match eval.stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(512.5 / 2.0);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn op_div_type1_large_int() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut commands = CaptureCommandSink::default();
    let Ok(mut eval) = Evaluator::new(&NullContext(CharstringKind::Type1), None, &mut commands);
    let mut cursor = {
      let Ok(value) = ({
        let Ok(explicit_value) = FontData::new(&[]);
        explicit_value
      })
      .cursor();
      value
    };

    eval.stack.push(32001)?;
    eval.stack.push(2)?;
    eval.evaluate_operator(Operator::Div, &mut cursor, 0)?;
    assert_eq!(
      match eval.stack.pop_fixed() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      ({
        let Ok(explicit_value) = Fixed::from_f64(32001.0 / 2.0);
        explicit_value
      })
    );
    ::core::result::Result::Ok(())
  }

  fn eval_h_sbw(operator: Operator, kind: CharstringKind) -> Result<[Fixed; 2], Error> {
    let mut commands = CaptureCommandSink::default();
    let context = NullContext(kind);
    let Ok(mut eval) = Evaluator::new(&context, None, &mut commands);
    let Ok(data) = FontData::new(&[]);
    let Ok(mut cursor) = data.cursor();
    let Ok(side_bearing) = Fixed::from_f64(42.5);
    eval.stack.push(side_bearing)?;

    match crate::Presence::from(operator == Operator::Sbw) {
      crate::Presence::Present => eval.stack.push(0)?,
      crate::Presence::Absent => {}
    }

    eval.stack.push(501)?;
    eval.stack.push(1000)?;
    let _flow = eval.evaluate_operator(operator, &mut cursor, 0)?;
    Ok([eval.sbx, eval.wx])
  }

  #[test]
  fn op_sbw_type1() -> Result<(), Error> {
    let [sbx, wx] = eval_h_sbw(Operator::Sbw, CharstringKind::Type1)?;
    assert_eq!(
      sbx,
      ({
        let Ok(explicit_value) = Fixed::from_f64(42.5);
        explicit_value
      })
    );
    assert_eq!(
      wx,
      ({
        let Ok(explicit_value) = Fixed::from_f64(501.0);
        explicit_value
      })
    );
    Ok(())
  }

  #[test]
  fn op_hsbw_type1() -> Result<(), Error> {
    let [sbx, wx] = eval_h_sbw(Operator::Hsbw, CharstringKind::Type1)?;
    assert_eq!(
      sbx,
      ({
        let Ok(explicit_value) = Fixed::from_f64(42.5);
        explicit_value
      })
    );
    assert_eq!(
      wx,
      ({
        let Ok(explicit_value) = Fixed::from_f64(501.0);
        explicit_value
      })
    );
    Ok(())
  }

  #[test]
  fn op_sbw_type2_no_effect() -> Result<(), Error> {
    let [sbx, wx] = eval_h_sbw(Operator::Sbw, CharstringKind::Type2)?;
    assert_eq!(sbx, Fixed::ZERO);
    assert_eq!(wx, Fixed::ZERO);
    Ok(())
  }

  #[test]
  fn op_hsbw_type2_no_effect() -> Result<(), Error> {
    let [sbx, wx] = eval_h_sbw(Operator::Hsbw, CharstringKind::Type2)?;
    assert_eq!(sbx, Fixed::ZERO);
    assert_eq!(wx, Fixed::ZERO);
    Ok(())
  }

  #[test]
  fn op_callothersubr_flex() -> Result<(), crate::test_support::Failure> {
    fn execute(
      evaluator: &mut Evaluator<'_, CaptureCommandSink>,
      cursor: &mut Cursor<'_>,
      operands: &[i32],
      operator: Operator,
    ) -> Result<(), Error> {
      for &operand in operands {
        evaluator.stack.push(operand)?;
      }

      let _operator_flow = evaluator.evaluate_operator(operator, cursor, 0)?;
      Ok(())
    }

    let mut commands = CaptureCommandSink::default();
    let Ok(mut eval) = Evaluator::new(&NullContext(CharstringKind::Type1), None, &mut commands);
    let mut cursor = {
      let Ok(value) = ({
        let Ok(explicit_value) = FontData::new(&[]);
        explicit_value
      })
      .cursor();
      value
    };

    crate::test_support::fixture_value(
      execute(&mut eval, &mut cursor, &[0, 1], Operator::CallOtherSubr),
      "begin Type 1 flex",
    )?;

    for point in [[1, 2]; 7] {
      crate::test_support::fixture_value(
        execute(&mut eval, &mut cursor, &point, Operator::RMoveTo),
        "record Type 1 flex point",
      )?;
    }

    for operand in [0, 100, 200] {
      crate::test_support::fixture_value(eval.stack.push(operand), "Type 1 flex result operand")?;
    }

    crate::test_support::fixture_value(
      execute(&mut eval, &mut cursor, &[3, 0], Operator::CallOtherSubr),
      "finish Type 1 flex",
    )?;
    crate::test_support::fixture_value(
      execute(&mut eval, &mut cursor, &[], Operator::SetCurrentPoint),
      "set Type 1 current point",
    )?;
    let expected = [
      Command::Move(0, 0),
      Command::Curve(2, 4, 3, 6, 4, 8),
      Command::Curve(5, 10, 6, 12, 7, 14),
    ]
    .map(|command| {
      let Ok(expected_command) = command.to_fixed();
      expected_command
    });
    assert_eq!(
      eval.x,
      ({
        let Ok(explicit_value) = Fixed::from_i32(100);
        explicit_value
      })
    );
    assert_eq!(
      eval.y,
      ({
        let Ok(explicit_value) = Fixed::from_i32(200);
        explicit_value
      })
    );
    assert_eq!(commands.0, expected);
    Ok(())
  }

  struct NullContext(CharstringKind);

  impl CharstringContext for NullContext {
    fn kind(&self) -> CharstringKind {
      self.0
    }

    fn seac_components(&self, base_code: i32, _accent_code: i32) -> Result<[&[u8]; 2], Error> {
      Err(Error::InvalidSeacCode(base_code))
    }

    fn global_subr(&self, _index: i32) -> Result<&[u8], Error> {
      Err(Error::MissingSubroutines)
    }

    fn subr(&self, _index: i32) -> Result<&[u8], Error> {
      Err(Error::MissingSubroutines)
    }
  }

  #[test]
  fn nop_filter_sink() {
    let mut commands = CaptureCommandSink::default();
    let mut nop_filter = NopFilterSink::new(&mut commands);
    let [sx, sy, cx0, cy0, cx1, cy1, x, y] =
      [10.2, 20.4, 5.0, -5.0, 1.5, 2.0, 4.5, -10.0].map(|coordinate| {
        let Ok(value) = Fixed::from_f64(coordinate);
        value
      });

    nop_filter.move_to(Fixed::ZERO, Fixed::ZERO);
    nop_filter.move_to(sx, sy);
    nop_filter.line_to(sx, sy);
    nop_filter.curve_to(cx0, cy0, cx1, cy1, x, y);
    nop_filter.line_to(x, y);
    nop_filter.line_to(sx, sy);
    nop_filter.close();

    assert_eq!(
      commands.0,
      [
        Command::Move(sx, sy),
        Command::Curve(cx0, cy0, cx1, cy1, x, y),
        Command::Line(sx, sy)
      ]
    );
  }

  #[test]
  fn scaled_matrix_transform_sink() {
    let input = [(150i32, 46i32), (176, 8), (217, -13), (267, -13)].map(|(x, y)| {
      (
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(x, 16_u32));
          value
        },
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(y, 16_u32));
          value
        },
      )
    });
    let expected = [(404, 118i32), (453, 20), (550, -33), (678, -33)].map(|(x, y)| {
      (
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(x, 10_u32));
          value
        },
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(y, 10_u32));
          value
        },
      )
    });
    let mut dummy = ();
    let sink = TransformSink::from_matrix_scale(
      &mut dummy,
      TRANSFORM,
      Some({
        let Ok(explicit_value) = Fixed::from_bits(167772);
        explicit_value
      }),
    );
    let transformed = input.map(|(x, y)| {
      let Ok(transform) = sink.transform(x, y);

      transform
    });
    assert_eq!(transformed, expected);
  }

  #[test]
  fn unscaled_matrix_transform_sink() {
    let input = [(150i32, 46i32), (176, 8), (217, -13), (267, -13)].map(|(x, y)| {
      (
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(x, 16_u32));
          value
        },
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(y, 16_u32));
          value
        },
      )
    });
    let expected = [(158, 46i32), (177, 8), (215, -13), (265, -13)].map(|(x, y)| {
      (
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(x, 16_u32));
          value
        },
        {
          let Ok(value) = Fixed::from_bits(::core::primitive::i32::wrapping_shl(y, 16_u32));
          value
        },
      )
    });
    let mut dummy = ();
    let sink = TransformSink::from_matrix_scale(&mut dummy, TRANSFORM, None);
    let transformed = input.map(|(x, y)| {
      let Ok(transform) = sink.transform(x, y);

      transform
    });
    assert_eq!(transformed, expected);
  }

  const TRANSFORM: FontMatrix = {
    let Ok(explicit_value) = FontMatrix::from_elements([
      Fixed::ONE,
      Fixed::ZERO,
      ({
        let Ok(explicit_value) = Fixed::from_bits(10945);
        explicit_value
      }),
      Fixed::ONE,
      Fixed::ZERO,
      Fixed::ZERO,
    ]);
    explicit_value
  };

  #[test]
  fn unscaled_transform_sink_produces_integers() {
    let nothing = &mut ();
    let sink = TransformSink::new(nothing, Transform::default());

    for coord in [50.0, 50.1, 50.125, 50.5, 50.9] {
      assert_eq!(
        ({
          let Ok(transform) = sink.transform(
            {
              let Ok(explicit_value) = Fixed::from_f64(coord);
              explicit_value
            },
            Fixed::ZERO,
          );
          let Ok(value) = transform.0.to_f32();
          value
        }),
        50.0
      );
    }
  }

  #[test]
  fn scaled_transform_sink() {
    let ppem = 20.0;
    let upem = 1000.0;

    let scale = ({
      let Ok(explicit_value) = Fixed::from_bits({
        let Ok(migrated_value) = console_core_number_conversion::toward_zero_i32(ppem * 64.);
        migrated_value
      });
      explicit_value
    }) / ({
      let Ok(explicit_value) = Fixed::from_bits({
        let Ok(migrated_value) = console_core_number_conversion::toward_zero_i32(upem);
        migrated_value
      });
      explicit_value
    });
    let nothing = &mut ();
    let sink = TransformSink::from_matrix_scale(nothing, FontMatrix::IDENTITY, Some(scale));

    let inputs = [
      (0.0, 0.0),
      (8.0, 0.15625),
      (16.0, 0.3125),
      (32.0, 0.640625),
      (72.0, 1.4375),
      (128.0, 2.5625),
    ];
    for (coord, expected) in inputs {
      assert_eq!(
        ({
          let Ok(transform) = sink.transform(
            {
              let Ok(explicit_value) = Fixed::from_f64(coord);
              explicit_value
            },
            Fixed::ZERO,
          );
          let Ok(value) = transform.0.to_f32();
          value
        }),
        expected,
        "scaling coord {coord}"
      );
    }
  }

  #[test]
  fn mm_blend() -> ::core::result::Result<(), crate::ps::error::Error> {
    let mut commands = CaptureCommandSink::default();
    let ctx = MmContext([0.0, -0.25, 1.0].map(|argument| {
      let Ok(value_37) = Fixed::from_f64(argument);

      value_37
    }));
    let Ok(mut eval) = Evaluator::new(&ctx, None, &mut commands);
    let mut cursor = {
      let Ok(value) = ({
        let Ok(explicit_value) = FontData::new(&[]);
        explicit_value
      })
      .cursor();
      value
    };

    for i in [[1, 0], [2, 3], [4, -8], [6, 15]].into_iter().flatten() {
      match eval.stack.push(i) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(read_error);
        }
      };
    }

    eval.evaluate_operator(Operator::CallOtherSubr, &mut cursor, 0)?;
    let [a, b] = eval.stack.fixed_array(0)?;

    assert_eq!(
      ({
        let Ok(value) = a.to_f32();
        value
      }),
      3.5
    );

    assert_eq!(
      ({
        let Ok(value) = b.to_f32();
        value
      }),
      -9.0
    );
    ::core::result::Result::Ok(())
  }

  struct MmContext([Fixed; 3]);

  impl CharstringContext for MmContext {
    fn kind(&self) -> CharstringKind {
      CharstringKind::Type1
    }

    fn seac_components(&self, base_code: i32, _accent_code: i32) -> Result<[&[u8]; 2], Error> {
      Err(Error::InvalidSeacCode(base_code))
    }

    fn global_subr(&self, _index: i32) -> Result<&[u8], Error> {
      Err(Error::MissingSubroutines)
    }

    fn subr(&self, _index: i32) -> Result<&[u8], Error> {
      Err(Error::MissingSubroutines)
    }

    fn weight_vector(&self) -> &[Fixed] {
      &self.0
    }
  }
}
