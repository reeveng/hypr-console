use super::{coverage_binary_cached, coverage_index, covered, glyph_class};
use crate::hb::buffer::GlyphInfo;
use crate::hb::ot::{ClassDefInfo, CoverageInfo};
use crate::hb::ot_layout_gsubgpos::OT::hb_ot_apply_context_t;
use crate::hb::ot_layout_gsubgpos::{
  Apply, BinaryCache, ChainContextFormat2Cache, ContextFormat2Cache, SubtableExternalCache,
  SubtableExternalCacheMode, WouldApply, WouldApplyContext, apply_lookup, match_always,
  match_backtrack, match_glyph, match_input, match_lookahead, may_skip_t, skipping_iterator_t,
};
use read_fonts::tables::gsub::ClassDef;
use read_fonts::tables::layout::{
  ChainedClassSequenceRule, ChainedSequenceContextFormat1, ChainedSequenceContextFormat2,
  ChainedSequenceContextFormat3, ChainedSequenceRule, ClassSequenceRule, SequenceContextFormat1,
  SequenceContextFormat2, SequenceContextFormat3, SequenceLookupRecord, SequenceRule,
};
use read_fonts::types::{BigEndian, FixedSize, GlyphId, Offset16};
use read_fonts::{ArrayOfOffsets, FontData, FontRead};

impl WouldApply for SequenceContextFormat1<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    coverage_index(self.coverage(), ctx.glyphs[0])
      .and_then(|index| {
        let Ok(seq_rule_sets) = self.seq_rule_sets();

        seq_rule_sets
          .get(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                  migrated_value
                }),
              );
              value
            }),
          )
          .ok()
          .flatten()
      })
      .is_some_and(|set| {
        ({
          let items = ({
            let Ok(array) = set.seq_rules();
            let Ok(items) = array.iter();
            items
          });
          items
        })
        .any(|rule| {
          rule.is_ok_and(|rule| {
            let Ok(input) = rule.input_sequence();
            ctx.glyphs.len() == input.len() + 1
              && input.iter().enumerate().all(|(i, value)| {
                let mut info = GlyphInfo {
                  glyph_id: ctx.glyphs[i + 1].into(),
                  ..GlyphInfo::default()
                };
                match_glyph(
                  &mut info,
                  ({
                    let Ok(value) = ({
                      let Ok(value) = value.get();
                      value
                    })
                    .to_u32();
                    value
                  }),
                )
              })
          })
        })
      })
  }
}

impl Apply for SequenceContextFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let index = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(self.coverage().ok()?.get(glyph)?);
      migrated_value
    });
    let Ok(seq_rule_sets) = self.seq_rule_sets();
    let set = seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }),
      )
      .ok()??;
    let Ok(seq_rules) = set.seq_rules();

    apply_context_rules(ctx, &seq_rules, match_glyph)
  }
}

impl WouldApply for SequenceContextFormat2<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    let class_def = self.class_def().ok();
    let match_fn = &match_class(&class_def);
    let class = glyph_class(self.class_def(), ctx.glyphs[0]);
    let Ok(class_seq_rule_sets) = self.class_seq_rule_sets();

    class_seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(class);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()
      .flatten()
      .is_some_and(|set| {
        ({
          let items = ({
            let Ok(array) = set.class_seq_rules();
            let Ok(items) = array.iter();
            items
          });
          items
        })
        .any(|rule| {
          rule.is_ok_and(|rule| {
            let Ok(input) = rule.input_sequence();
            ctx.glyphs.len() == input.len() + 1
              && input.iter().enumerate().all(|(i, value)| {
                let mut info = GlyphInfo {
                  glyph_id: ctx.glyphs[i + 1].into(),
                  ..GlyphInfo::default()
                };
                match_fn(
                  &mut info,
                  ({
                    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                      ({
                        let Ok(value) = value.get();
                        value
                      }),
                    );
                    migrated_value
                  }),
                )
              })
          })
        })
      })
  }
}

impl Apply for SequenceContextFormat2<'_> {
  fn apply_with_external_cache(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let SubtableExternalCache::ContextFormat2Cache(cache) = external_cache else {
      return None;
    };
    let offset_data = ({
      let Ok(data) = self.offset_data();
      data
    });
    coverage_binary_cached(
      |gid| cache.coverage.index(&offset_data, gid),
      glyph,
      &cache.coverage_cache,
    )?;
    let input_class = |gid| cache.input.class(&offset_data, gid);
    let index = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(input_class(glyph));
      migrated_value
    });
    let Ok(class_seq_rule_sets) = self.class_seq_rule_sets();
    let set = class_seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }),
      )
      .ok()??;
    let Ok(class_seq_rules) = set.class_seq_rules();

    apply_context_rules(ctx, &class_seq_rules, |info, value| {
      u32::from(input_class(info.as_glyph())) == value
    })
  }

  fn apply_cached(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let SubtableExternalCache::ContextFormat2Cache(cache) = external_cache else {
      return None;
    };
    let offset_data = ({
      let Ok(data) = self.offset_data();
      data
    });
    coverage_binary_cached(
      |gid| cache.coverage.index(&offset_data, gid),
      glyph,
      &cache.coverage_cache,
    )?;
    let input_class = |gid| cache.input.class(&offset_data, gid);
    let index = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(get_class_cached(
        &input_class,
        &mut ctx.buffer.info[ctx.buffer.idx],
      ));
      migrated_value
    });
    let Ok(class_seq_rule_sets) = self.class_seq_rule_sets();
    let set = class_seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }),
      )
      .ok()??;
    let Ok(class_seq_rules) = set.class_seq_rules();

    apply_context_rules(ctx, &class_seq_rules, match_class_cached(&input_class))
  }

  fn cache_cost(&self) -> u32 {
    self
      .class_def()
      .ok()
      .map_or(0, |class_def| class_def.cost())
  }

  fn external_cache_create(&self, _mode: SubtableExternalCacheMode) -> SubtableExternalCache {
    let data = ({
      let Ok(data) = self.offset_data();
      data
    });
    SubtableExternalCache::ContextFormat2Cache(ContextFormat2Cache {
      coverage_cache: BinaryCache::new(),
      coverage: CoverageInfo::new(
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
      )
      .unwrap_or_default(),
      input: ClassDefInfo::new(
        &data,
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
            ({
              let Ok(class_def_offset) = self.class_def_offset();
              let Ok(value) = class_def_offset.to_u32();
              value
            }),
          );
          migrated_value
        }),
      )
      .unwrap_or_default(),
    })
  }
}

