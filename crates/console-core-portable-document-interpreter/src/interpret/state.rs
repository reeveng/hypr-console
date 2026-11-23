//! The graphics state a content stream draws with, the text state beside it, and the `gs`
//! operator that sets many of their parts at once from a dictionary.
//!
//! The text state and the transform are parts of the graphics state in the specification and are
//! kept beside it here, and the clips a state has pushed are kept with it too, so restoring a
//! state knows how many to pop. A part of an `ExtGState` that cannot be read is skipped with a
//! warning and the rest of the dictionary still applies, as it does in a reader.

use crate::StrokeProps;
use crate::WritingMode;
use crate::color::{AlphaColor, ColorComponents, ColorSpace};
use crate::context::Context;
use crate::convert::{convert_line_cap, convert_line_join};
use crate::font::{CharacterCode, Font, UNITS_PER_EM};
use crate::interpret::text::TextRenderingMode;
use crate::pattern::Pattern;
use crate::soft_mask::SoftMask;
use crate::types::BlendMode;
use console_core_never::Never;
use console_core_number_conversion::narrow_f32;
use console_core_portable_document_functions::{Function, PdfSample};
use console_core_portable_document_syntax::content::ops::{LineCap, LineJoin};
use console_core_portable_document_syntax::object::dict::keys::{FONT, SMASK, TR, TR2};
use console_core_portable_document_syntax::object::{Array, Dict, Name, Number, Object};
use console_core_portable_document_syntax::page::Resources;
use kurbo::{Affine, BezPath, Vec2};
use std::ops::Deref;

#[derive(Clone, Debug)]
pub enum ActiveTransferFunction {
  Single(Function),
  Four([Function; 4]),
}

impl ActiveTransferFunction {
  pub fn apply(&self, color: &AlphaColor) -> Result<AlphaColor, Never> {
    let Ok([red, green, blue, alpha]) = color.components();
    let (red_function, green_function, blue_function) = match self {
      Self::Single(function) => (function, function, function),
      Self::Four([red_function, green_function, blue_function, _gray]) => {
        (red_function, green_function, blue_function)
      }
    };
    let Ok(red_2) = transferred(red_function, PdfSample(red));
    let Ok(green_2) = transferred(green_function, PdfSample(green));
    let Ok(blue_2) = transferred(blue_function, PdfSample(blue));

    AlphaColor::new([red_2, green_2, blue_2, alpha])
  }
}

fn transferred(function: &Function, PdfSample(component): PdfSample) -> Result<f32, Never> {
  let Ok(evaluated) = function.eval(vec![component]);

  Ok(match evaluated.as_ref().and_then(|values| values.first()) {
    Some(value) => *value,
    None => component,
  })
}

#[derive(Clone, Debug)]
pub(crate) enum ClipType {
  Dummy,
  Real,
}

#[derive(Clone, Debug)]
pub(crate) struct State<'a> {
  pub(crate) graphics_state: GraphicsState<'a>,
  pub(crate) text_state: TextState<'a>,
  pub(crate) ctm: Affine,
  pub(crate) clips: Vec<ClipType>,
}

impl Default for State<'_> {
  fn default() -> Self {
    State {
      ctm: Affine::IDENTITY,
      clips: vec![],
      text_state: TextState::default(),
      graphics_state: GraphicsState::default(),
    }
  }
}

impl<'a> State<'a> {
  pub(crate) fn new(initial_transform: Affine) -> Result<Self, Never> {
    Ok(Self {
      ctm: initial_transform,
      ..Self::default()
    })
  }

  pub(crate) fn stroke_data(&self) -> Result<PaintData<'a>, Never> {
    Ok(PaintData {
      alpha: self.graphics_state.stroke_alpha,
      color: self.graphics_state.stroke_color.clone(),
      color_space: self.graphics_state.stroke_cs.clone(),
      pattern: self.graphics_state.stroke_pattern.clone(),
      transfer_function: self.graphics_state.transfer_function.clone(),
    })
  }

  pub(crate) fn non_stroke_data(&self) -> Result<PaintData<'a>, Never> {
    Ok(PaintData {
      alpha: self.graphics_state.non_stroke_alpha,
      color: self.graphics_state.non_stroke_color.clone(),
      color_space: self.graphics_state.none_stroke_cs.clone(),
      pattern: self.graphics_state.non_stroke_pattern.clone(),
      transfer_function: self.graphics_state.transfer_function.clone(),
    })
  }
}

