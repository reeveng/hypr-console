use super::layout::*;
use super::map::{AatMap, AatMapBuilder, RangeFlags};
use crate::U32Set;
use crate::hb::aat::layout_common::{
  AatApplyContext, ClassCache, START_OF_TEXT, TypedCollectGlyphs, get_class,
};
use crate::hb::ot_layout::MAX_CONTEXT_LENGTH;
use crate::hb::{GlyphInfo, hb_font_t};
use alloc::vec;
use read_fonts::tables::aat;
use read_fonts::tables::aat::{ExtendedStateTable, NoPayload, StateEntry};
use read_fonts::tables::morx::{
  ContextualEntryData, ContextualSubtable, InsertionEntryData, LigatureSubtable, Subtable,
  SubtableKind,
};
use read_fonts::types::{BigEndian, FixedSize, GlyphId16};

pub fn compile_flags(face: &hb_font_t, builder: &AatMapBuilder, map: &mut AatMap) -> Option<()> {
  let has_feature = |kind: u16, setting: u16| {
    builder
      .current_features
      .binary_search_by(|probe| match probe.kind != kind {
        true => probe.kind.cmp(&kind),
        false => probe.setting.cmp(&setting),
      })
      .is_ok()
  };

  let Ok(chains) = face.aat_tables.morx.as_ref()?.0.chains();
  let chain_len = ({
    let Ok(value) = chains.iter();
    value
  })
  .count();
  map.chain_flags.resize(chain_len, vec![]);

  for (chain, chain_flags) in ({
    let Ok(value) = chains.iter();
    value
  })
  .zip(map.chain_flags.iter_mut())
  {
    let chain = match chain {
      Ok(chain) => chain,
      Err(_) => {
        continue;
      }
    };
    let Ok(mut flags) = chain.default_flags();
    let Ok(features) = chain.features();
    for feature in features {
      let Ok(feature_type) = feature.feature_type();
      let Ok(feature_settings) = feature.feature_settings();

      match has_feature(feature_type, feature_settings) {
        true => {
          flags &= ({
            let Ok(value) = feature.disable_flags();
            value
          });
          flags |= ({
            let Ok(value) = feature.enable_flags();
            value
          });
        }
        false => {
          let Ok(feature_feature_type) = feature.feature_type();

          match feature_feature_type
            == ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u16>(FEATURE_TYPE_LETTER_CASE);
              migrated_value
            })
            && ({
              let Ok(value) = feature.feature_settings();
              value
            }) == u16::from(FEATURE_SELECTOR_SMALL_CAPS)
          {
            true => {
              let ok = has_feature(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u16>(FEATURE_TYPE_LOWER_CASE);
                  migrated_value
                }),
                u16::from(FEATURE_SELECTOR_LOWER_CASE_SMALL_CAPS),
              );
              match ok {
                true => {
                  flags &= ({
                    let Ok(value) = feature.disable_flags();
                    value
                  });
                  flags |= ({
                    let Ok(value) = feature.enable_flags();
                    value
                  });
                }
                false => {}
              }
            }
            false => {}
          }
        }
      }
    }

    chain_flags.push(RangeFlags {
      flags,
      cluster_first: ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(builder.range_first);
        migrated_value
      }),
      cluster_last: ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(builder.range_last);
        migrated_value
      }),
    });
  }

  Some(())
}