impl WouldApply for SequenceContextFormat3<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    let Ok(coverages) = self.coverages();
    ctx.glyphs.len()
      == ({
        let Ok(value) = console_core_number_conversion::index(
          ({
            let Ok(value) = coverages.len();
            value
          }) + 1,
        );
        value
      })
      && ({
        let Ok(value) = coverages.iter();
        value
      })
      .enumerate()
      .all(|(i, coverage)| covered(coverage, ctx.glyphs[i + 1]))
  }
}

impl Apply for SequenceContextFormat3<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let Ok(input_coverages) = self.coverages();
    input_coverages.get(0).ok()?.get(glyph)?;
    let input = |info: &mut GlyphInfo, index: u32| {
      input_coverages
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                migrated_value
              }) + 1,
            );
            value
          }),
        )
        .is_ok_and(|cov| cov.get(info.glyph_id).is_some())
    };
    let mut match_end = 0;
    if match_input(
      ctx,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
          ({
            let Ok(value) = input_coverages.len();
            value
          }),
        );
        migrated_value
      }) - 1,
      input,
      &mut match_end,
      None,
    ) {
      ctx
        .buffer
        .unsafe_to_break(Some(ctx.buffer.idx), Some(match_end));
      apply_lookup(
        ctx,
        ({
          let Ok(value) = console_core_number_conversion::index(
            ({
              let Ok(value) = input_coverages.len();
              value
            }) - 1,
          );
          value
        }),
        match_end,
        ({
          let Ok(explicit_value) = self.seq_lookup_records();
          explicit_value
        }),
      );
      Some(())
    } else {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(match_end));
      None
    }
  }
}

impl WouldApply for ChainedSequenceContextFormat1<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    coverage_index(self.coverage(), ctx.glyphs[0])
      .and_then(|index| {
        let Ok(chained_seq_rule_sets) = self.chained_seq_rule_sets();

        chained_seq_rule_sets
          .get(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                  migrated_value
                }),
              );
              value
            }),
          )
          .ok()
          .flatten()
      })
      .is_some_and(|set| {
        ({
          let items = ({
            let Ok(array) = set.chained_seq_rules();
            let Ok(items) = array.iter();
            items
          });
          items
        })
        .any(|rule| {
          rule.is_ok_and(|rule| {
            let Ok(input) = rule.input_sequence();
            (!ctx.zero_context
              || (({
                let Ok(explicit_value) = rule.backtrack_glyph_count();
                explicit_value
              }) == 0
                && ({
                  let Ok(explicit_value) = rule.lookahead_glyph_count();
                  explicit_value
                }) == 0))
              && ctx.glyphs.len() == input.len() + 1
              && input.iter().enumerate().all(|(i, value)| {
                let mut info = GlyphInfo {
                  glyph_id: ctx.glyphs[i + 1].into(),
                  ..GlyphInfo::default()
                };
                match_glyph(
                  &mut info,
                  ({
                    let Ok(value) = ({
                      let Ok(value) = value.get();
                      value
                    })
                    .to_u32();
                    value
                  }),
                )
              })
          })
        })
      })
  }
}

impl Apply for ChainedSequenceContextFormat1<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let index = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(self.coverage().ok()?.get(glyph)?);
      migrated_value
    });
    let Ok(chained_seq_rule_sets) = self.chained_seq_rule_sets();
    let set = chained_seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }),
      )
      .ok()??;
    let Ok(chained_seq_rules) = set.chained_seq_rules();

    apply_chain_context_rules(
      ctx,
      &chained_seq_rules,
      (match_glyph, match_glyph, match_glyph),
    )
  }
}

impl WouldApply for ChainedSequenceContextFormat2<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    let class_def = self.input_class_def().ok();
    let match_fn = &match_class(&class_def);
    let class = glyph_class(self.input_class_def(), ctx.glyphs[0]);
    let Ok(chained_class_seq_rule_sets) = self.chained_class_seq_rule_sets();

    chained_class_seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(class);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()
      .flatten()
      .is_some_and(|set| {
        ({
          let items = ({
            let Ok(array) = set.chained_class_seq_rules();
            let Ok(items) = array.iter();
            items
          });
          items
        })
        .any(|rule| {
          rule.is_ok_and(|rule| {
            let Ok(input) = rule.input_sequence();
            (!ctx.zero_context
              || (({
                let Ok(explicit_value) = rule.backtrack_glyph_count();
                explicit_value
              }) == 0
                && ({
                  let Ok(explicit_value) = rule.lookahead_glyph_count();
                  explicit_value
                }) == 0))
              && ctx.glyphs.len() == input.len() + 1
              && input.iter().enumerate().all(|(i, value)| {
                let mut info = GlyphInfo {
                  glyph_id: ctx.glyphs[i + 1].into(),
                  ..GlyphInfo::default()
                };
                match_fn(
                  &mut info,
                  ({
                    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                      ({
                        let Ok(value) = value.get();
                        value
                      }),
                    );
                    migrated_value
                  }),
                )
              })
          })
        })
      })
  }
}

