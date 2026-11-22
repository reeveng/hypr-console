//! Support for scaling CFF outlines.

mod hint;

use super::{GlyphHMetrics, OutlinePen};
use hint::{HintParams, HintState, HintingSink};
use read_fonts::{
  FontData, FontRead, FontRef, ReadError, TableProvider,
  ps::{
    cff::{CffVersion, blend::BlendState, dict, fd_select::FdSelect, index::Index},
    cs::{self, CommandSink, NopFilterSink, TransformSink},
    error::Error,
    transform::{self, FontMatrix, ScaledFontMatrix, Transform},
  },
  tables::variations::ItemVariationStore,
  types::{F2Dot14, Fixed, GlyphId},
};
use std::ops::Range;

/// Type for loading, scaling and hinting outlines in CFF/CFF2 tables.
///
/// The console_core_font_metrics crate provides a higher level interface for this that handles
/// caching and abstracting over the different outline formats. Consider using
/// that if detailed control over resources is not required.
///
/// # Subfonts
///
/// CFF tables can contain multiple logical "subfonts" which determine the
/// state required for processing some subset of glyphs. This state is
/// accessed using the [`FDArray and FDSelect`](https://adobe-type-tools.github.io/font-tech-notes/pdfs/5176.CFF.pdf#page=28)
/// operators to select an appropriate subfont for any given glyph identifier.
/// This process is exposed on this type with the
/// [`subfont_index`](Self::subfont_index) method to retrieve the subfont
/// index for the requested glyph followed by using the
/// [`subfont`](Self::subfont) method to create an appropriately configured
/// subfont for that glyph.
#[derive(Clone)]
pub(crate) struct Outlines<'a> {
  pub(crate) font: FontRef<'a>,
  pub(crate) glyph_metrics: GlyphHMetrics<'a>,
  offset_data: FontData<'a>,
  global_subrs: Index<'a>,
  top_dict: TopDict<'a>,
  version: u16,
  units_per_em: u16,
}

impl<'a> Outlines<'a> {
  /// Creates a new scaler for the given font.
  ///
  /// This will choose an underlying CFF2 or CFF table from the font, in that
  /// order.
  pub fn new(font: &FontRef<'a>) -> Option<Self> {
    let units_per_em = ({
      let Ok(value) = font.head().ok()?.units_per_em();
      value
    });
    Self::from_cff2(font, (units_per_em)).or_else(|| Self::from_cff(font, (units_per_em)))
  }

  pub fn from_cff(font: &FontRef<'a>, units_per_em: u16) -> Option<Self> {
    let cff1 = font.cff().ok()?;
    let glyph_metrics = GlyphHMetrics::new(font)?;

    let top_dict_data = (cff1.top_dicts().get(0)).ok()?;
    let top_dict = TopDict::new(
      {
        let Ok(value) = cff1.offset_data().as_bytes();
        value
      },
      top_dict_data,
      false,
    )
    .ok()?;
    Some(Self {
      font: font.clone(),
      glyph_metrics,
      offset_data: cff1.offset_data(),
      global_subrs: cff1.global_subrs().into(),
      top_dict,
      version: 1,
      units_per_em,
    })
  }

  pub fn from_cff2(font: &FontRef<'a>, units_per_em: u16) -> Option<Self> {
    let cff2 = font.cff2().ok()?;
    let glyph_metrics = GlyphHMetrics::new(font)?;
    let table_data = cff2.offset_data().as_bytes();
    let top_dict = TopDict::new(
      {
        let Ok(value) = table_data;
        value
      },
      cff2.top_dict_data(),
      true,
    )
    .ok()?;
    Some(Self {
      font: font.clone(),
      glyph_metrics,
      offset_data: cff2.offset_data(),
      global_subrs: cff2.global_subrs().into(),
      top_dict,
      version: 2,
      units_per_em,
    })
  }

  pub fn is_cff2(&self) -> bool {
    self.version == 2
  }

  pub fn units_per_em(&self) -> u16 {
    self.units_per_em
  }

