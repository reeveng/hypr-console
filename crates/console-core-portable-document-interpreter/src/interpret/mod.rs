use crate::color::ColorSpace;
use crate::context::Context;
use crate::convert::{convert_line_cap, convert_line_join};
use crate::device::Device;
use crate::font::{Font, FontData, FontQuery, StandardFont};
use crate::interpret::path::{
  close_path, fill_path, fill_path_impl, fill_stroke_path, stroke_path,
};
use crate::interpret::state::{
  Opacity, TextAdjustment, TextScalePercent, TextSpace, TextStateFont, handle_gs,
};
use crate::interpret::text::TextRenderingMode;
use crate::pattern::{Pattern, ShadingPattern};
use crate::shading::Shading;
use crate::util::{OptionLog, RectExt};
use crate::x_object::{
  FormXObject, ImageXObject, XObject, draw_form_xobject, draw_image_xobject, draw_xobject,
};
use crate::{BlendMode, FillRule};
use console_core_never::Never;
use console_core_portable_document_syntax::content::TypedIter;
use console_core_portable_document_syntax::content::ops::TypedInstruction;
use console_core_portable_document_syntax::object::dict::keys::{
  ANNOTS, AP, AS, C, CA, F, MCID, N, OC, QUADPOINTS, RECT, SUBTYPE,
};
use console_core_portable_document_syntax::object::{
  Array, Dict, Name, Object, Rect, Stream, dict_or_stream,
};
use console_core_portable_document_syntax::page::{Page, Resources};
use kurbo::{Affine, BezPath, Point, Shape};
use std::sync::Arc;

pub(crate) mod path;
pub(crate) mod state;
pub(crate) mod text;

pub use state::ActiveTransferFunction;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public font resolver is shared by cached graphics states and deferred devices across threads"
  )
)]
pub type FontResolverFn = Arc<dyn Fn(&FontQuery) -> Option<(FontData, u32)> + Send + Sync>;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public character-map resolver is retained by independently interpreted device operations"
  )
)]
pub type CMapResolverFn =
  Arc<dyn Fn(console_core_character_maps::CMapName<'_>) -> Option<&'static [u8]> + Send + Sync>;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public warning sink is retained through deferred image decoding and is callable from device threads"
  )
)]
pub type WarningSinkFn = Arc<dyn Fn(InterpreterWarning) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationRendering {
  All,
  WidgetsAndLinks,
  Omit,
}

#[derive(Clone)]

pub struct InterpreterSettings {
  pub font_resolver: FontResolverFn,

  pub cmap_resolver: CMapResolverFn,

  pub warning_sink: WarningSinkFn,

  pub annotations: AnnotationRendering,

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public optional-content override list is shared by all contexts for the same reader"
    )
  )]
  pub ocg_overrides: Arc<Vec<(i32, i32, crate::Visibility)>>,
}

impl Default for InterpreterSettings {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Default settings initialize the public retained callbacks and overrides used by cloned contexts"
    )
  )]
  fn default() -> Self {
    Self {
      #[cfg(not(feature = "embed-fonts"))]
      font_resolver: Arc::new(|_| None),
      #[cfg(feature = "embed-fonts")]
      font_resolver: Arc::new(|query| match query {
        FontQuery::Standard(s) => {
          let Ok(font_data) = s.get_font_data();

          Some(font_data)
        }
        FontQuery::Fallback(f) => {
          let Ok(pick_standard_font) = f.pick_standard_font();
          let Ok(font_data) = pick_standard_font.get_font_data();

          Some(font_data)
        }
      }),
      #[cfg(feature = "embed-cmaps")]
      cmap_resolver: Arc::new(|name| {
        let Ok(load_embedded) = console_core_character_maps::load_embedded(name);

        load_embedded
      }),
      #[cfg(not(feature = "embed-cmaps"))]
      cmap_resolver: Arc::new(|_| None),
      warning_sink: Arc::new(|_| {}),
      annotations: AnnotationRendering::All,
      ocg_overrides: Arc::new(Vec::new()),
    }
  }
}

#[derive(Copy, Clone, Debug)]

pub enum InterpreterWarning {
  UnsupportedFont,

  ImageDecodeFailure,
}

