use std::sync::Arc;

use console_core_font_database::{Database, ID};
use console_core_scalable_vector_graphics_values::FontFamily;

use self::layout::DatabaseExt;
use crate::{Cache, Font, FontStretch, FontStyle, Text};

pub(crate) mod flatten;
pub(crate) mod transform;

mod colr;

pub mod layout;

pub(crate) const OPSZ: skrifa::Tag = {
  let Ok(explicit_value) = skrifa::Tag::from_be_bytes(*b"opsz");
  explicit_value
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphId(pub u32);

impl From<GlyphId> for skrifa::raw::types::GlyphId {
  fn from(value: GlyphId) -> Self {
    let Ok(glyph_id) = Self::new(value.0);

    glyph_id
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public callback can extend its private copy of the immutable font catalogue before selecting a face"
  )
)]
pub type FontSelectionFn<'a> =
  Box<dyn Fn(&Font, &mut Arc<Database>) -> Option<ID> + Send + Sync + 'a>;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public fallback callback can extend its private copy of the immutable font catalogue before selecting a face"
  )
)]
pub type FallbackSelectionFn<'a> =
  Box<dyn Fn(char, &[ID], &mut Arc<Database>) -> Option<ID> + Send + Sync + 'a>;

pub struct FontResolver<'a> {
  pub select_font: FontSelectionFn<'a>,

  pub select_fallback: FallbackSelectionFn<'a>,
}

impl Default for FontResolver<'_> {
  fn default() -> Self {
    FontResolver {
      select_font: FontResolver::default_font_selector(),
      select_fallback: FontResolver::default_fallback_selector(),
    }
  }
}

impl FontResolver<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "The public resolver factory returns a ready-to-install font selection callback")
  )]
  pub fn default_font_selector() -> FontSelectionFn<'static> {
    Box::new(move |font, console_core_font_database| {
      let mut name_list = Vec::new();

      for family in &font.families {
        name_list.push(match family {
          FontFamily::Serif => console_core_font_database::Family::Serif,
          FontFamily::SansSerif => console_core_font_database::Family::SansSerif,
          FontFamily::Cursive => console_core_font_database::Family::Cursive,
          FontFamily::Fantasy => console_core_font_database::Family::Fantasy,
          FontFamily::Monospace => console_core_font_database::Family::Monospace,
          FontFamily::Named(s) => console_core_font_database::Family::Name(s),
        });
      }

      name_list.push(console_core_font_database::Family::Serif);

      let stretch = match font.stretch {
        FontStretch::UltraCondensed => console_core_font_database::Stretch::UltraCondensed,
        FontStretch::ExtraCondensed => console_core_font_database::Stretch::ExtraCondensed,
        FontStretch::Condensed => console_core_font_database::Stretch::Condensed,
        FontStretch::SemiCondensed => console_core_font_database::Stretch::SemiCondensed,
        FontStretch::Normal => console_core_font_database::Stretch::Normal,
        FontStretch::SemiExpanded => console_core_font_database::Stretch::SemiExpanded,
        FontStretch::Expanded => console_core_font_database::Stretch::Expanded,
        FontStretch::ExtraExpanded => console_core_font_database::Stretch::ExtraExpanded,
        FontStretch::UltraExpanded => console_core_font_database::Stretch::UltraExpanded,
      };

      let style = match font.style {
        FontStyle::Normal => console_core_font_database::Style::Normal,
        FontStyle::Italic => console_core_font_database::Style::Italic,
        FontStyle::Oblique => console_core_font_database::Style::Oblique,
      };

      let query = console_core_font_database::Query {
        families: &name_list,
        weight: console_core_font_database::Weight(font.weight),
        stretch,
        style,
      };

      let Ok(id) = console_core_font_database.query(&query);

      match id {
        ::core::option::Option::None => {
          console_core_logging::warn!(
            "No match for '{}' font-family.",
            font
              .families
              .iter()
              .map(|f| f.to_string())
              .collect::<Vec<_>>()
              .join(", ")
          );
        }
        ::core::option::Option::Some(_) => {}
      }

      {
        let explicit_value = id;
        explicit_value
      }
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "The public resolver factory returns a ready-to-install fallback callback")
  )]
  pub fn default_fallback_selector() -> FallbackSelectionFn<'static> {
    Box::new(|c, exclude_fonts, console_core_font_database| {
      let base_font_id = ::core::option::Option::<&_>::copied(exclude_fonts.first())?;
      let Ok(base_face_lookup) = console_core_font_database.face(base_font_id);
      let base_face = base_face_lookup?;
      let excluded: std::collections::HashSet<_> = exclude_fonts.iter().copied().collect();
      let english = console_core_font_database::Language::English_UnitedStates;
      let base_family = base_face
        .families
        .iter()
        .find(|family| family.1 == english)
        .or_else(|| base_face.families.first());
      let Ok(faces) = console_core_font_database.faces();

      for face in faces {
        match ::core::primitive::bool::then_some(excluded.contains(&face.id), ()) {
          ::core::option::Option::Some(()) => continue,
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(base_face.style != face.style
          && base_face.weight != face.weight
          && base_face.stretch != face.stretch, ())
        {
          ::core::option::Option::Some(()) => {
            continue;
          }
          ::core::option::Option::None => {}
        }

        match console_core_font_database.glyph_coverage(face.id, c) {
          crate::Presence::Absent => continue,
          crate::Presence::Present => {}
        }

        match base_family {
          Some(previous_family) => {
            #[cfg_attr(
              dylint_lib = "explicit028_no_search_in_a_loop",
              expect(
                explicit028_no_search_in_a_loop,
                reason = "this search runs once for the winning face, immediately before the return"
              )
            )]
            let new_family = match face.families.iter().find(|family| family.1 == english) {
              Some(family) => family,
              None => previous_family,
            };
            console_core_logging::warn!("Fallback from {} to {}.", previous_family.0, new_family.0);
          }
          None => {}
        }

        return Some(face.id);
      }

      None
    })
  }
}

impl std::fmt::Debug for FontResolver<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("FontResolver { .. }")
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Flattened color glyph documents enter the same SVG document depth limit"
  )
)]
pub(crate) fn convert(text: &mut Text, resolver: &FontResolver, cache: &mut Cache) -> Result<Option<()>, console_core_never::Never> {
  let Ok(layout) = layout::layout_text(text, resolver, &mut cache.console_core_font_database);
  let (text_fragments, bbox) = match layout {
    Some(layout_2) => layout_2,
    None => return Ok(None),
  };
  text.layouted = text_fragments;
  let Ok(rect) = bbox.to_rect();
  text.bounding_box = rect;
  let Ok(transform) = bbox.transform(text.abs_transform);
  let absolute = match transform {
    Some(bounds) => bounds,
    None => return Ok(None),
  };
  let Ok(absolute_box) = absolute.to_rect();
  text.abs_bounding_box = absolute_box;

  let Ok(flattened) = flatten::flatten(text, cache);
  let (group, stroke_bbox) = match flattened {
    Some(group) => group,
    None => return Ok(None),
  };
  *text.flattened = group;
  let Ok(stroke_box) = stroke_bbox.to_rect();
  text.stroke_bounding_box = stroke_box;
  let Ok(stroke_transform) = stroke_bbox.transform(text.abs_transform);
  let absolute_stroke = match stroke_transform {
    Some(bounds) => bounds,
    None => return Ok(None),
  };
  let Ok(stroke_bounds) = absolute_stroke.to_rect();
  text.abs_stroke_bounding_box = stroke_bounds;

  Ok(Some(()))
}
