use super::layout::DELETED_GLYPH;
use crate::U32Set;
use crate::hb::aat::layout_common::{
  AatApplyContext, ClassCache, START_OF_TEXT, TypedCollectGlyphs, get_class,
};
use crate::hb::{
  buffer::*,
  ot_layout::TableIndex,
  ot_layout_common::lookup_flags,
  ot_layout_gpos_table::attach_type,
  ot_layout_gsubgpos::{OT::hb_ot_apply_context_t, skipping_iterator_t},
};
use alloc::boxed::Box;
use core::convert::TryFrom;
use read_fonts::{
  tables::{
    aat,
    ankr::Ankr,
    kerx::{
      Subtable, Subtable0, Subtable1, Subtable2, Subtable4, Subtable4Actions, Subtable6,
      SubtableKind,
    },
  },
  types::{BigEndian, FixedSize, GlyphId},
};

pub(crate) fn apply(c: &mut AatApplyContext) -> Option<()> {
  c.buffer.unsafe_to_concat(None, None);

  c.setup_buffer_glyph_set();

  let (kerx, subtable_caches) = c.face.aat_tables.kerx.as_ref()?;

  let mut subtable_idx = 0;

  let mut seen_cross_stream = false;
  for subtable in ({
    let items = ({
      let Ok(array) = kerx.subtables();
      let Ok(items) = array.iter();
      items
    });
    items
  }) {
    let subtable = match subtable {
      Ok(subtable) => subtable,
      Err(_) => {
        continue;
      }
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

    match !seen_cross_stream && subtable.is_cross_stream() {
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

    match &kind {
      SubtableKind::Format0(format0) => {
        match !c.plan.requested_kerning {
          true => {
            continue;
          }
          false => {}
        }
        apply_simple_kerning(c, &subtable, format0);
      }
      SubtableKind::Format1(format1) => {
        let mut driver = Driver1 {
          stack: [0; 8],
          depth: 0,
        };
        apply_state_machine_kerning(c, &subtable, format1, &format1.state_table, &mut driver);
      }
      SubtableKind::Format2(format2) => {
        match !c.plan.requested_kerning {
          true => {
            continue;
          }
          false => {}
        }
        apply_simple_kerning(c, &subtable, format2);
      }
      SubtableKind::Format4(format4) => {
        let mut driver = Driver4 {
          mark_set: false,
          mark: 0,
          ankr_table: c.face.aat_tables.ankr.clone(),
        };
        apply_state_machine_kerning(c, &subtable, format4, &format4.state_table, &mut driver);
      }
      SubtableKind::Format6(format6) => {
        match !c.plan.requested_kerning {
          true => {
            continue;
          }
          false => {}
        }
        apply_simple_kerning(c, &subtable, format6);
      }
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

pub trait SimpleKerning {
  fn simple_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32>;
  fn collect_glyphs(&self, _first_set: &mut U32Set, _second_set: &mut U32Set, _num_glyphs: u32);
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
  fn collect_glyphs(&self, first_set: &mut U32Set, second_set: &mut U32Set, num_glyphs: u32) {
    let left_classes = &self.left_offset_table;
    let right_classes = &self.right_offset_table;

    left_classes.collect_glyphs(first_set, num_glyphs);
    right_classes.collect_glyphs(second_set, num_glyphs);
  }
}

impl SimpleKerning for Subtable6<'_> {
  fn simple_kerning(&self, left: GlyphId, right: GlyphId) -> Option<i32> {
    self.kerning(left, right)
  }
  fn collect_glyphs(&self, first_set: &mut U32Set, second_set: &mut U32Set, num_glyphs: u32) {
    match &self {
      Self::ShortValues(rows, columns, ..) => {
        rows.collect_glyphs(first_set, num_glyphs);
        columns.collect_glyphs(second_set, num_glyphs);
      }
      Self::LongValues(rows, columns, ..) => {
        rows.collect_glyphs(first_set, num_glyphs);
        columns.collect_glyphs(second_set, num_glyphs);
      }
    }
  }
}

fn apply_simple_kerning<T: SimpleKerning>(
  c: &mut AatApplyContext,
  subtable: &Subtable,
  kind: &T,
) {
  let scale = c.scale;
  let mut ctx = hb_ot_apply_context_t::new(TableIndex::GPOS, c.face, c.scale, c.buffer);
  ctx.set_lookup_mask(c.plan.kern_mask);
  ctx.lookup_props = u32::from(lookup_flags::IGNORE_MARKS);
  ctx.update_matchers();

  let horizontal = ctx.buffer.direction.is_horizontal();
  let cross_stream = subtable.is_cross_stream();
  let use_x_scale = horizontal ^ cross_stream;

  let first_set = c.first_set.as_ref().unwrap();
  let second_set = c.second_set.as_ref().unwrap();

  let mut i = 0;
  let mut iter = skipping_iterator_t::new(&mut ctx, false);
  while i < iter.buffer.len {
    match (iter.buffer.info[i].mask & c.plan.kern_mask) == 0 {
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
        iter.buffer.unsafe_to_concat(Some(i), Some(unsafe_to));
        i += 1;
        continue;
      }
      false => {}
    }

    let j = iter.index();

    let info = &iter.buffer.info;
    let a = info[i].as_glyph();
    let b = info[j].as_glyph();
    let kern = if !{
      let Ok(value) = first_set.contains({
        let Ok(value) = a.to_u32();
        value
      });
      matches!(value, console_core_integer_sets::Presence::Present)
    } || !{
      let Ok(value) = second_set.contains({
        let Ok(value) = b.to_u32();
        value
      });
      matches!(value, console_core_integer_sets::Presence::Present)
    } {
      0
    } else {
      kind.simple_kerning(a, b).unwrap_or(0)
    };
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

pub(crate) trait KerxStateEntryExt {
  fn flags(&self) -> u16;
  fn action_index(&self) -> u16;

  fn is_action_initiable(&self) -> bool {
    self.flags() & 0x8000 != 0
  }

  fn is_actionable(&self) -> bool {
    self.action_index() != 0xFFFF
  }

  fn has_advance(&self) -> bool {
    self.flags() & 0x4000 == 0
  }

  fn has_reset(&self) -> bool {
    self.flags() & 0x2000 != 0
  }

  fn has_push(&self) -> bool {
    self.flags() & 0x8000 != 0
  }

  fn has_mark(&self) -> bool {
    self.flags() & 0x8000 != 0
  }
}

impl KerxStateEntryExt for aat::StateEntry<BigEndian<u16>> {
  fn flags(&self) -> u16 {
    self.flags
  }

  fn action_index(&self) -> u16 {
    ({
      let Ok(value) = self.payload.get();
      value
    })
  }
}

fn collect_initial_glyphs<T>(
  machine: &aat::ExtendedStateTable<T>,
  glyphs: &mut U32Set,
  num_glyphs: u32,
) where
  T: FixedSize + bytemuck::AnyBitPattern,
  aat::StateEntry<T>: KerxStateEntryExt,
{
  let mut classes = U32Set::default();

  let class_table = &machine.class_table;
  for i in 0..machine.n_classes {
    match machine.entry(
      START_OF_TEXT,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(i);
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

  let filter = |class: u16| {
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

  match (filter(
    ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u16>(aat::class::DELETED_GLYPH);
      migrated_value
    }),
  )) {
    true => {
      glyphs.insert(DELETED_GLYPH);
    }
    false => {}
  }

  class_table.collect_glyphs_filtered(glyphs, num_glyphs, filter);
}

fn collect_start_end_safe_to_break<T>(machine: &aat::ExtendedStateTable<T>) -> u64
where
  T: FixedSize + bytemuck::AnyBitPattern,
  aat::StateEntry<T>: KerxStateEntryExt,
{
  let mut result = 0u64;
  for state in 0..64 {
    let bit = match machine.entry(
      state,
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(aat::class::END_OF_TEXT);
        migrated_value
      }),
    ) {
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

fn apply_state_machine_kerning<T, E, Driver: StateTableDriver<T, E>>(
  c: &mut AatApplyContext,
  subtable: &Subtable,
  kind: &T,
  state_table: &aat::ExtendedStateTable<E>,
  driver: &mut Driver,
) where
  E: FixedSize + bytemuck::AnyBitPattern,
  aat::StateEntry<E>: KerxStateEntryExt,
{
  let mut state = START_OF_TEXT;
  c.buffer.idx = 0;
  loop {
    let class = match c.buffer.idx < c.buffer.len {
      true => get_class(
        state_table,
        c.buffer.cur(0).as_glyph(),
        c.machine_class_cache.unwrap(),
      ),
      false => u16::from(aat::class::END_OF_TEXT),
    };

    let entry = match state_table.entry(state, class) {
      Ok(entry) => entry,
      Err(_) => {
        break;
      }
    };

    let next_state = entry.new_state;

    let is_safe_to_break = !entry.is_actionable()
      && (state == START_OF_TEXT || (!entry.has_advance() && next_state == START_OF_TEXT) || {
        match state_table.entry(START_OF_TEXT, class) {
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
        false => match state_table.entry(state, u16::from(aat::class::END_OF_TEXT)) {
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

    let _ = driver.transition(
      kind,
      &entry,
      subtable.is_cross_stream(),
      ({
        let Ok(explicit_value) = subtable.tuple_count();
        explicit_value
      }),
      c,
    );

    state = next_state;

    match c.buffer.idx >= c.buffer.len {
      true => {
        break;
      }
      false => {}
    }

    match entry.has_advance() || c.buffer.max_ops <= 0 {
      true => {
        c.buffer.next_glyph();
      }
      false => {}
    }
    c.buffer.max_ops -= 1;
  }
}

trait StateTableDriver<T, E> {
  fn transition(
    &mut self,
    aat: &T,
    entry: &aat::StateEntry<E>,
    has_cross_stream: bool,
    tuple_count: u32,
    c: &mut AatApplyContext,
  ) -> Option<()>;
}

struct Driver1 {
  stack: [usize; 8],
  depth: usize,
}

impl StateTableDriver<Subtable1<'_>, BigEndian<u16>> for Driver1 {
  #[inline(always)]
  fn transition(
    &mut self,
    aat: &Subtable1,
    entry: &aat::StateEntry<BigEndian<u16>>,
    has_cross_stream: bool,
    tuple_count: u32,
    c: &mut AatApplyContext,
  ) -> Option<()> {
    match entry.has_reset() {
      true => {
        self.depth = 0;
      }
      false => {}
    }

    match entry.has_push() {
      true => match self.depth < self.stack.len() {
        true => {
          self.stack[self.depth] = c.buffer.idx;
          self.depth += 1;
        }
        false => {
          self.depth = 0;
        }
      },
      false => {}
    }

    match entry.is_actionable() && self.depth != 0 {
      true => {
        let tuple_count = u16::try_from(tuple_count.max(1)).ok()?;

        let mut action_index = entry.action_index();

        let mut last = false;
        let use_x_scale = c.buffer.direction.is_horizontal() ^ has_cross_stream;
        while !last && self.depth != 0 {
          self.depth -= 1;
          let idx = self.stack[self.depth];
          let mut v = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
              ({
                let Ok(value) = aat
                  .values
                  .get(
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped_index(action_index);
                      migrated_value
                    }),
                  )?
                  .get();
                value
              }),
            );
            migrated_value
          });
          action_index = action_index.checked_add(tuple_count)?;
          match idx >= c.buffer.len {
            true => {
              continue;
            }
            false => {}
          }

          last = v & 1 != 0;
          v &= !1;
          let scaled_v = match use_x_scale {
            true => c.scale_x(v),
            false => c.scale_y(v),
          };

          let mut has_gpos_attachment = false;
          let glyph_mask = c.buffer.info[idx].mask;
          let pos = &mut c.buffer.pos[idx];

          match c.buffer.direction.is_horizontal() {
            true => match has_cross_stream {
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
              false => match glyph_mask & c.plan.kern_mask != 0 {
                true => {
                  pos.x_advance = pos.x_advance.saturating_add(scaled_v);
                  pos.x_offset = pos.x_offset.saturating_add(scaled_v);
                }
                false => {}
              },
            },
            false => match has_cross_stream {
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
              false => match glyph_mask & c.plan.kern_mask != 0 {
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
              c.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
            }
            false => {}
          }
        }
      }
      false => {}
    }

    Some(())
  }
}
struct Driver4<'a> {
  mark_set: bool,
  mark: usize,
  ankr_table: Option<Ankr<'a>>,
}

impl StateTableDriver<Subtable4<'_>, BigEndian<u16>> for Driver4<'_> {
  #[inline(always)]
  fn transition(
    &mut self,
    aat: &Subtable4,
    entry: &aat::StateEntry<BigEndian<u16>>,
    _has_cross_stream: bool,
    _tuple_count: u32,
    c: &mut AatApplyContext,
  ) -> Option<()> {
    match self.mark_set && entry.is_actionable() && c.buffer.idx < c.buffer.len {
      true => {
        match (self.ankr_table.as_ref(), &aat.actions) {
          (Some(ankr_table), Subtable4Actions::AnchorPoints(ankr_data)) => {
            let action_idx = ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped_index(entry.action_index());
              migrated_value
            }) * 2;
            let mark_action_idx = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
                ({
                  let Ok(value) = ankr_data.get(action_idx)?.get();
                  value
                }),
              );
              migrated_value
            });
            let curr_action_idx = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
                ({
                  let Ok(value) = ankr_data.get(action_idx + 1)?.get();
                  value
                }),
              );
              migrated_value
            });
            let mark_idx = c.buffer.info[self.mark].as_glyph();
            let mark_anchor = ankr_table
              .anchor_points(mark_idx)
              .ok()
              .and_then(|list| list.get(mark_action_idx))
              .map(|point| {
                let Ok(x) = point.x();
                let Ok(y) = point.y();
                (x, y)
              })
              .unwrap_or_default();

            let curr_idx = c.buffer.cur(0).as_glyph();
            let curr_anchor = ankr_table
              .anchor_points(curr_idx)
              .ok()
              .and_then(|list| list.get(curr_action_idx))
              .map(|point| {
                let Ok(x) = point.x();
                let Ok(y) = point.y();
                (x, y)
              })
              .unwrap_or_default();

            let x_offset =
              c.scale_x(i32::from(mark_anchor.0)) - c.scale_x(i32::from(curr_anchor.0));
            let y_offset =
              c.scale_y(i32::from(mark_anchor.1)) - c.scale_y(i32::from(curr_anchor.1));
            let pos = c.buffer.cur_pos_mut();
            pos.x_offset = x_offset;
            pos.y_offset = y_offset;
          }
          (_, Subtable4Actions::ControlPointCoords(coords)) => {
            let action_idx = ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped_index(entry.action_index());
              migrated_value
            }) * 4;
            let mark_x = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                ({
                  let Ok(value) = coords.get(action_idx)?.get();
                  value
                }),
              );
              migrated_value
            });
            let mark_y = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                ({
                  let Ok(value) = coords.get(action_idx + 1)?.get();
                  value
                }),
              );
              migrated_value
            });
            let curr_x = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                ({
                  let Ok(value) = coords.get(action_idx + 2)?.get();
                  value
                }),
              );
              migrated_value
            });
            let curr_y = ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                ({
                  let Ok(value) = coords.get(action_idx + 3)?.get();
                  value
                }),
              );
              migrated_value
            });
            let x_offset = c.scale_x(mark_x) - c.scale_x(curr_x);
            let y_offset = c.scale_y(mark_y) - c.scale_y(curr_y);
            let pos = c.buffer.cur_pos_mut();
            pos.x_offset = x_offset;
            pos.y_offset = y_offset;
          }
          _ => {}
        }

        c.buffer.cur_pos_mut().set_attach_type(attach_type::MARK);
        let idx = c.buffer.idx;
        let mut attach_chain = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(self.mark);
          migrated_value
        }) - ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(idx);
          migrated_value
        });
        match c.buffer_is_reversed {
          true => {
            attach_chain = -attach_chain;
          }
          false => {}
        }
        c.buffer.cur_pos_mut().set_attach_chain(attach_chain);
        c.buffer.scratch_flags |= HB_BUFFER_SCRATCH_FLAG_HAS_GPOS_ATTACHMENT;
      }
      false => {}
    }

    match entry.has_mark() {
      true => {
        self.mark_set = true;
        self.mark = c.buffer.idx;
      }
      false => {}
    }

    Some(())
  }
}

pub(crate) struct KerxSubtableCache {
  start_end_safe_to_break: u64,
  first_set: U32Set,
  second_set: U32Set,
  class_cache: Box<ClassCache>,
}

impl KerxSubtableCache {
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
          start_end_safe_to_break = collect_start_end_safe_to_break(&format1.state_table);
          collect_initial_glyphs(&format1.state_table, &mut first_set, num_glyphs);
        }
        SubtableKind::Format2(format2) => {
          format2.collect_glyphs(&mut first_set, &mut second_set, num_glyphs);
        }
        SubtableKind::Format4(format4) => {
          start_end_safe_to_break = collect_start_end_safe_to_break(&format4.state_table);
          collect_initial_glyphs(&format4.state_table, &mut first_set, num_glyphs);
        }
        SubtableKind::Format6(format6) => {
          format6.collect_glyphs(&mut first_set, &mut second_set, num_glyphs);
        }
      },
      Err(_) => {}
    }
    KerxSubtableCache {
      start_end_safe_to_break,
      first_set,
      second_set,
      class_cache: Box::new(ClassCache::new()),
    }
  }
}