pub fn interpret_page<'a>(
  page: &Page<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(resources) = page.resources();
  let Ok(typed_operations) = page.typed_operations();
  let Ok(()) = interpret(typed_operations, resources, context, device);
  let annotations = context.settings.annotations;

  match annotations {
    AnnotationRendering::Omit => return Ok(()),
    AnnotationRendering::All | AnnotationRendering::WidgetsAndLinks => {}
  }

  let Ok(page_dict) = page.raw();
  let Ok(entries) = page_dict.get::<Array<'_>>(ANNOTS);
  let array = match entries {
    Some(array) => array,
    None => return Ok(()),
  };
  let Ok(iter) = array.iter::<Dict<'_>>();

  for annotation in iter {
    let Ok(flags) = annotation.get::<u32>(F);

    match flags.filter(|bits| bits & (2 | 32) != 0) {
      Some(_hidden_flags) => continue,
      None => {}
    }

    let Ok(subtype) = annotation.get::<Name<'_>>(SUBTYPE);

    match (annotations, subtype.as_deref()) {
      (AnnotationRendering::WidgetsAndLinks, Some(b"Widget" | b"Link")) => {}
      (AnnotationRendering::WidgetsAndLinks, Some(_) | None) => continue,
      (AnnotationRendering::All | AnnotationRendering::Omit, _) => {}
    }

    let Ok(appearance) = annotation_appearance(&annotation);

    match appearance {
      Some(form) => {
        let Ok(()) = draw_annotation_form(&annotation, &form, resources, context, device);
      }
      None => match subtype.as_deref() {
        Some(b"Highlight") => {
          let Ok(()) = draw_highlight_without_appearance(&annotation, context, device);
        }
        Some(_) | None => {}
      },
    }
  }

  Ok(())
}

fn annotation_appearance<'a>(annotation: &Dict<'a>) -> Result<Option<FormXObject<'a>>, Never> {
  let Ok(appearances) = annotation.get::<Dict<'_>>(AP);
  let appearance = match appearances {
    Some(appearance) => appearance,
    None => return Ok(None),
  };
  let Ok(direct) = appearance.get::<Stream<'_>>(N);

  match direct {
    Some(stream) => return FormXObject::new(&stream),
    None => {}
  }

  let Ok(states) = appearance.get::<Dict<'_>>(N);
  let Ok(state) = annotation.get::<Name<'_>>(AS);
  let (states_3, state_3) = match (states, state) {
    (Some(states_2), Some(state_2)) => (states_2, state_2),
    (None, _) | (_, None) => return Ok(None),
  };
  let Ok(normal) = states_3.get::<Stream<'_>>(state_3.as_ref());

  match normal {
    Some(stream) => FormXObject::new(&stream),
    None => Ok(None),
  }
}

fn draw_annotation_form<'a>(
  annotation: &Dict<'a>,
  form: &FormXObject<'a>,
  resources: &Resources<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(rect) = annotation.get::<Rect>(RECT);
  let annot_rect = match rect {
    Some(rect_2) => rect_2.to_kurbo(),
    None => return Ok(()),
  };
  let [x0, y0, x1, y1] = form.bbox.map(f64::from);
  let transformed_rect =
    (form.matrix * kurbo::Rect::new(x0, y0, x1, y1).to_path(0.1)).bounding_box();
  let sx = annot_rect.width() / transformed_rect.width();
  let sy = annot_rect.height() / transformed_rect.height();
  let affine = Affine::new([
    sx,
    0.0,
    0.0,
    sy,
    annot_rect.x0 - sx * transformed_rect.x0,
    annot_rect.y0 - sy * transformed_rect.y0,
  ]);

  match (transformed_rect.is_finite()
    && transformed_rect.width() > 0.0
    && transformed_rect.height() > 0.0
    && affine.is_finite())
  .then_some(())
  {
    Some(()) => {}
    None => return Ok(()),
  }

  let Ok(()) = context.save_state();
  let Ok(()) = context.pre_concat_affine(affine);
  let Ok(()) = context.push_root_transform();
  let Ok(()) = draw_form_xobject(resources, form, context, device);
  let Ok(()) = context.pop_root_transform();
  context.restore_state(device)
}