pub fn apply<'a>(c: &mut AatApplyContext<'a>, map: &'a AatMap) -> Option<()> {
  c.buffer.unsafe_to_concat(None, None);

  c.setup_buffer_glyph_set();

  let (morx, subtable_caches) = c.face.aat_tables.morx.as_ref()?;

  let Ok(chains) = morx.chains();

  let mut subtable_idx = 0;

  'outer: for (chain, chain_flags) in ({
    let Ok(value) = chains.iter();
    value
  })
  .zip(map.chain_flags.iter())
  {
    let chain = match chain {
      Ok(chain) => chain,
      Err(_) => {
        continue;
      }
    };
    c.range_flags = Some(chain_flags.as_slice());
    for subtable in ({
      let items = ({
        let Ok(array) = chain.subtables();
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
          break 'outer;
        }
      };
      subtable_idx += 1;

      match c.range_flags.as_ref() {
        Some(range_flags) => {
          match range_flags.len() == 1
            && (({
              let Ok(value) = subtable.sub_feature_flags();
              value
            }) & range_flags[0].flags
              == 0)
          {
            true => {
              continue;
            }
            false => {}
          }
        }
        None => {}
      }

      match !subtable.is_all_directions()
        && c.buffer.direction.is_vertical() != subtable.is_vertical()
      {
        true => {
          continue;
        }
        false => {}
      }

      let Ok(sub_feature_flags) = subtable.sub_feature_flags();
      c.subtable_flags = sub_feature_flags;
      c.first_set = Some(&subtable_cache.glyph_set);
      c.machine_class_cache = Some(&subtable_cache.class_cache);
      c.start_end_safe_to_break = subtable_cache.start_end_safe_to_break;

      match !c.buffer_intersects_machine() {
        true => {
          continue;
        }
        false => {}
      }

      let reverse = match subtable.is_logical() {
        true => subtable.is_backwards(),
        false => subtable.is_backwards() != c.buffer.direction.is_backward(),
      };

      match reverse != c.buffer_is_reversed {
        true => {
          c.reverse_buffer();
        }
        false => {}
      }

      match subtable.kind() {
        Ok(kind) => {
          apply_subtable(kind, c);
        }
        Err(_) => {}
      }
    }
    match c.buffer_is_reversed {
      true => {
        c.reverse_buffer();
      }
      false => {}
    }
  }

  Some(())
}

fn collect_initial_glyphs<T, Ctx: DriverContext<T>>(
  machine: &ExtendedStateTable<T>,
  glyphs: &mut U32Set,
  num_glyphs: u32,
) where
  T: FixedSize + bytemuck::AnyBitPattern,
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
          && !Ctx::is_action_initiable(&entry)
          && !Ctx::is_actionable(&entry)
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

fn collect_start_end_safe_to_break<T, Ctx: DriverContext<T>>(
  machine: &ExtendedStateTable<T>,
) -> u64
where
  T: FixedSize + bytemuck::AnyBitPattern,
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
      Ok(entry) => !Ctx::is_actionable(&entry),
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

pub(crate) trait DriverContext<T> {
  fn in_place() -> bool;
  fn can_advance(entry: &StateEntry<T>) -> bool;
  fn is_action_initiable(entry: &StateEntry<T>) -> bool;
  fn is_actionable(entry: &StateEntry<T>) -> bool;
  fn transition(&mut self, entry: &StateEntry<T>, ac: &mut AatApplyContext) -> Option<()>;
}

