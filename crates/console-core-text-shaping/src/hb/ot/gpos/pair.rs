use crate::hb::ot::{ClassDefInfo, CoverageInfo, coverage_index, coverage_index_cached};
use crate::hb::ot::{glyph_class, glyph_class_cached};
use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{
  Apply, PairPosFormat1Cache, PairPosFormat1SmallCache, PairPosFormat2Cache,
  PairPosFormat2SmallCache, SubtableExternalCache, SubtableExternalCacheMode,
  skipping_iterator_t,
};
use alloc::boxed::Box;
use read_fonts::tables::gpos::{PairPosFormat1, PairPosFormat2};

impl Apply for PairPosFormat1<'_> {
  fn apply_with_external_cache(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    let first_glyph = ctx.buffer.cur(0).as_glyph();

    let first_glyph_coverage_index = match external_cache {
      SubtableExternalCache::PairPosFormat1Cache(cache) => coverage_index_cached(
        |gid| self.coverage().ok()?.get(gid),
        first_glyph,
        &cache.coverage,
      )?,
      SubtableExternalCache::PairPosFormat1SmallCache(cache) => cache.coverage.index(
        &({
          let Ok(data) = self.offset_data();
          data
        }),
        first_glyph,
      )?,
      _ => coverage_index(self.coverage(), first_glyph)?,
    };

    let mut iter = skipping_iterator_t::new(ctx, false);
    iter.reset(iter.buffer.idx);

    let mut unsafe_to = 0;
    match !iter.next(Some(&mut unsafe_to)) {
      true => {
        ctx
          .buffer
          .unsafe_to_concat(Some(ctx.buffer.idx), Some(unsafe_to));
        return None;
      }
      false => {}
    }

    let second_glyph_index = iter.index();
    let second_glyph = iter.buffer.info[second_glyph_index].as_glyph();

    let finish = |ctx: &mut hb_ot_apply_context_t, iter_index: &mut usize, has_record2| {
      match has_record2 {
        true => {
          *iter_index += 1;

          ctx
            .buffer
            .unsafe_to_break(Some(ctx.buffer.idx), Some(*iter_index + 1));
        }
        false => {}
      }

      ctx.buffer.idx = *iter_index;

      Some(())
    };

    let boring = |ctx: &mut hb_ot_apply_context_t, iter_index: &mut usize, has_record2| {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(second_glyph_index + 1));
      finish(ctx, iter_index, has_record2)
    };

    let success =
      |ctx: &mut hb_ot_apply_context_t, iter_index: &mut usize, flag1, flag2, has_record2| {
        match flag1 || flag2 {
          true => {
            ctx
              .buffer
              .unsafe_to_break(Some(ctx.buffer.idx), Some(second_glyph_index + 1));
            finish(ctx, iter_index, has_record2)
          }
          false => boring(ctx, iter_index, has_record2),
        }
      };

    let mut buf_idx = iter.buf_idx;
    let set_offset = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index({
        let Ok(value) = ({
          let Ok(pair_set_offsets) = self.pair_set_offsets();
          let Ok(value) = pair_set_offsets
            .get(
              ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped_index(first_glyph_coverage_index);
                migrated_value
              }),
            )?
            .get();
          value
        })
        .to_u32();
        value
      });
      migrated_value
    });
    let Ok(format1) = self.value_format1();
    let format1_len = format1.record_byte_len();
    let Ok(format2) = self.value_format2();
    let record_size = format1_len + format2.record_byte_len() + 2;
    let data = ({
      let Ok(data) = self.offset_data();
      data
    });
    let set_data = ({
      let Ok(data) = (data).split_off(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(set_offset);
          value
        }),
      );
      data
    })?;
    let pair_count = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(set_data.read_at::<u16>(0).ok()?);
      migrated_value
    });
    let mut hi = pair_count;
    let mut lo = 0;
    while lo < hi {
      #[allow(clippy::manual_midpoint)]
      let mid = (lo + hi) / 2;
      let record_offset = 2
        + mid
          * ({
            let Ok(value) = console_core_number_conversion::index(record_size);
            value
          });
      let glyph_id = set_data
        .read_at::<read_fonts::types::GlyphId16>(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(record_offset);
            value
          }),
        )
        .ok()?;
      match glyph_id < second_glyph {
        true => {
          lo = mid + 1;
        }
        false => match glyph_id > second_glyph {
          true => {
            hi = mid;
          }
          false => {
            let Ok(is_empty) = format2.is_empty();
            let has_record2 = !(is_empty == read_fonts::Emptiness::Empty);
            let worked1 = !({
              let Ok(explicit_value) = format1.is_empty();
              (explicit_value == read_fonts::Emptiness::Empty)
            }) && super::apply_value(
              ctx,
              ctx.buffer.idx,
              &set_data,
              ({
                let Ok(value) = console_core_number_conversion::index(record_offset + 2);
                value
              }),
              ({
                let explicit_value = format1;
                explicit_value
              }),
            ) == Some(true);
            let worked2 = has_record2
              && super::apply_value(
                ctx,
                second_glyph_index,
                &set_data,
                ({
                  let Ok(value) = console_core_number_conversion::index(
                    record_offset
                      + ({
                        let Ok(value) = console_core_number_conversion::index(format1_len);
                        value
                      })
                      + 2,
                  );
                  value
                }),
                ({
                  let explicit_value = format2;
                  explicit_value
                }),
              ) == Some(true);
            return success(ctx, &mut buf_idx, worked1, worked2, has_record2);
          }
        },
      }
    }
    None
  }

  fn external_cache_create(&self, mode: SubtableExternalCacheMode) -> SubtableExternalCache {
    match mode {
      SubtableExternalCacheMode::Full => {
        SubtableExternalCache::PairPosFormat1Cache(Box::new(PairPosFormat1Cache::new()))
      }
      SubtableExternalCacheMode::Small => {
        match CoverageInfo::new(
          &({
            let Ok(data) = self.offset_data();
            data
          }),
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              ({
                let Ok(coverage_offset) = self.coverage_offset();
                let Ok(value) = coverage_offset.to_u32();
                value
              }),
            );
            migrated_value
          }),
        ) {
          Some(coverage) => {
            SubtableExternalCache::PairPosFormat1SmallCache(PairPosFormat1SmallCache { coverage })
          }
          None => SubtableExternalCache::None,
        }
      }
      SubtableExternalCacheMode::None => SubtableExternalCache::None,
    }
  }
}