fn draw_highlight_without_appearance<'a>(
  annot: &Dict<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let numbers = |key| -> Option<Vec<f32>> {
      let Ok(entry) = annot.get::<Array<'_>>(key);
      let array = entry?;
      let Ok(iter) = array.iter::<Object<'_>>();

      iter
        .map(|o| match o {
          Object::Number(n) => Some({
            let Ok(explicit_value) = n.as_f32();
            explicit_value
          }),
          Object::Null(_)
          | Object::Boolean(_)
          | Object::Name(_)
          | Object::String(_)
          | Object::Dict(_)
          | Object::Array(_)
          | Object::Stream(_) => None,
        })
        .collect()
    };
    let quad_points =
      match numbers(QUADPOINTS).filter(|q| !q.is_empty() && q.len().wrapping_rem(8) == 0) {
        Some(quad_points) => quad_points,
        None => {
          return Ok(());
        }
      };
    let (color_space, color) = match numbers(C).as_deref() {
      Some(&[g]) => {
        let Ok(device_gray) = ColorSpace::device_gray();

        (device_gray, vec![g])
      }
      Some(&[r, g, b]) => {
        let Ok(device_rgb) = ColorSpace::device_rgb();

        (device_rgb, vec![r, g, b])
      }
      Some(&[c, m, y, k]) => {
        let Ok(device_cmyk) = ColorSpace::device_cmyk();

        (device_cmyk, vec![c, m, y, k])
      }
      ::core::option::Option::None | ::core::option::Option::Some(_) => return Ok(()),
    };

    let mut path = BezPath::new();

    for &[x1, y1, x2, y2, x3, y3, x4, y4] in quad_points.as_chunks::<8>().0 {
      path.move_to((f64::from(x1), f64::from(y1)));
      path.line_to((f64::from(x2), f64::from(y2)));
      path.line_to((f64::from(x4), f64::from(y4)));
      path.line_to((f64::from(x3), f64::from(y3)));
      path.close_path();
    }

    let Ok(()) = context.save_state();
    let state = &mut ({
      let Ok(explicit_value) = context.get_mut();
      explicit_value
    })
    .graphics_state;
    state.none_stroke_cs = color_space;
    state.non_stroke_color = color;
    state.non_stroke_pattern = None;
    let Ok(opacity_entry) = annot.get::<f32>(CA);
    state.non_stroke_alpha = Opacity(
      match opacity_entry {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 1.0,
      }
      .clamp(0.0, 1.0),
    );
    state.soft_mask = None;
    state.blend_mode = BlendMode::Multiply;
    context.path = path;
    let Ok(()) = fill_path(context, device, FillRule::NonZero);
    let Ok(()) = context.restore_state(device);
  };

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit053_no_while",
  expect(
    explicit053_no_while,
    reason = "TypedIter lends each operator from its content reader and reaches None when its input is exhausted"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Form and glyph dispatch reenter the interpreter only after consuming the context's bounded nesting budget"
  )
)]
pub fn interpret<'a>(
  mut ops: TypedIter<'_>,
  resources: &Resources<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(initial_states) = context.num_states();
  let Ok(()) = context.save_state();

  while let Some(op) = {
    let Ok(next) = ops.next();
    next
  } {
    let Ok(()) = execute_operation(op, resources, context, device);
  }

  let Ok(final_states) = context.num_states();

  for _restored in initial_states..final_states {
    let Ok(()) = context.restore_state(device);
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every form operator that can reenter interpretation passes the fixed depth and shared paint accounting"
  )
)]
fn execute_operation<'a>(
  op: TypedInstruction<'_, '_>,
  resources: &Resources<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  match op {
    TypedInstruction::SaveState(_) => {
      let Ok(save_state) = context.save_state();

      save_state
    }
    TypedInstruction::StrokeColorDeviceRgb(s) => {
      let Ok(device_rgb) = ColorSpace::device_rgb();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_cs = device_rgb;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_color = vec![
        ({
          let Ok(explicit_value) = s.0.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.1.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.2.as_f32();
          explicit_value
        }),
      ];
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_pattern = None;
    }
    TypedInstruction::StrokeColorDeviceGray(s) => {
      let Ok(device_gray) = ColorSpace::device_gray();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_cs = device_gray;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_color = vec![
        ({
          let Ok(explicit_value) = s.0.as_f32();
          explicit_value
        }),
      ];
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_pattern = None;
    }
    TypedInstruction::StrokeColorCmyk(s) => {
      let Ok(device_cmyk) = ColorSpace::device_cmyk();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_cs = device_cmyk;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_color = vec![
        ({
          let Ok(explicit_value) = s.0.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.1.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.2.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.3.as_f32();
          explicit_value
        }),
      ];
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_pattern = None;
    }
    TypedInstruction::LineWidth(w) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_props
      .line_width = {
        let Ok(explicit_value) = w.0.as_f32();
        explicit_value
      };
    }
    TypedInstruction::LineCap(c) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_props
      .line_cap = {
        let Ok(explicit_value) = convert_line_cap(c);
        explicit_value
      };
    }
    TypedInstruction::LineJoin(j) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_props
      .line_join = {
        let Ok(explicit_value) = convert_line_join(j);
        explicit_value
      };
    }
    TypedInstruction::MiterLimit(l) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_props
      .miter_limit = {
        let Ok(explicit_value) = l.0.as_f32();
        explicit_value
      };
    }
    TypedInstruction::Transform(t) => {
      let Ok(()) = context.pre_concat_transform(t);
    }
    TypedInstruction::RectPath(r) => {
      let rect = kurbo::Rect::new(
        {
          let Ok(explicit_value) = r.0.as_f64();
          explicit_value
        },
        {
          let Ok(explicit_value) = r.1.as_f64();
          explicit_value
        },
        ({
          let Ok(explicit_value) = r.0.as_f64();
          explicit_value
        }) + ({
          let Ok(explicit_value) = r.2.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = r.1.as_f64();
          explicit_value
        }) + ({
          let Ok(explicit_value) = r.3.as_f64();
          explicit_value
        }),
      )
      .to_path(0.1);
      let path_mut = &mut context.path;
      path_mut.extend(rect);
    }
    TypedInstruction::MoveTo(m) => {
      let p = Point::new(
        {
          let Ok(explicit_value) = m.0.as_f64();
          explicit_value
        },
        {
          let Ok(explicit_value) = m.1.as_f64();
          explicit_value
        },
      );
      context.last_point = p;
      context.sub_path_start = p;
      let path_mut = &mut context.path;
      path_mut.move_to(p);
    }
    TypedInstruction::FillPathEvenOdd(_) => {
      let Ok(()) = fill_path(context, device, FillRule::EvenOdd);
    }
    TypedInstruction::FillPathNonZero(_) => {
      let Ok(()) = fill_path(context, device, FillRule::NonZero);
    }
    TypedInstruction::FillPathNonZeroCompatibility(_) => {
      let Ok(()) = fill_path(context, device, FillRule::NonZero);
    }
    TypedInstruction::FillAndStrokeEvenOdd(_) => {
      let Ok(()) = fill_stroke_path(context, device, FillRule::EvenOdd);
    }
    TypedInstruction::FillAndStrokeNonZero(_) => {
      let Ok(()) = fill_stroke_path(context, device, FillRule::NonZero);
    }
    TypedInstruction::CloseAndStrokePath(_) => {
      let Ok(()) = close_path(context);
      let Ok(()) = stroke_path(context, device);
    }
    TypedInstruction::CloseFillAndStrokeEvenOdd(_) => {
      let Ok(()) = close_path(context);
      let Ok(()) = fill_stroke_path(context, device, FillRule::EvenOdd);
    }
    TypedInstruction::CloseFillAndStrokeNonZero(_) => {
      let Ok(()) = close_path(context);
      let Ok(()) = fill_stroke_path(context, device, FillRule::NonZero);
    }
    TypedInstruction::NonStrokeColorDeviceGray(s) => {
      let Ok(device_gray) = ColorSpace::device_gray();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .none_stroke_cs = device_gray;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_color = vec![
        ({
          let Ok(explicit_value) = s.0.as_f32();
          explicit_value
        }),
      ];
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_pattern = None;
    }
    TypedInstruction::NonStrokeColorDeviceRgb(s) => {
      let Ok(device_rgb) = ColorSpace::device_rgb();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .none_stroke_cs = device_rgb;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_color = vec![
        ({
          let Ok(explicit_value) = s.0.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.1.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.2.as_f32();
          explicit_value
        }),
      ];
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_pattern = None;
    }
    TypedInstruction::NonStrokeColorCmyk(s) => {
      let Ok(device_cmyk) = ColorSpace::device_cmyk();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .none_stroke_cs = device_cmyk;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_color = vec![
        ({
          let Ok(explicit_value) = s.0.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.1.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.2.as_f32();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = s.3.as_f32();
          explicit_value
        }),
      ];
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_pattern = None;
    }
    TypedInstruction::LineTo(line) => {
      let Ok(x) = line.0.as_f64();
      let Ok(y) = line.1.as_f64();
      let Ok(()) = append_line(context, Point::new(x, y));
    }
    TypedInstruction::CubicTo(c) => {
      let path = &context.path;

      match ::core::primitive::bool::then_some(!path.elements().is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          let p1 = Point::new(
            {
              let Ok(explicit_value) = c.0.as_f64();
              explicit_value
            },
            {
              let Ok(explicit_value) = c.1.as_f64();
              explicit_value
            },
          );
          let p2 = Point::new(
            {
              let Ok(explicit_value) = c.2.as_f64();
              explicit_value
            },
            {
              let Ok(explicit_value) = c.3.as_f64();
              explicit_value
            },
          );
          let p3 = Point::new(
            {
              let Ok(explicit_value) = c.4.as_f64();
              explicit_value
            },
            {
              let Ok(explicit_value) = c.5.as_f64();
              explicit_value
            },
          );

          context.last_point = p3;

          let path_mut = &mut context.path;
          path_mut.curve_to(p1, p2, p3);
        }
        ::core::option::Option::None => {}
      }
    }
    TypedInstruction::CubicStartTo(c) => {
      let path = &context.path;

      match ::core::primitive::bool::then_some(!path.elements().is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          let context_last_point = &context.last_point;
          let p1 = *context_last_point;
          let p2 = Point::new(
            {
              let Ok(explicit_value) = c.0.as_f64();
              explicit_value
            },
            {
              let Ok(explicit_value) = c.1.as_f64();
              explicit_value
            },
          );
          let p3 = Point::new(
            {
              let Ok(explicit_value) = c.2.as_f64();
              explicit_value
            },
            {
              let Ok(explicit_value) = c.3.as_f64();
              explicit_value
            },
          );

          context.last_point = p3;

          let path_mut = &mut context.path;
          path_mut.curve_to(p1, p2, p3);
        }
        ::core::option::Option::None => {}
      }
    }
    TypedInstruction::CubicEndTo(c) => {
      let path = &context.path;

      match ::core::primitive::bool::then_some(!path.elements().is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          let p2 = Point::new(
            {
              let Ok(explicit_value) = c.0.as_f64();
              explicit_value
            },
            {
              let Ok(explicit_value) = c.1.as_f64();
              explicit_value
            },
          );
          let p3 = Point::new(
            {
              let Ok(explicit_value) = c.2.as_f64();
              explicit_value
            },
            {
              let Ok(explicit_value) = c.3.as_f64();
              explicit_value
            },
          );

          context.last_point = p3;

          let path_mut = &mut context.path;
          path_mut.curve_to(p2, p3, p3);
        }
        ::core::option::Option::None => {}
      }
    }
    TypedInstruction::ClosePath(_) => {
      let Ok(()) = close_path(context);
    }
    TypedInstruction::SetGraphicsState(gs) => {
      let Ok(ext_g_state) = resources.get_ext_g_state(gs.0);

      match ext_g_state.warn_none(&format!(
        "failed to get extgstate {}",
        ({
          let Ok(explicit_value) = gs.0.as_str();
          explicit_value
        })
      )) {
        Some(gs_2) => {
          let Ok(()) = handle_gs(&gs_2, context, resources);
        }
        None => {}
      }
    }
    TypedInstruction::StrokePath(_) => {
      let Ok(()) = stroke_path(context, device);
    }
    TypedInstruction::EndPath(_) => {
      match (context.clip, context.path.elements()) {
        (Some(clip), [_, ..]) => {
          let Ok(state) = context.get();
          let clip_path = state.ctm * context.path.clone();
          let Ok(()) = context.push_clip_path(clip_path, clip, device);
          context.clip = None;
        }
        _ => {}
      }

      let path_mut = &mut context.path;
      path_mut.truncate(0);
    }
    TypedInstruction::NonStrokeColor(c) => {
      let gs = &mut ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state;
      gs.non_stroke_color = c
        .0
        .into_iter()
        .map(|n| {
          let Ok(explicit_value) = n.as_f32();
          explicit_value
        })
        .collect();
      gs.non_stroke_pattern = None;
    }
    TypedInstruction::StrokeColor(c) => {
      let gs = &mut ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state;
      gs.stroke_color = c
        .0
        .into_iter()
        .map(|n| {
          let Ok(explicit_value) = n.as_f32();
          explicit_value
        })
        .collect();
      gs.stroke_pattern = None;
    }
    TypedInstruction::ClipNonZero(_) => {
      context.clip = Some(FillRule::NonZero);
    }
    TypedInstruction::ClipEvenOdd(_) => {
      context.clip = Some(FillRule::EvenOdd);
    }
    TypedInstruction::RestoreState(_) => {
      let Ok(restore_state) = context.restore_state(device);

      restore_state
    }
    TypedInstruction::FlatnessTolerance(_) => {}
    TypedInstruction::ColorSpaceStroke(instruction) => {
      let Ok(cs) = resolve_color_space(context, resources, instruction.0);
      let Ok(is_pattern) = cs.is_pattern();

      match ::core::primitive::bool::then_some(
        !matches!(is_pattern, crate::PatternSpace::Pattern),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          ({
            let Ok(explicit_value) = context.get_mut();
            explicit_value
          })
          .graphics_state
          .stroke_pattern = None;
        }
        ::core::option::Option::None => {}
      }

      let Ok(initial_color) = cs.initial_color();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_color = initial_color;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_cs = cs;
    }
    TypedInstruction::ColorSpaceNonStroke(instruction) => {
      let Ok(cs) = resolve_color_space(context, resources, instruction.0);
      let Ok(is_pattern) = cs.is_pattern();

      match ::core::primitive::bool::then_some(
        !matches!(is_pattern, crate::PatternSpace::Pattern),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          ({
            let Ok(explicit_value) = context.get_mut();
            explicit_value
          })
          .graphics_state
          .non_stroke_pattern = None;
        }
        ::core::option::Option::None => {}
      }

      let Ok(initial_color) = cs.initial_color();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_color = initial_color;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .none_stroke_cs = cs;
    }
    TypedInstruction::DashPattern(p) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_props
      .dash_offset = {
        let Ok(explicit_value) = p.1.as_f32();
        explicit_value
      };

      let Ok(iter) = p.0.iter::<f32>();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_props
      .dash_array = iter
        .map(|n| match n.classify() {
          ::core::num::FpCategory::Zero => 0.01,
          ::core::num::FpCategory::Nan
          | ::core::num::FpCategory::Infinite
          | ::core::num::FpCategory::Subnormal
          | ::core::num::FpCategory::Normal => n,
        })
        .collect();
    }
    TypedInstruction::RenderingIntent(_) => {}
    TypedInstruction::NonStrokeColorNamed(n) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_color = n
        .0
        .into_iter()
        .map(|n_2| {
          let Ok(explicit_value) = n_2.as_f32();
          explicit_value
        })
        .collect();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .non_stroke_pattern = n.1.and_then(|name| {
        let Ok(pattern) = resources.get_pattern(name);

        pattern.and_then(|d| {
          let Ok(explicit_value) = Pattern::new(d, context, resources);
          explicit_value
        })
      });
    }
    TypedInstruction::StrokeColorNamed(n) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_color = n
        .0
        .into_iter()
        .map(|n_3| {
          let Ok(explicit_value) = n_3.as_f32();
          explicit_value
        })
        .collect();
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .graphics_state
      .stroke_pattern = n.1.and_then(|name| {
        let Ok(pattern) = resources.get_pattern(name);

        pattern.and_then(|d| {
          let Ok(explicit_value) = Pattern::new(d, context, resources);
          explicit_value
        })
      });
    }
    TypedInstruction::BeginMarkedContentWithProperties(bdc) => {
      let mcid = ({
        let Ok(explicit_value) = dict_or_stream(bdc.1);
        explicit_value
      })
      .and_then(|(props, _)| {
        let Ok(prop) = props.get::<i32>(MCID);

        prop
      });

      let Ok(bdc_name) = bdc.1.clone().into_name();
      let oc = bdc_name
        .and_then(|name| {
          let r = ({
            let Ok(explicit_value) = resources.properties.get_ref(name.as_ref());
            explicit_value
          })?;
          let Ok(property) = resources.properties.get::<Dict<'_>>(name);
          let d = match property {
            Some(present_value) => present_value,
            None => Default::default(),
          };
          Some((d, r))
        })
        .or_else(|| {
          let (props, _) = ({
            let Ok(explicit_value) = dict_or_stream(bdc.1);
            explicit_value
          })?;
          let r = ({
            let Ok(explicit_value) = props.get_ref(OC);
            explicit_value
          })?;
          let Ok(prop) = props.get::<Dict<'_>>(OC);
          let d = match prop {
            Some(present_value) => present_value,
            None => Default::default(),
          };
          Some((d, r))
        });

      match oc {
        Some((dict, oc_ref)) => {
          let Ok(()) = context.ocg_state.begin_ocg(&dict, <::console_core_portable_document_syntax::object::ObjectIdentifier as ::core::convert::From<::console_core_portable_document_syntax::object::ObjRef>>::from(oc_ref));
        }
        ::core::option::Option::None => {
          let Ok(()) = context.ocg_state.begin_marked_content();
        }
      }

      let Ok(()) = device.begin_marked_content(bdc.0, mcid);
    }
    TypedInstruction::MarkedContentPointWithProperties(_) => {}
    TypedInstruction::EndMarkedContent(_) => {
      let Ok(()) = context.ocg_state.end_marked_content();
      let Ok(()) = device.end_marked_content();
    }
    TypedInstruction::MarkedContentPoint(_) => {}
    TypedInstruction::BeginMarkedContent(bmc) => {
      let Ok(()) = context.ocg_state.begin_marked_content();
      let Ok(()) = device.begin_marked_content(bmc.0, None);
    }
    TypedInstruction::BeginText(_) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .text_matrix = Affine::IDENTITY;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .text_line_matrix = Affine::IDENTITY;
    }
    TypedInstruction::SetTextMatrix(m) => {
      let m_2 = Affine::new([
        ({
          let Ok(explicit_value) = m.0.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = m.1.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = m.2.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = m.3.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = m.4.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = m.5.as_f64();
          explicit_value
        }),
      ]);
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .text_line_matrix = m_2;
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .text_matrix = m_2;
    }
    TypedInstruction::EndText(_) => {
      let has_outline = ({
        let Ok(explicit_value) = context.get();
        explicit_value
      })
      .text_state
      .clip_paths
      .segments()
      .next()
      .is_some();

      match ::core::primitive::bool::then_some(has_outline, ()) {
        ::core::option::Option::Some(()) => {
          let clip_path = ({
            let Ok(explicit_value) = context.get();
            explicit_value
          })
          .ctm
            * ({
              let Ok(explicit_value) = context.get();
              explicit_value
            })
            .text_state
            .clip_paths
            .clone();

          let Ok(()) = context.push_clip_path(clip_path, FillRule::NonZero, device);
        }
        ::core::option::Option::None => {}
      }

      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .clip_paths
      .truncate(0);
    }
    TypedInstruction::TextFont(t) => {
      let name = t.0;

      let Ok(resources_font) = resources.get_font(name);
      let font = match resources_font {
        Some(font_dict) => {
          let Ok(resolve_font) = context.resolve_font(&font_dict);

          resolve_font
        }
        None => ({
          let Ok(explicit_value) =
            Font::new_standard(StandardFont::Helvetica, &context.settings.font_resolver);
          explicit_value
        })
        .map(TextStateFont::Fallback),
      };

      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .font_size = TextSpace({
        let Ok(explicit_value) = t.1.as_f32();
        explicit_value
      });
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .font = font;
    }
    TypedInstruction::ShowText(instruction) => {
      let Ok(()) = ensure_text_font(context);
      let Ok(()) = text::show_text_string(context, device, resources, instruction.0);
    }
    TypedInstruction::ShowTexts(instruction) => {
      let Ok(()) = ensure_text_font(context);
      let Ok(()) = show_positioned_texts(instruction.0, context, device, resources);
    }
    TypedInstruction::HorizontalScaling(h) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .horizontal_scaling = TextScalePercent({
        let Ok(explicit_value) = h.0.as_f32();
        explicit_value
      });
    }
    TypedInstruction::TextLeading(tl) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .leading = TextSpace({
        let Ok(explicit_value) = tl.0.as_f32();
        explicit_value
      });
    }
    TypedInstruction::CharacterSpacing(c) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .char_space = TextSpace({
        let Ok(explicit_value) = c.0.as_f32();
        explicit_value
      });
    }
    TypedInstruction::WordSpacing(w) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .word_space = TextSpace({
        let Ok(explicit_value) = w.0.as_f32();
        explicit_value
      });
    }
    TypedInstruction::NextLine(n) => {
      let (tx, ty) = (
        ({
          let Ok(explicit_value) = n.0.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = n.1.as_f64();
          explicit_value
        }),
      );
      let Ok(()) = text::next_line(context, kurbo::Vec2::new(tx, ty));
    }
    TypedInstruction::NextLineUsingLeading(_) => {
      let Ok(()) = text::next_line(
        context,
        kurbo::Vec2::new(
          0.0,
          f64::from(
            -({
              let Ok(explicit_value) = context.get();
              explicit_value
            })
            .text_state
            .leading
            .0,
          ),
        ),
      );
    }
    TypedInstruction::NextLineAndShowText(n) => {
      let Ok(()) = text::next_line(
        context,
        kurbo::Vec2::new(
          0.0,
          f64::from(
            -({
              let Ok(explicit_value) = context.get();
              explicit_value
            })
            .text_state
            .leading
            .0,
          ),
        ),
      );
      let Ok(()) = text::show_text_string(context, device, resources, n.0);
    }
    TypedInstruction::TextRenderingMode(r) => {
      let Ok(mode_number) = r.0.as_i64();
      let mode = match mode_number {
        0 => TextRenderingMode::Fill,
        1 => TextRenderingMode::Stroke,
        2 => TextRenderingMode::FillStroke,
        3 => TextRenderingMode::Invisible,
        4 => TextRenderingMode::FillAndClip,
        5 => TextRenderingMode::StrokeAndClip,
        6 => TextRenderingMode::FillAndStrokeAndClip,
        7 => TextRenderingMode::Clip,
        _ => {
          warn!(
            "unknown text rendering mode {}",
            ({
              let Ok(explicit_value) = r.0.as_i64();
              explicit_value
            })
          );

          TextRenderingMode::Fill
        }
      };

      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .render_mode = mode;
    }
    TypedInstruction::NextLineAndSetLeading(n) => {
      let (tx, ty) = (
        ({
          let Ok(explicit_value) = n.0.as_f64();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = n.1.as_f64();
          explicit_value
        }),
      );
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .leading = TextSpace(
        match ::console_core_number_conversion::nearest_f32_from_f64(-ty) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
      );
      let Ok(()) = text::next_line(context, kurbo::Vec2::new(tx, ty));
    }
    TypedInstruction::ShapeGlyph(_) => {}
    TypedInstruction::XObject(x) => {
      let cache = &context.interpreter_cache.object_cache;
      let transfer_function = ({
        let Ok(explicit_value) = context.get();
        explicit_value
      })
      .graphics_state
      .transfer_function
      .clone();
      let Ok(resources_x_object) = resources.get_x_object(x.0);

      match resources_x_object.and_then(|s| {
        let Ok(explicit_value) =
          XObject::new(&s, &context.settings.warning_sink, cache, transfer_function);
        explicit_value
      }) {
        Some(x_object) => {
          let Ok(()) = draw_xobject(&x_object, resources, context, device);
        }
        None => {}
      }
    }
    TypedInstruction::InlineImage(i) => {
      let transfer_function = ({
        let Ok(explicit_value) = context.get();
        explicit_value
      })
      .graphics_state
      .transfer_function
      .clone();
      let cache = context.interpreter_cache.object_cache.clone();
      let Ok(image_x_object) = ImageXObject::new(
        i.0,
        |name| {
          let Ok(color_space) = context.get_color_space(resources, name);

          color_space
        },
        &context.settings.warning_sink,
        &cache,
        crate::x_object::ImageSource::Image,
        transfer_function,
      );

      match image_x_object {
        Some(x_object) => {
          let Ok(()) = draw_image_xobject(&x_object, context, device);
        }
        None => {}
      }
    }
    TypedInstruction::TextRise(t) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .rise = TextSpace({
        let Ok(explicit_value) = t.0.as_f32();
        explicit_value
      });
    }
    TypedInstruction::Shading(instruction) => {
      let Ok(()) = draw_shading(instruction.0, resources, context, device);
    }
    TypedInstruction::BeginCompatibility(_) => {}
    TypedInstruction::EndCompatibility(_) => {}
    TypedInstruction::ColorGlyph(_) => {}
    TypedInstruction::ShowTextWithParameters(t) => {
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .word_space = TextSpace({
        let Ok(explicit_value) = t.0.as_f32();
        explicit_value
      });
      ({
        let Ok(explicit_value) = context.get_mut();
        explicit_value
      })
      .text_state
      .char_space = TextSpace({
        let Ok(explicit_value) = t.1.as_f32();
        explicit_value
      });
      let Ok(()) = text::next_line(
        context,
        kurbo::Vec2::new(
          0.0,
          f64::from(
            -({
              let Ok(explicit_value) = context.get();
              explicit_value
            })
            .text_state
            .leading
            .0,
          ),
        ),
      );
      let Ok(()) = text::show_text_string(context, device, resources, t.2);
    }
    TypedInstruction::Fallback(_) => {
      warn!("failed to read an operator");
    }
  }

  Ok(())
}