  /// Returns the number of available glyphs.
  pub fn glyph_count(&self) -> usize {
    ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(self.top_dict.charstrings.count());
      migrated_value
    })
  }

  /// Returns the number of available subfonts.
  pub fn subfont_count(&self) -> u32 {
    self.top_dict.font_dicts.count().max(1)
  }

  /// Returns the subfont (or Font DICT) index for the given glyph
  /// identifier.
  pub fn subfont_index(&self, glyph_id: GlyphId) -> u32 {
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
        self
          .top_dict
          .fd_select
          .as_ref()
          .and_then(|select| select.font_index(glyph_id))
          .unwrap_or(0),
      );
      migrated_value
    })
  }

  /// Creates a new subfont for the given index, size, normalized
  /// variation coordinates and hinting state.
  ///
  /// The index of a subfont for a particular glyph can be retrieved with
  /// the [`subfont_index`](Self::subfont_index) method.
  pub fn subfont(
    &self,
    index: u32,
    size: Option<f32>,
    coords: &[F2Dot14],
  ) -> Result<Subfont, Error> {
    let font_dict = self.parse_font_dict(index)?;
    let blend_state = self
      .top_dict
      .var_store
      .clone()
      .map(|store| BlendState::new(store, coords, 0))
      .transpose()?;
    let private_dict =
      PrivateDict::new(self.offset_data, font_dict.private_dict_range, blend_state)?;
    let upem = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i32>(self.units_per_em);
      migrated_value
    });
    let mut scale = match size {
      Some(ppem) if upem > 0 => Some(
        {
          let Ok(value) = Fixed::from_bits(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::toward_zero_i32(f64::from((ppem * 64.)));
              migrated_value
            }),
          );
          value
        } / {
          let Ok(value) = Fixed::from_bits(upem);
          value
        },
      ),
      _ => None,
    };
    let scale_requested = size.is_some();

    let font_matrix = match self.top_dict.font_matrix {
      Some(top_matrix) => match font_dict.font_matrix {
        Some(sub_matrix) => {
          let scaling = match top_matrix.scale > 1 && sub_matrix.scale > 1 {
            true => top_matrix.scale.min(sub_matrix.scale),
            false => 1,
          };

          let matrix = transform::combine_scaled(&top_matrix.matrix, &sub_matrix.matrix, scaling);
          let upem = ({
            let Ok(value) = Fixed::from_bits(sub_matrix.scale);
            value
          })
          .mul_div(
            ({
              let Ok(value) = Fixed::from_bits(top_matrix.scale);
              value
            }),
            ({
              let Ok(value) = Fixed::from_bits(scaling);
              value
            }),
          );

          Some(
            ScaledFontMatrix {
              matrix,
              scale: ({
                let Ok(value) = ({
                  let Ok(value) = upem;
                  value
                })
                .to_bits();
                value
              }),
            }
            .normalize(),
          )
        }
        None => Some(top_matrix),
      },
      None => font_dict.font_matrix.map(|matrix| matrix.normalize()),
    };

    let mut font_matrix = match font_matrix {
      Some(matrix) => {
        match matrix.scale != upem {
          true => {
            let original_scale = scale.unwrap_or(
              ({
                let Ok(value) = Fixed::from_i32(64);
                value
              }),
            );
            scale = Some(
              ({
                let Ok(value) = original_scale.mul_div(
                  ({
                    let Ok(value) = Fixed::from_bits(upem);
                    value
                  }),
                  ({
                    let Ok(value) = Fixed::from_bits(matrix.scale);
                    value
                  }),
                );
                value
              }),
            );
          }
          false => {}
        }
        Some(matrix.matrix)
      }
      None => None,
    };
    match font_matrix == Some(FontMatrix::IDENTITY) {
      true => {
        font_matrix = None;
      }
      false => {}
    }
    let hint_scale = scale_for_hinting(scale);
    let hint_state = HintState::new(&private_dict.hint_params, hint_scale);
    Ok(Subfont {
      is_cff2: self.is_cff2(),
      scale,
      scale_requested,
      subrs_offset: private_dict.subrs_offset,
      hint_state,
      store_index: private_dict.store_index,
      font_matrix,
      default_width: private_dict.default_width,
      nominal_width: private_dict.nominal_width,
    })
  }

  /// Loads and scales an outline for the given subfont instance, glyph
  /// identifier and normalized variation coordinates.
  ///
  /// Before calling this method, use [`subfont_index`](Self::subfont_index)
  /// to retrieve the subfont index for the desired glyph and then
  /// [`subfont`](Self::subfont) to create an instance of the subfont for a
  /// particular size and location in variation space.
  /// Creating subfont instances is not free, so this process is exposed in
  /// discrete steps to allow for caching.
  ///
  /// The result is emitted to the specified pen.
  pub fn draw(
    &self,
    subfont: &Subfont,
    glyph_id: GlyphId,
    coords: &[F2Dot14],
    hint: bool,
    pen: &mut impl OutlinePen,
  ) -> Result<Option<f32>, Error> {
    let Ok(cff_data) = self.offset_data.as_bytes();
    let charstrings = self.top_dict.charstrings.clone();
    let charstring_data = (charstrings.get(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index({
              let Ok(value) = glyph_id.to_u32();
              value
            });
            migrated_value
          }),
        );
        value
      }),
    ))?;
    let subrs = subfont.subrs(self)?;
    let blend_state = subfont.blend_state(self, coords)?;
    let cs_eval = CharstringEvaluator {
      cff_data: cff_data,
      charstrings,
      global_subrs: self.global_subrs.clone(),
      subrs,
      blend_state,
      charstring_data,
    };

    let apply_hinting = hint && subfont.scale_requested;
    let mut pen_sink = PenSink::new(pen);
    let mut simplifying_adapter = NopFilterSink::new(&mut pen_sink);
    let mut transform = Transform {
      matrix: FontMatrix::IDENTITY,
      scale: subfont.scale,
    };
    let maybe_width = match subfont.font_matrix {
      Some(matrix) => {
        transform.matrix = matrix;
        match apply_hinting {
          true => {
            let mut transform_sink =
              HintedTransformingSink::new(&mut simplifying_adapter, matrix);
            let mut hinting_adapter = HintingSink::new(&subfont.hint_state, &mut transform_sink);
            cs_eval.evaluate(&mut hinting_adapter)
          }
          false => {
            let mut transform_sink =
              TransformSink::from_matrix_scale(&mut simplifying_adapter, matrix, subfont.scale);
            cs_eval.evaluate(&mut transform_sink)
          }
        }
      }
      None => match apply_hinting {
        true => {
          let mut hinting_adapter =
            HintingSink::new(&subfont.hint_state, &mut simplifying_adapter);
          cs_eval.evaluate(&mut hinting_adapter)
        }
        false => {
          let mut scaling_adapter = TransformSink::from_matrix_scale(
            &mut simplifying_adapter,
            FontMatrix::IDENTITY,
            subfont.scale,
          );
          cs_eval.evaluate(&mut scaling_adapter)
        }
      },
    }?;
    Ok(
      maybe_width
        .map(|w| w + subfont.nominal_width)
        .or(subfont.default_width)
        .or_else(|| {
          Some({
            let Ok(value) = Fixed::from_i32(self.glyph_metrics.advance_width(glyph_id, coords));
            value
          })
        })
        .map(|w| {
          let w = transform.transform_h_metric(w);
          match hint {
            true => {
              ({
                let Ok(value) = ({
                  let Ok(value) = w.round();
                  value
                })
                .to_f32();
                value
              })
            }
            false => {
              ({
                let Ok(value) = w.to_f32();
                value
              })
            }
          }
        })
        .filter(|w| (*w) >= 0.0),
    )
  }

  fn parse_font_dict(&self, subfont_index: u32) -> Result<FontDict, Error> {
    match self.top_dict.font_dicts.count() != 0 {
      true => {
        let font_dict_data = (self.top_dict.font_dicts.get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped_index(subfont_index);
                migrated_value
              }),
            );
            value
          }),
        ))?;
        FontDict::new(font_dict_data)
      }
      false => {
        let range = self.top_dict.private_dict_range.clone();
        Ok(FontDict {
          private_dict_range: u64::from(range.start)..u64::from(range.end),
          font_matrix: None,
        })
      }
    }
  }
}