/// Value represents glyph class.
fn match_class<'a>(
  class_def: &'a Option<ClassDef<'a>>,
) -> impl Fn(&mut GlyphInfo, u32) -> bool + 'a {
  |&mut info, value| {
    class_def
      .as_ref()
      .is_some_and(|class_def| u32::from(class_def.get(info.as_glyph())) == value)
  }
}

fn get_class_cached(class_def: &impl Fn(GlyphId) -> u16, info: &mut GlyphInfo) -> u16 {
  let mut klass = ({
    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(info.syllable());
    migrated_value
  });
  match klass < 255 {
    true => {
      return klass;
    }
    false => {}
  }
  klass = class_def(info.as_glyph());
  match klass < 255 {
    true => {
      info.set_syllable(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(klass);
          migrated_value
        }),
      );
    }
    false => {}
  }

  klass
}

fn match_class_cached<'a>(
  class_def: impl Fn(GlyphId) -> u16 + 'a,
) -> impl Fn(&mut GlyphInfo, u32) -> bool + 'a {
  move |info: &mut GlyphInfo, value| u32::from(get_class_cached(&class_def, info)) == value
}

fn get_class_cached1(class_def: &impl Fn(GlyphId) -> u16, info: &mut GlyphInfo) -> u16 {
  let mut klass = ({
    let Ok(migrated_value) =
      console_core_number_conversion::wrapped::<_, u16>((info.syllable() & 0x0F));
    migrated_value
  });
  match klass < 15 {
    true => {
      return klass;
    }
    false => {}
  }

  klass = class_def(info.as_glyph());

  match klass < 15 {
    true => {
      info.set_syllable(
        (info.syllable() & 0xF0)
          | ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(klass);
            migrated_value
          }),
      );
    }
    false => {}
  }

  klass
}

fn match_class_cached1<'a>(
  class_def: impl Fn(GlyphId) -> u16 + 'a,
) -> impl Fn(&mut GlyphInfo, u32) -> bool + 'a {
  move |info: &mut GlyphInfo, value| u32::from(get_class_cached1(&class_def, info)) == value
}

fn get_class_cached2(class_def: &impl Fn(GlyphId) -> u16, info: &mut GlyphInfo) -> u16 {
  let mut klass = ({
    let Ok(migrated_value) =
      console_core_number_conversion::wrapped::<_, u16>((info.syllable() & 0xF0));
    migrated_value
  }) >> 4;
  match klass < 15 {
    true => {
      return klass;
    }
    false => {}
  }
  klass = class_def(info.as_glyph());
  match klass < 15 {
    true => {
      info.set_syllable(
        (info.syllable() & 0x0F)
          | (({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(klass);
            migrated_value
          }) << 4),
      );
    }
    false => {}
  }
  klass
}

fn match_class_cached2<'a>(
  class_def: impl Fn(GlyphId) -> u16 + 'a,
) -> impl Fn(&mut GlyphInfo, u32) -> bool + 'a {
  move |info: &mut GlyphInfo, value| u32::from(get_class_cached2(&class_def, info)) == value
}