fn drive<T: bytemuck::AnyBitPattern + FixedSize + core::fmt::Debug, Ctx: DriverContext<T>>(
  machine: &ExtendedStateTable<'_, T>,
  c: &mut Ctx,
  ac: &mut AatApplyContext,
) {
  match !Ctx::in_place() {
    true => {
      ac.buffer.clear_output();
    }
    false => {}
  }

  let mut state = START_OF_TEXT;
  let mut last_range = ac.range_flags.as_ref().and_then(|rf| match rf.len() > 1 {
    true => rf.first().map(|_| 0usize),
    false => None,
  });
  ac.buffer.idx = 0;
  loop {
    match ac.range_flags.as_ref() {
      Some(range_flags) => match last_range.as_mut() {
        Some(last_range) => {
          let mut range = *last_range;
          match ac.buffer.idx < ac.buffer.len {
            true => {
              let cluster = ac.buffer.cur(0).cluster;
              while cluster < range_flags[range].cluster_first {
                range -= 1;
              }

              while cluster > range_flags[range].cluster_last {
                range += 1;
              }

              *last_range = range;
            }
            false => {}
          }

          match range_flags[range].flags & ac.subtable_flags == 0 {
            true => {
              match ac.buffer.idx == ac.buffer.len || !ac.buffer.successful {
                true => {
                  break;
                }
                false => {}
              }

              state = START_OF_TEXT;

              ac.buffer.next_glyph();
              continue;
            }
            false => {}
          }
        }
        None => {}
      },
      None => {}
    }

    let class = match ac.buffer.idx < ac.buffer.len {
      true => get_class(
        machine,
        ac.buffer.cur(0).as_glyph(),
        ac.machine_class_cache.unwrap(),
      ),
      false => u16::from(aat::class::END_OF_TEXT),
    };

    let entry = match machine.entry(state, class) {
      Ok(entry) => entry,
      Err(_) => {
        break;
      }
    };

    let next_state = entry.new_state;

    let is_safe_to_break = !Ctx::is_actionable(&entry)
      && (state == START_OF_TEXT
        || (!Ctx::can_advance(&entry) && next_state == START_OF_TEXT)
        || {
          match machine.entry(START_OF_TEXT, class) {
            Ok(wouldbe_entry) => {
              !Ctx::is_actionable(&wouldbe_entry)
                && (next_state == wouldbe_entry.new_state
                  && Ctx::can_advance(&entry) == Ctx::can_advance(&wouldbe_entry))
            }
            Err(_) => false,
          }
        })
      && (match state < 64 {
        true => (ac.start_end_safe_to_break & (1 << state)) != 0,
        false => match machine.entry(state, u16::from(aat::class::END_OF_TEXT)) {
          Ok(end_entry) => !Ctx::is_actionable(&end_entry),
          Err(_) => false,
        },
      });

    match !is_safe_to_break && ac.buffer.backtrack_len() > 0 && ac.buffer.idx < ac.buffer.len {
      true => {
        ac.buffer.unsafe_to_break_from_outbuffer(
          Some(ac.buffer.backtrack_len() - 1),
          Some(ac.buffer.idx + 1),
        );
      }
      false => {}
    }

    c.transition(&entry, ac);

    state = next_state;

    match ac.buffer.idx >= ac.buffer.len || !ac.buffer.successful {
      true => {
        break;
      }
      false => {}
    }

    match Ctx::can_advance(&entry) {
      true => {
        ac.buffer.next_glyph();
      }
      false => {
        match ac.buffer.max_ops <= 0 {
          true => {
            ac.buffer.next_glyph();
          }
          false => {}
        }
        ac.buffer.max_ops -= 1;
      }
    }
  }

  match !Ctx::in_place() {
    true => {
      ac.buffer.sync();
    }
    false => {}
  }
}

fn apply_subtable<'a>(kind: SubtableKind<'a>, ac: &mut AatApplyContext<'a>) {
  match kind {
    SubtableKind::Rearrangement(table) => {
      let mut c = RearrangementCtx { start: 0, end: 0 };
      drive(&table, &mut c, ac);
    }
    SubtableKind::Contextual(table) => {
      let mut c = ContextualCtx {
        mark_set: false,
        mark: 0,
        table: table.clone(),
      };
      drive(&table.state_table, &mut c, ac);
    }
    SubtableKind::Ligature(table) => {
      let mut c = LigatureCtx {
        table: table.clone(),
        match_length: 0,
        match_positions: [0; LIGATURE_MAX_MATCHES],
      };
      drive(&table.state_table, &mut c, ac);
    }
    SubtableKind::NonContextual(ref lookup) => {
      let mut last_range = ac.range_flags.as_ref().and_then(|rf| match rf.len() > 1 {
        true => rf.first().map(|_| 0usize),
        false => None,
      });

      for i in 0..ac.buffer.len {
        match ac.range_flags.as_ref() {
          Some(range_flags) => match last_range.as_mut() {
            Some(last_range) => {
              let mut range = *last_range;
              match ac.buffer.idx < ac.buffer.len {
                true => {
                  let cluster = ac.buffer.cur(0).cluster;
                  while cluster < range_flags[range].cluster_first {
                    range -= 1;
                  }

                  while cluster > range_flags[range].cluster_last {
                    range += 1;
                  }

                  *last_range = range;
                }
                false => {}
              }

              match range_flags[range].flags & ac.subtable_flags == 0 {
                true => {
                  continue;
                }
                false => {}
              }
            }
            None => {}
          },
          None => {}
        }

        match ac.buffer.info[i].as_gid16() {
          Some(glyph) => {
            match lookup.value({
              let Ok(value) = glyph.to_u16();
              value
            }) {
              Ok(replacement) => {
                ac.replace_glyph_inplace(i, replacement.into());
              }
              Err(_) => {}
            }
          }
          None => {}
        }
      }
    }
    SubtableKind::Insertion(table) => {
      let mut c = InsertionCtx {
        mark: 0,
        glyphs: table.glyphs,
      };
      drive(&table.state_table, &mut c, ac);
    }
  }
}