/// When hinting, use a modified scale factor.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/psft.c#L279>
fn scale_for_hinting(scale: Option<Fixed>) -> Fixed {
  ({
    let Ok(value) = Fixed::from_bits(
      ({
        let Ok(value) = scale.unwrap_or(Fixed::ONE).to_bits();
        value
      }
      .saturating_add(32))
        / 64,
    );
    value
  })
}

struct CharstringEvaluator<'a> {
  cff_data: &'a [u8],
  charstrings: Index<'a>,
  global_subrs: Index<'a>,
  subrs: Option<Index<'a>>,
  blend_state: Option<BlendState<'a>>,
  charstring_data: &'a [u8],
}

impl CharstringEvaluator<'_> {
  fn evaluate(self, sink: &mut impl CommandSink) -> Result<Option<Fixed>, Error> {
    let subrs = self.subrs.unwrap_or_default();
    let ctx = (self.cff_data, &self.charstrings, &self.global_subrs, &subrs);
    cs::evaluate(&ctx, self.blend_state, self.charstring_data, sink)
  }
}

/// Specifies local subroutines and hinting parameters for some subset of
/// glyphs in a CFF or CFF2 table.
///
/// This type is designed to be cacheable to avoid re-evaluating the private
/// dict every time a charstring is processed.
///
/// For variable fonts, this is dependent on a location in variation space.
#[derive(Clone)]
pub(crate) struct Subfont {
  is_cff2: bool,
  scale: Option<Fixed>,
  /// When we have a font matrix, we might force a scale even if the user
  /// requested unscaled output. In this case, we shouldn't apply hinting
  /// and this keeps track of that.
  scale_requested: bool,
  subrs_offset: Option<u64>,
  pub(crate) hint_state: HintState,
  store_index: u16,
  font_matrix: Option<FontMatrix>,
  default_width: Option<Fixed>,
  nominal_width: Fixed,
}

