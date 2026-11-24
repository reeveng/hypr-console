use crate::CMapResolverFn;
use crate::context::Context;
use crate::device::Device;
use crate::font::glyph_simulator::GlyphSimulator;
use crate::font::true_type::{Width, read_encoding, read_widths};
use crate::font::{Encoding, Glyph, Type3Glyph, UNITS_PER_EM, read_to_unicode};
use crate::interpret::state::TextState;
use crate::soft_mask::SoftMask;
use crate::util::RectExt;
use crate::{BlendMode, interpret};
use crate::{CacheKey, ClipPath, GlyphDrawMode, PathDrawMode};
use crate::{Image, Paint};
use console_core_character_maps::{BfString, CMap};
use console_core_font_metrics::GlyphId;
use console_core_portable_document_syntax::content::TypedIter;
use console_core_portable_document_syntax::content::ops::TypedInstruction;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::{
  CHAR_PROCS, FONT_BBOX, FONT_MATRIX, RESOURCES,
};
use console_core_portable_document_syntax::page::Resources;
use kurbo::{Affine, BezPath, Rect};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct Type3<'a> {
  widths: Vec<Width>,
  missing_width: crate::font::FontUnits,
  encoding: Encoding,
  encodings: HashMap<u8, String>,
  dict: Dict<'a>,
  char_procs: HashMap<String, Stream<'a>>,
  glyph_simulator: GlyphSimulator,
  font_bbox: Rect,
  matrix: Affine,
  to_unicode: Option<CMap>,
}

impl<'a> Type3<'a> {
  pub(crate) fn new(
    dict: &Dict<'a>,
    cmap_resolver: &CMapResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok((encoding, encodings)) = read_encoding(dict);
    let Ok(measured_widths) = read_widths(dict, dict);
    let (widths, missing_width) = match measured_widths {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(bbox_entry) =
      dict.get::<console_core_portable_document_syntax::object::Rectangle>(FONT_BBOX);
    let font_bbox = match bbox_entry {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => {
        console_core_portable_document_syntax::object::Rectangle::ZERO
      }
    }
    .to_kurbo();

    let Ok(matrix_entry) = dict.get::<[f64; 6]>(FONT_MATRIX);
    let matrix = Affine::new(match matrix_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [0.001, 0.0, 0.0, 0.001, 0.0, 0.0],
    });

    let char_procs = {
      let mut procs = HashMap::new();
      let Ok(procedures_entry) = dict.get::<Dict<'_>>(CHAR_PROCS);
      let dict_2 = match procedures_entry {
        Some(present_value) => present_value,
        None => Default::default(),
      };

      let Ok(keys) = dict_2.keys();

      for name in keys {
        let Ok(procedure_stream) = dict_2.get::<Stream<'_>>(name.as_ref());

        match procedure_stream {
          Some(prog) => {
            procs.insert(
              ({
                let Ok(explicit_value) = name.as_str();
                explicit_value
              })
              .to_string(),
              prog,
            );
          }
          None => {}
        }
      }

      procs
    };

    let Ok(to_unicode) = read_to_unicode(dict, cmap_resolver);

    let Ok(glyph_simulator) = GlyphSimulator::new();

    Ok(Some(Self {
      glyph_simulator,
      encoding,
      font_bbox,
      char_procs,
      widths,
      missing_width,
      encodings,
      matrix,
      dict: dict.clone(),
      to_unicode,
    }))
  }

