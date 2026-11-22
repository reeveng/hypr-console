use super::layout::DELETED_GLYPH;
use super::map::RangeFlags;
use crate::U32Set;
use crate::hb::buffer::{HB_BUFFER_SCRATCH_FLAG_SHAPER0, hb_buffer_t};
use crate::hb::face::Scale;
use crate::hb::face::hb_font_t;
use crate::hb::hb_mask_t;
use crate::hb::ot_layout_gsubgpos::MappingCache;
use crate::hb::ot_shape_plan::hb_ot_shape_plan_t;
use read_fonts::tables::aat::*;
use read_fonts::types::{FixedSize, GlyphId};

pub const HB_BUFFER_SCRATCH_FLAG_AAT_HAS_DELETED: u32 = HB_BUFFER_SCRATCH_FLAG_SHAPER0;

pub(crate) const START_OF_TEXT: u16 = 0;

pub(crate) type ClassCache = MappingCache;

pub(crate) fn get_class<T: bytemuck::AnyBitPattern + FixedSize>(
  machine: &ExtendedStateTable<'_, T>,
  glyph_id: GlyphId,
  cache: &ClassCache,
) -> u16 {
  match cache.get({
    let Ok(value) = glyph_id.to_u32();
    value
  }) {
    Some(klass) => {
      return ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(klass);
        migrated_value
      });
    }
    None => {}
  }
  let klass = machine.class(glyph_id).unwrap_or(
    ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u16>(class::OUT_OF_BOUNDS);
      migrated_value
    }),
  );
  cache.set(
    {
      let Ok(value) = glyph_id.to_u32();
      value
    },
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(klass);
      migrated_value
    }),
  );
  klass
}

/// HB: hb_aat_apply_context_t
///
/// See <https://github.com/harfbuzz/harfbuzz/blob/2c22a65f0cb99544c36580b9703a43b5dc97a9e1/src/hb-aat-layout-common.hh#L108>
#[doc(alias = "hb_aat_apply_context_t")]
pub struct AatApplyContext<'a> {
  pub plan: &'a hb_ot_shape_plan_t,
  pub face: &'a hb_font_t<'a>,
  pub scale: Scale,
  pub buffer: &'a mut hb_buffer_t,
  pub has_glyph_classes: bool,
  pub range_flags: Option<&'a [RangeFlags]>,
  pub subtable_flags: hb_mask_t,
  pub(crate) buffer_is_reversed: bool,

  using_buffer_glyph_set: bool,
  pub(crate) first_set: Option<&'a U32Set>,
  pub(crate) second_set: Option<&'a U32Set>,
  pub(crate) machine_class_cache: Option<&'a ClassCache>,
  pub(crate) start_end_safe_to_break: u64,
}

impl<'a> AatApplyContext<'a> {
  pub fn new(
    plan: &'a hb_ot_shape_plan_t,
    face: &'a hb_font_t<'a>,
    scale: Scale,
    buffer: &'a mut hb_buffer_t,
  ) -> Self {
    Self {
      plan,
      face,
      scale,
      buffer,
      has_glyph_classes: face.ot_tables.has_glyph_classes(),
      range_flags: None,
      subtable_flags: 0,
      buffer_is_reversed: false,
      using_buffer_glyph_set: false,
      first_set: None,
      second_set: None,
      machine_class_cache: None,
      start_end_safe_to_break: 0,
    }
  }

  #[inline(always)]
  pub(crate) fn scale_x(&self, value: i32) -> i32 {
    self.scale.scale_x(value)
  }

  #[inline(always)]
  pub(crate) fn scale_y(&self, value: i32) -> i32 {
    self.scale.scale_y(value)
  }

  pub(crate) fn reverse_buffer(&mut self) {
    self.buffer.reverse();
    self.buffer_is_reversed = !self.buffer_is_reversed;
  }

  pub(crate) fn setup_buffer_glyph_set(&mut self) {
    self.using_buffer_glyph_set = self.buffer.len >= 4;

    match self.using_buffer_glyph_set {
      true => {
        self.buffer.update_glyph_set();
      }
      false => {}
    }
  }