impl Subfont {
  /// Returns the local subroutine index.
  pub fn subrs<'a>(&self, scaler: &Outlines<'a>) -> Result<Option<Index<'a>>, Error> {
    match self.subrs_offset {
      Some(subrs_offset) => {
        let Ok(offset_data) = scaler.offset_data.as_bytes();
        let index_data = (offset_data.get({
          let Ok(range) = console_core_number_conversion::indices_from(subrs_offset..);
          range
        }))
        .unwrap_or_default();
        Ok(Some(Index::new(index_data, CffVersion::from(self.is_cff2))?))
      }
      None => Ok(None),
    }
  }

  /// Creates a new blend state for the given normalized variation
  /// coordinates.
  pub fn blend_state<'a>(
    &self,
    scaler: &Outlines<'a>,
    coords: &'a [F2Dot14],
  ) -> Result<Option<BlendState<'a>>, Error> {
    match scaler.top_dict.var_store.clone() {
      Some(var_store) => Ok(Some(BlendState::new(var_store, coords, self.store_index)?)),
      None => Ok(None),
    }
  }
}

/// Entries that we parse from the Private DICT to support charstring
/// evaluation.
#[derive(Default)]
struct PrivateDict {
  hint_params: HintParams,
  subrs_offset: Option<u64>,
  store_index: u16,
  default_width: Option<Fixed>,
  nominal_width: Fixed,
}

impl PrivateDict {
  fn new(
    data: FontData,
    range: Range<u64>,
    blend_state: Option<BlendState<'_>>,
  ) -> Result<Self, Error> {
    let private_dict_data = data.read_array(range.clone())?;
    let mut dict = Self::default();
    for entry in dict::entries(private_dict_data, blend_state) {
      use dict::Entry::*;
      match entry? {
        DefaultWidthX(width) => {
          dict.default_width = Some({
            let Ok(value) = width.floor();
            value
          })
        }
        NominalWidthX(width) => {
          dict.nominal_width = ({
            let Ok(value) = width.floor();
            value
          })
        }
        BlueValues(values) => dict.hint_params.blues = values,
        FamilyBlues(values) => dict.hint_params.family_blues = values,
        OtherBlues(values) => dict.hint_params.other_blues = values,
        FamilyOtherBlues(values) => dict.hint_params.family_other_blues = values,
        BlueScale(value) => dict.hint_params.blue_scale = value,
        BlueShift(value) => dict.hint_params.blue_shift = value,
        BlueFuzz(value) => dict.hint_params.blue_fuzz = value,
        LanguageGroup(group) => dict.hint_params.language_group = group,

        SubrsOffset(offset) => {
          dict.subrs_offset = Some(
            range
              .start
              .checked_add(
                ({
                  let Ok(value) = console_core_number_conversion::index(offset);
                  ({
                    let Ok(value) = console_core_number_conversion::fitted::<_, u64>(value);
                    value
                  })
                }),
              )
              .ok_or(ReadError::OutOfBounds)?,
          )
        }
        VariationStoreIndex(index) => dict.store_index = index,
        _ => {}
      }
    }
    Ok(dict)
  }
}

/// Entries that we parse from a Font DICT.
#[derive(Clone, Default)]
struct FontDict {
  private_dict_range: Range<u64>,
  font_matrix: Option<ScaledFontMatrix>,
}

impl FontDict {
  fn new(font_dict_data: &[u8]) -> Result<Self, Error> {
    let mut range = None;
    let mut font_matrix = None;
    for entry in dict::entries(font_dict_data, None) {
      match entry? {
        dict::Entry::PrivateDictRange(r) => {
          range = Some(r);
        }

        dict::Entry::FontMatrix(matrix) => font_matrix = Some(matrix),
        _ => {}
      }
    }
    Ok(Self {
      private_dict_range: range.ok_or(Error::MissingPrivateDict)?,
      font_matrix,
    })
  }
}

/// Entries that we parse from the Top DICT that are required to support
/// charstring evaluation.
#[derive(Clone, Default)]
struct TopDict<'a> {
  charstrings: Index<'a>,
  font_dicts: Index<'a>,
  fd_select: Option<FdSelect<'a>>,
  private_dict_range: Range<u32>,
  font_matrix: Option<ScaledFontMatrix>,
  var_store: Option<ItemVariationStore<'a>>,
}