struct RearrangementCtx {
  start: usize,
  end: usize,
}

impl RearrangementCtx {
  const MARK_FIRST: u16 = 0x8000;
  const DONT_ADVANCE: u16 = 0x4000;
  const MARK_LAST: u16 = 0x2000;
  const VERB: u16 = 0x000F;
}

impl DriverContext<NoPayload> for RearrangementCtx {
  fn in_place() -> bool {
    true
  }

  fn can_advance(entry: &StateEntry) -> bool {
    entry.flags & Self::DONT_ADVANCE == 0
  }

  fn is_action_initiable(entry: &StateEntry) -> bool {
    entry.flags & Self::MARK_FIRST != 0
  }

  fn is_actionable(entry: &StateEntry) -> bool {
    entry.flags & Self::VERB != 0
  }

  #[inline(always)]
  fn transition(&mut self, entry: &StateEntry, ac: &mut AatApplyContext) -> Option<()> {
    let buffer = &mut *ac.buffer;
    let flags = entry.flags;

    match flags & Self::MARK_FIRST != 0 {
      true => {
        self.start = buffer.idx;
      }
      false => {}
    }

    match flags & Self::MARK_LAST != 0 {
      true => {
        self.end = (buffer.idx + 1).min(buffer.len);
      }
      false => {}
    }

    match flags & Self::VERB != 0 && self.start < self.end {
      true => {
        static MAP: [u8; 16] = [
          0x00, 0x10, 0x01, 0x11, 0x20, 0x30, 0x02, 0x03, 0x12, 0x13, 0x21, 0x31, 0x22, 0x32,
          0x23, 0x33,
        ];

        let m = MAP[usize::from(flags & Self::VERB)];
        let l = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(2.min(m >> 4));
          migrated_value
        });
        let r = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(2.min(m & 0x0F));
          migrated_value
        });
        let reverse_l = 3 == (m >> 4);
        let reverse_r = 3 == (m & 0x0F);

        match (self.end - self.start >= l + r) && (self.end - self.start <= MAX_CONTEXT_LENGTH) {
          true => {
            buffer.merge_clusters(self.start, (buffer.idx + 1).min(buffer.len));
            buffer.merge_clusters(self.start, self.end);

            let mut buf = [GlyphInfo::default(); 4];

            for (i, glyph_info) in buf[..l].iter_mut().enumerate() {
              *glyph_info = buffer.info[self.start + i];
            }

            for i in 0..r {
              buf[i + 2] = buffer.info[self.end - r + i];
            }

            match l > r {
              true => {
                for i in 0..(self.end - self.start - l - r) {
                  buffer.info[self.start + r + i] = buffer.info[self.start + l + i];
                }
              }
              false => match l < r {
                true => {
                  for i in (0..(self.end - self.start - l - r)).rev() {
                    buffer.info[self.start + r + i] = buffer.info[self.start + l + i];
                  }
                }
                false => {}
              },
            }

            for i in 0..r {
              buffer.info[self.start + i] = buf[2 + i];
            }

            for i in 0..l {
              buffer.info[self.end - l + i] = buf[i];
            }

            match reverse_l {
              true => {
                buffer.info.swap(self.end - 1, self.end - 2);
              }
              false => {}
            }

            match reverse_r {
              true => {
                buffer.info.swap(self.start, self.start + 1);
              }
              false => {}
            }
          }
          false => {}
        }
      }
      false => {}
    }

    Some(())
  }
}