fn ensure_text_font(context: &mut Context<'_>) -> Result<(), Never> {
  let Ok(state) = context.get();

  match &state.text_state.font {
    Some(_selected) => return Ok(()),
    None => {}
  }

  let Ok(font) = Font::new_standard(StandardFont::Helvetica, &context.settings.font_resolver);
  let Ok(mutable_state) = context.get_mut();
  mutable_state.text_state.font = font.map(TextStateFont::Fallback);

  Ok(())
}

fn show_positioned_texts<'a>(
  entries: &Array<'_>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
  resources: &Resources<'a>,
) -> Result<(), Never> {
  let Ok(iter) = entries.iter::<Object<'_>>();

  for entry in iter {
    match entry {
      Object::Number(number) => {
        let Ok(adjustment) = number.as_f32();
        let Ok(state) = context.get_mut();
        let Ok(()) = state
          .text_state
          .apply_adjustment(TextAdjustment(adjustment));
      }
      Object::String(content) => {
        let Ok(()) = text::show_text_string(context, device, resources, &content);
      }
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => {}
    }
  }

  Ok(())
}

fn resolve_color_space(
  context: &Context<'_>,
  resources: &Resources<'_>,
  name: &Name<'_>,
) -> Result<ColorSpace, Never> {
  let Ok(named) = ColorSpace::new_from_name(name);

  match named {
    Some(space) => return Ok(space),
    None => {}
  }

  let Ok(resource) = context.get_color_space(resources, name);

  match resource {
    Some(space) => Ok(space),
    None => ColorSpace::device_gray(),
  }
}