impl Apply for ChainedSequenceContextFormat2<'_> {
  fn apply_with_external_cache(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let SubtableExternalCache::ChainContextFormat2Cache(cache) = external_cache else {
      return None;
    };
    let offset_data = ({
      let Ok(data) = self.offset_data();
      data
    });
    coverage_binary_cached(
      |gid| cache.coverage.index(&offset_data, gid),
      glyph,
      &cache.coverage_cache,
    )?;
    let index = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(cache.input.class(&offset_data, glyph));
      migrated_value
    });
    let Ok(chained_class_seq_rule_sets) = self.chained_class_seq_rule_sets();
    let set = chained_class_seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }),
      )
      .ok()??;
    let Ok(chained_class_seq_rules) = set.chained_class_seq_rules();

    apply_chain_context_rules(
      ctx,
      &chained_class_seq_rules,
      (
        |info, val| u32::from(cache.backtrack.class(&offset_data, info.as_glyph())) == val,
        |info, val| u32::from(cache.input.class(&offset_data, info.as_glyph())) == val,
        |info, val| u32::from(cache.lookahead.class(&offset_data, info.as_glyph())) == val,
      ),
    )
  }
  fn apply_cached(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    external_cache: &SubtableExternalCache,
  ) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();
    let SubtableExternalCache::ChainContextFormat2Cache(cache) = external_cache else {
      return None;
    };
    let offset_data = ({
      let Ok(data) = self.offset_data();
      data
    });
    coverage_binary_cached(
      |gid| cache.coverage.index(&offset_data, gid),
      glyph,
      &cache.coverage_cache,
    )?;
    let input_class = |gid| cache.input.class(&offset_data, gid);
    let lookahead_class = |gid| cache.lookahead.class(&offset_data, gid);
    let index = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(get_class_cached2(
        &input_class,
        &mut ctx.buffer.info[ctx.buffer.idx],
      ));
      migrated_value
    });
    let Ok(chained_class_seq_rule_sets) = self.chained_class_seq_rule_sets();
    let set = chained_class_seq_rule_sets
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        }),
      )
      .ok()??;
    let Ok(chained_class_seq_rules) = set.chained_class_seq_rules();

    apply_chain_context_rules(
      ctx,
      &chained_class_seq_rules,
      (
        |info, val| u32::from(cache.backtrack.class(&offset_data, info.as_glyph())) == val,
        match_class_cached2(&input_class),
        match_class_cached1(&lookahead_class),
      ),
    )
  }
  fn cache_cost(&self) -> u32 {
    self
      .input_class_def()
      .ok()
      .map_or(0, |class_def| class_def.cost())
      + self
        .lookahead_class_def()
        .ok()
        .map_or(0, |class_def| class_def.cost())
  }

  fn external_cache_create(&self, _mode: SubtableExternalCacheMode) -> SubtableExternalCache {
    let data = ({
      let Ok(data) = self.offset_data();
      data
    });
    SubtableExternalCache::ChainContextFormat2Cache(ChainContextFormat2Cache {
      coverage_cache: BinaryCache::new(),
      coverage: CoverageInfo::new(
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
      )
      .unwrap_or_default(),
      backtrack: ClassDefInfo::new(
        &data,
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
            ({
              let Ok(backtrack_class_def_offset) = self.backtrack_class_def_offset();
              let Ok(value) = backtrack_class_def_offset.to_u32();
              value
            }),
          );
          migrated_value
        }),
      )
      .unwrap_or_default(),
      input: ClassDefInfo::new(
        &data,
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
            ({
              let Ok(input_class_def_offset) = self.input_class_def_offset();
              let Ok(value) = input_class_def_offset.to_u32();
              value
            }),
          );
          migrated_value
        }),
      )
      .unwrap_or_default(),
      lookahead: ClassDefInfo::new(
        &data,
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
            ({
              let Ok(lookahead_class_def_offset) = self.lookahead_class_def_offset();
              let Ok(value) = lookahead_class_def_offset.to_u32();
              value
            }),
          );
          migrated_value
        }),
      )
      .unwrap_or_default(),
    })
  }
}

impl WouldApply for ChainedSequenceContextFormat3<'_> {
  fn would_apply(&self, ctx: &WouldApplyContext) -> bool {
    let Ok(input_coverages) = self.input_coverages();
    (!ctx.zero_context
      || (({
        let Ok(explicit_value) = self.backtrack_coverage_offsets();
        explicit_value
      })
      .is_empty()
        && ({
          let Ok(explicit_value) = self.lookahead_coverage_offsets();
          explicit_value
        })
        .is_empty()))
      && (ctx.glyphs.len()
        == ({
          let Ok(value) = input_coverages.len();
          ({
            let Ok(value) = console_core_number_conversion::index(value);
            value
          })
        })
        && ({
          let Ok(value) = input_coverages.iter();
          value
        })
        .skip(1)
        .enumerate()
        .all(|(i, coverage)| coverage.is_ok_and(|cov| cov.get(ctx.glyphs[i + 1]).is_some())))
  }
}

impl Apply for ChainedSequenceContextFormat3<'_> {
  fn apply(&self, ctx: &mut hb_ot_apply_context_t) -> Option<()> {
    let glyph = ctx.buffer.cur(0).as_glyph();

    let Ok(input_coverages) = self.input_coverages();
    input_coverages.get(0).ok()?.get(glyph)?;

    let Ok(backtrack_coverages) = self.backtrack_coverages();
    let Ok(lookahead_coverages) = self.lookahead_coverages();

    let back = |info: &mut GlyphInfo, index: u32| {
      backtrack_coverages
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                migrated_value
              }),
            );
            value
          }),
        )
        .is_ok_and(|cov| cov.get(info.glyph_id).is_some())
    };

    let ahead = |info: &mut GlyphInfo, index: u32| {
      lookahead_coverages
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                migrated_value
              }),
            );
            value
          }),
        )
        .is_ok_and(|cov| cov.get(info.glyph_id).is_some())
    };

    let input = |info: &mut GlyphInfo, index: u32| {
      input_coverages
        .get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                migrated_value
              }) + 1,
            );
            value
          }),
        )
        .is_ok_and(|cov| cov.get(info.glyph_id).is_some())
    };

    let mut end_index = ctx.buffer.idx;
    let mut match_end = 0;

    let input_matches = match_input(
      ctx,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
          ({
            let Ok(value) = input_coverages.len();
            value
          }),
        );
        migrated_value
      }) - 1,
      input,
      &mut match_end,
      None,
    );

    match input_matches {
      true => {
        end_index = match_end;
      }
      false => {}
    }

    if !(input_matches
      && match_lookahead(
        ctx,
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
            ({
              let Ok(value) = lookahead_coverages.len();
              value
            }),
          );
          migrated_value
        }),
        ahead,
        match_end,
        &mut end_index,
      ))
    {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(end_index));
      return None;
    }

    let mut start_index = ctx.buffer.out_len;

    if !match_backtrack(
      ctx,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
          ({
            let Ok(value) = backtrack_coverages.len();
            value
          }),
        );
        migrated_value
      }),
      back,
      &mut start_index,
    ) {
      ctx
        .buffer
        .unsafe_to_concat_from_outbuffer(Some(start_index), Some(end_index));
      return None;
    }

    ctx
      .buffer
      .unsafe_to_break_from_outbuffer(Some(start_index), Some(end_index));
    apply_lookup(
      ctx,
      ({
        let Ok(value) = console_core_number_conversion::index(
          ({
            let Ok(value) = input_coverages.len();
            value
          }) - 1,
        );
        value
      }),
      match_end,
      ({
        let Ok(explicit_value) = self.seq_lookup_records();
        explicit_value
      }),
    );

    Some(())
  }
}