struct ContextualCtx<'a> {
  mark_set: bool,
  mark: usize,
  table: ContextualSubtable<'a>,
}

impl ContextualCtx<'_> {
  const SET_MARK: u16 = 0x8000;
  const DONT_ADVANCE: u16 = 0x4000;
}

impl DriverContext<ContextualEntryData> for ContextualCtx<'_> {
  fn in_place() -> bool {
    true
  }

  fn can_advance(entry: &StateEntry<ContextualEntryData>) -> bool {
    entry.flags & Self::DONT_ADVANCE == 0
  }

  fn is_action_initiable(entry: &StateEntry<ContextualEntryData>) -> bool {
    entry.flags & Self::SET_MARK != 0
  }

  fn is_actionable(entry: &StateEntry<ContextualEntryData>) -> bool {
    ({
      let Ok(value) = entry.payload.mark_index.get();
      value
    }) != 0xFFFF
      || ({
        let Ok(value) = entry.payload.current_index.get();
        value
      }) != 0xFFFF
  }

  #[inline(always)]
  fn transition(
    &mut self,
    entry: &StateEntry<ContextualEntryData>,
    ac: &mut AatApplyContext,
  ) -> Option<()> {
    match ac.buffer.idx == ac.buffer.len && !self.mark_set {
      true => {
        return Some(());
      }
      false => {}
    }

    let mut replacement = None;

    if ({
      let Ok(value) = entry.payload.mark_index.get();
      value
    }) != 0xFFFF
    {
      let lookup = self
        .table
        .lookups
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(usize::from(
              ({
                let Ok(value) = entry.payload.mark_index.get();
                value
              }),
            ));
            value
          }),
        )
        .ok()?;
      match ac.buffer.info[self.mark].as_gid16() {
        Some(gid) => {
          replacement = lookup
            .value({
              let Ok(value) = gid.to_u16();
              value
            })
            .ok();
        }
        None => {}
      }
    }

    match replacement {
      Some(replacement) => {
        ac.buffer.unsafe_to_break(
          Some(self.mark),
          Some((ac.buffer.idx + 1).min(ac.buffer.len)),
        );
        ac.replace_glyph_inplace(self.mark, replacement.into());
      }
      None => {}
    }

    replacement = None;
    let idx = ac.buffer.idx.min(ac.buffer.len - 1);
    if ({
      let Ok(value) = entry.payload.current_index.get();
      value
    }) != 0xFFFF
    {
      let lookup = self
        .table
        .lookups
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(usize::from(
              ({
                let Ok(value) = entry.payload.current_index.get();
                value
              }),
            ));
            value
          }),
        )
        .ok()?;
      match ac.buffer.info[idx].as_gid16() {
        Some(gid) => {
          replacement = lookup
            .value({
              let Ok(value) = gid.to_u16();
              value
            })
            .ok();
        }
        None => {}
      }
    }

    match replacement {
      Some(replacement) => {
        ac.replace_glyph_inplace(idx, replacement.into());
      }
      None => {}
    }

    match entry.flags & Self::SET_MARK != 0 {
      true => {
        self.mark_set = true;
        self.mark = ac.buffer.idx;
      }
      false => {}
    }

    Some(())
  }
}

struct InsertionCtx<'a> {
  mark: u32,
  glyphs: &'a [BigEndian<GlyphId16>],
}