#[derive(Clone, Debug)]
pub(crate) enum TextStateFont<'a> {
  Font(Font<'a>),
  Fallback(Font<'a>),
}

impl<'a> Deref for TextStateFont<'a> {
  type Target = Font<'a>;

  fn deref(&self) -> &Self::Target {
    match self {
      TextStateFont::Font(font) | TextStateFont::Fallback(font) => font,
    }
  }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Distances retain the PDF text-space scalar representation through matrix calculations"
  )
)]
pub(crate) struct TextSpace(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The PDF text state stores horizontal scale as a percentage"
  )
)]
pub(crate) struct TextScalePercent(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "TJ adjustments are expressed in thousandths of a text-space unit"
  )
)]
pub(crate) struct TextAdjustment(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "PDF graphics state stores an unrounded opacity until the device applies it"
  )
)]
pub(crate) struct Opacity(pub(crate) f32);

#[derive(Clone, Debug)]
pub(crate) struct TextState<'a> {
  pub(crate) char_space: TextSpace,
  pub(crate) word_space: TextSpace,
  pub(crate) horizontal_scaling: TextScalePercent,
  pub(crate) leading: TextSpace,
  pub(crate) font: Option<TextStateFont<'a>>,
  pub(crate) font_size: TextSpace,
  pub(crate) rise: TextSpace,
  pub(crate) render_mode: TextRenderingMode,
  pub(crate) text_matrix: Affine,
  pub(crate) text_line_matrix: Affine,
  pub(crate) clip_paths: BezPath,
}

impl TextState<'_> {
  fn temp_transform(&self) -> Result<Affine, Never> {
    let Ok(scaling) = self.horizontal_scaling();

    Ok(Affine::new([
      f64::from(self.font_size.0) * f64::from(scaling),
      0.0,
      0.0,
      f64::from(self.font_size.0),
      0.0,
      f64::from(self.rise.0),
    ]))
  }

  fn horizontal_scaling(&self) -> Result<f32, Never> {
    Ok(self.horizontal_scaling.0 / 100.0)
  }

  fn writing_mode(&self) -> Result<WritingMode, Never> {
    match &self.font {
      Some(font) => font.is_horizontal(),
      None => Ok(WritingMode::Vertical),
    }
  }

  pub(crate) fn apply_adjustment(
    &mut self,
    TextAdjustment(adjustment): TextAdjustment,
  ) -> Result<(), Never> {
    let Ok(mode) = self.writing_mode();
    let Ok(scaling) = self.horizontal_scaling();
    let moved = -adjustment / UNITS_PER_EM * self.font_size.0;
    let (across, down) = match mode {
      WritingMode::Horizontal => (moved * scaling, 0.0),
      WritingMode::Vertical => (0.0, moved),
    };

    self.text_matrix *= Affine::translate((f64::from(across), f64::from(down)));

    Ok(())
  }

  pub(crate) fn apply_code_advance(&mut self, code: CharacterCode) -> Result<(), Never> {
    let advance = match &self.font {
      Some(font) => {
        let Ok(advance) = font.code_advance(code.value);

        advance
      }
      None => Vec2::ZERO,
    };
    let word_space = match (code.value, code.length) {
      (32, 1) => self.word_space.0,
      _ => 0.0,
    };
    let Ok(mode) = self.writing_mode();
    let Ok(scaling) = self.horizontal_scaling();
    let Ok(across) = narrow_f32(advance.x);
    let Ok(down) = narrow_f32(advance.y);
    let spaced =
      |length: f32| length / UNITS_PER_EM * self.font_size.0 + self.char_space.0 + word_space;
    let (across_2, down_2) = match mode {
      WritingMode::Horizontal => (spaced(across) * scaling, 0.0),
      WritingMode::Vertical => (0.0, spaced(down)),
    };

    self.text_matrix *= Affine::translate((f64::from(across_2), f64::from(down_2)));

    Ok(())
  }

  pub(crate) fn full_transform(&self) -> Result<Affine, Never> {
    let Ok(temporary) = self.temp_transform();

    Ok(self.text_matrix * temporary)
  }
}