fn append_line(context: &mut Context<'_>, point: Point) -> Result<(), Never> {
  match context.path.elements().first() {
    Some(_start) => {}
    None => return Ok(()),
  }

  let previous = context.last_point;
  context.last_point = point;
  let mut endpoint = point;

  match (previous == point).then_some(()) {
    Some(()) => endpoint.x += 0.0001,
    None => {}
  }

  context.path.line_to(endpoint);
  Ok(())
}

fn draw_shading<'a>(
  name: &Name<'_>,
  resources: &Resources<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(visibility) = context.ocg_state.is_visible();

  match visibility {
    crate::Visibility::Hidden => return Ok(()),
    crate::Visibility::Visible => {}
  }

  let Ok(state) = context.get();
  let opacity = state.graphics_state.non_stroke_alpha;
  let transfer_function = state.graphics_state.transfer_function.clone();
  let Ok(resource) = resources.get_shading(name);
  let parsed_shading = resource.and_then(|object| {
    let Ok(contents) = dict_or_stream(&object);
    let (dict, stream) = contents?;
    let Ok(shading) = Shading::new(dict, stream, &context.interpreter_cache.object_cache);
    shading
  });
  let shading = match parsed_shading {
    Some(shading_2) => shading_2,
    None => {
      warn!("failed to process shading");
      return Ok(());
    }
  };
  let Ok(pattern) =
    ShadingPattern::from_shading(shading, Affine::IDENTITY, opacity, transfer_function);
  let Ok(()) = context.save_state();
  let Ok(()) = context.push_root_transform();
  let Ok(state_2) = context.get_mut();
  state_2.graphics_state.non_stroke_pattern = Some(Pattern::Shading(pattern));
  let Ok(pattern_space) = ColorSpace::pattern();
  state_2.graphics_state.none_stroke_cs = pattern_space;
  device.set_soft_mask(state_2.graphics_state.soft_mask.clone());
  device.set_blend_mode(state_2.graphics_state.blend_mode);

  let Ok(bounds) = context.bbox();
  let Ok(state_3) = context.get();
  let inverted_bbox = state_3.ctm.inverse() * bounds.to_path(0.1);
  let Ok(()) = fill_path_impl(context, device, FillRule::NonZero, Some(&inverted_bbox));
  let Ok(()) = context.pop_root_transform();
  context.restore_state(device)
}