impl InsertionCtx<'_> {
  const SET_MARK: u16 = 0x8000;
  const DONT_ADVANCE: u16 = 0x4000;
  const CURRENT_INSERT_BEFORE: u16 = 0x0800;
  const MARKED_INSERT_BEFORE: u16 = 0x0400;
  const CURRENT_INSERT_COUNT: u16 = 0x03E0;
  const MARKED_INSERT_COUNT: u16 = 0x001F;
}

impl DriverContext<InsertionEntryData> for InsertionCtx<'_> {
  fn in_place() -> bool {
    false
  }

  fn can_advance(entry: &StateEntry<InsertionEntryData>) -> bool {
    entry.flags & Self::DONT_ADVANCE == 0
  }

  fn is_action_initiable(entry: &StateEntry<InsertionEntryData>) -> bool {
    entry.flags & Self::SET_MARK != 0
  }

  fn is_actionable(entry: &StateEntry<InsertionEntryData>) -> bool {
    (entry.flags & (Self::CURRENT_INSERT_COUNT | Self::MARKED_INSERT_COUNT) != 0)
      && (({
        let Ok(value) = entry.payload.current_insert_index.get();
        value
      }) != 0xFFFF
        || ({
          let Ok(value) = entry.payload.marked_insert_index.get();
          value
        }) != 0xFFFF)
  }

  #[inline(always)]
  fn transition(
    &mut self,
    entry: &StateEntry<InsertionEntryData>,
    ac: &mut AatApplyContext,
  ) -> Option<()> {
    let flags = entry.flags;
    let mark_loc = ac.buffer.out_len;

    if ({
      let Ok(value) = entry.payload.marked_insert_index.get();
      value
    }) != 0xFFFF
    {
      let count = flags & Self::MARKED_INSERT_COUNT;
      ac.buffer.max_ops -= i32::from(count);
      match ac.buffer.max_ops <= 0 {
        true => {
          return Some(());
        }
        false => {}
      }

      let start = ({
        let Ok(value) = entry.payload.marked_insert_index.get();
        value
      });
      let before = flags & Self::MARKED_INSERT_BEFORE != 0;

      let end = ac.buffer.out_len;
      match !ac.buffer.move_to(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.mark);
          migrated_value
        }),
      ) {
        true => {
          return Some(());
        }
        false => {}
      }

      match ac.buffer.idx < ac.buffer.len && !before {
        true => {
          ac.buffer.copy_glyph();
        }
        false => {}
      }

      for i in 0..count {
        let i = usize::from((start) + i);
        ac.output_glyph(u32::from(
          ({
            let Ok(value) = {
              let Ok(value) = self.glyphs.get(i)?.get();
              value
            }
            .to_u16();
            value
          }),
        ));
      }

      match ac.buffer.idx < ac.buffer.len && !before {
        true => {
          ac.buffer.skip_glyph();
        }
        false => {}
      }

      match !ac.buffer.move_to(end + usize::from(count)) {
        true => {
          return Some(());
        }
        false => {}
      }

      ac.buffer.unsafe_to_break_from_outbuffer(
        Some(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.mark);
            migrated_value
          }),
        ),
        Some((ac.buffer.idx + 1).min(ac.buffer.len)),
      );
    }

    match flags & Self::SET_MARK != 0 {
      true => {
        self.mark = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(mark_loc);
          migrated_value
        });
      }
      false => {}
    }

    if ({
      let Ok(value) = entry.payload.current_insert_index.get();
      value
    }) != 0xFFFF
    {
      let count = (flags & Self::CURRENT_INSERT_COUNT) >> 5;
      ac.buffer.max_ops -= i32::from(count);
      match ac.buffer.max_ops < 0 {
        true => {
          return Some(());
        }
        false => {}
      }

      let start = ({
        let Ok(value) = entry.payload.current_insert_index.get();
        value
      });
      let before = flags & Self::CURRENT_INSERT_BEFORE != 0;
      let end = ac.buffer.out_len;

      match ac.buffer.idx < ac.buffer.len && !before {
        true => {
          ac.buffer.copy_glyph();
        }
        false => {}
      }

      for i in 0..count {
        let i = usize::from((start) + i);
        ac.output_glyph(u32::from(
          ({
            let Ok(value) = {
              let Ok(value) = self.glyphs.get(i)?.get();
              value
            }
            .to_u16();
            value
          }),
        ));
      }

      match ac.buffer.idx < ac.buffer.len && !before {
        true => {
          ac.buffer.skip_glyph();
        }
        false => {}
      }

      match !ac.buffer.move_to(match flags & Self::DONT_ADVANCE != 0 {
        true => end,
        false => end + usize::from(count),
      }) {
        true => {
          return Some(());
        }
        false => {}
      }
    }

    Some(())
  }
}