  pub(crate) fn buffer_intersects_machine(&self) -> bool {
    match &self.first_set {
      Some(first_set) => {
        match self.using_buffer_glyph_set {
          true => {
            return ({
              let Ok(value) = self.buffer.glyph_set.intersects_set(first_set);
              matches!(value, console_core_integer_sets::Overlap::Intersecting)
            });
          }
          false => {}
        }
        for info in &self.buffer.info {
          if ({
            let Ok(value) = first_set.contains(info.glyph_id);
            matches!(value, console_core_integer_sets::Presence::Present)
          }) {
            return true;
          }
        }
        false
      }
      None => true,
    }
  }

  pub fn output_glyph(&mut self, glyph: u32) {
    match self.using_buffer_glyph_set {
      true => {
        self.buffer.glyph_set.insert(glyph);
      }
      false => {}
    }
    match glyph == DELETED_GLYPH {
      true => {
        self.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_AAT_HAS_DELETED;
        self.buffer.cur_mut(0).set_aat_deleted();
      }
      false => match self.has_glyph_classes {
        true => {
          self
            .buffer
            .cur_mut(0)
            .set_glyph_props(self.face.ot_tables.glyph_props(glyph.into()));
        }
        false => {}
      },
    }
    self.buffer.output_glyph(glyph);
  }

  pub fn replace_glyph(&mut self, glyph: u32) {
    match glyph == DELETED_GLYPH {
      true => {
        self.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_AAT_HAS_DELETED;
        self.buffer.cur_mut(0).set_aat_deleted();
      }
      false => {}
    }

    match self.using_buffer_glyph_set {
      true => {
        self.buffer.glyph_set.insert(glyph);
      }
      false => {}
    }
    match self.has_glyph_classes {
      true => {
        self
          .buffer
          .cur_mut(0)
          .set_glyph_props(self.face.ot_tables.glyph_props(glyph.into()));
      }
      false => {}
    }
    self.buffer.replace_glyph(glyph);
  }

  pub fn delete_glyph(&mut self) {
    self.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_AAT_HAS_DELETED;
    self.buffer.cur_mut(0).set_aat_deleted();
    self.buffer.replace_glyph(DELETED_GLYPH);
  }

  pub fn replace_glyph_inplace(&mut self, i: usize, glyph: u32) {
    self.buffer.info[i].glyph_id = glyph;
    match glyph == DELETED_GLYPH {
      true => {
        self.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_AAT_HAS_DELETED;
        self.buffer.info[i].set_aat_deleted();
      }
      false => {}
    }
    match self.using_buffer_glyph_set {
      true => {
        self.buffer.glyph_set.insert(glyph);
      }
      false => {}
    }
    match self.has_glyph_classes {
      true => {
        self.buffer.info[i].set_glyph_props(self.face.ot_tables.glyph_props(glyph.into()));
      }
      false => {}
    }
  }
}

pub trait TypedCollectGlyphs<T: LookupValue> {
  /// Add all indices into `set`.
  fn collect_glyphs(&self, set: &mut U32Set, num_glyphs: u32) {
    self.collect_glyphs_filtered::<_>(set, num_glyphs, |_| true);
  }

  /// For each valid index, read the value of type `T`.
  /// If `filter(&value)` returns true, insert the index into `set`.
  fn collect_glyphs_filtered<F>(&self, _set: &mut U32Set, _num_glyphs: u32, _filter: F)
  where
    F: Fn(T) -> bool;
}

impl<T> TypedCollectGlyphs<T> for TypedLookup<'_, T>
where
  T: LookupValue,
{
  fn collect_glyphs(&self, set: &mut U32Set, num_glyphs: u32) {
    self.lookup.collect_glyphs::<T>(set, num_glyphs);
  }
  fn collect_glyphs_filtered<F>(&self, set: &mut U32Set, num_glyphs: u32, filter: F)
  where
    F: Fn(T) -> bool,
  {
    self
      .lookup
      .collect_glyphs_filtered::<T, F>(set, num_glyphs, filter);
  }
}

pub trait CollectGlyphs {
  /// Add all indices into `set`.
  fn collect_glyphs<T>(&self, set: &mut U32Set, num_glyphs: u32)
  where
    T: LookupValue,
  {
    self.collect_glyphs_filtered::<T, _>(set, num_glyphs, |_| true);
  }

