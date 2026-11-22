use super::aat::layout::DELETED_GLYPH;
use alloc::boxed::Box;
use read_fonts::{
  tables::{
    aat,
    kern::{Subtable, Subtable0, Subtable2, Subtable3, SubtableKind},
  },
  types::{GlyphId, GlyphId16},
};

use super::aat::layout_common::{AatApplyContext, ClassCache, START_OF_TEXT};
use super::aat::layout_kerx_table::SimpleKerning;
use super::buffer::*;
use super::face::Scale;
use super::ot_layout::TableIndex;
use super::ot_layout_common::lookup_flags;
use super::ot_layout_gpos_table::attach_type;
use super::ot_layout_gsubgpos::{OT::hb_ot_apply_context_t, skipping_iterator_t};
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::{hb_font_t, hb_mask_t};
use crate::U32Set;

pub(crate) fn get_class(machine: &aat::StateTable, glyph_id: GlyphId, cache: &ClassCache) -> u8 {
  match cache.get({
    let Ok(value) = glyph_id.to_u32();
    value
  }) {
    Some(klass) => {
      return ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(klass);
        migrated_value
      });
    }
    None => {}
  }
  let klass = machine
    .class({
      let Ok(value) = GlyphId16::new(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>({
            let Ok(value) = glyph_id.to_u32();
            value
          });
          migrated_value
        }),
      );
      value
    })
    .unwrap_or(aat::class::OUT_OF_BOUNDS);
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

pub fn hb_ot_layout_kern(
  plan: &hb_ot_shape_plan_t,
  face: &hb_font_t,
  scale: Scale,
  buffer: &mut hb_buffer_t,
) -> Option<()> {
  let mut c = AatApplyContext::new(plan, face, scale, buffer);

  let (kern, subtable_caches) = c.face.aat_tables.kern.as_ref()?;

  let mut subtable_idx = 0;

  let mut seen_cross_stream = false;
  for subtable in kern.subtables() {
    let subtable = match subtable {
      Ok(subtable) => subtable,
      Err(_) => continue,
    };

    let subtable_cache = subtable_caches.get(subtable_idx);
    let subtable_cache = match subtable_cache.as_ref() {
      Some(subtable_cache) => subtable_cache,
      None => {
        break;
      }
    };
    subtable_idx += 1;

    match subtable.is_variable() {
      true => {
        continue;
      }
      false => {}
    }

    match c.buffer.direction.is_horizontal() != subtable.is_horizontal() {
      true => {
        continue;
      }
      false => {}
    }

    c.first_set = Some(&subtable_cache.first_set);
    c.second_set = Some(&subtable_cache.second_set);
    c.machine_class_cache = Some(&subtable_cache.class_cache);
    c.start_end_safe_to_break = subtable_cache.start_end_safe_to_break;

    match !c.buffer_intersects_machine() {
      true => {
        continue;
      }
      false => {}
    }

    let reverse = c.buffer.direction.is_backward();
    let is_cross_stream = subtable.is_cross_stream();

    match !seen_cross_stream && is_cross_stream {
      true => {
        seen_cross_stream = true;

        for pos in &mut c.buffer.pos {
          pos.set_attach_type(attach_type::CURSIVE);
          pos.set_attach_chain(match c.buffer.direction.is_forward() {
            true => -1,
            false => 1,
          });
        }
      }
      false => {}
    }

    let kind = match subtable.kind() {
      Ok(kind) => kind,
      Err(_) => {
        continue;
      }
    };

    match reverse != c.buffer_is_reversed {
      true => {
        c.reverse_buffer();
      }
      false => {}
    }

    match kind {
      SubtableKind::Format0(format0) if plan.requested_kerning => {
        apply_simple_kerning(&mut c, &format0, is_cross_stream);
      }
      SubtableKind::Format1(format1) => {
        apply_state_machine_kerning(&mut c, &format1, is_cross_stream);
      }
      SubtableKind::Format2(format2) if plan.requested_kerning => {
        apply_simple_kerning(&mut c, &format2, is_cross_stream);
      }
      SubtableKind::Format3(format3) if plan.requested_kerning => {
        apply_simple_kerning(&mut c, &format3, is_cross_stream);
      }
      _ => {}
    }
  }
  match c.buffer_is_reversed {
    true => {
      c.reverse_buffer();
    }
    false => {}
  }
  Some(())
}