/// All of a context rule's fields, parsed in a single pass.
///
/// The generated getters re-derive the positions of all preceding fields on
/// every call, so fetching fields individually in the rule-matching loops
/// re-reads the leading counts many times over; this parses the whole rule
/// once instead. Glyph ids and classes are both read as raw u16s.
#[derive(Clone, Copy, Default)]
struct ParsedRule<'a> {
  backtrack: &'a [BigEndian<u16>],
  input: &'a [BigEndian<u16>],
  lookahead: &'a [BigEndian<u16>],
  records: &'a [SequenceLookupRecord],
}

impl<'a> ParsedRule<'a> {
  /// Parse a SequenceRule or ClassSequenceRule.
  fn from_rule_data(data: FontData<'a>) -> Option<Self> {
    let glyph_count: u16 = data.read_at(0).ok()?;
    let record_count: u16 = data.read_at(2).ok()?;
    let input_end = 4
      + ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(glyph_count);
        migrated_value
      })
      .saturating_sub(1)
        * ({
          let Ok(value) = console_core_number_conversion::index(u16::RAW_BYTE_LEN);
          value
        });
    let records_end = input_end
      + ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(record_count);
        migrated_value
      }) * ({
        let Ok(value) = console_core_number_conversion::index(SequenceLookupRecord::RAW_BYTE_LEN);
        value
      });
    Some(ParsedRule {
      input: data
        .read_array(
          4..({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(input_end);
            value
          }),
        )
        .ok()?,
      records: data
        .read_array(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(input_end);
            value
          })..({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(records_end);
            value
          }),
        )
        .ok()?,
      ..ParsedRule::default()
    })
  }

  /// Parse a ChainedSequenceRule or ChainedClassSequenceRule.
  fn from_chain_rule_data(data: FontData<'a>) -> Option<Self> {
    let backtrack_count: u16 = data.read_at(0).ok()?;
    let backtrack_end = 2
      + ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(backtrack_count);
        migrated_value
      }) * ({
        let Ok(value) = console_core_number_conversion::index(u16::RAW_BYTE_LEN);
        value
      });
    let input_count: u16 = data
      .read_at(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(backtrack_end);
          value
        }),
      )
      .ok()?;
    let input_end = backtrack_end
      + 2
      + ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(input_count);
        migrated_value
      })
      .saturating_sub(1)
        * ({
          let Ok(value) = console_core_number_conversion::index(u16::RAW_BYTE_LEN);
          value
        });
    let lookahead_count: u16 = data
      .read_at(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(input_end);
          value
        }),
      )
      .ok()?;
    let lookahead_end = input_end
      + 2
      + ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(lookahead_count);
        migrated_value
      }) * ({
        let Ok(value) = console_core_number_conversion::index(u16::RAW_BYTE_LEN);
        value
      });
    let record_count: u16 = data
      .read_at(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(lookahead_end);
          value
        }),
      )
      .ok()?;
    let records_end = lookahead_end
      + 2
      + ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(record_count);
        migrated_value
      }) * ({
        let Ok(value) = console_core_number_conversion::index(SequenceLookupRecord::RAW_BYTE_LEN);
        value
      });
    Some(ParsedRule {
      backtrack: data
        .read_array(
          2..({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(backtrack_end);
            value
          }),
        )
        .ok()?,
      input: data
        .read_array(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(backtrack_end + 2);
            value
          })..({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(input_end);
            value
          }),
        )
        .ok()?,
      lookahead: data
        .read_array(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(input_end + 2);
            value
          })..({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(lookahead_end);
            value
          }),
        )
        .ok()?,
      records: data
        .read_array(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(lookahead_end + 2);
            value
          })..({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(records_end);
            value
          }),
        )
        .ok()?,
    })
  }

  /// Match this rule's input sequence and apply its lookup records.
  ///
  /// Backtrack/lookahead are not consulted; chain rules go through
  /// [`apply_chain_with_sequences`] instead.
  fn apply(
    &self,
    ctx: &mut hb_ot_apply_context_t,
    match_func: &impl Fn(&mut GlyphInfo, u32) -> bool,
  ) -> Option<()> {
    let inputs = self.input;
    let match_func = |info: &mut GlyphInfo, index| {
      inputs
        .get(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
            migrated_value
          }),
        )
        .is_some_and(|value| {
          match_func(
            info,
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
                ({
                  let Ok(value) = value.get();
                  value
                }),
              );
              migrated_value
            }),
          )
        })
    };

    let mut match_end = 0;

    match match_input(
      ctx,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(inputs.len());
        migrated_value
      }),
      match_func,
      &mut match_end,
      None,
    ) {
      true => {
        ctx
          .buffer
          .unsafe_to_break(Some(ctx.buffer.idx), Some(match_end));
        apply_lookup(ctx, inputs.len(), match_end, self.records);
        return Some(());
      }
      false => {}
    }
    None
  }
}

trait ContextRule<'a>: FontRead<'a> {
  /// Parse all of the rule's fields in one pass.
  fn parse(&self) -> ParsedRule<'a>;