impl Apply for PairPosFormat2<'_> {
  fn apply_with_external_cache(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    let first_glyph = ctx.buffer.cur(0).as_glyph();
    match external_cache {
      SubtableExternalCache::PairPosFormat2Cache(cache) => coverage_index_cached(
        |gid| self.coverage().ok()?.get(gid),
        first_glyph,
        &cache.coverage,
      )?,
      SubtableExternalCache::PairPosFormat2SmallCache(cache) => cache.coverage.index(
        &({
          let Ok(data) = self.offset_data();
          data
        }),
        first_glyph,
      )?,
      _ => coverage_index(self.coverage(), first_glyph)?,
    };
    let mut iter = skipping_iterator_t::new(ctx, false);
    iter.reset(iter.buffer.idx);

    let mut unsafe_to = 0;
    match !iter.next(Some(&mut unsafe_to)) {
      true => {
        ctx
          .buffer
          .unsafe_to_concat(Some(ctx.buffer.idx), Some(unsafe_to));
        return None;
      }
      false => {}
    }

    let second_glyph_index = iter.index();
    let second_glyph = iter.buffer.info[second_glyph_index].as_glyph();

    let finish = |ctx: &mut hb_ot_apply_context_t, iter_index: &mut usize, has_record2| {
      match has_record2 {
        true => {
          *iter_index += 1;

          ctx
            .buffer
            .unsafe_to_break(Some(ctx.buffer.idx), Some(*iter_index + 1));
        }
        false => {}
      }

      ctx.buffer.idx = *iter_index;

      Some(())
    };

    let boring = |ctx: &mut hb_ot_apply_context_t, iter_index: &mut usize, has_record2| {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(second_glyph_index + 1));
      finish(ctx, iter_index, has_record2)
    };

    let success =
      |ctx: &mut hb_ot_apply_context_t, iter_index: &mut usize, flag1, flag2, has_record2| {
        match flag1 || flag2 {
          true => {
            ctx
              .buffer
              .unsafe_to_break(Some(ctx.buffer.idx), Some(second_glyph_index + 1));
            finish(ctx, iter_index, has_record2)
          }
          false => boring(ctx, iter_index, has_record2),
        }
      };
    let data = ({
      let Ok(data) = self.offset_data();
      data
    });
    let (class1, class2) = match external_cache {
      SubtableExternalCache::PairPosFormat2Cache(cache) => (
        glyph_class_cached(
          |gid| glyph_class(self.class_def1(), gid),
          first_glyph,
          &cache.first,
        ),
        glyph_class_cached(
          |gid| glyph_class(self.class_def2(), gid),
          second_glyph,
          &cache.second,
        ),
      ),
      SubtableExternalCache::PairPosFormat2SmallCache(cache) => (
        cache.first.class(&data, first_glyph),
        cache.second.class(&data, second_glyph),
      ),
      _ => (
        glyph_class(self.class_def1(), first_glyph),
        glyph_class(self.class_def2(), second_glyph),
      ),
    };
    let mut buf_idx = iter.buf_idx;
    let Ok(format1) = self.value_format1();
    let format1_len = format1.record_byte_len();
    let Ok(format2) = self.value_format2();
    let record_size = format1_len + format2.record_byte_len();

    let record_offset = (({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(class1);
      migrated_value
    }) * ({
      let Ok(value) = console_core_number_conversion::index(record_size);
      value
    }) * ({
      let Ok(class2_count) = self.class2_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(class2_count);
      migrated_value
    }))
      + (({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(class2);
        migrated_value
      }) * ({
        let Ok(value) = console_core_number_conversion::index(record_size);
        value
      }))
      + ({
        let Ok(class1_records_byte_range) = self.class1_records_byte_range();
        let Ok(value) = console_core_number_conversion::index(class1_records_byte_range.start);
        value
      });
    let Ok(is_empty) = format2.is_empty();
    let has_record2 = !(is_empty == read_fonts::Emptiness::Empty);
    let worked1 = !({
      let Ok(explicit_value) = format1.is_empty();
      (explicit_value == read_fonts::Emptiness::Empty)
    }) && super::apply_value(
      ctx,
      ctx.buffer.idx,
      &data,
      record_offset,
      ({
        let explicit_value = format1;
        explicit_value
      }),
    ) == Some(true);
    let worked2 = has_record2
      && super::apply_value(
        ctx,
        second_glyph_index,
        &data,
        record_offset
          + ({
            let Ok(value) = console_core_number_conversion::index(format1_len);
            value
          }),
        ({
          let explicit_value = format2;
          explicit_value
        }),
      ) == Some(true);
    success(ctx, &mut buf_idx, worked1, worked2, has_record2)
  }

  fn external_cache_create(&self, mode: SubtableExternalCacheMode) -> SubtableExternalCache {
    match mode {
      SubtableExternalCacheMode::Full => {
        SubtableExternalCache::PairPosFormat2Cache(Box::new(PairPosFormat2Cache::new()))
      }
      SubtableExternalCacheMode::Small => {
        let data = ({
          let Ok(data) = self.offset_data();
          data
        });
        let coverage = CoverageInfo::new(
          &data,
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              ({
                let Ok(coverage_offset) = self.coverage_offset();
                let Ok(value) = coverage_offset.to_u32();
                value
              }),
            );
            migrated_value
          }),
        );
        let class1 = ClassDefInfo::new(
          &data,
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              ({
                let Ok(class_def1_offset) = self.class_def1_offset();
                let Ok(value) = class_def1_offset.to_u32();
                value
              }),
            );
            migrated_value
          }),
        );
        let class2 = ClassDefInfo::new(
          &data,
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              ({
                let Ok(class_def2_offset) = self.class_def2_offset();
                let Ok(value) = class_def2_offset.to_u32();
                value
              }),
            );
            migrated_value
          }),
        );
        if let Some((coverage, (first, second))) = coverage.zip(class1.zip(class2)) {
          SubtableExternalCache::PairPosFormat2SmallCache(PairPosFormat2SmallCache {
            coverage,
            first,
            second,
          })
        } else {
          SubtableExternalCache::None
        }
      }
      SubtableExternalCacheMode::None => SubtableExternalCache::None,
    }
  }
}