fn machine_kern<F>(
  face: &hb_font_t,
  scale: Scale,
  buffer: &mut hb_buffer_t,
  kern_mask: hb_mask_t,
  cross_stream: bool,
  get_kerning: F,
) where
  F: Fn(u32, u32) -> i32,
{
  buffer.unsafe_to_concat(None, None);
  let mut ctx = hb_ot_apply_context_t::new(TableIndex::GPOS, face, scale, buffer);
  ctx.set_lookup_mask(kern_mask);
  ctx.lookup_props = u32::from(lookup_flags::IGNORE_MARKS);
  ctx.update_matchers();

  let horizontal = ctx.buffer.direction.is_horizontal();
  let use_x_scale = horizontal ^ cross_stream;
  let mut i = 0;
  let mut iter = skipping_iterator_t::new(&mut ctx, false);
  while i < iter.buffer.len {
    match (iter.buffer.info[i].mask & kern_mask) == 0 {
      true => {
        i += 1;
        continue;
      }
      false => {}
    }

    iter.reset_fast(i);

    let mut unsafe_to = 0;
    match !iter.next(Some(&mut unsafe_to)) {
      true => {
        i += 1;
        continue;
      }
      false => {}
    }

    let j = iter.index();

    let info = &iter.buffer.info;
    let kern = get_kerning(info[i].glyph_id, info[j].glyph_id);
    let kern = match use_x_scale {
      true => scale.scale_x(kern),
      false => scale.scale_y(kern),
    };
    let pos = &mut iter.buffer.pos;
    match kern != 0 {
      true => {
        match horizontal {
          true => match cross_stream {
            true => {
              pos[j].y_offset = kern;
              iter.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
            }
            false => {
              let kern1 = kern >> 1;
              let kern2 = kern - kern1;
              pos[i].x_advance = pos[i].x_advance.saturating_add(kern1);
              pos[j].x_advance = pos[j].x_advance.saturating_add(kern2);
              pos[j].x_offset = pos[j].x_offset.saturating_add(kern2);
            }
          },
          false => match cross_stream {
            true => {
              pos[j].x_offset = kern;
              iter.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
            }
            false => {
              let kern1 = kern >> 1;
              let kern2 = kern - kern1;
              pos[i].y_advance = pos[i].y_advance.saturating_add(kern1);
              pos[j].y_advance = pos[j].y_advance.saturating_add(kern2);
              pos[j].y_offset = pos[j].y_offset.saturating_add(kern2);
            }
          },
        }

        iter.buffer.unsafe_to_break(Some(i), Some(j + 1));
      }
      false => {}
    }

    i = j;
  }
}

fn apply_simple_kerning<T: SimpleKerning>(
  c: &mut AatApplyContext,
  subtable: &T,
  is_cross_stream: bool,
) {
  let first_set = c.first_set.as_ref().unwrap();
  let second_set = c.second_set.as_ref().unwrap();

  machine_kern(
    c.face,
    c.scale,
    c.buffer,
    c.plan.kern_mask,
    is_cross_stream,
    |left, right| {
      if !({
        let Ok(value) = first_set.contains(left);
        matches!(value, console_core_integer_sets::Presence::Present)
      }) || !({
        let Ok(value) = second_set.contains(right);
        matches!(value, console_core_integer_sets::Presence::Present)
      }) {
        0
      } else {
        subtable
          .simple_kerning(left.into(), right.into())
          .unwrap_or(0)
      }
    },
  );
}

struct StateMachineDriver {
  stack: [usize; 8],
  depth: usize,
}

pub trait CollectGlyphs {
  /// For each valid index, read the value of type `T`.
  /// If `filter(&value)` returns true, insert the index into `set`.
  fn collect_glyphs_filtered<F>(&self, _set: &mut U32Set, _num_glyphs: u32, _filter: F)
  where
    F: Fn(u8) -> bool;
}