const LIGATURE_MAX_MATCHES: usize = 64;

struct LigatureCtx<'a> {
  table: LigatureSubtable<'a>,
  match_length: usize,
  match_positions: [usize; LIGATURE_MAX_MATCHES],
}

impl LigatureCtx<'_> {
  const SET_COMPONENT: u16 = 0x8000;
  const DONT_ADVANCE: u16 = 0x4000;
  const PERFORM_ACTION: u16 = 0x2000;

  const LIG_ACTION_LAST: u32 = 0x8000_0000;
  const LIG_ACTION_STORE: u32 = 0x4000_0000;
  const LIG_ACTION_OFFSET: u32 = 0x3FFF_FFFF;
}

impl DriverContext<BigEndian<u16>> for LigatureCtx<'_> {
  fn in_place() -> bool {
    false
  }

  fn can_advance(entry: &StateEntry<BigEndian<u16>>) -> bool {
    entry.flags & Self::DONT_ADVANCE == 0
  }

  fn is_action_initiable(entry: &StateEntry<BigEndian<u16>>) -> bool {
    entry.flags & Self::SET_COMPONENT != 0
  }

  fn is_actionable(entry: &StateEntry<BigEndian<u16>>) -> bool {
    entry.flags & Self::PERFORM_ACTION != 0
  }

  #[inline(always)]
  fn transition(
    &mut self,
    entry: &StateEntry<BigEndian<u16>>,
    ac: &mut AatApplyContext,
  ) -> Option<()> {
    match entry.flags & Self::SET_COMPONENT != 0 {
      true => {
        match self.match_length != 0
          && self.match_positions[(self.match_length - 1) % LIGATURE_MAX_MATCHES]
            == ac.buffer.out_len
        {
          true => {
            self.match_length -= 1;
          }
          false => {}
        }

        self.match_positions[self.match_length % LIGATURE_MAX_MATCHES] = ac.buffer.out_len;
        self.match_length += 1;
      }
      false => {}
    }

    match entry.flags & Self::PERFORM_ACTION != 0 {
      true => {
        let end = ac.buffer.out_len;

        match self.match_length == 0 {
          true => {
            return Some(());
          }
          false => {}
        }

        match ac.buffer.idx >= ac.buffer.len {
          true => {
            return Some(());
          }
          false => {}
        }

        let mut cursor = self.match_length;

        let mut ligature_actions_index = ({
          let Ok(value) = entry.payload.get();
          value
        });
        let mut ligature_idx = 0;
        loop {
          match cursor == 0 {
            true => {
              self.match_length = 0;
              break;
            }
            false => {}
          }

          cursor -= 1;
          match !ac
            .buffer
            .move_to(self.match_positions[cursor % LIGATURE_MAX_MATCHES])
          {
            true => {
              return Some(());
            }
            false => {}
          }

          let action = match self
            .table
            .ligature_actions
            .get(usize::from(ligature_actions_index))
          {
            Some(v) => {
              ({
                let Ok(value) = v.get();
                value
              })
            }
            None => break,
          };

          let mut uoffset = (action) & Self::LIG_ACTION_OFFSET;
          match uoffset & 0x2000_0000 != 0 {
            true => {
              uoffset |= 0xC000_0000;
            }
            false => {}
          }

          let offset = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(uoffset);
            migrated_value
          });
          let component_idx = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
              (({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(ac.buffer.cur(0).glyph_id);
                migrated_value
              }) + offset),
            );
            migrated_value
          });
          ligature_idx += match self.table.components.get(component_idx) {
            Some(v) => {
              ({
                let Ok(value) = v.get();
                value
              })
            }
            None => break,
          };

          match ((action) & (Self::LIG_ACTION_STORE | Self::LIG_ACTION_LAST)) != 0 {
            true => {
              let lig = match self.table.ligatures.get(usize::from(ligature_idx)) {
                Some(v) => {
                  ({
                    let Ok(value) = v.get();
                    value
                  })
                }
                None => break,
              };

              ac.replace_glyph(u32::from(
                ({
                  let Ok(value) = (lig).to_u16();
                  value
                }),
              ));

              let lig_end =
                self.match_positions[(self.match_length - 1) % LIGATURE_MAX_MATCHES] + 1;

              while self.match_length - 1 > cursor {
                self.match_length -= 1;
                match !ac
                  .buffer
                  .move_to(self.match_positions[self.match_length % LIGATURE_MAX_MATCHES])
                {
                  true => {
                    return Some(());
                  }
                  false => {}
                }
                ac.delete_glyph();
              }

              match !ac.buffer.move_to(lig_end) {
                true => {
                  return Some(());
                }
                false => {}
              }
              ac.buffer.merge_out_clusters(
                self.match_positions[cursor % LIGATURE_MAX_MATCHES],
                ac.buffer.out_len,
              );
            }
            false => {}
          }

          ligature_actions_index += 1;

          match (action) & Self::LIG_ACTION_LAST != 0 {
            true => {
              break;
            }
            false => {}
          }
        }

        match !ac.buffer.move_to(end) {
          true => {
            return Some(());
          }
          false => {}
        }
      }
      false => {}
    }

    Some(())
  }
}