  pub(crate) fn map_code(
    &self,
    code: u8,
  ) -> core::result::Result<GlyphId, console_core_never::Never> {
    Ok(
      match self
        .encodings
        .get(&code)
        .map(|s| s.as_str())
        .or_else(|| {
          let Ok(map_code) = self.encoding.map_code(code);

          map_code
        })
        .map(|g| {
          let Ok(string_to_glyph) = self.glyph_simulator.string_to_glyph(g);

          string_to_glyph
        }) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => GlyphId::NOTDEF,
      },
    )
  }

  pub(crate) fn glyph_width(
    &self,
    code: u8,
  ) -> core::result::Result<f32, console_core_never::Never> {
    let w = match self
      .widths
      .get({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(code);
        numeric_value
      })
      .copied()
    {
      Some(Width::Value(w)) => w,
      ::core::option::Option::None | ::core::option::Option::Some(_) => self.missing_width.0,
    };

    Ok(
      (w * match ::console_core_number_conversion::nearest_f32_from_f64(*match &self
        .matrix
        .as_coeffs()
      {
        [element, ..] => element,
      }) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }) * UNITS_PER_EM,
    )
  }

  pub(crate) fn char_code_to_unicode(
    &self,
    char_code: u32,
  ) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    Ok(
      self
        .to_unicode
        .as_ref()
        .and_then(|t| {
          let Ok(lookup_bf_string) = t.lookup_bf_string(char_code);

          lookup_bf_string
        })
        .or_else(|| {
          let code = match u8::try_from(char_code) {
            ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
            ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
          }?;
          let name = self.encodings.get(&code).map(|s| s.as_str()).or_else(|| {
            let Ok(map_code) = self.encoding.map_code(code);

            map_code
          });
          name
            .and_then(|explicit_argument_0| {
              let Ok(glyph_name_to_unicode) =
                crate::font::glyph_name_to_unicode(explicit_argument_0);

              glyph_name_to_unicode
            })
            .or_else(|| {
              (0x20..0x7f)
                .contains(&code)
                .then_some(<char as ::core::convert::From<::core::primitive::u8>>::from(code))
            })
            .map(BfString::Char)
        }),
    )
  }

  pub(crate) fn render_glyph(
    &self,
    glyph: &Type3Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    paint: &Paint<'a>,
    device: &mut impl Device<'a>,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(
      glyph.nesting_depth > crate::context::MAX_PAINT_NESTING
        || !matches!(
          {
            let Ok(explicit_value) = crate::context::take_nested_paint(glyph.nesting_depth);
            explicit_value
          },
          crate::PaintBudget::Granted
        ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let mut state = glyph.state.clone();
    let root_transform =
      transform * glyph_transform * self.matrix * Affine::scale(f64::from(UNITS_PER_EM));
    state.ctm = root_transform;

    state.text_state = TextState::default();

    let Ok(glyph_to_string) = self.glyph_simulator.glyph_to_string(glyph.glyph_id);
    let name = match glyph_to_string {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let program = match self.char_procs.get(&name) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let decoded = match match program.decoded() {
      Ok(parsed_value) => Some(parsed_value),
      Err(_invalid_stream_encoding) => None,
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(iter) = TypedIter::new(decoded.as_ref());

    let is_shape_glyph = {
      let mut program_2 = iter.clone();
      let Ok(bytes) = console_core_number_conversion::fitted::<_, u32>(decoded.len());
      let mut shape = true;

      for _ in 0..bytes {
        let Ok(next) = program_2.next();
        let operation = match next {
          Some(operation) => operation,
          None => break,
        };

        match (
          matches!(operation, TypedInstruction::ShapeGlyph(_)),
          matches!(operation, TypedInstruction::ColorGlyph(_)),
        ) {
          (true, true | false) => break,
          (false, true) => {
            shape = false;
            break;
          }
          (false, false) => {}
        }
      }

      shape
    };

    let Ok(mut context) = Context::new_with(
      state.ctm,
      self.font_bbox,
      &glyph.cache,
      glyph.xref,
      glyph.settings.clone(),
      state,
      glyph.nesting_depth,
    );

    let Ok(dict) = self.dict.get(RESOURCES);
    let Ok(mut resources) = Resources::from_parent(
      match dict {
        Some(present_value) => present_value,
        None => Default::default(),
      },
      glyph.parent_resources.clone(),
    );

    let Ok(program_dict) = program.dict();

    let Ok(resources_entry) = program_dict.get::<Dict<'_>>(RESOURCES);

    match resources_entry {
      Some(procs_resources) => {
        resources = {
          let Ok(explicit_value) = Resources::from_parent(procs_resources, resources);
          explicit_value
        };
      }
      None => {}
    }

    match ::core::primitive::bool::then_some(is_shape_glyph, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(mut device_2) = Type3ShapeGlyphDevice::new(device, paint.clone());
        let Ok(()) = interpret(iter, &resources, &mut context, &mut device_2);
      }
      ::core::option::Option::None => {
        let Ok(()) = interpret(iter, &resources, &mut context, device);
      }
    }

    Ok(Some(()))
  }
}

impl CacheKey for Type3<'_> {
  fn cache_key(&self) -> u128 {
    self.dict.cache_key()
  }
}

struct Type3ShapeGlyphDevice<'a, 'b, T: Device<'a>> {
  inner: &'b mut T,
  paint: Paint<'a>,
}

impl<'a, 'b, T: Device<'a>> Type3ShapeGlyphDevice<'a, 'b, T> {
  pub(crate) fn new(
    device: &'b mut T,
    paint: Paint<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      inner: device,
      paint,
    })
  }
}

impl<'a, T: Device<'a>> Device<'a> for Type3ShapeGlyphDevice<'a, '_, T> {
  fn set_soft_mask(&mut self, m: Option<SoftMask<'a>>) {
    self.inner.set_soft_mask(m);
  }

  fn draw_path(
    &mut self,
    path: &BezPath,
    transform: Affine,
    _: &Paint<'_>,
    draw_mode: &PathDrawMode,
  ) {
    self
      .inner
      .draw_path(path, transform, &self.paint, draw_mode);
  }

  fn push_clip_path(&mut self, clip_path: &ClipPath) {
    self.inner.push_clip_path(clip_path);
  }

  fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}

  fn draw_glyph(
    &mut self,
    g: &Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    p: &Paint<'a>,
    draw_mode: &GlyphDrawMode,
  ) {
    self
      .inner
      .draw_glyph(g, transform, glyph_transform, p, draw_mode);
  }

  fn pop_clip_path(&mut self) {
    self.inner.pop_clip_path();
  }

  fn pop_transparency_group(&mut self) {}

  fn draw_image(&mut self, image: Image<'a, '_>, transform: Affine) {
    match image {
      Image::Stencil(mut stencil) => {
        stencil.paint = self.paint.clone();
        self.inner.draw_image(Image::Stencil(stencil), transform);
      }
      Image::Raster(_) => {}
    }
  }

  fn set_blend_mode(&mut self, _: BlendMode) {}
}