impl CollectGlyphs for aat::ClassSubtable<'_> {
  fn collect_glyphs_filtered<F>(&self, set: &mut U32Set, _num_glyphs: u32, filter: F)
  where
    F: Fn(u8) -> bool,
  {
    let first_glyph = ({
      let Ok(own_first_glyph) = self.first_glyph();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(own_first_glyph);
      migrated_value
    });
    let Ok(class_array) = self.class_array();
    for (i, class) in class_array.iter().enumerate() {
      let gid = first_glyph
        + ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
          migrated_value
        });
      match filter(*class) {
        true => {
          set.insert(gid);
        }
        false => {}
      }
    }
  }
}

fn collect_initial_glyphs(machine: &aat::StateTable, glyphs: &mut U32Set, num_glyphs: u32) {
  let mut classes = U32Set::default();

  let class_table = machine.header.class_table().ok();
  let class_table = match class_table {
    Some(class_table) => class_table,
    None => {
      return;
    }
  };

  let Ok(n_classes) = machine.header.state_size();
  for i in 0..n_classes {
    match machine.entry(
      START_OF_TEXT,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(i);
        migrated_value
      }),
    ) {
      Ok(entry) => {
        match entry.new_state == START_OF_TEXT
          && !entry.is_action_initiable()
          && !entry.is_actionable()
        {
          true => {
            continue;
          }
          false => {}
        }
        classes.insert(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
            migrated_value
          }),
        );
      }
      Err(_) => {}
    }
  }

  let filter = |class: u8| {
    ({
      let Ok(value) = classes.contains(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(class);
          migrated_value
        }),
      );
      matches!(value, console_core_integer_sets::Presence::Present)
    })
  };

  match (filter(aat::class::DELETED_GLYPH)) {
    true => {
      glyphs.insert(DELETED_GLYPH);
    }
    false => {}
  }

  class_table.collect_glyphs_filtered(glyphs, num_glyphs, filter);
}

fn collect_start_end_safe_to_break(machine: &aat::StateTable) -> u64 {
  let mut result = 0u64;
  for state in 0..64 {
    let bit = match machine.entry(state, aat::class::END_OF_TEXT) {
      Ok(entry) => !entry.is_actionable(),
      Err(_) => true,
    };
    match bit {
      true => {
        result |= 1 << state;
      }
      false => {}
    }
  }
  result
}

fn apply_state_machine_kerning(
  c: &mut AatApplyContext,
  subtable: &aat::StateTable,
  is_cross_stream: bool,
) {
  let mut driver = StateMachineDriver {
    stack: [0; 8],
    depth: 0,
  };

  let mut state = START_OF_TEXT;
  c.buffer.idx = 0;
  loop {
    let class = match c.buffer.idx < c.buffer.len {
      true => get_class(
        subtable,
        c.buffer.cur(0).as_glyph(),
        c.machine_class_cache.unwrap(),
      ),
      false => aat::class::END_OF_TEXT,
    };

    let entry = match subtable.entry(state, class) {
      Ok(entry) => entry,
      Err(_) => {
        break;
      }
    };

    let next_state = entry.new_state;

    let is_safe_to_break = !entry.is_actionable()
      && (state == START_OF_TEXT || (!entry.has_advance() && next_state == START_OF_TEXT) || {
        match subtable.entry(START_OF_TEXT, class) {
          Ok(wouldbe_entry) => {
            !wouldbe_entry.is_actionable()
              && (next_state == wouldbe_entry.new_state
                && entry.has_advance() == wouldbe_entry.has_advance())
          }
          Err(_) => false,
        }
      })
      && (match state < 64 {
        true => (c.start_end_safe_to_break & (1 << state)) != 0,
        false => match subtable.entry(state, aat::class::END_OF_TEXT) {
          Ok(end_entry) => !end_entry.is_actionable(),
          Err(_) => false,
        },
      });

    match !is_safe_to_break && c.buffer.backtrack_len() > 0 && c.buffer.idx < c.buffer.len {
      true => {
        c.buffer.unsafe_to_break_from_outbuffer(
          Some(c.buffer.backtrack_len() - 1),
          Some(c.buffer.idx + 1),
        );
      }
      false => {}
    }

    state_machine_transition(c, subtable, &entry, is_cross_stream, &mut driver);

    state = next_state;

    match c.buffer.idx >= c.buffer.len {
      true => {
        break;
      }
      false => {}
    }

    c.buffer.max_ops -= 1;
    match entry.has_advance() || c.buffer.max_ops <= 0 {
      true => {
        c.buffer.next_glyph();
      }
      false => {}
    }
  }
}