impl Default for TextState<'_> {
  fn default() -> Self {
    Self {
      char_space: TextSpace(0.0),
      word_space: TextSpace(0.0),
      horizontal_scaling: TextScalePercent(100.0),
      leading: TextSpace(0.0),
      font: None,
      font_size: TextSpace(1.0),
      render_mode: TextRenderingMode::default(),
      text_matrix: Affine::IDENTITY,
      text_line_matrix: Affine::IDENTITY,
      rise: TextSpace(0.0),
      clip_paths: BezPath::default(),
    }
  }
}

#[derive(Clone, Debug)]
pub(crate) struct GraphicsState<'a> {
  pub(crate) stroke_props: StrokeProps,
  pub(crate) stroke_color: ColorComponents,
  pub(crate) stroke_pattern: Option<Pattern<'a>>,
  pub(crate) stroke_cs: ColorSpace,
  pub(crate) stroke_alpha: Opacity,
  pub(crate) non_stroke_color: ColorComponents,
  pub(crate) non_stroke_pattern: Option<Pattern<'a>>,
  pub(crate) none_stroke_cs: ColorSpace,
  pub(crate) non_stroke_alpha: Opacity,
  pub(crate) soft_mask: Option<SoftMask<'a>>,
  pub(crate) transfer_function: Option<ActiveTransferFunction>,
  pub(crate) blend_mode: BlendMode,
}

impl Default for GraphicsState<'_> {
  fn default() -> Self {
    let Ok(gray) = ColorSpace::device_gray();

    GraphicsState {
      stroke_props: StrokeProps::default(),
      non_stroke_alpha: Opacity(1.0),
      stroke_cs: gray.clone(),
      stroke_color: vec![0.0],
      none_stroke_cs: gray,
      non_stroke_color: vec![0.0],
      stroke_alpha: Opacity(1.0),
      stroke_pattern: None,
      non_stroke_pattern: None,
      soft_mask: None,
      transfer_function: None,
      blend_mode: BlendMode::default(),
    }
  }
}

pub(crate) struct PaintData<'a> {
  pub(crate) alpha: Opacity,
  pub(crate) color: ColorComponents,
  pub(crate) color_space: ColorSpace,
  pub(crate) pattern: Option<Pattern<'a>>,
  pub(crate) transfer_function: Option<ActiveTransferFunction>,
}

pub(crate) fn handle_gs<'a>(
  dict: &Dict<'a>,
  context: &mut Context<'a>,
  parent_resources: &Resources<'a>,
) -> Result<(), Never> {
  let Ok(keys) = dict.keys();

  for key in keys {
    let Ok(handled) = handle_gs_single(dict, key.clone(), context, parent_resources);

    match handled {
      Some(()) => {}
      None => {
        let Ok(_said) = key.as_str();

        warn!("invalid value in graphics state for {}", _said);
      }
    }
  }

  Ok(())
}