  /// The first glyph/class of the input sequence, or `None` if the input
  /// sequence is empty.
  ///
  /// Much cheaper than a full parse; used by the skip-ahead loops, which
  /// discard most of the rules they visit.
  fn first_input(&self) -> Option<u16>;
}

/// `first_input` for SequenceRule/ClassSequenceRule.
///
/// The glyph count is at offset 0 and the input sequence starts at offset 4,
/// after the lookup record count. (The input sequence's first entry covers
/// the *second* glyph of the matched sequence.)
fn plain_rule_first_input(data: &FontData) -> Option<u16> {
  let glyph_count: u16 = data.read_at(0).ok()?;
  match glyph_count <= 1 {
    true => {
      return None;
    }
    false => {}
  }
  data.read_at(4).ok()
}

/// `first_input` for ChainedSequenceRule/ChainedClassSequenceRule.
///
/// The input glyph count follows the variable-length backtrack sequence, and
/// the input sequence follows it directly.
fn chain_rule_first_input(data: &FontData) -> Option<u16> {
  let backtrack_count: u16 = data.read_at(0).ok()?;
  let count_pos = 2
    + ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(backtrack_count);
      migrated_value
    }) * ({
      let Ok(value) = console_core_number_conversion::index(u16::RAW_BYTE_LEN);
      value
    });
  let input_count: u16 = data
    .read_at(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(count_pos);
        value
      }),
    )
    .ok()?;
  match input_count <= 1 {
    true => {
      return None;
    }
    false => {}
  }
  data
    .read_at(
      ({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(count_pos + 2);
        value
      }),
    )
    .ok()
}

impl<'a> ContextRule<'a> for SequenceRule<'a> {
  fn parse(&self) -> ParsedRule<'a> {
    ParsedRule::from_rule_data({
      let Ok(value) = self.offset_data();
      value
    })
    .unwrap_or_default()
  }

  fn first_input(&self) -> Option<u16> {
    plain_rule_first_input(
      &({
        let Ok(data) = self.offset_data();
        data
      }),
    )
  }
}

impl<'a> ContextRule<'a> for ClassSequenceRule<'a> {
  fn parse(&self) -> ParsedRule<'a> {
    ParsedRule::from_rule_data({
      let Ok(value) = self.offset_data();
      value
    })
    .unwrap_or_default()
  }

  fn first_input(&self) -> Option<u16> {
    plain_rule_first_input(
      &({
        let Ok(data) = self.offset_data();
        data
      }),
    )
  }
}

impl<'a> ContextRule<'a> for ChainedSequenceRule<'a> {
  fn parse(&self) -> ParsedRule<'a> {
    ParsedRule::from_chain_rule_data({
      let Ok(value) = self.offset_data();
      value
    })
    .unwrap_or_default()
  }

  fn first_input(&self) -> Option<u16> {
    chain_rule_first_input(
      &({
        let Ok(data) = self.offset_data();
        data
      }),
    )
  }
}

impl<'a> ContextRule<'a> for ChainedClassSequenceRule<'a> {
  fn parse(&self) -> ParsedRule<'a> {
    ParsedRule::from_chain_rule_data({
      let Ok(value) = self.offset_data();
      value
    })
    .unwrap_or_default()
  }

  fn first_input(&self) -> Option<u16> {
    chain_rule_first_input(
      &({
        let Ok(data) = self.offset_data();
        data
      }),
    )
  }
}