#[inline(always)]
fn state_machine_transition(
  c: &mut AatApplyContext,
  subtable: &aat::StateTable,
  entry: &aat::StateEntry,
  is_cross_stream: bool,
  driver: &mut StateMachineDriver,
) {
  let scale = c.scale;
  let use_x_scale = c.buffer.direction.is_horizontal() ^ is_cross_stream;
  let buffer = &mut *c.buffer;
  let kern_mask = c.plan.kern_mask;

  match entry.has_push() {
    true => match driver.depth < driver.stack.len() {
      true => {
        driver.stack[driver.depth] = buffer.idx;
        driver.depth += 1;
      }
      false => {
        driver.depth = 0;
      }
    },
    false => {}
  }

  match entry.has_offset() && driver.depth != 0 {
    true => {
      let mut value_offset = entry.value_offset();
      let mut value = match subtable.read_value::<i16>(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped_index(value_offset);
              migrated_value
            }),
          );
          value
        }),
      ) {
        Ok(value) => value,
        Err(_) => {
          driver.depth = 0;
          return;
        }
      };

      let mut last = false;
      while !last && driver.depth != 0 {
        driver.depth -= 1;
        let idx = driver.stack[driver.depth];
        let mut v = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value);
          migrated_value
        });
        value_offset = value_offset.wrapping_add(2);
        value = subtable
          .read_value::<i16>(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped_index(value_offset);
                  migrated_value
                }),
              );
              value
            }),
          )
          .unwrap_or(0);
        match idx >= buffer.len {
          true => {
            continue;
          }
          false => {}
        }

        last = v & 1 != 0;
        v &= !1;
        let scaled_v = match use_x_scale {
          true => scale.scale_x(v),
          false => scale.scale_y(v),
        };

        let mut has_gpos_attachment = false;
        let glyph_mask = buffer.info[idx].mask;
        let pos = &mut buffer.pos[idx];

        match buffer.direction.is_horizontal() {
          true => match is_cross_stream {
            true => match v == -0x8000 {
              true => {
                pos.set_attach_type(0);
                pos.set_attach_chain(0);
                pos.y_offset = 0;
              }
              false => match pos.attach_type() != 0 {
                true => {
                  pos.y_offset = pos.y_offset.saturating_add(scaled_v);
                  has_gpos_attachment = true;
                }
                false => {}
              },
            },
            false => match glyph_mask & kern_mask != 0 {
              true => {
                pos.x_advance = pos.x_advance.saturating_add(scaled_v);
                pos.x_offset = pos.x_offset.saturating_add(scaled_v);
              }
              false => {}
            },
          },
          false => match is_cross_stream {
            true => match v == -0x8000 {
              true => {
                pos.set_attach_type(0);
                pos.set_attach_chain(0);
                pos.x_offset = 0;
              }
              false => match pos.attach_type() != 0 {
                true => {
                  pos.x_offset = pos.x_offset.saturating_add(scaled_v);
                  has_gpos_attachment = true;
                }
                false => {}
              },
            },
            false => match glyph_mask & kern_mask != 0 {
              true => match pos.y_offset == 0 {
                true => {
                  pos.y_advance = pos.y_advance.saturating_add(scaled_v);
                  pos.y_offset = pos.y_offset.saturating_add(scaled_v);
                }
                false => {}
              },
              false => {}
            },
          },
        }

        match has_gpos_attachment {
          true => {
            buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
          }
          false => {}
        }
      }
    }
    false => {}
  }
}

trait KernStateEntryExt {
  fn flags(&self) -> u16;

  fn is_action_initiable(&self) -> bool {
    self.flags() & 0x8000 != 0
  }

  fn is_actionable(&self) -> bool {
    self.flags() & 0x3FFF != 0
  }

  fn has_offset(&self) -> bool {
    self.flags() & 0x3FFF != 0
  }