  /// For each valid index, read the value of type `T`.
  /// If `filter(&value)` returns true, insert the index into `set`.
  fn collect_glyphs_filtered<T, F>(&self, _set: &mut U32Set, _num_glyphs: u32, _filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool;
}

impl CollectGlyphs for Lookup<'_> {
  fn collect_glyphs<T>(&self, set: &mut U32Set, num_glyphs: u32)
  where
    T: LookupValue,
  {
    match self {
      Lookup::Format0(lookup) => lookup.collect_glyphs::<T>(set, num_glyphs),
      Lookup::Format2(lookup) => lookup.collect_glyphs::<T>(set, num_glyphs),
      Lookup::Format4(lookup) => lookup.collect_glyphs::<T>(set, num_glyphs),
      Lookup::Format6(lookup) => lookup.collect_glyphs::<T>(set, num_glyphs),
      Lookup::Format8(lookup) => lookup.collect_glyphs::<T>(set, num_glyphs),
      Lookup::Format10(lookup) => lookup.collect_glyphs::<T>(set, num_glyphs),
    }
  }
  fn collect_glyphs_filtered<T, F>(&self, set: &mut U32Set, num_glyphs: u32, filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool,
  {
    match self {
      Lookup::Format0(lookup) => {
        lookup.collect_glyphs_filtered::<T, F>(set, num_glyphs, filter);
      }
      Lookup::Format2(lookup) => {
        lookup.collect_glyphs_filtered::<T, F>(set, num_glyphs, filter);
      }
      Lookup::Format4(lookup) => {
        lookup.collect_glyphs_filtered::<T, F>(set, num_glyphs, filter);
      }
      Lookup::Format6(lookup) => {
        lookup.collect_glyphs_filtered::<T, F>(set, num_glyphs, filter);
      }
      Lookup::Format8(lookup) => {
        lookup.collect_glyphs_filtered::<T, F>(set, num_glyphs, filter);
      }
      Lookup::Format10(lookup) => {
        lookup.collect_glyphs_filtered::<T, F>(set, num_glyphs, filter);
      }
    }
  }
}

impl CollectGlyphs for Lookup0<'_> {
  fn collect_glyphs<T>(&self, set: &mut U32Set, num_glyphs: u32)
  where
    T: LookupValue,
  {
    set.insert_range(0..=num_glyphs.saturating_sub(1));
  }
  fn collect_glyphs_filtered<T, F>(&self, set: &mut U32Set, num_glyphs: u32, filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool,
  {
    match self.values::<T>() {
      Ok(values) => {
        for (i, value) in values
          .iter()
          .take(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(num_glyphs);
              migrated_value
            }),
          )
          .enumerate()
        {
          if filter({
            let Ok(value) = value.get();
            value
          }) {
            set.insert(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
                migrated_value
              }),
            );
          }
        }
      }
      Err(_) => {}
    }
  }
}
impl CollectGlyphs for Lookup2<'_> {
  fn collect_glyphs_filtered<T, F>(&self, set: &mut U32Set, _num_glyphs: u32, filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool,
  {
    match self.segments::<T>() {
      Ok(segments) => {
        for segment in segments {
          let value = segment.value;
          if filter({
            let Ok(value) = value.get();
            value
          }) {
            if ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                ({
                  let Ok(value) = segment.first_glyph.get();
                  value
                }),
              );
              migrated_value
            }) == DELETED_GLYPH
            {
              continue;
            }
            set.insert_range(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                  ({
                    let Ok(value) = segment.first_glyph.get();
                    value
                  }),
                );
                migrated_value
              })..=({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                  ({
                    let Ok(value) = segment.last_glyph.get();
                    value
                  }),
                );
                migrated_value
              }),
            );
          }
        }
      }
      Err(_) => {}
    }
  }
}
impl CollectGlyphs for Lookup4<'_> {
  fn collect_glyphs<T>(&self, set: &mut U32Set, _num_glyphs: u32)
  where
    T: LookupValue,
  {
    let Ok(segments) = self.segments();

    for segment in segments {
      if ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
          ({
            let Ok(value) = segment.first_glyph.get();
            value
          }),
        );
        migrated_value
      }) == DELETED_GLYPH
      {
        continue;
      }
      set.insert_range(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
            let Ok(value) = segment.first_glyph.get();
            value
          });
          migrated_value
        })..=({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
            ({
              let Ok(value) = segment.last_glyph.get();
              value
            }),
          );
          migrated_value
        }),
      );
    }
  }
  fn collect_glyphs_filtered<T, F>(&self, set: &mut U32Set, _num_glyphs: u32, filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool,
  {
    let Ok(segments) = self.segments();

    for (segment_idx, segment) in segments.iter().enumerate() {
      if ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
          ({
            let Ok(value) = segment.first_glyph.get();
            value
          }),
        );
        migrated_value
      }) == DELETED_GLYPH
      {
        continue;
      }
      let segment_values = self.segment_values(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(segment_idx);
          value
        }),
      );
      match segment_values {
        Ok(segment_values) => {
          for (i, value) in segment_values.iter().enumerate() {
            if filter({
              let Ok(value) = value.get();
              value
            }) {
              set.insert(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
                    let Ok(value) = segment.first_glyph.get();
                    value
                  });
                  migrated_value
                }) + ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
                  migrated_value
                }),
              );
            }
          }
        }
        Err(_) => {}
      }
    }
  }
}
impl CollectGlyphs for Lookup6<'_> {
  fn collect_glyphs_filtered<T, F>(&self, set: &mut U32Set, _num_glyphs: u32, filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool,
  {
    let entries = self.entries();
    match entries {
      Ok(entries) => {
        for entry in entries {
          let value = entry.value;
          if filter({
            let Ok(value) = value.get();
            value
          }) {
            if ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                ({
                  let Ok(value) = entry.glyph.get();
                  value
                }),
              );
              migrated_value
            }) == DELETED_GLYPH
            {
              continue;
            }
            set.insert(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
                  let Ok(value) = entry.glyph.get();
                  value
                });
                migrated_value
              }),
            );
          }
        }
      }
      Err(_) => {}
    }
  }
}
impl CollectGlyphs for Lookup8<'_> {
  fn collect_glyphs<T>(&self, set: &mut U32Set, _num_glyphs: u32)
  where
    T: LookupValue,
  {
    let Ok(value_array) = self.value_array();
    let n_values = value_array.len();
    let Ok(first_glyph) = self.first_glyph();
    match ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
      migrated_value
    }) == DELETED_GLYPH
    {
      true => {
        return;
      }
      false => {}
    }
    set.insert_range(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
        migrated_value
      })..=({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
        migrated_value
      }) + ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(n_values.saturating_sub(1));
        migrated_value
      }),
    );
  }
  fn collect_glyphs_filtered<T, F>(&self, set: &mut U32Set, _num_glyphs: u32, filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool,
  {
    let Ok(values) = self.value_array();
    let Ok(first_glyph) = self.first_glyph();
    match ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
      migrated_value
    }) == DELETED_GLYPH
    {
      true => {
        return;
      }
      false => {}
    }
    for (i, value) in values.iter().enumerate() {
      if filter(T::from_u16({
        let value = value.get();
        ({
          let Ok(explicit_value) = value;
          explicit_value
        })
      })) {
        set.insert(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(first_glyph);
            migrated_value
          }) + ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
            migrated_value
          }),
        );
      }
    }
  }
}
impl CollectGlyphs for Lookup10<'_> {
  fn collect_glyphs<T>(&self, set: &mut U32Set, _num_glyphs: u32)
  where
    T: LookupValue,
  {
    let Ok(n_values) = self.glyph_count();
    let Ok(first_glyph) = self.first_glyph();
    match ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
      migrated_value
    }) == DELETED_GLYPH
    {
      true => {
        return;
      }
      false => {}
    }
    set.insert_range(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
        migrated_value
      })..=({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
        migrated_value
      }) + ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(n_values.saturating_sub(1));
        migrated_value
      }),
    );
  }
  fn collect_glyphs_filtered<T, F>(&self, set: &mut U32Set, _num_glyphs: u32, filter: F)
  where
    T: LookupValue,
    F: Fn(T) -> bool,
  {
    let Ok(first_glyph) = self.first_glyph();
    match ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
      migrated_value
    }) == DELETED_GLYPH
    {
      true => {
        return;
      }
      false => {}
    }
    let Ok(glyph_count) = self.glyph_count();

    for i in 0..glyph_count {
      let idx = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(first_glyph);
        migrated_value
      }) + ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
        migrated_value
      });

      let value = self.value::<T>(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(idx);
          migrated_value
        }),
      );
      match value {
        Ok(value) => match filter(value) {
          true => {
            set.insert(idx);
          }
          false => {}
        },
        Err(_) => {}
      }
    }
  }
}