fn apply_context_rules<'a, 'b, R: ContextRule<'a>>(
  ctx: &mut hb_ot_apply_context_t,
  rules: &'b ArrayOfOffsets<'a, R, Offset16>,
  match_func: impl Fn(&mut GlyphInfo, u32) -> bool,
) -> Option<()> {
  match false {
    true => {
      for rule in ({
        let Ok(value) = rules.iter();
        value
      })
      .filter_map(|r| r.ok())
      {
        match rule.parse().apply(ctx, &match_func).is_some() {
          true => {
            return Some(());
          }
          false => {}
        }
      }
      return None;
    }
    false => {}
  }

  let mut skippy_iter = skipping_iterator_t::with_match_fn(ctx, true, Some(match_always));
  skippy_iter.reset(skippy_iter.buffer.idx);
  skippy_iter.set_glyph_data(0);
  let mut unsafe_to = None;
  let unsafe_to1;
  let mut unsafe_to2 = 0;
  let mut second = None;
  let first = match skippy_iter.next(None) {
    true => {
      let g1 = skippy_iter.index();
      match skippy_iter.may_skip(&skippy_iter.buffer.info[g1]) != may_skip_t::SKIP_NO {
        true => {
          for rule in ({
            let Ok(value) = rules.iter();
            value
          })
          .filter_map(|r| r.ok())
          {
            match rule.parse().apply(ctx, &match_func).is_some() {
              true => {
                return Some(());
              }
              false => {}
            }
          }
          return None;
        }
        false => {}
      }
      unsafe_to1 = skippy_iter.index() + 1;
      g1
    }
    false => {
      for rule in ({
        let Ok(value) = rules.iter();
        value
      })
      .filter_map(|r| r.ok().map(|r| r.parse()))
      .filter(|r| r.input.len() <= 1)
      {
        match rule.apply(ctx, &match_func).is_some() {
          true => {
            return Some(());
          }
          false => {}
        }
      }
      return None;
    }
  };
  let matched = skippy_iter.next(None);
  let g2 = skippy_iter.index();
  match matched {
    true => {
      second = Some(g2);
      unsafe_to2 = skippy_iter.index() + 1;
      match skippy_iter.may_skip(&skippy_iter.buffer.info[g2]) != may_skip_t::SKIP_NO {
        true => {
          for rule in ({
            let Ok(value) = rules.iter();
            value
          })
          .filter_map(|r| r.ok())
          {
            match rule.parse().apply(ctx, &match_func).is_some() {
              true => {
                return Some(());
              }
              false => {}
            }
          }
          return None;
        }
        false => {}
      }
    }
    false => {}
  }
  let mut rules_iter = ({
    let Ok(value) = rules.iter();
    value
  })
  .filter_map(|r| r.ok());
  let mut rule_box = rules_iter.next().map(|r| r.parse());
  while let Some(rule) = rule_box {
    let inputs = rule.input;
    let match_func2 = |info: &mut GlyphInfo, index| match inputs
      .get(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
          migrated_value
        }),
      )
      .map(|v| {
        ({
          let Ok(value) = v.get();
          value
        })
      }) {
      Some(value) => match_func(info, u32::from(value)),
      None => false,
    };
    match inputs.len() <= 1 || match_func2(&mut ctx.buffer.info[first], 0) {
      true => {
        match second.is_none()
          || (inputs.len() <= 2 || match_func2(&mut ctx.buffer.info[second.unwrap()], 1))
        {
          true => match rule.apply(ctx, &match_func).is_some() {
            true => {
              match unsafe_to {
                Some(unsafe_to) => {
                  ctx
                    .buffer
                    .unsafe_to_concat(Some(ctx.buffer.idx), Some(unsafe_to));
                }
                None => {}
              }
              return Some(());
            }
            false => {}
          },
          false => {
            unsafe_to = Some(unsafe_to2);
          }
        }
        rule_box = rules_iter.next().map(|r| r.parse());
      }
      false => {
        match unsafe_to.is_none() {
          true => {
            unsafe_to = Some(unsafe_to1);
          }
          false => {}
        }

        let first_glyph_value = ({
          let Ok(value) = inputs.first().unwrap().get();
          value
        });
        loop {
          let next_rule = match rules_iter.next() {
            Some(next_rule) => next_rule,
            None => {
              rule_box = None;
              break;
            }
          };
          match next_rule.first_input() != Some((first_glyph_value)) {
            true => {
              rule_box = Some(next_rule.parse());
              break;
            }
            false => {}
          }
        }
      }
    }
  }
  match unsafe_to {
    Some(unsafe_to) => {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(unsafe_to));
    }
    None => {}
  }
  None
}

fn apply_chain_with_sequences<
  F1: Fn(&mut GlyphInfo, u32) -> bool,
  F2: Fn(&mut GlyphInfo, u32) -> bool,
  F3: Fn(&mut GlyphInfo, u32) -> bool,
>(
  ctx: &mut hb_ot_apply_context_t,
  rule: &ParsedRule<'_>,
  match_funcs: &(F1, F2, F3),
) -> Option<()> {
  let input = rule.input;
  let f3 = |info: &mut GlyphInfo, index| {
    input
      .get(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
          migrated_value
        }),
      )
      .is_some_and(|value| {
        match_funcs.1(
          info,
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
              ({
                let Ok(value) = value.get();
                value
              }),
            );
            migrated_value
          }),
        )
      })
  };

  let mut end_index = ctx.buffer.idx;
  let mut match_end = 0;

  let input_matches = match_input(
    ctx,
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(input.len());
      migrated_value
    }),
    f3,
    &mut match_end,
    None,
  );

  match input_matches {
    true => {
      end_index = match_end;
    }
    false => {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(end_index));
      return None;
    }
  }

  let lookahead = rule.lookahead;
  let f2 = |info: &mut GlyphInfo, index| {
    lookahead
      .get(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
          migrated_value
        }),
      )
      .is_some_and(|value| {
        match_funcs.2(
          info,
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
              ({
                let Ok(value) = value.get();
                value
              }),
            );
            migrated_value
          }),
        )
      })
  };

  match !match_lookahead(
    ctx,
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(lookahead.len());
      migrated_value
    }),
    f2,
    match_end,
    &mut end_index,
  ) {
    true => {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(end_index));
      return None;
    }
    false => {}
  }

  let mut start_index = ctx.buffer.out_len;

  let backtrack = rule.backtrack;
  let f1 = |info: &mut GlyphInfo, index| {
    backtrack
      .get(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
          migrated_value
        }),
      )
      .is_some_and(|value| {
        match_funcs.0(
          info,
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
              ({
                let Ok(value) = value.get();
                value
              }),
            );
            migrated_value
          }),
        )
      })
  };

  match !match_backtrack(
    ctx,
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(backtrack.len());
      migrated_value
    }),
    f1,
    &mut start_index,
  ) {
    true => {
      ctx
        .buffer
        .unsafe_to_concat_from_outbuffer(Some(start_index), Some(end_index));
      return None;
    }
    false => {}
  }

  ctx
    .buffer
    .unsafe_to_break_from_outbuffer(Some(start_index), Some(end_index));
  apply_lookup(ctx, input.len(), match_end, rule.records);

  Some(())
}

fn apply_chain_context_rules<
  'a,
  'b,
  R: ContextRule<'a>,
  F1: Fn(&mut GlyphInfo, u32) -> bool,
  F2: Fn(&mut GlyphInfo, u32) -> bool,
  F3: Fn(&mut GlyphInfo, u32) -> bool,