pub(crate) fn handle_gs_single<'a>(
  dict: &Dict<'a>,
  key: Name<'_>,
  context: &mut Context<'a>,
  parent_resources: &Resources<'a>,
) -> Result<Option<()>, Never> {
  let Ok(word) = key.as_str();
  let Ok(state) = context.get_mut();
  let graphics = &mut state.graphics_state;

  match word {
    "LW" => set_number(dict, &key, &mut graphics.stroke_props.line_width),
    "ML" => set_number(dict, &key, &mut graphics.stroke_props.miter_limit),
    "CA" => set_number(dict, &key, &mut graphics.stroke_alpha.0),
    "ca" => set_number(dict, &key, &mut graphics.non_stroke_alpha.0),
    "LC" => {
      let Ok(found) = dict.get::<Number>(&key);
      let number = match found {
        Some(number) => number,
        None => return Ok(None),
      };
      let Ok(cap) = convert_line_cap(LineCap(number));

      graphics.stroke_props.line_cap = cap;

      Ok(Some(()))
    }
    "LJ" => {
      let Ok(found) = dict.get::<Number>(&key);
      let number = match found {
        Some(number) => number,
        None => return Ok(None),
      };
      let Ok(join) = convert_line_join(LineJoin(number));

      graphics.stroke_props.line_join = join;

      Ok(Some(()))
    }
    "TR" | "TR2" => {
      let Ok(transfer) = read_transfer_function(dict);

      match transfer {
        Transfer::Function(function) => {
          graphics.transfer_function = Some(function);

          Ok(Some(()))
        }
        Transfer::Identity => {
          graphics.transfer_function = None;

          Ok(Some(()))
        }
        Transfer::Invalid => Ok(None),
      }
    }
    "BM" => {
      let Ok(mode) = read_blend_mode(dict, &key);

      graphics.blend_mode = mode;

      Ok(Some(()))
    }
    "D" => read_dash(dict, &key, &mut graphics.stroke_props),
    "SMask" => set_soft_mask(dict, context, parent_resources),
    "Font" => set_font(dict, context),
    _ => Ok(Some(())),
  }
}

fn set_number(dict: &Dict<'_>, key: &Name<'_>, number: &mut f32) -> Result<Option<()>, Never> {
  let Ok(found) = dict.get::<f32>(key);

  Ok(match found {
    Some(found_2) => {
      *number = found_2;

      Some(())
    }
    None => None,
  })
}

enum Transfer {
  Function(ActiveTransferFunction),
  Identity,
  Invalid,
}

fn read_transfer_function(dict: &Dict<'_>) -> Result<Transfer, Never> {
  let Ok(second) = dict.get::<Object<'_>>(TR2);
  let Ok(first) = dict.get::<Object<'_>>(TR);
  let object = match second.or(first) {
    Some(object) => object,
    None => return Ok(Transfer::Invalid),
  };

  Ok(match object {
    Object::Array(array) => {
      let Ok(functions) = array.iter::<Object<'_>>();
      let functions_2: Option<Vec<Function>> = functions
        .take(4)
        .map(|object_2| {
          let Ok(function) = Function::new(&object_2);

          function
        })
        .collect();

      match functions_2.map(<[Function; 4]>::try_from) {
        Some(Ok(four)) => Transfer::Function(ActiveTransferFunction::Four(four)),
        Some(Err(_wrong_function_count)) => Transfer::Invalid,
        None => Transfer::Invalid,
      }
    }
    Object::Name(_) => Transfer::Identity,
    object_3 @ (Object::Null(_)
    | Object::Boolean(_)
    | Object::Number(_)
    | Object::String(_)
    | Object::Dict(_)
    | Object::Stream(_)) => {
      let Ok(function) = Function::new(&object_3);

      match function {
        Some(function_2) => Transfer::Function(ActiveTransferFunction::Single(function_2)),
        None => Transfer::Invalid,
      }
    }
  })
}

fn read_blend_mode(dict: &Dict<'_>, key: &Name<'_>) -> Result<BlendMode, Never> {
  let Ok(name) = dict.get::<Name<'_>>(key);
  let Ok(names) = dict.get::<Array<'_>>(key);
  let mut candidates = Vec::new();

  match (name, names) {
    (Some(name_2), _) => candidates.push(name_2),
    (None, Some(names_2)) => {
      let Ok(names_3) = names_2.iter::<Name<'_>>();

      candidates.extend(names_3);
    }
    (None, None) => {}
  }

  for candidate in candidates {
    let Ok(word) = candidate.as_str();
    let Ok(mode) = convert_blend_mode(word);

    match mode {
      Some(mode_2) => return Ok(mode_2),
      None => {}
    }
  }

  warn!("unknown blend mode, defaulting to Normal");

  Ok(BlendMode::Normal)
}