pub(crate) struct MorxSubtableCache {
  start_end_safe_to_break: u64,
  glyph_set: U32Set,
  class_cache: ClassCache,
}

impl MorxSubtableCache {
  pub(crate) fn new(subtable: &Subtable, num_glyphs: u32) -> Self {
    let mut start_end_safe_to_break = 0u64;
    let mut glyph_set = U32Set::default();
    match subtable.kind() {
      Ok(kind) => match &kind {
        SubtableKind::Rearrangement(table) => {
          start_end_safe_to_break = collect_start_end_safe_to_break::<_, RearrangementCtx>(table);
          collect_initial_glyphs::<_, RearrangementCtx>(table, &mut glyph_set, num_glyphs);
        }
        SubtableKind::Contextual(table) => {
          start_end_safe_to_break =
            collect_start_end_safe_to_break::<_, ContextualCtx>(&table.state_table);
          collect_initial_glyphs::<_, ContextualCtx>(
            &table.state_table,
            &mut glyph_set,
            num_glyphs,
          );
        }
        SubtableKind::Ligature(table) => {
          start_end_safe_to_break =
            collect_start_end_safe_to_break::<_, LigatureCtx>(&table.state_table);
          collect_initial_glyphs::<_, LigatureCtx>(
            &table.state_table,
            &mut glyph_set,
            num_glyphs,
          );
        }
        SubtableKind::NonContextual(ref lookup) => {
          lookup.collect_glyphs(&mut glyph_set, num_glyphs);
        }
        SubtableKind::Insertion(table) => {
          start_end_safe_to_break =
            collect_start_end_safe_to_break::<_, InsertionCtx>(&table.state_table);
          collect_initial_glyphs::<_, InsertionCtx>(
            &table.state_table,
            &mut glyph_set,
            num_glyphs,
          );
        }
      },
      Err(_) => {}
    }
    MorxSubtableCache {
      start_end_safe_to_break,
      glyph_set,
      class_cache: ClassCache::new(),
    }
  }
}