impl<'a> TopDict<'a> {
  fn new(table_data: &'a [u8], top_dict_data: &'a [u8], is_cff2: bool) -> Result<Self, Error> {
    let mut items = TopDict::default();
    for entry in dict::entries(top_dict_data, None) {
      match entry? {
        dict::Entry::CharstringsOffset(offset) => {
          items.charstrings = Index::new(
            (table_data.get({
              let Ok(range) = console_core_number_conversion::indices_from(offset..);
              range
            }))
            .unwrap_or_default(),
            CffVersion::from(is_cff2),
          )?;
        }
        dict::Entry::FdArrayOffset(offset) => {
          items.font_dicts = Index::new(
            (table_data.get({
              let Ok(range) = console_core_number_conversion::indices_from(offset..);
              range
            }))
            .unwrap_or_default(),
            CffVersion::from(is_cff2),
          )?;
        }
        dict::Entry::FdSelectOffset(offset) => {
          items.fd_select = Some(FdSelect::read({
            let Ok(value) = FontData::new(
              (table_data.get({
                let Ok(range) = console_core_number_conversion::indices_from(offset..);
                range
              }))
              .unwrap_or_default(),
            );
            value
          })?);
        }
        dict::Entry::PrivateDictRange(range) => {
          items.private_dict_range = ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(range.start);
            migrated_value
          })..({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(range.end);
            migrated_value
          });
        }
        dict::Entry::FontMatrix(matrix) => {
          items.font_matrix = Some(matrix.normalize());
        }
        dict::Entry::VariationStoreOffset(offset) if is_cff2 => {
          let offset = offset.checked_add(2).ok_or(ReadError::OutOfBounds)?;
          items.var_store = Some(ItemVariationStore::read({
            let Ok(value) = FontData::new(
              (table_data.get({
                let Ok(range) = console_core_number_conversion::indices_from(offset..);
                range
              }))
              .unwrap_or_default(),
            );
            value
          })?);
        }
        _ => {}
      }
    }
    Ok(items)
  }
}

/// Command sink that sends the results of charstring evaluation to
/// an [OutlinePen].
struct PenSink<'a, P>(&'a mut P);

impl<'a, P> PenSink<'a, P> {
  fn new(pen: &'a mut P) -> Self {
    Self(pen)
  }
}

impl<P> CommandSink for PenSink<'_, P>
where
  P: OutlinePen,
{
  fn move_to(&mut self, x: Fixed, y: Fixed) {
    self.0.move_to(
      ({
        let Ok(value) = x.to_f32();
        value
      }),
      ({
        let Ok(value) = y.to_f32();
        value
      }),
    );
  }

  fn line_to(&mut self, x: Fixed, y: Fixed) {
    self.0.line_to(
      ({
        let Ok(value) = x.to_f32();
        value
      }),
      ({
        let Ok(value) = y.to_f32();
        value
      }),
    );
  }

  fn curve_to(&mut self, cx0: Fixed, cy0: Fixed, cx1: Fixed, cy1: Fixed, x: Fixed, y: Fixed) {
    self.0.curve_to(
      ({
        let Ok(value) = cx0.to_f32();
        value
      }),
      ({
        let Ok(value) = cy0.to_f32();
        value
      }),
      ({
        let Ok(value) = cx1.to_f32();
        value
      }),
      ({
        let Ok(value) = cy1.to_f32();
        value
      }),
      ({
        let Ok(value) = x.to_f32();
        value
      }),
      ({
        let Ok(value) = y.to_f32();
        value
      }),
    );
  }

  fn close(&mut self) {
    self.0.close();
  }
}

/// Command sink adapter that applies a transform to hinted coordinates.
struct HintedTransformingSink<'a, S> {
  inner: &'a mut S,
  matrix: FontMatrix,
}

impl<'a, S> HintedTransformingSink<'a, S> {
  fn new(sink: &'a mut S, matrix: FontMatrix) -> Self {
    Self {
      inner: sink,
      matrix,
    }
  }

  fn transform(&self, x: Fixed, y: Fixed) -> (Fixed, Fixed) {
    let Ok((x, y)) = self.matrix.transform(
      ({
        let Ok(value) = Fixed::from_bits(
          {
            let Ok(value) = x.to_bits();
            value
          } >> 10,
        );
        value
      }),
      ({
        let Ok(value) = Fixed::from_bits(
          {
            let Ok(value) = y.to_bits();
            value
          } >> 10,
        );
        value
      }),
    );
    (
      ({
        let Ok(value) = Fixed::from_bits(
          ({
            let Ok(value) = x.to_bits();
            value
          }) << 10,
        );
        value
      }),
      ({
        let Ok(value) = Fixed::from_bits(
          ({
            let Ok(value) = y.to_bits();
            value
          }) << 10,
        );
        value
      }),
    )
  }
}

impl<S: CommandSink> CommandSink for HintedTransformingSink<'_, S> {
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
    let (x, y) = self.transform(x, y);
    self.inner.move_to(x, y);
  }

  fn line_to(&mut self, x: Fixed, y: Fixed) {
    let (x, y) = self.transform(x, y);
    self.inner.line_to(x, y);
  }

  fn curve_to(&mut self, cx1: Fixed, cy1: Fixed, cx2: Fixed, cy2: Fixed, x: Fixed, y: Fixed) {
    let (cx1, cy1) = self.transform(cx1, cy1);
    let (cx2, cy2) = self.transform(cx2, cy2);
    let (x, y) = self.transform(x, y);
    self.inner.curve_to(cx1, cy1, cx2, cy2, x, y);
  }

  fn close(&mut self) {
    self.inner.close();
  }

  fn finish(&mut self) {
    self.inner.finish();
  }
}