fn read_dash(
  dict: &Dict<'_>,
  key: &Name<'_>,
  stroke_props: &mut StrokeProps,
) -> Result<Option<()>, Never> {
  let Ok(found) = dict.get::<Array<'_>>(key);
  let array = match found {
    Some(array) => array,
    None => return Ok(None),
  };
  let Ok(mut parts) = array.iter::<Object<'_>>();
  let (dashes, phase) = match (parts.next(), parts.next()) {
    (Some(dashes), Some(phase)) => (dashes, phase),
    _ => return Ok(None),
  };
  let Ok(dashes_2) = dashes.into_array();
  let Ok(phase_2) = phase.into_number();
  let (dashes_4, phase_4) = match (dashes_2, phase_2) {
    (Some(dashes_3), Some(phase_3)) => (dashes_3, phase_3),
    _ => return Ok(None),
  };
  let Ok(phase_5) = phase_4.as_f32();
  let Ok(lengths) = dashes_4.iter::<f32>();

  stroke_props.dash_offset = phase_5;
  stroke_props.dash_array = lengths
    .map(|length| match length.classify() {
      ::core::num::FpCategory::Zero => 0.01,
      ::core::num::FpCategory::Nan
      | ::core::num::FpCategory::Infinite
      | ::core::num::FpCategory::Subnormal
      | ::core::num::FpCategory::Normal => length,
    })
    .collect();

  Ok(Some(()))
}

fn set_soft_mask<'a>(
  dict: &Dict<'a>,
  context: &mut Context<'a>,
  parent_resources: &Resources<'a>,
) -> Result<Option<()>, Never> {
  let Ok(name) = dict.get::<Name<'_>>(SMASK);

  match name {
    Some(name_2) => match ::core::primitive::bool::then_some(name_2.deref() == b"None", ()) {
      ::core::option::Option::Some(()) => {
        let Ok(state) = context.get_mut();

        state.graphics_state.soft_mask = None;
      }
      ::core::option::Option::None => {}
    },
    None => {
      let Ok(found) = dict.get::<Dict<'_>>(SMASK);
      let mask = match found {
        Some(mask) => {
          let Ok(mask_2) = SoftMask::new(&mask, context, parent_resources.clone());

          mask_2
        }
        None => None,
      };
      let Ok(state) = context.get_mut();

      state.graphics_state.soft_mask = mask;
    }
  }

  Ok(Some(()))
}

fn set_font<'a>(dict: &Dict<'a>, context: &mut Context<'a>) -> Result<Option<()>, Never> {
  let Ok(found) = dict.get::<Array<'_>>(FONT);
  let array = match found {
    Some(array) => array,
    None => return Ok(None),
  };
  let Ok(mut parts) = array.iter::<Object<'_>>();
  let (font, size) = match (parts.next(), parts.next()) {
    (Some(font), Some(size)) => (font, size),
    _ => return Ok(None),
  };
  let Ok(font_2) = font.into_dict();
  let Ok(size_2) = size.into_number();
  let (font_dict, size_4) = match (font_2, size_2) {
    (Some(font_3), Some(size_3)) => (font_3, size_3),
    _ => return Ok(None),
  };
  let Ok(size_5) = size_4.as_f32();
  let Ok(font_4) = context.resolve_font(&font_dict);
  let Ok(state) = context.get_mut();

  state.text_state.font_size = TextSpace(size_5);
  state.text_state.font = font_4;

  Ok(Some(()))
}

fn convert_blend_mode(name: &str) -> Result<Option<BlendMode>, Never> {
  Ok(Some(match name {
    "Normal" => BlendMode::Normal,
    "Multiply" => BlendMode::Multiply,
    "Screen" => BlendMode::Screen,
    "Overlay" => BlendMode::Overlay,
    "Darken" => BlendMode::Darken,
    "Lighten" => BlendMode::Lighten,
    "ColorDodge" => BlendMode::ColorDodge,
    "ColorBurn" => BlendMode::ColorBurn,
    "HardLight" => BlendMode::HardLight,
    "SoftLight" => BlendMode::SoftLight,
    "Difference" => BlendMode::Difference,
    "Exclusion" => BlendMode::Exclusion,
    "Hue" => BlendMode::Hue,
    "Saturation" => BlendMode::Saturation,
    "Color" => BlendMode::Color,
    "Luminosity" => BlendMode::Luminosity,
    _ => return Ok(None),
  }))
}