>(
  ctx: &mut hb_ot_apply_context_t,
  rules: &'b ArrayOfOffsets<'a, R, Offset16>,
  match_funcs: (F1, F2, F3),
) -> Option<()> {
  if ({
    let Ok(value) = rules.len();
    value
  }) <= 4
  {
    for rule in ({
      let Ok(value) = rules.iter();
      value
    })
    .filter_map(|r| r.ok())
    {
      match apply_chain_with_sequences(ctx, &rule.parse(), &match_funcs).is_some() {
        true => {
          return Some(());
        }
        false => {}
      }
    }
    return None;
  }

  let mut skippy_iter = skipping_iterator_t::with_match_fn(ctx, true, Some(match_always));
  skippy_iter.reset(skippy_iter.buffer.idx);
  skippy_iter.set_glyph_data(0);
  let mut unsafe_to = None;
  let unsafe_to1;
  let mut unsafe_to2 = 0;
  let mut second = None;
  let first = match skippy_iter.next(None) {
    true => {
      let g1 = skippy_iter.index();
      match skippy_iter.may_skip(&skippy_iter.buffer.info[g1]) != may_skip_t::SKIP_NO {
        true => {
          for rule in ({
            let Ok(value) = rules.iter();
            value
          })
          .filter_map(|r| r.ok())
          {
            match apply_chain_with_sequences(ctx, &rule.parse(), &match_funcs).is_some() {
              true => {
                return Some(());
              }
              false => {}
            }
          }
          return None;
        }
        false => {}
      }
      unsafe_to1 = skippy_iter.index() + 1;
      g1
    }
    false => {
      for rule in ({
        let Ok(value) = rules.iter();
        value
      })
      .filter_map(|r| r.ok().map(|r| r.parse()))
      .filter(|r| r.input.len() <= 1 && r.lookahead.is_empty())
      {
        match apply_chain_with_sequences(ctx, &rule, &match_funcs).is_some() {
          true => {
            return Some(());
          }
          false => {}
        }
      }
      return None;
    }
  };
  let matched = skippy_iter.next(None);
  let g2 = skippy_iter.index();
  match matched {
    true => {
      second = Some(g2);
      unsafe_to2 = skippy_iter.index() + 1;
      match skippy_iter.may_skip(&skippy_iter.buffer.info[g2]) != may_skip_t::SKIP_NO {
        true => {
          for rule in ({
            let Ok(value) = rules.iter();
            value
          })
          .filter_map(|r| r.ok())
          {
            match apply_chain_with_sequences(ctx, &rule.parse(), &match_funcs).is_some() {
              true => {
                return Some(());
              }
              false => {}
            }
          }
          return None;
        }
        false => {}
      }
    }
    false => {}
  }
  let mut rules_iter = ({
    let Ok(value) = rules.iter();
    value
  })
  .filter_map(|r| r.ok());
  let mut rule_box = rules_iter.next().map(|r| r.parse());
  while let Some(rule) = rule_box {
    let input = rule.input;
    let lookahead = rule.lookahead;
    let match_input = |info: &mut GlyphInfo, index: usize| {
      input.get(index).is_some_and(|v| {
        match_funcs.1(
          info,
          u32::from(
            ({
              let Ok(value) = v.get();
              value
            }),
          ),
        )
      })
    };
    let match_lookahead = |info: &mut GlyphInfo, index: usize| {
      lookahead.get(index).is_some_and(|v| {
        match_funcs.2(
          info,
          u32::from(
            ({
              let Ok(value) = v.get();
              value
            }),
          ),
        )
      })
    };
    let len_p1 = input.len() + 1;
    let matched_first = match len_p1 > 1 {
      true => match_input(&mut ctx.buffer.info[first], 0),
      false => lookahead.is_empty() || match_lookahead(&mut ctx.buffer.info[first], 0),
    };
    match matched_first {
      true => {
        let matched_second = match second {
          Some(second) => match len_p1 > 2 {
            true => match_input(&mut ctx.buffer.info[second], 1),
            false => {
              (lookahead.len() <= 2 - len_p1)
                || match_lookahead(&mut ctx.buffer.info[second], 2 - len_p1)
            }
          },
          None => true,
        };
        match matched_second {
          true => match apply_chain_with_sequences(ctx, &rule, &match_funcs).is_some() {
            true => {
              match unsafe_to {
                Some(unsafe_to) => {
                  ctx
                    .buffer
                    .unsafe_to_concat(Some(ctx.buffer.idx), Some(unsafe_to));
                }
                None => {}
              }
              return Some(());
            }
            false => {}
          },
          false => {
            unsafe_to = Some(unsafe_to2);
          }
        }

        rule_box = rules_iter.next().map(|r| r.parse());
      }
      false => {
        match unsafe_to.is_none() {
          true => {
            unsafe_to = Some(unsafe_to1);
          }
          false => {}
        }

        match len_p1 > 1 {
          true => {
            let first_glyph_value = ({
              let Ok(value) = input.first().unwrap().get();
              value
            });
            loop {
              let next_rule = match rules_iter.next() {
                Some(next_rule) => next_rule,
                None => {
                  rule_box = None;
                  break;
                }
              };
              match next_rule.first_input() != Some((first_glyph_value)) {
                true => {
                  rule_box = Some(next_rule.parse());
                  break;
                }
                false => {}
              }
            }
          }
          false => {
            rule_box = rules_iter.next().map(|r| r.parse());
          }
        }
      }
    }
  }
  match unsafe_to {
    Some(unsafe_to) => {
      ctx
        .buffer
        .unsafe_to_concat(Some(ctx.buffer.idx), Some(unsafe_to));
    }
    None => {}
  }
  None
}