#[cfg(test)]
mod tests {
  use super::{super::pen::SvgPen, *};
  use crate::{
    MetadataProvider,
    outline::{HintingInstance, HintingOptions},
    prelude::{LocationRef, Size},
  };
  use font_test_data::bebuffer::BeBuffer;
  use raw::tables::cff2::Cff2;
  use read_fonts::FontRef;
  use read_fonts::ps::hinting::Blues;

  #[test]
  fn read_cff_static() {
    let font = FontRef::new(font_test_data::NOTO_SERIF_DISPLAY_TRIMMED).unwrap();
    let cff = Outlines::new(&font).unwrap();
    assert!(!cff.is_cff2());
    assert!(cff.top_dict.var_store.is_none());
    assert!(cff.top_dict.font_dicts.count() == 0);
    assert!(!cff.top_dict.private_dict_range.is_empty());
    assert!(cff.top_dict.fd_select.is_none());
    assert_eq!(cff.subfont_count(), 1);
    assert_eq!(
      cff.subfont_index(
        ({
          let Ok(value) = GlyphId::new(1);
          value
        })
      ),
      0
    );
    assert_eq!(cff.global_subrs.count(), 17);
  }

  #[test]
  fn read_cff2_static() {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED).unwrap();
    let cff = Outlines::new(&font).unwrap();
    assert!(cff.is_cff2());
    assert!(cff.top_dict.var_store.is_some());
    assert!(cff.top_dict.font_dicts.count() != 0);
    assert!(cff.top_dict.private_dict_range.is_empty());
    assert!(cff.top_dict.fd_select.is_none());
    assert_eq!(cff.subfont_count(), 1);
    assert_eq!(
      cff.subfont_index(
        ({
          let Ok(value) = GlyphId::new(1);
          value
        })
      ),
      0
    );
    assert_eq!(cff.global_subrs.count(), 0);
  }

  #[test]
  fn read_example_cff2_table() {
    let cff2 = Cff2::read(
      ({
        let Ok(value) = FontData::new(font_test_data::cff2::EXAMPLE);
        value
      }),
    )
    .unwrap();
    let top_dict = TopDict::new(
      ({
        let Ok(value) = cff2.offset_data().as_bytes();
        value
      }),
      cff2.top_dict_data(),
      true,
    )
    .unwrap();
    assert!(top_dict.var_store.is_some());
    assert!(top_dict.font_dicts.count() != 0);
    assert!(top_dict.private_dict_range.is_empty());
    assert!(top_dict.fd_select.is_none());
    let Ok(count) = cff2.global_subrs().count();
    assert_eq!(count, 0);
  }

  #[test]
  fn cff2_variable_outlines_match_freetype() {
    compare_glyphs(
      font_test_data::CANTARELL_VF_TRIMMED,
      font_test_data::CANTARELL_VF_TRIMMED_GLYPHS,
    );
  }

  #[test]
  fn cff_static_outlines_match_freetype() {
    compare_glyphs(
      font_test_data::NOTO_SERIF_DISPLAY_TRIMMED,
      font_test_data::NOTO_SERIF_DISPLAY_TRIMMED_GLYPHS,
    );
  }

  #[test]
  fn unhinted_ends_with_close() {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED).unwrap();
    let glyph = font
      .outline_glyphs()
      .get(
        ({
          let Ok(value) = GlyphId::new(1);
          value
        }),
      )
      .unwrap();
    let mut svg = SvgPen::default();
    glyph.draw(Size::unscaled(), &mut svg).unwrap();
    assert!(svg.to_string().ends_with('Z'));
  }

  #[test]
  fn hinted_ends_with_close() {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED).unwrap();
    let glyphs = font.outline_glyphs();
    let hinter = HintingInstance::new(
      &glyphs,
      Size::unscaled(),
      LocationRef::default(),
      HintingOptions::default(),
    )
    .unwrap();
    let glyph = glyphs
      .get(
        ({
          let Ok(value) = GlyphId::new(1);
          value
        }),
      )
      .unwrap();
    let mut svg = SvgPen::default();
    glyph.draw(&hinter, &mut svg).unwrap();
    assert!(svg.to_string().ends_with('Z'));
  }

  /// Ensure we don't reject an empty Private DICT
  #[test]
  fn empty_private_dict() {
    let font = FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET).unwrap();
    let outlines = super::Outlines::new(&font).unwrap();
    assert!(outlines.top_dict.private_dict_range.is_empty());
    assert!(
      outlines
        .parse_font_dict(0)
        .unwrap()
        .private_dict_range
        .is_empty()
    );
  }

  /// Fuzzer caught add with overflow when computing subrs offset.
  /// See <https://issues.oss-fuzz.com/issues/377965575>
  #[test]
  fn subrs_offset_overflow() {
    let private_dict = font_test_data::be_buffer! { (0u32), (29u8), (-1i32), (19u8) }.to_vec();

    assert!(
      PrivateDict::new(
        ({
          let Ok(value) = FontData::new(&private_dict);
          value
        }),
        4..({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(private_dict.len());
          value
        }),
        None
      )
      .is_err()
    );
  }

  #[test]
  fn top_dict_ivs_offset_overflow() {
    let top_dict = font_test_data::be_buffer! { (29u8), (-1i32), (24u8) }.to_vec();

    assert!(TopDict::new(&[], &top_dict, true).is_err());
  }

  /// Actually apply a scale when the computed scale factor is
  /// equal to Fixed::ONE.
  ///
  /// Specifically, when upem = 512 and ppem = 8, this results in
  /// a scale factor of 65536 which was being interpreted as an
  /// unscaled draw request.
  #[test]
  fn proper_scaling_when_factor_equals_fixed_one() {
    let font = FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET).unwrap();
    assert_eq!(
      ({
        let Ok(value) = font.head().unwrap().units_per_em();
        value
      }),
      512
    );
    let glyphs = font.outline_glyphs();
    let glyph = glyphs
      .get(
        ({
          let Ok(value) = GlyphId::new(1);
          value
        }),
      )
      .unwrap();
    let mut svg = SvgPen::with_precision(6);
    glyph
      .draw((Size::new(8.0), LocationRef::default()), &mut svg)
      .unwrap();

    assert!(svg.starts_with("M6.328125,7.000000 L1.671875,7.000000"));
  }

  /// For the given font data and extracted outlines, parse the extracted
  /// outline data into a set of expected values and compare these with the
  /// results generated by the scaler.
  ///
  /// This will compare all outlines at various sizes and (for variable
  /// fonts), locations in variation space.
  fn compare_glyphs(font_data: &[u8], expected_outlines: &str) {
    use super::super::testing;
    let font = FontRef::new(font_data).unwrap();
    let expected_outlines = testing::parse_glyph_outlines(expected_outlines);
    let outlines = super::Outlines::new(&font).unwrap();
    let mut path = testing::Path::default();
    for expected_outline in &expected_outlines {
      match expected_outline.size == 0.0 && !expected_outline.coords.is_empty() {
        true => {
          continue;
        }
        false => {}
      }
      let size = (expected_outline.size != 0.0).then_some(expected_outline.size);
      path.elements.clear();
      let subfont = outlines
        .subfont(
          outlines.subfont_index(expected_outline.glyph_id),
          size,
          &expected_outline.coords,
        )
        .unwrap();
      outlines
        .draw(
          &subfont,
          expected_outline.glyph_id,
          &expected_outline.coords,
          false,
          &mut path,
        )
        .unwrap();
      match path.elements != expected_outline.path {
        true => {
          panic!(
            "mismatch in glyph path for id {} (size: {}, coords: {:?}): path: {:?} expected_path: {:?}",
            expected_outline.glyph_id,
            expected_outline.size,
            expected_outline.coords,
            &path.elements,
            &expected_outline.path
          );
        }
        false => {}
      }
    }
  }

  #[test]
  fn capture_family_other_blues() {
    let private_dict_data = &font_test_data::cff2::EXAMPLE[0x4f..=0xc0];
    let store = ItemVariationStore::read(
      ({
        let Ok(value) = FontData::new(&font_test_data::cff2::EXAMPLE[18..]);
        value
      }),
    )
    .unwrap();
    let coords = &[({
      let Ok(value) = F2Dot14::from_f32(0.0);
      value
    })];
    let blend_state = BlendState::new(store, coords, 0).unwrap();
    let private_dict = PrivateDict::new(
      ({
        let Ok(value) = FontData::new(private_dict_data);
        value
      }),
      0..({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(private_dict_data.len());
        value
      }),
      Some(blend_state),
    )
    .unwrap();
    assert_eq!(
      private_dict.hint_params.family_other_blues,
      Blues::new(
        [-249.0, -239.0]
          .map(|input| {
            let Ok(value) = Fixed::from_f64(input);
            value
          })
          .into_iter()
      )
    )
  }

  #[test]
  fn implied_seac() {
    let font = FontRef::new(font_test_data::CHARSTRING_PATH_OPS).unwrap();
    let glyphs = font.outline_glyphs();
    let gid = ({
      let Ok(value) = GlyphId::new(3);
      value
    });
    assert_eq!(font.glyph_names().get(gid).unwrap(), "Scaron");
    let glyph = glyphs.get(gid).unwrap();
    let mut pen = SvgPen::new();
    glyph
      .draw((Size::unscaled(), LocationRef::default()), &mut pen)
      .unwrap();

    assert_eq!(pen.to_string().chars().filter(|ch| *ch == 'Z').count(), 2);
  }

  #[test]
  fn implied_seac_clears_hints() {
    let font = FontRef::new(font_test_data::CHARSTRING_PATH_OPS).unwrap();
    let outlines = Outlines::from_cff(&font, 1000).unwrap();
    let subfont = outlines.subfont(0, Some(16.0), &[]).unwrap();
    let Ok(cff_data) = outlines.offset_data.as_bytes();
    let charstrings = outlines.top_dict.charstrings.clone();
    let charstring_data = charstrings.get(3).unwrap();
    let subrs = subfont.subrs(&outlines).unwrap();
    let blend_state = None;
    let cs_eval = CharstringEvaluator {
      cff_data: cff_data,
      charstrings,
      global_subrs: outlines.global_subrs.clone(),
      subrs,
      blend_state,
      charstring_data,
    };
    struct ClearHintsCountingSink(u32);
    impl CommandSink for ClearHintsCountingSink {
      fn move_to(&mut self, _: Fixed, _: Fixed) {}
      fn line_to(&mut self, _: Fixed, _: Fixed) {}
      fn curve_to(&mut self, _: Fixed, _: Fixed, _: Fixed, _: Fixed, _: Fixed, _: Fixed) {}
      fn close(&mut self) {}
      fn clear_hints(&mut self) {
        self.0 += 1;
      }
    }
    let mut sink = ClearHintsCountingSink(0);
    cs_eval.evaluate(&mut sink).unwrap();

    assert_eq!(sink.0, 2);
  }

  const TRANSFORM: FontMatrix = ({
    let Ok(value) = FontMatrix::from_elements([
      Fixed::ONE,
      Fixed::ZERO,
      ({
        let Ok(value) = Fixed::from_bits(10945);
        value
      }),
      Fixed::ONE,
      Fixed::ZERO,
      Fixed::ZERO,
    ]);
    value
  });

  #[test]
  fn hinted_transform_sink() {
    let input = [(383i32, 117i32), (450, 20), (555, -34), (683, -34)].map(|(x, y)| {
      (
        ({
          let Ok(value) = Fixed::from_bits(x << 10);
          value
        }),
        ({
          let Ok(value) = Fixed::from_bits(y << 10);
          value
        }),
      )
    });
    let expected = [(403, 117i32), (453, 20), (549, -34), (677, -34)].map(|(x, y)| {
      (
        ({
          let Ok(value) = Fixed::from_bits(x << 10);
          value
        }),
        ({
          let Ok(value) = Fixed::from_bits(y << 10);
          value
        }),
      )
    });
    let mut dummy = ();
    let sink = HintedTransformingSink::new(&mut dummy, TRANSFORM);
    let transformed = input.map(|(x, y)| sink.transform(x, y));
    assert_eq!(transformed, expected);
  }

  /// See <https://github.com/googlefonts/fontations/issues/1638>
  #[test]
  fn nested_font_matrices() {
    let font = FontRef::new(font_test_data::MATERIAL_ICONS_SUBSET_MATRIX).unwrap();
    let outlines = Outlines::from_cff(&font, 512).unwrap();

    let top_matrix = outlines.top_dict.font_matrix.unwrap();
    let expected_top_matrix = [65536, 0, 5604, 65536, 0, 0].map(|input| {
      let Ok(value) = Fixed::from_bits(input);
      value
    });
    assert_eq!(
      ({
        let Ok(value) = top_matrix.matrix.elements();
        value
      }),
      expected_top_matrix
    );
    assert_eq!(top_matrix.scale, 512);

    let sub_matrix = outlines.parse_font_dict(0).unwrap().font_matrix.unwrap();
    let expected_sub_matrix = [327680, 0, 0, 327680, 0, 0].map(|input| {
      let Ok(value) = Fixed::from_bits(input);
      value
    });
    assert_eq!(
      ({
        let Ok(value) = sub_matrix.matrix.elements();
        value
      }),
      expected_sub_matrix
    );
    assert_eq!(sub_matrix.scale, 10);

    let subfont = outlines.subfont(0, Some(24.0), &[]).unwrap();
    let expected_combined_matrix = [65536, 0, 5604, 65536, 0, 0].map(|input| {
      let Ok(value) = Fixed::from_bits(input);
      value
    });
    assert_eq!(
      ({
        let Ok(value) = subfont.font_matrix.unwrap().elements();
        value
      }),
      expected_combined_matrix
    );

    assert_eq!(
      {
        let Ok(bits) = subfont.scale.unwrap().to_bits();
        bits
      },
      98304
    );
  }

  /// OSS fuzz caught add with overflow for hint scale computation.
  /// See <https://oss-fuzz.com/testcase-detail/6498790355042304>
  /// and <https://issues.oss-fuzz.com/issues/444024349>
  #[test]
  fn subfont_hint_scale_overflow() {
    let _ = scale_for_hinting(Some(
      ({
        let Ok(value) = Fixed::from_bits(i32::MAX);
        value
      }),
    ));
  }
}