  fn value_offset(&self) -> u16 {
    self.flags() & 0x3FFF
  }

  fn has_advance(&self) -> bool {
    self.flags() & 0x4000 == 0
  }

  fn has_push(&self) -> bool {
    self.flags() & 0x8000 != 0
  }
}

impl<T> KernStateEntryExt for aat::StateEntry<T> {
  fn flags(&self) -> u16 {
    self.flags
  }
}

impl SimpleKerning for Subtable0<'_> {
  fn simple_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    self.kerning(left, right)
  }
  fn collect_glyphs(&self, first_set: &mut U32Set, second_set: &mut U32Set, _num_glyphs: u32) {
    let Ok(pairs) = self.pairs();

    for &pair in pairs {
      first_set.insert({
        let Ok(value) = ({
          let Ok(value) = pair.left.get();
          value
        })
        .to_u32();
        value
      });
      second_set.insert({
        let Ok(value) = ({
          let Ok(value) = pair.right.get();
          value
        })
        .to_u32();
        value
      });
    }
  }
}

impl SimpleKerning for Subtable2<'_> {
  fn simple_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    self.kerning(left, right)
  }
  fn collect_glyphs(&self, first_set: &mut U32Set, second_set: &mut U32Set, _num_glyphs: u32) {
    let left_classes = &self.left_offset_table;
    let right_classes = &self.right_offset_table;

    let first_glyph = ({
      let Ok(left_classes_first_glyph) = left_classes.first_glyph();
      let Ok(value) = left_classes_first_glyph.to_u32();
      value
    });
    let last_glyphs = (first_glyph)
      + ({
        let Ok(n_glyphs) = left_classes.n_glyphs();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(n_glyphs.saturating_sub(1));
        migrated_value
      });
    first_set.insert_range((first_glyph)..=last_glyphs);

    let first_glyph = ({
      let Ok(right_classes_first_glyph) = right_classes.first_glyph();
      let Ok(value) = right_classes_first_glyph.to_u32();
      value
    });
    let last_glyphs = (first_glyph)
      + ({
        let Ok(n_glyphs) = right_classes.n_glyphs();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(n_glyphs.saturating_sub(1));
        migrated_value
      });
    second_set.insert_range((first_glyph)..=last_glyphs);
  }
}

impl SimpleKerning for Subtable3<'_> {
  fn simple_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    self.kerning(left, right)
  }
  fn collect_glyphs(&self, first_set: &mut U32Set, second_set: &mut U32Set, _num_glyphs: u32) {
    first_set.insert_range(
      0..=({
        let Ok(glyph_count) = self.glyph_count();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(glyph_count.saturating_sub(1));
        migrated_value
      }),
    );
    second_set.insert_range(
      0..=({
        let Ok(glyph_count) = self.glyph_count();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(glyph_count.saturating_sub(1));
        migrated_value
      }),
    );
  }
}

pub(crate) struct KernSubtableCache {
  start_end_safe_to_break: u64,
  first_set: U32Set,
  second_set: U32Set,
  class_cache: Box<ClassCache>,
}

impl KernSubtableCache {
  pub(crate) fn new(subtable: &Subtable, num_glyphs: u32) -> Self {
    let mut start_end_safe_to_break = 0u64;
    let mut first_set = U32Set::default();
    let mut second_set = U32Set::default();
    match subtable.kind() {
      Ok(kind) => match &kind {
        SubtableKind::Format0(format0) => {
          format0.collect_glyphs(&mut first_set, &mut second_set, num_glyphs);
        }
        SubtableKind::Format1(format1) => {
          start_end_safe_to_break = collect_start_end_safe_to_break(format1);
          collect_initial_glyphs(format1, &mut first_set, num_glyphs);
        }
        SubtableKind::Format2(format2) => {
          format2.collect_glyphs(&mut first_set, &mut second_set, num_glyphs);
        }
        SubtableKind::Format3(format3) => {
          format3.collect_glyphs(&mut first_set, &mut second_set, num_glyphs);
        }
      },
      Err(_) => {}
    }
    KernSubtableCache {
      start_end_safe_to_break,
      first_set,
      second_set,
      class_cache: Box::new(ClassCache::new()),
    }
  }
}
