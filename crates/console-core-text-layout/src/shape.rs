use crate::fallback::FontFallbackIter;
use crate::{
  Align, Attrs, AttrsList, CacheKeyFlags, Color, DecorationMetrics, DecorationSpan, Ellipsize,
  EllipsizeHeightLimit, Family, Font, FontSystem, GlyphDecorationData, Hinting, LayoutGlyph,
  LayoutLine, Metrics, Wrap,
};

use alloc::collections::VecDeque;
use core::fmt;
use core::mem;
use core::ops::Range;

use fontdb::{Pitch, Style};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::{ClusterKind, UnicodeSegmentation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shaping {
  Basic,

  Advanced,
}

impl Shaping {
  fn run(
    self,
    glyphs: &mut Vec<ShapeGlyph>,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    options: RunShaping,
  ) -> Result<(), console_core_never::Never> {
    let RunShaping {
      start_run,
      end_run,
      span_direction,
    } = options;

    {
      match self {
        Self::Basic => match shape_skip(
          font_system,
          glyphs,
          line,
          attrs_list,
          start_run,
          end_run,
        ) {
          Ok(value) => value,
        },
        #[cfg(not(feature = "shape-run-cache"))]
        Self::Advanced => match shape_run(
          glyphs,
          font_system,
          line,
          attrs_list,
          start_run,
          end_run,
          span_direction,
        ) {
          Ok(value) => value,
        },
        #[cfg(feature = "shape-run-cache")]
        Self::Advanced => match shape_run_cached(
          glyphs,
          font_system,
          line,
          attrs_list,
          start_run,
          end_run,
          span_direction,
        ) {
          Ok(value) => value,
        },
      };

      Ok(())
    }
  }
}

const NUM_SHAPE_PLANS: u32 = 6;

#[derive(Default)]
pub struct ShapeBuffer {
  shape_plan_cache: VecDeque<(fontdb::ID, harfrust::ShapePlan)>,

  harfrust_buffer: Option<harfrust::UnicodeBuffer>,

  scripts: Vec<Script>,

  spans: Vec<ShapeSpan>,

  words: Vec<ShapeWord>,

  visual_lines: Vec<VisualLine>,
  cached_visual_lines: Vec<VisualLine>,

  glyph_sets: Vec<Vec<LayoutGlyph>>,
}

#[derive(Debug)]
struct LayoutStorage {
  visual_lines: Vec<VisualLine>,
  cached_visual_lines: Vec<VisualLine>,
  cached_glyph_sets: Vec<Vec<LayoutGlyph>>,
}

impl ShapeBuffer {
  fn recycle_layout(
    &mut self,
    layout_lines: &mut Vec<LayoutLine>,
  ) -> Result<LayoutStorage, console_core_never::Never> {
    let mut visual_lines = mem::take(&mut self.visual_lines);
    let mut cached_visual_lines = mem::take(&mut self.cached_visual_lines);
    cached_visual_lines.clear();
    cached_visual_lines.extend(visual_lines.drain(..).map(|mut l| {
      match l.clear() {
        Ok(value) => value,
      };

      l
    }));

    let mut cached_glyph_sets = mem::take(&mut self.glyph_sets);
    cached_glyph_sets.clear();
    cached_glyph_sets.extend(layout_lines.drain(..).rev().map(|mut v| {
      v.glyphs.clear();
      v.glyphs
    }));

    Ok(LayoutStorage {
      visual_lines,
      cached_visual_lines,
      cached_glyph_sets,
    })
  }
}

impl fmt::Debug for ShapeBuffer {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad("ShapeBuffer { .. }")
  }
}

fn text_range(line: &str, range: Range<u32>) -> Result<Option<&str>, console_core_never::Never> {
  let Ok(start) = console_core_number_conversion::index(range.start);
  let Ok(end) = console_core_number_conversion::index(range.end);
  Ok(line.get(start..end))
}

fn glyph_cluster_ends(
  glyphs: &mut [ShapeGlyph],
  direction: crate::TextDirection,
  run_end: crate::RunEnd,
) -> Result<(), console_core_never::Never> {
  match direction {
    crate::TextDirection::LeftToRight => assign_cluster_ends(glyphs.iter_mut().rev(), run_end),
    crate::TextDirection::RightToLeft => assign_cluster_ends(glyphs.iter_mut(), run_end),
  }
}

fn assign_cluster_ends<'a>(
  glyphs: impl Iterator<Item = &'a mut ShapeGlyph>,
  run_end: crate::RunEnd,
) -> Result<(), console_core_never::Never> {
  let mut next_start = run_end.0;
  let mut cluster_end = run_end.0;

  for glyph in glyphs {
    match (glyph.start != next_start).then_some(()) {
      Some(()) => cluster_end = next_start,
      None => {}
    }

    glyph.end = cluster_end;
    next_start = glyph.start;
  }

  Ok(())
}

fn shape_fallback(
  scratch: &mut ShapeBuffer,
  glyphs: &mut Vec<ShapeGlyph>,
  font: &Font,
  run: &str,
  attrs_list: &AttrsList,
  options: FallbackShaping,
) -> Result<Vec<u32>, console_core_never::Never> {
  let mut buffer = match scratch.harfrust_buffer.take() {
    Some(recycled) => recycled,
    None => harfrust::UnicodeBuffer::new(),
  };
  let direction = match options.span_direction {
    crate::TextDirection::LeftToRight => harfrust::Direction::LeftToRight,
    crate::TextDirection::RightToLeft => harfrust::Direction::RightToLeft,
  };
  buffer.set_direction(direction);

  match run.contains('\t').then_some(()) {
    Some(()) => buffer.push_str(&run.replace('\t', " ")),
    None => buffer.push_str(run),
  }

  buffer.guess_segment_properties();
  assert_eq!(buffer.direction(), direction);

  let Ok(attrs) = attrs_list.get_span(options.start_run);
  let Ok(feature_start) = console_core_number_conversion::index(0);
  let Ok(feature_end) = console_core_number_conversion::index(u32::MAX);
  let features: Vec<_> = attrs
    .font_features
    .features
    .iter()
    .map(|feature| {
      let Ok(bytes) = feature.tag.as_bytes();
      let Ok(tag) = harfrust::Tag::new(bytes);
      harfrust::Feature::new(tag, feature.value, feature_start..feature_end)
    })
    .collect();

  let Ok(shaper) = font.shaper();
  let Ok(plan) = shape_plan(&mut scratch.shape_plan_cache, font, &buffer, &features);
  let shaped = shaper.shape(
    buffer,
    harfrust::ShapeOptions::new().plan(plan).features(&features),
  );
  let Ok(missing) = append_shaped_glyphs(glyphs, font, attrs_list, options, &shaped);
  scratch.harfrust_buffer = Some(shaped.clear());

  Ok(missing)
}

fn shape_plan<'a>(
  cache: &'a mut VecDeque<(fontdb::ID, harfrust::ShapePlan)>,
  font: &Font,
  buffer: &harfrust::UnicodeBuffer,
  features: &[harfrust::Feature],
) -> Result<Option<&'a harfrust::ShapePlan>, console_core_never::Never> {
  let Ok(id) = font.id();
  let Ok(instance) = font.shaper_instance();
  let language = buffer.language();
  let key = harfrust::ShapePlanKey::new(Some(buffer.script()), buffer.direction())
    .features(features)
    .instance(Some(instance))
    .language(language.as_ref());
  let existing = cache
    .iter()
    .position(|(font_id, plan)| *font_id == id && key.matches(plan))
    .map(|index| {
      let Ok(position) = console_core_number_conversion::fitted::<_, u32>(index);
      position
    });

  match existing {
    Some(position) => {
      let Ok(index) = console_core_number_conversion::index(position);
      return Ok(cache.get(index).map(|(_, plan)| plan));
    }
    None => {}
  }

  let Ok(shaper) = font.shaper();
  let plan = harfrust::ShapePlan::new(
    shaper,
    buffer.direction(),
    Some(buffer.script()),
    language.as_ref(),
    features,
  );
  let Ok(limit) = console_core_number_conversion::index(NUM_SHAPE_PLANS);

  match (cache.len() >= limit).then_some(()) {
    Some(()) => {
      let _evicted = cache.pop_front();
    }
    None => {}
  }

  cache.push_back((id, plan));
  Ok(cache.back().map(|(_, cached)| cached))
}

fn append_shaped_glyphs(
  glyphs: &mut Vec<ShapeGlyph>,
  font: &Font,
  attrs_list: &AttrsList,
  options: FallbackShaping,
  shaped: &harfrust::GlyphBuffer,
) -> Result<Vec<u32>, console_core_never::Never> {
  let Ok(metrics) = font.metrics();
  let Ok(font_id) = font.id();
  let Ok(font_monospace_em_width) = font.monospace_em_width();
  let font_scale = f32::from(metrics.units_per_em);
  let ascent = metrics.ascent / font_scale;
  let descent = -metrics.descent / font_scale;
  let em_distance = |units| {
    let Ok(value) = console_core_number_conversion::narrow_f32(f64::from(units));
    value / font_scale
  };
  let infos = shaped.glyph_infos();
  let positions = shaped.glyph_positions();
  let mut missing = Vec::new();
  let Ok(first_glyph) = console_core_number_conversion::fitted::<_, u32>(glyphs.len());
  let Ok(glyph_start) = console_core_number_conversion::index(first_glyph);
  glyphs.reserve(infos.len());

  for (info, position) in infos.iter().zip(positions) {
    let Ok(cluster) = console_core_number_conversion::fitted::<_, u32>(info.cluster);
    let start = options.start_run.saturating_add(cluster);
    let Ok(glyph_id) = console_core_number_conversion::fitted::<_, u16>(info.glyph_id);

    match (info.glyph_id == 0).then_some(()) {
      Some(()) => missing.push(start),
      None => {}
    }

    let Ok(attrs) = attrs_list.get_span(start);
    let Ok(cache_key_flags) = override_fake_italic(attrs.cache_key_flags, font, &attrs);
    glyphs.push(ShapeGlyph {
      start,
      end: options.end_run.0,
      x_advance: em_distance(position.x_advance)
        + attrs.letter_spacing_opt.map_or(0.0, |spacing| spacing.0),
      y_advance: em_distance(position.y_advance),
      x_offset: em_distance(position.x_offset),
      y_offset: em_distance(position.y_offset),
      ascent,
      descent,
      font_monospace_em_width,
      font_id,
      font_weight: attrs.weight,
      glyph_id,
      color_opt: attrs.color_opt,
      metadata: attrs.metadata,
      cache_key_flags,
      metrics_opt: attrs.metrics_opt.map(Into::into),
    });
  }

  match glyphs.get_mut(glyph_start..) {
    Some(new_glyphs) => {
      let Ok(()) = glyph_cluster_ends(new_glyphs, options.span_direction, options.end_run);
    }
    None => {}
  }

  Ok(missing)
}

fn collect_run_scripts(
  scripts: &mut Vec<Script>,
  run: &str,
) -> Result<(), console_core_never::Never> {
  scripts.clear();
  let mut seen = crate::HashMap::default();

  for character in run.chars() {
    let Ok(script) = character.script();

    match matches!(
      script,
      Script::Common | Script::Inherited | Script::Latin | Script::Unknown
    )
    .then_some(())
    {
      Some(()) => continue,
      None => {}
    }

    match seen.insert(script, ()) {
      Some(()) => {}
      None => scripts.push(script),
    }
  }

  Ok(())
}

fn shape_run(
  glyphs: &mut Vec<ShapeGlyph>,
  font_system: &mut FontSystem,
  line: &str,
  attrs_list: &AttrsList,
  start_run: u32,
  end_run: crate::RunEnd,
  span_direction: crate::TextDirection,
) -> Result<(), console_core_never::Never> {
  let Ok(checked_range) = text_range(line, start_run..end_run.0);
  let run = match checked_range {
    Some(text) => text,
    None => return Ok(()),
  };
  let mut scratch = mem::take(&mut font_system.shape_buffer);
  let Ok(()) = collect_run_scripts(&mut scratch.scripts, run);
  console_core_logging::trace!("      Run {:?}: '{}'", scratch.scripts, run);

  let Ok(attrs) = attrs_list.get_span(start_run);
  let Ok(fonts) = font_system.get_font_matches(&attrs);
  let default_families = [&attrs.family];
  let scripts = mem::take(&mut scratch.scripts);
  let Ok(mut font_iter) = FontFallbackIter::new(
    font_system,
    &fonts,
    &default_families,
    &scripts,
    run,
    attrs.weight,
  );

  let font = match font_iter.next() {
    Some(selected) => selected,
    None => return Ok(()),
  };
  let Ok(first_glyph) = console_core_number_conversion::fitted::<_, u32>(glyphs.len());
  let options = FallbackShaping {
    start_run,
    end_run,
    span_direction,
  };
  let Ok(initial_missing) = shape_fallback(&mut scratch, glyphs, &font, run, attrs_list, options);
  let mut missing = initial_missing
    .into_iter()
    .collect::<alloc::collections::BTreeSet<_>>();

  'fallback_fonts: for fallback_font in font_iter.by_ref() {
    match missing.is_empty().then_some(()) {
      Some(()) => break 'fallback_fonts,
      None => {}
    }

    let Ok(font_id) = fallback_font.id();
    console_core_logging::trace!("Evaluating fallback with font '{font_id:?}'");
    let mut fallback_glyphs = Vec::new();
    let Ok(fallback_missing) = shape_fallback(
      &mut scratch,
      &mut fallback_glyphs,
      &fallback_font,
      run,
      attrs_list,
      options,
    );
    let Ok(()) = merge_fallback_glyphs(
      glyphs,
      crate::GlyphIndex(first_glyph),
      &mut missing,
      fallback_glyphs,
      fallback_missing,
    );
  }

  let Ok(()) = font_iter.check_missing(run);
  scratch.scripts = scripts;
  font_system.shape_buffer = scratch;
  Ok(())
}

fn merge_fallback_glyphs(
  glyphs: &mut Vec<ShapeGlyph>,
  glyph_start: crate::GlyphIndex,
  missing: &mut alloc::collections::BTreeSet<u32>,
  fallback_glyphs: Vec<ShapeGlyph>,
  fallback_missing: Vec<u32>,
) -> Result<(), console_core_never::Never> {
  let unavailable: alloc::collections::BTreeSet<_> = fallback_missing.into_iter().collect();
  let mut pending: Option<FallbackCluster> = None;

  for glyph in fallback_glyphs {
    match pending.as_mut() {
      Some(cluster) => {
        match (glyph.start >= cluster.range.start && glyph.end <= cluster.range.end).then_some(())
        {
          Some(()) => {
            cluster.glyphs.push(glyph);
            continue;
          }
          None => {}
        }
      }
      None => {}
    }

    let Ok(()) = commit_fallback_cluster(glyphs, glyph_start, missing, pending.take());

    pending =
      (missing.contains(&glyph.start) && !unavailable.contains(&glyph.start)).then(|| {
        FallbackCluster {
          range: glyph.start..glyph.end,
          glyphs: vec![glyph],
        }
      });
  }

  commit_fallback_cluster(glyphs, glyph_start, missing, pending)
}

struct FallbackCluster {
  range: Range<u32>,
  glyphs: Vec<ShapeGlyph>,
}

fn commit_fallback_cluster(
  glyphs: &mut Vec<ShapeGlyph>,
  glyph_start: crate::GlyphIndex,
  missing: &mut alloc::collections::BTreeSet<u32>,
  pending: Option<FallbackCluster>,
) -> Result<(), console_core_never::Never> {
  let cluster = match pending {
    Some(shaped) => shaped,
    None => return Ok(()),
  };
  let Ok(first_index) = console_core_number_conversion::index(glyph_start.0);
  let run_glyphs = match glyphs.get(first_index..) {
    Some(run) => run,
    None => return Ok(()),
  };
  let inside =
    |glyph: &ShapeGlyph| glyph.start >= cluster.range.start && glyph.end <= cluster.range.end;
  let Ok(prefix) = console_core_number_conversion::fitted::<_, u32>(
    run_glyphs.iter().take_while(|glyph| !inside(glyph)).count(),
  );
  let Ok(prefix_index) = console_core_number_conversion::index(prefix);
  let Ok(replaced) = console_core_number_conversion::fitted::<_, u32>(
    run_glyphs
      .iter()
      .skip(prefix_index)
      .take_while(|glyph| inside(glyph))
      .count(),
  );
  let Ok(start) = console_core_number_conversion::index(glyph_start.0.saturating_add(prefix));
  let Ok(end) = console_core_number_conversion::index(
    glyph_start
      .0
      .saturating_add(prefix)
      .saturating_add(replaced),
  );
  drop(glyphs.splice(start..end, cluster.glyphs));
  missing.retain(|position| !cluster.range.contains(position));
  Ok(())
}

#[cfg(feature = "shape-run-cache")]
fn shape_run_cached(
  glyphs: &mut Vec<ShapeGlyph>,
  font_system: &mut FontSystem,
  line: &str,
  attrs_list: &AttrsList,
  start_run: u32,
  end_run: crate::RunEnd,
  span_direction: crate::TextDirection,
) -> Result<(), console_core_never::Never> {
  let crate::RunEnd(end_run_2) = end_run;

  let span_rtl = span_direction == crate::TextDirection::RightToLeft;

  {
    use crate::{AttrsOwned, ShapeRunKey};

    let run_range = start_run..end_run_2;
    let mut key = ShapeRunKey {
      text: ((*(match (line).get({
        let core::ops::Range { start, end } = run_range.clone();
        (match console_core_number_conversion::index(start) {
          Ok(position) => position,
        })..(match console_core_number_conversion::index(end) {
          Ok(position) => position,
        })
      }) {
        Some(value) => value,
        None => return Ok(()),
      }))
      .to_string()),
      default_attrs: match AttrsOwned::new(&match attrs_list.defaults() {
        Ok(value) => value,
      }) {
        Ok(value) => value,
      },
      attrs_spans: Vec::new(),
    };

    'text_items_4: for (attrs_range, attrs) in attrs_list
      .spans
      .iter()
      .filter(|(range, _)| range.start < run_range.end && range.end > run_range.start)
    {
      match ::core::primitive::bool::then_some(attrs == &key.default_attrs, ()) {
        ::core::option::Option::Some(()) => {
          continue 'text_items_4;
        }
        ::core::option::Option::None => {}
      }

      let start = attrs_range.start.max(start_run).saturating_sub(start_run);
      let end = attrs_range.end.min(end_run_2).saturating_sub(start_run);

      match ::core::primitive::bool::then_some(end > start, ()) {
        ::core::option::Option::Some(()) => {
          let range = start..end;
          key.attrs_spans.push((range, attrs.clone()));
        }
        ::core::option::Option::None => {}
      }
    }

    match match font_system.shape_run_cache.get(&key) {
      Ok(value) => value,
    } {
      Some(cache_glyphs) => {
        for mut glyph in cache_glyphs.iter().cloned() {
          glyph.start = (glyph.start).saturating_add(start_run);
          glyph.end = (glyph.end).saturating_add(start_run);
          glyphs.push(glyph);
        }

        return Ok(());
      }
      None => {}
    }

    let mut cache_glyphs = Vec::new();

    match shape_run(
      &mut cache_glyphs,
      font_system,
      line,
      attrs_list,
      start_run,
      crate::RunEnd(end_run_2),
      match ::core::primitive::bool::then_some(span_rtl, ()) {
        ::core::option::Option::Some(()) => crate::TextDirection::RightToLeft,
        ::core::option::Option::None => crate::TextDirection::LeftToRight,
      },
    ) {
      Ok(value) => value,
    };

    glyphs.extend_from_slice(&cache_glyphs);

    for glyph in cache_glyphs.iter_mut() {
      glyph.start = (glyph.start).saturating_sub(start_run);
      glyph.end = (glyph.end).saturating_sub(start_run);
    }

    match font_system.shape_run_cache.insert(key, cache_glyphs) {
      Ok(value) => value,
    };

    Ok(())
  }
}

fn shape_skip(
  font_system: &mut FontSystem,
  glyphs: &mut Vec<ShapeGlyph>,
  line: &str,
  attrs_list: &AttrsList,
  start_run: u32,
  end_run: crate::RunEnd,
) -> Result<(), console_core_never::Never> {
  let Ok(attrs) = attrs_list.get_span(start_run);
  let Ok(fonts) = font_system.get_font_matches(&attrs);
  let default_families = [&attrs.family];
  let Ok(mut font_iter) = FontFallbackIter::new(
    font_system,
    &fonts,
    &default_families,
    &[],
    "",
    attrs.weight,
  );
  let font = match font_iter.next() {
    Some(selected) => selected,
    None => return Ok(()),
  };
  let Ok(first_glyph) = console_core_number_conversion::fitted::<_, u32>(glyphs.len());
  let Ok(glyph_start) = console_core_number_conversion::index(first_glyph);
  let Ok(()) = shape_skip_glyphs(glyphs, &font, line, attrs_list, start_run, end_run);

  let run_glyphs = match glyphs.get_mut(glyph_start..) {
    Some(present) => present,
    None => return Ok(()),
  };

  match (matches!(attrs.family, Family::Name(_))
    && run_glyphs.iter().any(|glyph| glyph.glyph_id == 0))
  .then_some(())
  {
    Some(()) => {}
    None => return Ok(()),
  }

  let Ok(font_id) = font.id();
  let Ok(db) = font_system.db();
  let Ok(fallback_face) = db.face(font_id);
  let family = match fallback_face.map(|face| face.pitch) {
    Some(Pitch::Fixed) => Family::Monospace,
    Some(Pitch::Proportional) | None => Family::SansSerif,
  };
  let Ok(defaults) = Attrs::new();
  let Ok(with_family) = defaults.family(family);
  let Ok(with_weight) = with_family.weight(attrs.weight);
  let Ok(with_style) = with_weight.style(attrs.style);
  let Ok(fallback_attrs) = with_style.stretch(attrs.stretch);
  let Ok(fallback_fonts) = font_system.get_font_matches(&fallback_attrs);
  let families = [&family];
  let Ok(mut fallback_iter) = FontFallbackIter::new(
    font_system,
    &fallback_fonts,
    &families,
    &[],
    "",
    attrs.weight,
  );

  match fallback_iter.next() {
    Some(fallback_font) => {
      fill_missing_basic_glyphs(run_glyphs, &fallback_font, line, attrs_list)
    }
    None => Ok(()),
  }
}

fn fill_missing_basic_glyphs(
  glyphs: &mut [ShapeGlyph],
  font: &Font,
  line: &str,
  attrs_list: &AttrsList,
) -> Result<(), console_core_never::Never> {
  let Ok(opened) = font.face();
  let face = match opened {
    Some(parsed) => parsed,
    None => return Ok(()),
  };
  let charmap = skrifa::MetadataProvider::charmap(&face);
  let Ok(metrics) = font.metrics();
  let Ok(glyph_metrics) = crate::font::advances_per_em(&face);
  let Ok(font_id) = font.id();
  let Ok(em_width) = font.monospace_em_width();
  let scale = f32::from(metrics.units_per_em);

  for glyph in glyphs.iter_mut().filter(|glyph| glyph.glyph_id == 0) {
    let Ok(checked_range) = text_range(line, glyph.start..glyph.end);
    let text = match checked_range {
      Some(cluster) => cluster,
      None => continue,
    };
    let codepoint = text.chars().next().map_or('\0', |value| value);
    let Ok(glyph_id) = crate::font::glyph_in(&charmap, codepoint);

    match (glyph_id != 0).then_some(()) {
      Some(()) => {}
      None => continue,
    }

    let Ok(attrs) = attrs_list.get_span(glyph.start);
    let Ok(advance) = crate::font::advance_in(&glyph_metrics, glyph_id);
    let Ok(flags) = override_fake_italic(attrs.cache_key_flags, font, &attrs);
    glyph.glyph_id = glyph_id;
    glyph.font_id = font_id;
    glyph.font_monospace_em_width = em_width;
    glyph.ascent = metrics.ascent / scale;
    glyph.descent = -metrics.descent / scale;
    glyph.x_advance = advance + attrs.letter_spacing_opt.map_or(0.0, |spacing| spacing.0);
    glyph.cache_key_flags = flags;
  }

  Ok(())
}

fn shape_skip_glyphs(
  glyphs: &mut Vec<ShapeGlyph>,
  font: &Font,
  line: &str,
  attrs_list: &AttrsList,
  start_run: u32,
  end_run: crate::RunEnd,
) -> Result<(), console_core_never::Never> {
  let Ok(opened) = font.face();
  let face = match opened {
    Some(parsed) => parsed,
    None => return Ok(()),
  };
  let Ok(checked_range) = text_range(line, start_run..end_run.0);
  let run = match checked_range {
    Some(text) => text,
    None => return Ok(()),
  };

  let Ok(font_id) = font.id();
  let Ok(font_monospace_em_width) = font.monospace_em_width();
  let charmap = skrifa::MetadataProvider::charmap(&face);
  let Ok(metrics) = font.metrics();
  let Ok(glyph_metrics) = crate::font::advances_per_em(&face);
  let scale = f32::from(metrics.units_per_em);
  let ascent = metrics.ascent / scale;
  let descent = -metrics.descent / scale;

  glyphs.extend(run.char_indices().map(|(index, codepoint)| {
    let Ok(offset) = console_core_number_conversion::fitted::<_, u32>(index);
    let Ok(length) = console_core_number_conversion::fitted::<_, u32>(codepoint.len_utf8());
    let start = start_run.saturating_add(offset);
    let Ok(glyph_id) = crate::font::glyph_in(&charmap, codepoint);
    let Ok(advance) = crate::font::advance_in(&glyph_metrics, glyph_id);
    let Ok(attrs) = attrs_list.get_span(start);
    let Ok(cache_key_flags) = override_fake_italic(attrs.cache_key_flags, font, &attrs);

    ShapeGlyph {
      start,
      end: start.saturating_add(length),
      x_advance: advance + attrs.letter_spacing_opt.map_or(0.0, |spacing| spacing.0),
      y_advance: 0.0,
      x_offset: 0.0,
      y_offset: 0.0,
      ascent,
      descent,
      font_monospace_em_width,
      font_id,
      font_weight: attrs.weight,
      glyph_id,
      color_opt: attrs.color_opt,
      metadata: attrs.metadata,
      cache_key_flags,
      metrics_opt: attrs.metrics_opt.map(Into::into),
    }
  }));

  Ok(())
}

fn override_fake_italic(
  cache_key_flags: CacheKeyFlags,
  font: &Font,
  attrs: &Attrs,
) -> Result<CacheKeyFlags, console_core_never::Never> {
  {
    Ok(
      match ::core::primitive::bool::then_some(!font.italic_or_oblique
        && (attrs.style == Style::Italic || attrs.style == Style::Oblique), ())
      {
        ::core::option::Option::Some(()) => cache_key_flags | CacheKeyFlags::FAKE_ITALIC,
        ::core::option::Option::None => cache_key_flags,
      },
    )
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the shaping advances, offsets, ascent and descent are fractions of an em"
  )
)]
pub struct ShapeGlyph {
  pub start: u32,
  pub end: u32,
  pub x_advance: f32,
  pub y_advance: f32,
  pub x_offset: f32,
  pub y_offset: f32,
  pub ascent: f32,
  pub descent: f32,
  pub font_monospace_em_width: Option<f32>,
  pub font_id: fontdb::ID,
  pub font_weight: fontdb::Weight,
  pub glyph_id: u16,
  pub color_opt: Option<Color>,
  pub metadata: u32,
  pub cache_key_flags: CacheKeyFlags,
  pub metrics_opt: Option<Metrics>,
}

impl ShapeGlyph {
  fn adjusted_font_size(
    &self,
    crate::FontSize(font_size): crate::FontSize,
    match_mono_em_width: Option<f32>,
  ) -> Result<f32, console_core_never::Never> {
    Ok(match (match_mono_em_width, self.font_monospace_em_width) {
      (Some(match_em_width), Some(glyph_em_width)) => {
        match ::core::primitive::bool::then_some(<::core::primitive::f32 as ::core::cmp::PartialOrd>::partial_cmp(
          &glyph_em_width,
          &match_em_width,
        ) != ::core::option::Option::Some(::core::cmp::Ordering::Equal), ())
        {
          ::core::option::Option::Some(()) => {
            let glyph_to_match_factor = glyph_em_width / match_em_width;
            let glyph_font_size = glyph_to_match_factor.round().max(1.0)
              / glyph_to_match_factor
              * font_size;
            console_core_logging::trace!(
              "Adjusted glyph font size ({font_size} => {glyph_font_size})"
            );
            glyph_font_size
          }
          ::core::option::Option::None => font_size,
        }
      }
      (Some(_), None) | (None, Some(_)) | (None, None) => font_size,
    })
  }

  const fn layout(
    &self,
    crate::FontSize(font_size): crate::FontSize,
    line_height_opt: Option<f32>,
    x: crate::HorizontalPosition,
    y: crate::VerticalPosition,
    w: crate::GlyphWidth,
    level: unicode_bidi::Level,
  ) -> Result<LayoutGlyph, console_core_never::Never> {
    let crate::HorizontalPosition(x_2) = x;

    let crate::VerticalPosition(y_2) = y;

    let crate::GlyphWidth(w_2) = w;

    {
      Ok(LayoutGlyph {
        start: self.start,
        end: self.end,
        font_size,
        line_height_opt,
        font_id: self.font_id,
        font_weight: self.font_weight,
        glyph_id: self.glyph_id,
        x: x_2,
        y: y_2,
        w: w_2,
        level,
        x_offset: self.x_offset,
        y_offset: self.y_offset,
        color_opt: self.color_opt,
        metadata: self.metadata,
        cache_key_flags: self.cache_key_flags,
      })
    }
  }

  pub fn width(
    &self,
    size: impl Into<crate::FontSize>,
  ) -> Result<f32, console_core_never::Never> {
    let crate::FontSize(font_size) = Into::<crate::FontSize>::into(size);
    Ok(self.metrics_opt.map_or(font_size, |x| x.font_size) * self.x_advance)
  }
}

fn decoration_metrics(
  font: &Font,
) -> Result<(DecorationMetrics, DecorationMetrics, f32), console_core_never::Never> {
  {
    let Ok(metrics) = font.metrics();
    let upem = f32::from(metrics.units_per_em);

    use core::num::FpCategory;

    match upem.classify() {
      FpCategory::Zero => {
        return Ok((
          DecorationMetrics::default(),
          DecorationMetrics::default(),
          0.0,
        ));
      }
      FpCategory::Normal | FpCategory::Subnormal | FpCategory::Infinite | FpCategory::Nan => {}
    }

    Ok((
      DecorationMetrics {
        offset: metrics.underline.map_or(-0.125, |d| d.offset / upem),
        thickness: metrics.underline.map_or(1.0 / 14.0, |d| d.thickness / upem),
      },
      DecorationMetrics {
        offset: metrics.strikeout.map_or(0.3, |d| d.offset / upem),
        thickness: metrics.strikeout.map_or(1.0 / 14.0, |d| d.thickness / upem),
      },
      metrics.ascent / upem,
    ))
  }
}

const ELLIPSIS_SPAN: u32 = u32::MAX;

fn shape_ellipsis(
  font_system: &mut FontSystem,
  attrs: &Attrs,
  shaping: Shaping,
  span_direction: crate::TextDirection,
) -> Result<Vec<ShapeGlyph>, console_core_never::Never> {
  let span_rtl = span_direction == crate::TextDirection::RightToLeft;

  {
    let Ok(attrs_list) = AttrsList::new(attrs);
    let level = match ::core::primitive::bool::then_some(span_rtl, ()) {
      ::core::option::Option::Some(()) => unicode_bidi::Level::RIGHT_TO_LEFT,
      ::core::option::Option::None => unicode_bidi::Level::LEFT_TO_RIGHT,
    };
    let Ok(word) = ShapeWord::new(
      font_system,
      "\u{2026}",
      &attrs_list,
      0..(match console_core_number_conversion::fitted::<_, u32>("\u{2026}".len()) {
        Ok(value) => value,
      }),
      level,
      crate::Whitespace::Text,
      shaping,
    );
    let mut glyphs = word.glyphs;

    match ::core::primitive::bool::then_some(glyphs.is_empty() || glyphs.iter().all(|g| g.glyph_id == 0), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(fallback) = ShapeWord::new(
          font_system,
          "...",
          &attrs_list,
          0..(match console_core_number_conversion::fitted::<_, u32>("...".len()) {
            Ok(value) => value,
          }),
          level,
          crate::Whitespace::Text,
          shaping,
        );
        glyphs = fallback.glyphs;
      }
      ::core::option::Option::None => {}
    }

    Ok(glyphs)
  }
}

#[derive(Clone, Debug)]
pub struct ShapeWord {
  pub whitespace: crate::Whitespace,
  pub glyphs: Vec<ShapeGlyph>,
}

impl ShapeWord {
  pub(crate) fn empty() -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        whitespace: crate::Whitespace::Blank,
        glyphs: Vec::default(),
      })
    }
  }

  pub fn new(
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    word_range: Range<u32>,
    level: unicode_bidi::Level,
    whitespace: crate::Whitespace,
    shaping: Shaping,
  ) -> Result<Self, console_core_never::Never> {
    {
      let Ok(mut empty) = Self::empty();

      match empty.build(
        font_system,
        line,
        attrs_list,
        WordShaping {
          word_range,
          level,
          whitespace,
          shaping,
        },
      ) {
        Ok(value) => value,
      };

      Ok(empty)
    }
  }

  pub fn build(
    &mut self,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    options: WordShaping,
  ) -> Result<(), console_core_never::Never> {
    let WordShaping {
      word_range,
      level,
      whitespace,
      shaping,
    } = options;

    {
      let word = match (line).get({
        let core::ops::Range { start, end } = word_range.clone();
        (match console_core_number_conversion::index(start) {
          Ok(position) => position,
        })..(match console_core_number_conversion::index(end) {
          Ok(position) => position,
        })
      }) {
        Some(value) => value,
        None => return Ok(()),
      };

      console_core_logging::trace!(
        "      Word{}: '{}'",
        match whitespace {
          crate::Whitespace::Blank => " BLANK",
          crate::Whitespace::Text => "",
        },
        word
      );

      let mut glyphs = mem::take(&mut self.glyphs);
      glyphs.clear();

      let Ok(bidi_direction) = level.direction();
      let span_direction = crate::TextDirection::from(bidi_direction);

      let is_simple_ascii =
        word.is_ascii() && !word.chars().any(|c| c.is_ascii_control() && c != '\t');

      match ::core::primitive::bool::then_some(is_simple_ascii && !word.is_empty() && {
        let Ok(attrs_start) = attrs_list.get_span(word_range.start);
        match attrs_list.spans_iter() {
          Ok(value) => value,
        }
        .all(|(other_range, other_attrs)| {
          word_range.end <= other_range.start
            || other_range.end <= word_range.start
            || match attrs_start.compatible(&match other_attrs.as_attrs() {
              Ok(value) => value,
            }) {
              Ok(value) => value,
            } == crate::Compatibility::Compatible
        })
      }, ()) {
        ::core::option::Option::Some(()) => {
          match shaping.run(
            &mut glyphs,
            font_system,
            line,
            attrs_list,
            RunShaping {
              start_run: word_range.start,
              end_run: crate::RunEnd(word_range.end),
              span_direction,
            },
          ) {
            Ok(value) => value,
          };
        }
        ::core::option::Option::None => {
          let Ok(()) = Self::shape_word_runs(&mut glyphs, font_system, line, attrs_list, WordShaping {
            word_range,
            level,
            whitespace,
            shaping,
          });
        }
      }

      self.whitespace = whitespace;
      self.glyphs = glyphs;

      Ok(())
    }
  }

  fn shape_word_runs(
    glyphs: &mut Vec<ShapeGlyph>,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    options: WordShaping,
  ) -> Result<(), console_core_never::Never> {
    let Ok(checked) = text_range(line, options.word_range.clone());
    let word = match checked {
      Some(text) => text,
      None => return Ok(()),
    };
    let Ok(direction) = options.level.direction();
    let span_direction = crate::TextDirection::from(direction);
    let mut start_run = options.word_range.start;
    let Ok(mut attrs) = attrs_list.defaults();
    let Ok(graphemes) = word.grapheme_indices(ClusterKind::Extended);

    for (index, _) in graphemes {
      let Ok(offset) = console_core_number_conversion::fitted::<_, u32>(index);
      let start_egc = options.word_range.start.saturating_add(offset);
      let Ok(next_attrs) = attrs_list.get_span(start_egc);
      let Ok(compatibility) = attrs.compatible(&next_attrs);

      match compatibility {
        crate::Compatibility::Compatible => continue,
        crate::Compatibility::Incompatible => {}
      }

      let Ok(()) = options.shaping.run(
        glyphs,
        font_system,
        line,
        attrs_list,
        RunShaping {
          start_run,
          end_run: crate::RunEnd(start_egc),
          span_direction,
        },
      );
      start_run = start_egc;
      attrs = next_attrs;
    }

    match (start_run < options.word_range.end).then_some(()) {
      Some(()) => options.shaping.run(
        glyphs,
        font_system,
        line,
        attrs_list,
        RunShaping {
          start_run,
          end_run: crate::RunEnd(options.word_range.end),
          span_direction,
        },
      ),
      None => Ok(()),
    }
  }

  pub fn width(
    &self,
    font_size: impl Into<crate::FontSize>,
  ) -> Result<f32, console_core_never::Never> {
    let size = Into::<crate::FontSize>::into(font_size);

    {
      let mut width = 0.0;

      for glyph in &self.glyphs {
        width += match glyph.width(size) {
          Ok(value) => value,
        };
      }

      Ok(width)
    }
  }
}

#[derive(Clone, Debug)]
pub struct ShapeSpan {
  pub level: unicode_bidi::Level,
  pub words: Vec<ShapeWord>,

  pub decoration_spans: Vec<(Range<u32>, GlyphDecorationData)>,
}

enum PunctuationBreak {
  KeepTogether,
  MayBreak,
}

fn punctuation_pair(
  text: &str,
  boundary: u32,
) -> Result<Option<[char; 2]>, console_core_never::Never> {
  let Ok(offset) = console_core_number_conversion::index(boundary);
  let (left, right) = match text.split_at_checked(offset) {
    Some(halves) => halves,
    None => return Ok(None),
  };

  Ok(match (left.chars().next_back(), right.chars().next()) {
    (Some(before), Some(after)) => {
      (before.is_ascii_punctuation() && after.is_ascii_punctuation()).then_some([before, after])
    }
    (None, Some(_)) | (Some(_), None) | (None, None) => None,
  })
}

fn punctuation_break(
  fonts: &mut FontSystem,
  attrs_list: &AttrsList,
  boundary: u32,
  [left, right]: [char; 2],
) -> Result<PunctuationBreak, console_core_never::Never> {
  let probe = format!("{left}{right}");
  let Ok(attrs) = attrs_list.get_span(boundary);
  let Ok(matches) = fonts.get_font_matches(&attrs);
  let families = [&attrs.family];
  let Ok(mut iter) = FontFallbackIter::new(fonts, &matches, &families, &[], &probe, attrs.weight);
  let font = match iter.next() {
    Some(selected) => selected,
    None => return Ok(PunctuationBreak::MayBreak),
  };
  let mut glyphs = Vec::new();
  let Ok(scratch) = iter.shape_caches();
  let Ok(end) = console_core_number_conversion::fitted::<_, u32>(probe.len());
  let _missing = shape_fallback(
    scratch,
    &mut glyphs,
    &font,
    &probe,
    attrs_list,
    FallbackShaping {
      start_run: 0,
      end_run: crate::RunEnd(end),
      span_direction: crate::TextDirection::LeftToRight,
    },
  );

  match glyphs.len().cmp(&probe.chars().count()) {
    core::cmp::Ordering::Less => return Ok(PunctuationBreak::KeepTogether),
    core::cmp::Ordering::Greater => return Ok(PunctuationBreak::MayBreak),
    core::cmp::Ordering::Equal => {}
  }

  let Ok(opened) = font.face();
  let face = match opened {
    Some(parsed) => parsed,
    None => return Ok(PunctuationBreak::KeepTogether),
  };
  let charmap = skrifa::MetadataProvider::charmap(&face);
  let decision = glyphs.iter().zip(probe.chars()).any(|(glyph, character)| {
    let Ok(expected) = crate::font::glyph_in(&charmap, character);
    glyph.glyph_id != expected
  });

  Ok(match decision.then_some(()) {
    Some(()) => PunctuationBreak::KeepTogether,
    None => PunctuationBreak::MayBreak,
  })
}

impl ShapeSpan {
  pub(crate) fn empty() -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        level: unicode_bidi::Level::LEFT_TO_RIGHT,
        words: Vec::default(),
        decoration_spans: Vec::new(),
      })
    }
  }

  pub fn new(
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    span_range: Range<u32>,
    line_direction: crate::TextDirection,
    level: unicode_bidi::Level,
    shaping: Shaping,
  ) -> Result<Self, console_core_never::Never> {
    let line_rtl = line_direction == crate::TextDirection::RightToLeft;

    {
      let Ok(mut empty) = Self::empty();

      match empty.build(
        font_system,
        line,
        attrs_list,
        SpanShaping {
          span_range,
          line_direction: match ::core::primitive::bool::then_some(line_rtl, ()) {
            ::core::option::Option::Some(()) => crate::TextDirection::RightToLeft,
            ::core::option::Option::None => crate::TextDirection::LeftToRight,
          },
          level,
          shaping,
        },
      ) {
        Ok(value) => value,
      };

      Ok(empty)
    }
  }

  pub fn build(
    &mut self,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    options: SpanShaping,
  ) -> Result<(), console_core_never::Never> {
    let Ok(checked_range) = text_range(line, options.span_range.clone());
    let span = match checked_range {
      Some(text) => text,
      None => return Ok(()),
    };
    let Ok(direction) = options.level.direction();
    console_core_logging::trace!(
      "  Span {}: '{}'",
      match direction {
        unicode_bidi::Direction::RightToLeft => "RTL",
        unicode_bidi::Direction::LeftToRight => "LTR",
      },
      span
    );

    let mut cached_words = mem::take(&mut font_system.shape_buffer.words);
    cached_words.clear();
    let reverse_words = options.line_direction != crate::TextDirection::from(direction);

    match reverse_words.then_some(()) {
      Some(()) => cached_words.append(&mut self.words),
      None => cached_words.extend(self.words.drain(..).rev()),
    }

    let Ok(()) = self.shape_words(font_system, line, attrs_list, &options, &mut cached_words);

    match options.line_direction {
      crate::TextDirection::RightToLeft => {
        for word in &mut self.words {
          word.glyphs.reverse();
        }
      }
      crate::TextDirection::LeftToRight => {}
    }

    match reverse_words.then_some(()) {
      Some(()) => self.words.reverse(),
      None => {}
    }

    self.level = options.level;
    let Ok(()) =
      self.build_decorations(font_system, attrs_list, options.span_range, cached_words);
    Ok(())
  }

  fn shape_words(
    &mut self,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    options: &SpanShaping,
    cached_words: &mut Vec<ShapeWord>,
  ) -> Result<(), console_core_never::Never> {
    let Ok(selected) = text_range(line, options.span_range.clone());
    let span = match selected {
      Some(text) => text,
      None => return Ok(()),
    };
    let mut start_word = 0;
    let Ok(breaks) = console_core_line_breaking::linebreaks(span);

    for (end_word, _) in breaks {
      let Ok(end_offset) = console_core_number_conversion::fitted::<_, u32>(end_word);
      let boundary = options.span_range.start.saturating_add(end_offset);

      let Ok(pair) = punctuation_pair(span, end_offset);

      match pair {
        Some([left, right]) => {
          let Ok(decision) = punctuation_break(font_system, attrs_list, boundary, [left, right]);

          match decision {
            PunctuationBreak::KeepTogether => continue,
            PunctuationBreak::MayBreak => {}
          }
        }
        None => {}
      }

      let Ok(checked_range) = text_range(span, start_word..end_offset);
      let section = match checked_range {
        Some(text) => text,
        None => continue,
      };
      let content = section.trim_end_matches(char::is_whitespace);
      let Ok(content_length) = console_core_number_conversion::fitted::<_, u32>(content.len());
      let absolute_start = options.span_range.start.saturating_add(start_word);
      let end_text = absolute_start.saturating_add(content_length);

      match (start_word < start_word.saturating_add(content_length)).then_some(()) {
        Some(()) => {
          let Ok(()) = self.push_word(
            font_system,
            line,
            attrs_list,
            cached_words,
            WordShaping {
              word_range: absolute_start..end_text,
              level: options.level,
              whitespace: crate::Whitespace::Text,
              shaping: options.shaping,
            },
          );
        }
        None => {}
      }

      let blanks = match section.get(content.len()..) {
        Some(remainder) => remainder,
        None => continue,
      };

      for (index, character) in blanks.char_indices() {
        let Ok(offset) = console_core_number_conversion::fitted::<_, u32>(index);
        let Ok(length) = console_core_number_conversion::fitted::<_, u32>(character.len_utf8());
        let start = end_text.saturating_add(offset);
        let Ok(()) = self.push_word(
          font_system,
          line,
          attrs_list,
          cached_words,
          WordShaping {
            word_range: start..start.saturating_add(length),
            level: options.level,
            whitespace: crate::Whitespace::Blank,
            shaping: options.shaping,
          },
        );
      }

      start_word = end_offset;
    }

    Ok(())
  }

  fn push_word(
    &mut self,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    cached_words: &mut Vec<ShapeWord>,
    options: WordShaping,
  ) -> Result<(), console_core_never::Never> {
    let mut word = match cached_words.pop() {
      Some(recycled) => recycled,
      None => {
        let Ok(empty) = ShapeWord::empty();
        empty
      }
    };
    let Ok(()) = word.build(font_system, line, attrs_list, options);
    self.words.push(word);
    Ok(())
  }

  fn build_decorations(
    &mut self,
    font_system: &mut FontSystem,
    attrs_list: &AttrsList,
    span_range: Range<u32>,
    cached_words: Vec<ShapeWord>,
  ) -> Result<(), console_core_never::Never> {
    self.decoration_spans.clear();
    font_system.shape_buffer.words = cached_words;
    let Ok(default_attrs) = attrs_list.defaults();
    let Ok(default_decoration) = default_attrs.text_decoration.has_decoration();
    let Ok(mut attrs) = attrs_list.spans_iter();
    let decorated = default_decoration == crate::Decoration::Decorated
      || attrs.any(|(range, owned)| {
        let Ok(span_attrs) = owned.as_attrs();
        let Ok(decoration) = span_attrs.text_decoration.has_decoration();
        range.start.max(span_range.start) < range.end.min(span_range.end)
          && decoration == crate::Decoration::Decorated
      });

    match decorated.then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let glyph = match self.words.iter().find_map(|word| word.glyphs.first()) {
      Some(primary) => primary,
      None => return Ok(()),
    };
    let Ok(font) = font_system.get_font(glyph.font_id, glyph.font_weight);
    let primary_font = match font {
      Some(present) => present,
      None => return Ok(()),
    };
    let Ok((underline_metrics, strikethrough_metrics, ascent)) =
      decoration_metrics(&primary_font);
    let basis = GlyphDecorationData {
      text_decoration: default_attrs.text_decoration,
      underline_metrics,
      strikethrough_metrics,
      ascent,
    };
    let mut covered_end = span_range.start;
    let Ok(spans) = attrs_list.spans_iter();

    for (range, owned) in spans {
      let start = range.start.max(span_range.start);
      let end = range.end.min(span_range.end);

      match (start < end).then_some(()) {
        Some(()) => {}
        None => continue,
      }

      let Ok(()) = self.push_decoration(covered_end..start, basis.clone());
      let Ok(span_attrs) = owned.as_attrs();
      let Ok(()) = self.push_decoration(
        start..end,
        GlyphDecorationData {
          text_decoration: span_attrs.text_decoration,
          ..basis.clone()
        },
      );
      covered_end = end;
    }

    self.push_decoration(covered_end..span_range.end, basis)
  }

  fn push_decoration(
    &mut self,
    range: Range<u32>,
    data: GlyphDecorationData,
  ) -> Result<(), console_core_never::Never> {
    let Ok(decoration) = data.text_decoration.has_decoration();

    match (!range.is_empty() && decoration == crate::Decoration::Decorated).then_some(()) {
      Some(()) => self.decoration_spans.push((range, data)),
      None => {}
    }

    Ok(())
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "Paragraph direction is independent of optional metrics overrides and the cached ellipsis span; either direction supports every cache and metrics combination."
  )
)]
pub struct ShapeLine {
  pub direction: crate::TextDirection,
  pub spans: Vec<ShapeSpan>,
  pub metrics_opt: Option<Metrics>,
  ellipsis_span: Option<ShapeSpan>,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct WordGlyphPos {
  word: u32,
  glyph: u32,
}

impl WordGlyphPos {
  const ZERO: Self = Self { word: 0, glyph: 0 };

  fn new(word: u32, glyph: crate::GlyphIndex) -> Result<Self, console_core_never::Never> {
    let crate::GlyphIndex(glyph_2) = glyph;

    {
      Ok(Self {
        word,
        glyph: glyph_2,
      })
    }
  }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct SpanWordGlyphPos {
  span: u32,
  word: u32,
  glyph: u32,
}

impl SpanWordGlyphPos {
  const ZERO: Self = Self {
    span: 0,
    word: 0,
    glyph: 0,
  };

  fn word_glyph_pos(&self) -> Result<WordGlyphPos, console_core_never::Never> {
    {
      Ok(WordGlyphPos {
        word: self.word,
        glyph: self.glyph,
      })
    }
  }

  fn with_wordglyph(
    span: u32,
    wordglyph: WordGlyphPos,
  ) -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        span,
        word: wordglyph.word,
        glyph: wordglyph.glyph,
      })
    }
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum LayoutDirection {
  Forward,
  Backward,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct VlRange {
  span: u32,
  start: WordGlyphPos,
  end: WordGlyphPos,
  level: unicode_bidi::Level,
}

impl Default for VlRange {
  fn default() -> Self {
    Self {
      span: Default::default(),
      start: Default::default(),
      end: Default::default(),
      level: unicode_bidi::Level::LEFT_TO_RIGHT,
    }
  }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
enum Elision {
  #[default]
  Full,
  Pending,
  Known((u32, u32)),
}

#[derive(Default, Debug)]
struct VisualLine {
  ranges: Vec<VlRange>,
  spaces: u32,
  w: crate::LineWidth,
  elision: Elision,
}

impl VisualLine {
  fn clear(&mut self) -> Result<(), console_core_never::Never> {
    {
      self.ranges.clear();
      self.spaces = 0;
      self.w = crate::LineWidth(0.);
      self.elision = Elision::Full;

      Ok(())
    }
  }
}

impl Default for ShapeLine {
  fn default() -> Self {
    let Ok(line) = Self::empty();

    line
  }
}

impl ShapeLine {
  pub(crate) fn empty() -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        direction: crate::TextDirection::LeftToRight,
        spans: Vec::default(),
        metrics_opt: None,
        ellipsis_span: None,
      })
    }
  }

  pub fn new(
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    shaping: Shaping,
    tab_width: u16,
  ) -> Result<Self, console_core_never::Never> {
    {
      let Ok(mut empty) = Self::empty();

      match empty.build(font_system, line, attrs_list, shaping, tab_width) {
        Ok(value) => value,
      };

      Ok(empty)
    }
  }

  pub fn build(
    &mut self,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    shaping: Shaping,
    tab_width: u16,
  ) -> Result<(), console_core_never::Never> {
    self.ellipsis_span = None;
    let mut cached_spans = mem::take(&mut font_system.shape_buffer.spans);
    cached_spans.clear();
    cached_spans.extend(self.spans.drain(..).rev());
    let Ok(bidi) = unicode_bidi::BidiInfo::new(line, None);
    let Ok(paragraphs) = bidi.paragraphs();
    let direction = match paragraphs.first() {
      Some(paragraph) => {
        let Ok(paragraph_direction) = paragraph.level.direction();
        crate::TextDirection::from(paragraph_direction)
      }
      None => crate::TextDirection::LeftToRight,
    };

    console_core_logging::trace!(
      "Line {}: '{}'",
      match direction {
        crate::TextDirection::RightToLeft => "RTL",
        crate::TextDirection::LeftToRight => "LTR",
      },
      line
    );

    for para_info in paragraphs {
      let Ok(paragraph_direction) = para_info.level.direction();
      let line_direction = crate::TextDirection::from(paragraph_direction);
      assert_eq!(line_direction, direction);
      let Ok(paragraph) = unicode_bidi::Paragraph::new(&bidi, para_info);
      let Ok(()) = self.append_paragraph(
        font_system,
        line,
        attrs_list,
        &mut cached_spans,
        &paragraph,
        shaping,
      );
    }

    let Ok(()) = self.expand_tabs(line, tab_width);
    let Ok(defaults) = attrs_list.defaults();
    self.direction = direction;
    self.metrics_opt = defaults.metrics_opt.map(Into::into);
    let Ok(ellipsis) = Self::ellipsis_for_line(font_system, attrs_list, shaping, direction);
    self.ellipsis_span = Some(ellipsis);
    font_system.shape_buffer.spans = cached_spans;

    Ok(())
  }

  fn append_paragraph(
    &mut self,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    cached_spans: &mut Vec<ShapeSpan>,
    paragraph: &unicode_bidi::Paragraph<'_, '_>,
    shaping: Shaping,
  ) -> Result<(), console_core_never::Never> {
    let Ok((_, info)) = paragraph.details();
    let Ok(direction) = info.level.direction();
    let line_direction = crate::TextDirection::from(direction);
    let Ok(levels) = Self::adjust_levels(paragraph);
    let range = info.range.clone();
    let Ok(first) = console_core_number_conversion::index(range.start);
    let Ok(last) = console_core_number_conversion::index(range.end);
    let paragraph_levels = match levels.get(first..last) {
      Some(present) => present,
      None => return Ok(()),
    };
    let mut changes = paragraph_levels.iter().copied().enumerate();
    let mut level = match changes.next() {
      Some((_, first_2)) => first_2,
      None => return Ok(()),
    };
    let base = range.start;
    let end = range.end;
    let mut start = base;

    for (offset, next_level) in changes {
      match (level != next_level).then_some(()) {
        Some(()) => {}
        None => continue,
      }

      let Ok(position) = console_core_number_conversion::fitted::<_, u32>(offset);
      let boundary = base.saturating_add(position);
      let Ok(()) = self.push_span(
        font_system,
        line,
        attrs_list,
        cached_spans,
        SpanShaping {
          span_range: start..boundary,
          line_direction,
          level,
          shaping,
        },
      );
      start = boundary;
      level = next_level;
    }

    self.push_span(
      font_system,
      line,
      attrs_list,
      cached_spans,
      SpanShaping {
        span_range: start..end,
        line_direction,
        level,
        shaping,
      },
    )
  }

  fn push_span(
    &mut self,
    font_system: &mut FontSystem,
    line: &str,
    attrs_list: &AttrsList,
    cached_spans: &mut Vec<ShapeSpan>,
    options: SpanShaping,
  ) -> Result<(), console_core_never::Never> {
    let mut span = match cached_spans.pop() {
      Some(recycled) => recycled,
      None => {
        let Ok(empty) = ShapeSpan::empty();
        empty
      }
    };
    let Ok(()) = span.build(font_system, line, attrs_list, options);
    self.spans.push(span);
    Ok(())
  }

  fn expand_tabs(&mut self, line: &str, tab_width: u16) -> Result<(), console_core_never::Never> {
    let mut x = 0.0;
    let glyphs = self
      .spans
      .iter_mut()
      .flat_map(|span| &mut span.words)
      .flat_map(|word| &mut word.glyphs);

    for glyph in glyphs {
      let Ok(cluster) = text_range(line, glyph.start..glyph.end);

      match cluster {
        Some("\t") => {
          let step = f32::from(tab_width) * glyph.x_advance;
          let next_stop = ((x / step).floor() + 1.0) * step;
          glyph.x_advance = next_stop - x;
        }
        Some(_) | None => {}
      }

      x += glyph.x_advance;
    }

    Ok(())
  }

  fn ellipsis_for_line(
    font_system: &mut FontSystem,
    attrs_list: &AttrsList,
    shaping: Shaping,
    direction: crate::TextDirection,
  ) -> Result<ShapeSpan, console_core_never::Never> {
    let Ok(attrs) = match attrs_list.spans.is_empty().then_some(()) {
      Some(()) => attrs_list.defaults(),
      None => attrs_list.get_span(0),
    };
    let Ok(mut glyphs) = shape_ellipsis(font_system, &attrs, shaping, direction);
    let level = match direction {
      crate::TextDirection::RightToLeft => {
        glyphs.reverse();
        unicode_bidi::Level::RIGHT_TO_LEFT
      }
      crate::TextDirection::LeftToRight => unicode_bidi::Level::LEFT_TO_RIGHT,
    };

    Ok(ShapeSpan {
      level,
      words: vec![ShapeWord {
        whitespace: crate::Whitespace::Text,
        glyphs,
      }],
      decoration_spans: Vec::new(),
    })
  }

  fn adjust_levels(
    para: &unicode_bidi::Paragraph,
  ) -> Result<Vec<unicode_bidi::Level>, console_core_never::Never> {
    use unicode_bidi::BidiClass::{
      AL, AN, B, BN, CS, EN, ES, ET, FSI, L, LRE, LRI, LRO, NSM, ON, PDF, PDI, R, RLE, RLI, RLO,
      S, WS,
    };
    let Ok((info, paragraph)) = para.details();
    let Ok(text) = info.text();
    let Ok(embedding) = info.levels();
    let Ok(line_classes) = info.original_classes();
    let mut levels = embedding.to_vec();
    let line_levels = levels.as_mut_slice();

    let mut reset_from: Option<u32> = Some(0);
    let mut reset_to: Option<u32> = None;

    'text_walk_16: for (i, c) in text.char_indices() {
      let class = match line_classes.get(i) {
        Some(value) => value,
        None => continue 'text_walk_16,
      };
      let Ok(position) = console_core_number_conversion::fitted::<_, u32>(i);
      let Ok(length) = console_core_number_conversion::fitted::<_, u32>(c.len_utf8());

      match *class {
        RLE | LRE | RLO | LRO | PDF | BN => {}

        B | S => {
          assert_eq!(reset_to, None);
          reset_to = Some(position.saturating_add(length));

          match reset_from {
            ::core::option::Option::None => {
              reset_from = Some(position);
            }
            ::core::option::Option::Some(_) => {}
          }
        }

        WS | FSI | LRI | RLI | PDI => match reset_from {
          ::core::option::Option::None => {
            reset_from = Some(position);
          }
          ::core::option::Option::Some(_) => {}
        },
        L | R | AL | EN | ES | ET | AN | CS | NSM | ON => {
          reset_from = None;
        }
      }

      match (reset_from, reset_to) {
        (Some(from), Some(to)) => {
          let Ok(start) = console_core_number_conversion::index(from);
          let Ok(end) = console_core_number_conversion::index(to);
          let affected = match line_levels.get_mut(start..end) {
            Some(slice) => slice,
            None => continue 'text_walk_16,
          };
          affected.fill(paragraph.level);

          reset_from = None;
          reset_to = None;
        }
        _ => {}
      }
    }

    match reset_from {
      Some(from) => {
        for level in &mut (*(match (line_levels).get_mut(
          (match console_core_number_conversion::index(from) {
            Ok(position) => position,
          })..,
        ) {
          Some(value) => value,
          None => return Ok(Vec::new()),
        })) {
          *level = paragraph.level;
        }
      }
      None => {}
    }

    Ok(levels)
  }

  fn reorder(&self, ranges: &[VlRange]) -> Result<Vec<Range<u32>>, console_core_never::Never> {
    let levels: Vec<_> = ranges.iter().map(|range| range.level).collect();
    let Ok(order) = unicode_bidi::BidiInfo::reorder_visual(&levels);

    Ok(
      order
        .into_iter()
        .map(|start| start..start.saturating_add(1))
        .collect(),
    )
  }

  pub fn layout(
    &self,
    size: impl Into<crate::FontSize>,
    width_opt: Option<f32>,
    wrap: Wrap,
    align: Option<Align>,
    match_mono_width: Option<f32>,
    hinting: Hinting,
  ) -> Result<Vec<LayoutLine>, console_core_never::Never> {
    let crate::FontSize(font_size) = Into::<crate::FontSize>::into(size);

    {
      let mut lines = Vec::with_capacity(1);
      let mut scratch = ShapeBuffer::default();

      match self.layout_to_buffer(
        &mut scratch,
        BufferLayout {
          font_size,
          width_opt,
          wrap,
          ellipsize: Ellipsize::None,
          align,
          match_mono_width,
          hinting,
        },
        &mut lines,
      ) {
        Ok(value) => value,
      };

      Ok(lines)
    }
  }

  fn get_glyph_start_end(
    word: &ShapeWord,
    start: SpanWordGlyphPos,
    span_index: u32,
    word_idx: crate::WordIndex,
    _direction: LayoutDirection,
    congruence: crate::RunAlignment,
  ) -> Result<(u32, u32), console_core_never::Never> {
    let crate::WordIndex(word_idx_2) = word_idx;

    let congruent = congruence == crate::RunAlignment::Congruent;

    {
      match ::core::primitive::bool::then_some(span_index != start.span || word_idx_2 != start.word, ()) {
        ::core::option::Option::Some(()) => {
          return Ok((
            0,
            (match console_core_number_conversion::fitted::<_, u32>(word.glyphs.len()) {
              Ok(value) => value,
            }),
          ));
        }
        ::core::option::Option::None => {}
      }

      let (start_glyph_pos, end_glyph_pos) = match ::core::primitive::bool::then_some(congruent, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(length) = console_core_number_conversion::fitted::<_, u32>(word.glyphs.len());

          (start.glyph, length)
        }
        ::core::option::Option::None => (0, start.glyph),
      };
      Ok((start_glyph_pos, end_glyph_pos))
    }
  }

  fn fit_glyphs(
    word: &ShapeWord,
    options: GlyphFitting,
  ) -> Result<(u32, f32), console_core_never::Never> {
    let Ok((start, end)) = Self::get_glyph_start_end(
      word,
      options.start,
      options.span_index,
      options.word_idx,
      options.direction,
      options.congruence,
    );

    match options.traversal {
      crate::SearchDirection::Forward => {
        Self::fit_glyph_range(word, &options, start..end, crate::GlyphIndex(start))
      }
      crate::SearchDirection::Backward => {
        let Ok(length) = console_core_number_conversion::fitted::<_, u32>(word.glyphs.len());
        Self::fit_glyph_range(
          word,
          &options,
          (start..end).rev(),
          crate::GlyphIndex(length),
        )
      }
    }
  }

  fn fit_glyph_range(
    word: &ShapeWord,
    options: &GlyphFitting,
    indices: impl Iterator<Item = u32>,
    crate::GlyphIndex(mut boundary): crate::GlyphIndex,
  ) -> Result<(u32, f32), console_core_never::Never> {
    let mut width = 0.0;

    'glyphs: for glyph_index in indices {
      let Ok(index) = console_core_number_conversion::index(glyph_index);
      let glyph = match word.glyphs.get(index) {
        Some(present) => present,
        None => continue 'glyphs,
      };
      let Ok(advance) = glyph.width(options.font_size);

      match (options.currently_used_width.0 + width + advance > options.total_available_width.0)
        .then_some(())
      {
        Some(()) => break 'glyphs,
        None => {}
      }

      width += advance;
      boundary = match options.traversal {
        crate::SearchDirection::Forward => glyph_index.saturating_add(1),
        crate::SearchDirection::Backward => glyph_index,
      };
    }

    Ok((boundary, width))
  }

  fn add_to_visual_line(
    &self,
    vl: &mut VisualLine,
    span_index: u32,
    start: WordGlyphPos,
    end: WordGlyphPos,
    width: impl Into<crate::UsedWidth>,
    number_of_blanks: crate::BlankCount,
  ) -> Result<(), console_core_never::Never> {
    let crate::UsedWidth(added_width) = Into::<crate::UsedWidth>::into(width);
    let crate::BlankCount(number_of_blanks_2) = number_of_blanks;

    {
      match ::core::primitive::bool::then_some(end == start, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      vl.ranges.push(VlRange {
        span: span_index,
        start,
        end,
        level:
          ((match (self.spans).get(match console_core_number_conversion::index(span_index) {
            Ok(position) => position,
          }) {
            Some(value) => value,
            None => return Ok(()),
          })
          .level),
      });
      vl.w.0 += added_width;
      vl.spaces = (vl.spaces).saturating_add(number_of_blanks_2);

      Ok(())
    }
  }

  fn remaining_content_exceeds(
    spans: &[ShapeSpan],
    options: RemainingContent,
  ) -> Result<crate::Overflow, console_core_never::Never> {
    let RemainingContent {
      font_size: crate::FontSize(font_size),
      span_index,
      word_idx,
      word_count,
      starting_word_index,
      direction,
      congruence,
      start_span,
      span_count,
      threshold,
    } = options;

    let crate::WordIndex(word_idx_2) = word_idx;

    let crate::WordCount(word_count_2) = word_count;

    let crate::FirstWordIndex(starting_word_index_2) = starting_word_index;

    let crate::FirstSpanIndex(start_span_2) = start_span;

    let crate::SpanCount(span_count_2) = span_count;

    let crate::WidthThreshold(threshold_2) = threshold;

    let congruent = congruence == crate::RunAlignment::Congruent;

    {
      let mut acc: f32 = 0.0;

      let word_range: Range<u32> = match (direction, congruent) {
        (LayoutDirection::Forward, true) => u32::saturating_add(word_idx_2, 1)..word_count_2,
        (LayoutDirection::Forward, false) => 0..word_idx_2,
        (LayoutDirection::Backward, true) => starting_word_index_2..word_idx_2,
        (LayoutDirection::Backward, false) => u32::saturating_add(word_idx_2, 1)..word_count_2,
      };

      let Ok(current_span_index) = console_core_number_conversion::index(span_index);
      let current_words: &[ShapeWord] = spans
        .get(current_span_index)
        .map_or(&[], |span| span.words.as_slice());

      for wi in word_range {
        let Ok(index) = console_core_number_conversion::index(wi);
        let word = match current_words.get(index) {
          Some(present) => present,
          None => continue,
        };
        let Ok(width) = word.width(font_size);
        acc += width;

        match ::core::primitive::bool::then_some(acc > threshold_2, ()) {
          ::core::option::Option::Some(()) => {
            return Ok(crate::Overflow::Exceeds);
          }
          ::core::option::Option::None => {}
        }
      }

      let span_range: Range<u32> = match direction {
        LayoutDirection::Forward => u32::saturating_add(span_index, 1)..span_count_2,
        LayoutDirection::Backward => start_span_2..span_index,
      };

      'text_walk_22: for si in span_range {
        for w in &((match (spans).get(match console_core_number_conversion::index(si) {
          Ok(position) => position,
        }) {
          Some(value) => value,
          None => continue 'text_walk_22,
        })
        .words)
        {
          acc += match w.width(font_size) {
            Ok(value) => value,
          };

          match ::core::primitive::bool::then_some(acc > threshold_2, ()) {
            ::core::option::Option::Some(()) => {
              return Ok(crate::Overflow::Exceeds);
            }
            ::core::option::Option::None => {}
          }
        }
      }

      Ok(crate::Overflow::Within)
    }
  }

  fn layout_spans(
    &self,
    line: &mut VisualLine,
    options: SpanLayout,
    spans: &[ShapeSpan],
  ) -> Result<(), console_core_never::Never> {
    let Ok(span_count) = console_core_number_conversion::fitted::<_, u32>(spans.len());
    let start = options
      .start_opt
      .map_or(SpanWordGlyphPos::ZERO, |position| position);
    let first = start.span.min(span_count);
    let Ok(skip) = console_core_number_conversion::index(first);
    let order = (first..span_count).zip(spans.iter().skip(skip));
    let scan = EllipsisScan {
      options,
      start,
      spans,
      span_count,
    };

    match scan.options.direction {
      LayoutDirection::Forward => {
        let Ok(()) = self.fill_visual_line(line, &scan, order);
      }
      LayoutDirection::Backward => {
        let Ok(()) = self.fill_visual_line(line, &scan, order.rev());
        line.ranges.reverse();
      }
    }

    Ok(())
  }

  fn fill_visual_line<'a>(
    &self,
    line: &mut VisualLine,
    scan: &EllipsisScan<'_>,
    spans: impl Iterator<Item = (u32, &'a ShapeSpan)>,
  ) -> Result<(), console_core_never::Never> {
    let mut total_width = crate::UsedWidth(0.0);

    for (index, span) in spans {
      let Ok(direction) = span.level.direction();
      let congruence = match (scan.options.text_direction
        == crate::TextDirection::from(direction))
      .then_some(())
      {
        Some(()) => crate::RunAlignment::Congruent,
        None => crate::RunAlignment::Incongruent,
      };
      let traversal = match (scan.options.direction, congruence) {
        (LayoutDirection::Forward, crate::RunAlignment::Congruent)
        | (LayoutDirection::Backward, crate::RunAlignment::Incongruent) => {
          crate::SearchDirection::Forward
        }
        (LayoutDirection::Forward, crate::RunAlignment::Incongruent)
        | (LayoutDirection::Backward, crate::RunAlignment::Congruent) => {
          crate::SearchDirection::Backward
        }
      };
      let Ok(count) = console_core_number_conversion::fitted::<_, u32>(span.words.len());
      let Ok(word_end) = WordGlyphPos::new(count, crate::GlyphIndex(0));
      let active = EllipsisSpan {
        index,
        words: &span.words,
        count: crate::WordCount(count),
        congruence,
        traversal,
        start: scan
          .options
          .start_opt
          .filter(|position| position.span == index),
        end: word_end,
      };
      let Ok(progress) = self.fill_span(line, scan, &active, &mut total_width);

      match progress {
        SpanProgress::Complete => return Ok(()),
        SpanProgress::Continue => {}
      }
    }

    Ok(())
  }

  fn fill_span(
    &self,
    line: &mut VisualLine,
    scan: &EllipsisScan<'_>,
    span: &EllipsisSpan<'_>,
    total_width: &mut crate::UsedWidth,
  ) -> Result<SpanProgress, console_core_never::Never> {
    let mut width = crate::UsedWidth(0.0);
    let mut blanks = crate::BlankCount(0);
    let Ok(indices) = Self::span_word_indices(
      scan.options.direction,
      span.congruence,
      span.start,
      span.count,
    );

    for index in indices {
      let Ok(offset) = console_core_number_conversion::index(index);
      let word = match span.words.get(offset) {
        Some(shaped) => shaped,
        None => continue,
      };
      let position = SpanWordGlyphPos {
        span: span.index,
        word: index,
        glyph: 0,
      };
      let Ok(word_width) =
        self.span_word_width(word, &scan.options, position, scan.start, span.congruence);
      let used_width = crate::UsedWidth(total_width.0 + width.0);
      let Ok(overflow) = self.ellipsis_overflow(
        scan,
        span,
        crate::WordIndex(index),
        crate::UsedWidth(used_width.0 + word_width),
      );

      match overflow {
        crate::Overflow::Exceeds => {
          let limit = scan
            .options
            .width_opt
            .map_or(f32::INFINITY, |maximum| maximum);
          let fitting = GlyphFitting {
            font_size: scan.options.font_size,
            start: scan.start,
            span_index: span.index,
            word_idx: crate::WordIndex(index),
            direction: scan.options.direction,
            congruence: span.congruence,
            currently_used_width: used_width,
            total_available_width: crate::AvailableWidth(
              (limit - scan.options.ellipsis_w.0).max(0.0),
            ),
            traversal: span.traversal,
          };
          let Ok(()) = self.append_fitted_span(line, word, span, fitting, width, blanks);
          return Ok(SpanProgress::Complete);
        }
        crate::Overflow::Within => {}
      }

      width.0 += word_width;
      blanks.0 = blanks
        .0
        .saturating_add(u32::from(word.whitespace == crate::Whitespace::Blank));

      match (scan.options.direction == LayoutDirection::Backward
        && position.span == scan.start.span
        && position.word == scan.start.word)
        .then_some(())
      {
        Some(()) => {
          let Ok(start) = scan.start.word_glyph_pos();
          let (first, end) = match span.traversal {
            crate::SearchDirection::Forward => (WordGlyphPos::ZERO, start),
            crate::SearchDirection::Backward => (start, span.end),
          };
          let Ok(()) = self.add_to_visual_line(line, span.index, first, end, width, blanks);
          return Ok(SpanProgress::Complete);
        }
        None => {}
      }
    }

    total_width.0 += width.0;
    let Ok(word_start) = scan.start.word_glyph_pos();
    let (first, end) = match (span.index == scan.start.span, span.congruence) {
      (true, crate::RunAlignment::Congruent) => (word_start, span.end),
      (true, crate::RunAlignment::Incongruent) => (WordGlyphPos::ZERO, word_start),
      (false, crate::RunAlignment::Congruent | crate::RunAlignment::Incongruent) => {
        (WordGlyphPos::ZERO, span.end)
      }
    };
    let Ok(()) = self.add_to_visual_line(line, span.index, first, end, width, blanks);
    Ok(SpanProgress::Continue)
  }

  fn ellipsis_overflow(
    &self,
    scan: &EllipsisScan<'_>,
    span: &EllipsisSpan<'_>,
    word: crate::WordIndex,
    width: crate::UsedWidth,
  ) -> Result<crate::Overflow, console_core_never::Never> {
    let limit = match scan.options.width_opt.filter(|maximum| maximum.is_finite()) {
      Some(maximum) => maximum,
      None => return Ok(crate::Overflow::Within),
    };

    match scan.options.ellipsize {
      Ellipsize::Start(_) | Ellipsize::End(_) => {}
      Ellipsize::None | Ellipsize::Middle(_) => return Ok(crate::Overflow::Within),
    }

    match (width.0 > limit).then_some(()) {
      Some(()) => return Ok(crate::Overflow::Exceeds),
      None => {}
    }

    let Ok(remaining) = Self::remaining_content_exceeds(
      scan.spans,
      RemainingContent {
        font_size: scan.options.font_size,
        span_index: span.index,
        word_idx: word,
        word_count: span.count,
        starting_word_index: crate::FirstWordIndex(
          span.start.map_or(0, |position| position.word),
        ),
        direction: scan.options.direction,
        congruence: span.congruence,
        start_span: crate::FirstSpanIndex(scan.start.span),
        span_count: crate::SpanCount(scan.span_count),
        threshold: crate::WidthThreshold(scan.options.ellipsis_w.0),
      },
    );

    Ok(
      match (remaining == crate::Overflow::Exceeds && width.0 + scan.options.ellipsis_w.0 > limit)
        .then_some(())
      {
        Some(()) => crate::Overflow::Exceeds,
        None => crate::Overflow::Within,
      },
    )
  }

  fn append_fitted_span(
    &self,
    line: &mut VisualLine,
    word: &ShapeWord,
    span: &EllipsisSpan<'_>,
    fitting: GlyphFitting,
    width: crate::UsedWidth,
    blanks: crate::BlankCount,
  ) -> Result<(), console_core_never::Never> {
    let start = fitting.start;
    let word_index = fitting.word_idx;
    let Ok((glyph_end, glyph_width)) = Self::fit_glyphs(word, fitting);
    let Ok(edge) = WordGlyphPos::new(word_index.0, crate::GlyphIndex(glyph_end));
    let Ok(word_start) = start.word_glyph_pos();
    let (first, end) = match (span.traversal, span.index == start.span, span.congruence) {
      (crate::SearchDirection::Forward, true, crate::RunAlignment::Congruent) => {
        (word_start, edge)
      }
      (crate::SearchDirection::Forward, true, crate::RunAlignment::Incongruent)
      | (
        crate::SearchDirection::Forward,
        false,
        crate::RunAlignment::Congruent | crate::RunAlignment::Incongruent,
      ) => (WordGlyphPos::ZERO, edge),
      (crate::SearchDirection::Backward, true, crate::RunAlignment::Incongruent) => {
        (edge, word_start)
      }
      (crate::SearchDirection::Backward, true, crate::RunAlignment::Congruent)
      | (
        crate::SearchDirection::Backward,
        false,
        crate::RunAlignment::Congruent | crate::RunAlignment::Incongruent,
      ) => (edge, span.end),
    };
    let Ok(()) = self.add_to_visual_line(
      line,
      span.index,
      first,
      end,
      crate::UsedWidth(width.0 + glyph_width),
      blanks,
    );
    line.elision = Elision::Pending;
    Ok(())
  }

  fn span_word_indices(
    direction: LayoutDirection,
    congruence: crate::RunAlignment,
    start: Option<SpanWordGlyphPos>,
    crate::WordCount(count): crate::WordCount,
  ) -> Result<Vec<u32>, console_core_never::Never> {
    let indices = match (direction, congruence) {
      (LayoutDirection::Forward, crate::RunAlignment::Congruent) => {
        (start.map_or(0, |position| position.word)..count).collect()
      }
      (LayoutDirection::Forward, crate::RunAlignment::Incongruent) => (0..start
        .map_or(count, |position| position.word))
        .rev()
        .collect(),
      (LayoutDirection::Backward, crate::RunAlignment::Congruent) => {
        (start.map_or(0, |position| position.word)..count)
          .rev()
          .collect()
      }
      (LayoutDirection::Backward, crate::RunAlignment::Incongruent) => (0..start
        .map_or(count, |position| {
          position.word.saturating_add(u32::from(position.glyph > 0))
        }))
        .collect(),
    };
    Ok(indices)
  }

  fn span_word_width(
    &self,
    word: &ShapeWord,
    options: &SpanLayout,
    position: SpanWordGlyphPos,
    start: SpanWordGlyphPos,
    congruence: crate::RunAlignment,
  ) -> Result<f32, console_core_never::Never> {
    match (position.span == start.span && position.word == start.word).then_some(()) {
      Some(()) => {}
      None => return word.width(options.font_size),
    }

    let Ok((start_glyph, end_glyph)) = Self::get_glyph_start_end(
      word,
      start,
      position.span,
      crate::WordIndex(position.word),
      options.direction,
      congruence,
    );
    let Ok(first) = console_core_number_conversion::index(start_glyph);
    let Ok(last) = console_core_number_conversion::index(end_glyph);
    let mut width = 0.0;

    for glyph in word.glyphs.iter().take(last).skip(first) {
      let Ok(advance) = glyph.width(options.font_size);
      width += advance;
    }

    Ok(width)
  }

  fn layout_middle(
    &self,
    current_visual_line: &mut VisualLine,
    options: MiddleLayout,
    spans: &[ShapeSpan],
  ) -> Result<(), console_core_never::Never> {
    let MiddleLayout {
      font_size: crate::FontSize(font_size),
      start_opt,
      text_direction,
      width,
      ellipsize,
      ellipsis_w,
    } = options;

    let crate::LineWidth(width_2) = width;

    let crate::EllipsisWidth(ellipsis_w_2) = ellipsis_w;

    let rtl = text_direction == crate::TextDirection::RightToLeft;

    {
      match ellipsize {
        Ellipsize::Middle(_) => {}
        Ellipsize::None | Ellipsize::Start(_) | Ellipsize::End(_) => return Ok(()),
      }

      {
        let mut test_line = VisualLine::default();

        match self.layout_spans(
          &mut test_line,
          SpanLayout {
            font_size: crate::FontSize(font_size),
            start_opt,
            text_direction: match ::core::primitive::bool::then_some(rtl, ()) {
              ::core::option::Option::Some(()) => crate::TextDirection::RightToLeft,
              ::core::option::Option::None => crate::TextDirection::LeftToRight,
            },
            width_opt: Some(width_2),
            ellipsize: Ellipsize::End(EllipsizeHeightLimit::Lines(1)),
            ellipsis_w: crate::EllipsisWidth(ellipsis_w_2),
            direction: LayoutDirection::Forward,
          },
          spans,
        ) {
          Ok(value) => value,
        };

        match ::core::primitive::bool::then_some((test_line.elision == Elision::Full) && test_line.w.0 <= width_2, ()) {
          ::core::option::Option::Some(()) => {
            *current_visual_line = test_line;
            return Ok(());
          }
          ::core::option::Option::None => {}
        }
      }

      let mut starting_line = VisualLine::default();

      match self.layout_spans(
        &mut starting_line,
        SpanLayout {
          font_size: crate::FontSize(font_size),
          start_opt,
          text_direction: match ::core::primitive::bool::then_some(rtl, ()) {
            ::core::option::Option::Some(()) => crate::TextDirection::RightToLeft,
            ::core::option::Option::None => crate::TextDirection::LeftToRight,
          },
          width_opt: Some(width_2 / 2.0),
          ellipsize: Ellipsize::End(EllipsizeHeightLimit::Lines(1)),
          ellipsis_w: crate::EllipsisWidth(0.),
          direction: LayoutDirection::Forward,
        },
        spans,
      ) {
        Ok(value) => value,
      };

      let forward_pass_overflowed = starting_line.elision != Elision::Full;
      let end_range_opt = starting_line.ranges.last();

      match (
        end_range_opt,
        forward_pass_overflowed,
        width_2 > ellipsis_w_2,
      ) {
        (Some(range), true, _) => {
          let congruent = rtl
            == ((match (self.spans).get(match console_core_number_conversion::index(range.span) {
              Ok(position) => position,
            }) {
              Some(value) => value,
              None => return Ok(()),
            })
            .level
            .direction() == Ok(unicode_bidi::Direction::RightToLeft));

          let mut ending_line = VisualLine::default();
          let start = match ::core::primitive::bool::then_some(congruent, ()) {
            ::core::option::Option::Some(()) => SpanWordGlyphPos {
              span: range.span,
              word: range.end.word,
              glyph: range.end.glyph,
            },
            ::core::option::Option::None => SpanWordGlyphPos {
              span: range.span,
              word: range.start.word,
              glyph: range.start.glyph,
            },
          };

          match self.layout_spans(
            &mut ending_line,
            SpanLayout {
              font_size: crate::FontSize(font_size),
              start_opt: Some(start),
              text_direction: match ::core::primitive::bool::then_some(rtl, ()) {
                ::core::option::Option::Some(()) => crate::TextDirection::RightToLeft,
                ::core::option::Option::None => crate::TextDirection::LeftToRight,
              },
              width_opt: Some((width_2 - starting_line.w.0 - ellipsis_w_2).max(0.0)),
              ellipsize: Ellipsize::Start(EllipsizeHeightLimit::Lines(1)),
              ellipsis_w: crate::EllipsisWidth(0.),
              direction: LayoutDirection::Backward,
            },
            spans,
          ) {
            Ok(value) => value,
          };

          let Ok(ellipsis_level) =
            self.ellipsis_level_between(starting_line.ranges.last(), ending_line.ranges.first());
          starting_line
            .ranges
            .push(match self.ellipsis_vlrange(ellipsis_level) {
              Ok(value) => value,
            });
          starting_line.ranges.extend(ending_line.ranges);
          current_visual_line.ranges = starting_line.ranges;
          current_visual_line.elision = Elision::Pending;
          current_visual_line.w.0 = starting_line.w.0 + ending_line.w.0 + ellipsis_w_2;
          current_visual_line.spaces =
            u32::saturating_add(starting_line.spaces, ending_line.spaces);
        }
        (None, true, true) => {
          current_visual_line.ranges.push(
            match self.ellipsis_vlrange(match self.direction {
              crate::TextDirection::RightToLeft => unicode_bidi::Level::RIGHT_TO_LEFT,
              crate::TextDirection::LeftToRight => unicode_bidi::Level::LEFT_TO_RIGHT,
            }) {
              Ok(value) => value,
            },
          );
          current_visual_line.elision = Elision::Pending;
          current_visual_line.w.0 = ellipsis_w_2;
          current_visual_line.spaces = 0;
        }
        (Some(_), false, _) | (None, false, _) | (None, true, false) => {
          current_visual_line.ranges = starting_line.ranges;
          current_visual_line.w.0 = starting_line.w.0;
          current_visual_line.spaces = starting_line.spaces;
          current_visual_line.elision = Elision::Full;
        }
      };

      Ok(())
    }
  }

  fn get_span_words(&self, span_index: u32) -> Result<&[ShapeWord], console_core_never::Never> {
    {
      Ok(match ::core::primitive::bool::then_some(span_index == ELLIPSIS_SPAN, ()) {
        ::core::option::Option::Some(()) => {
          &(match self.ellipsis_span.as_ref() {
            Some(present) => present,
            None => return Ok(&[]),
          })
          .words
        }
        ::core::option::Option::None => {
          &((match (self.spans).get(match console_core_number_conversion::index(span_index) {
            Ok(position) => position,
          }) {
            Some(value) => value,
            None => return Ok(&[]),
          })
          .words)
        }
      })
    }
  }

  fn byte_range_of_vlrange(
    &self,
    r: &VlRange,
  ) -> Result<Option<(u32, u32)>, console_core_never::Never> {
    {
      match (r.span == ELLIPSIS_SPAN).then_some(()) {
        Some(()) => return Ok(None),
        None => {}
      }

      let Ok(words) = self.get_span_words(r.span);
      let mut min_byte = u32::MAX;
      let mut max_byte = 0u32;
      let end_word = u32::saturating_add(r.end.word, u32::from(r.end.glyph != 0));

      for (i, word) in words
        .iter()
        .enumerate()
        .take(match console_core_number_conversion::index(end_word) {
          Ok(position) => position,
        })
        .skip(match console_core_number_conversion::index(r.start.word) {
          Ok(position) => position,
        })
      {
        let included_glyphs = match (
          i == (match console_core_number_conversion::index(r.start.word) {
            Ok(position) => position,
          }),
          i == (match console_core_number_conversion::index(r.end.word) {
            Ok(position) => position,
          }),
        ) {
          (false, false) => match (word.glyphs).get(..) {
            Some(value) => value,
            None => return Ok(None),
          },
          (true, false) => match (word.glyphs).get(
            (match console_core_number_conversion::index(r.start.glyph) {
              Ok(position) => position,
            })..,
          ) {
            Some(value) => value,
            None => return Ok(None),
          },
          (false, true) => match (word.glyphs).get(
            ..(match console_core_number_conversion::index(r.end.glyph) {
              Ok(position) => position,
            }),
          ) {
            Some(value) => value,
            None => return Ok(None),
          },
          (true, true) => match (word.glyphs).get(
            (match console_core_number_conversion::index(r.start.glyph) {
              Ok(position) => position,
            })..(match console_core_number_conversion::index(r.end.glyph) {
              Ok(position) => position,
            }),
          ) {
            Some(value) => value,
            None => return Ok(None),
          },
        };

        for glyph in included_glyphs {
          min_byte = min_byte.min(glyph.start);
          max_byte = max_byte.max(glyph.end);
        }
      }

      Ok(match ::core::primitive::bool::then_some(min_byte <= max_byte, ()) {
        ::core::option::Option::Some(()) => Some((min_byte, max_byte)),
        ::core::option::Option::None => None,
      })
    }
  }

  fn compute_elided_byte_range(
    &self,
    visual_line: &VisualLine,
    line_len: u32,
  ) -> Result<Option<(u32, u32)>, console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(visual_line.elision == Elision::Full, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      let Ok(ellipsis_idx) = console_core_number_conversion::fitted::<_, u32>(match visual_line
        .ranges
        .iter()
        .position(|r| r.span == ELLIPSIS_SPAN)
      {
        Some(value) => value,
        None => return Ok(None),
      });

      let before_end = match (0..ellipsis_idx)
        .rev()
        .find_map(|i| {
          match self.byte_range_of_vlrange(
            match (visual_line.ranges).get(match console_core_number_conversion::index(i) {
              Ok(position) => position,
            }) {
              Some(value) => value,
              None => return None,
            },
          ) {
            Ok(value) => value,
          }
        })
        .map(|(_, end)| end)
      {
        Some(present) => present,
        None => 0,
      };

      let after_start = match (u32::saturating_add(
        match console_core_number_conversion::fitted::<_, u32>(ellipsis_idx) {
          Ok(value) => value,
        },
        1,
      )
        ..(match console_core_number_conversion::fitted::<_, u32>(visual_line.ranges.len()) {
          Ok(value) => value,
        }))
        .find_map(|i| {
          match self.byte_range_of_vlrange(
            match (visual_line.ranges).get(match console_core_number_conversion::index(i) {
              Ok(position) => position,
            }) {
              Some(value) => value,
              None => return None,
            },
          ) {
            Ok(value) => value,
          }
        })
        .map(|(start, _)| start)
      {
        Some(present) => present,
        None => line_len,
      };

      Ok(Some((before_end, after_start)))
    }
  }

  fn max_byte_offset(&self) -> Result<u32, console_core_never::Never> {
    {
      Ok(
        match self
          .spans
          .iter()
          .flat_map(|span| span.words.iter())
          .flat_map(|word| word.glyphs.iter())
          .map(|g| g.end)
          .max()
        {
          Some(present) => present,
          None => 0,
        },
      )
    }
  }

  fn ellipsis_w(&self, font_size: crate::FontSize) -> Result<f32, console_core_never::Never> {
    {
      Ok(self.ellipsis_span.as_ref().map_or(0.0, |s| {
        s.words
          .iter()
          .map(|w| match w.width(font_size) {
            Ok(value) => value,
          })
          .sum()
      }))
    }
  }

  fn ellipsis_vlrange(
    &self,
    level: unicode_bidi::Level,
  ) -> Result<VlRange, console_core_never::Never> {
    {
      Ok(VlRange {
        span: ELLIPSIS_SPAN,
        start: WordGlyphPos::ZERO,
        end: match WordGlyphPos::new(1, crate::GlyphIndex(0)) {
          Ok(value) => value,
        },
        level,
      })
    }
  }

  fn ellipsis_level_between(
    &self,
    before: Option<&VlRange>,
    after: Option<&VlRange>,
  ) -> Result<unicode_bidi::Level, console_core_never::Never> {
    {
      let fallback = match self.direction {
        crate::TextDirection::RightToLeft => unicode_bidi::Level::RIGHT_TO_LEFT,
        crate::TextDirection::LeftToRight => unicode_bidi::Level::LEFT_TO_RIGHT,
      };

      Ok(match (before, after) {
        (Some(a), Some(b)) => match ::core::primitive::bool::then_some(a.level == b.level, ()) {
          ::core::option::Option::Some(()) => a.level,
          ::core::option::Option::None => fallback,
        },
        (Some(a), None) => a.level,
        (None, Some(b)) => b.level,
        (None, None) => fallback,
      })
    }
  }

  fn layout_line(
    &self,
    current_visual_line: &mut VisualLine,
    options: LineLayout,
    spans: &[ShapeSpan],
  ) -> Result<(), console_core_never::Never> {
    let LineLayout {
      font_size: crate::FontSize(font_size),
      start_opt,
      text_direction,
      width_opt,
      ellipsize,
    } = options;

    let Ok(ellipsis_w) = self.ellipsis_w(crate::FontSize(font_size));

    match (ellipsize, width_opt) {
      (Ellipsize::Start(_), Some(_)) => {
        match self.layout_spans(
          current_visual_line,
          SpanLayout {
            font_size: crate::FontSize(font_size),
            start_opt,
            text_direction,
            width_opt,
            ellipsize,
            ellipsis_w: crate::EllipsisWidth(ellipsis_w),
            direction: LayoutDirection::Backward,
          },
          spans,
        ) {
          Ok(value) => value,
        };

        match ::core::primitive::bool::then_some(current_visual_line.elision != Elision::Full, ())
        {
          ::core::option::Option::Some(()) => {
            let Ok(level) = self.ellipsis_level_between(None, current_visual_line.ranges.first());
            let Ok(ellipsis_range) = self.ellipsis_vlrange(level);
            current_visual_line.ranges.insert(0, ellipsis_range);
            current_visual_line.w.0 += ellipsis_w;
          }
          ::core::option::Option::None => {}
        }
      }
      (Ellipsize::Middle(_), Some(width)) => {
        match self.layout_middle(
          current_visual_line,
          MiddleLayout {
            font_size: crate::FontSize(font_size),
            start_opt,
            text_direction,
            width: crate::LineWidth(width),
            ellipsize,
            ellipsis_w: crate::EllipsisWidth(ellipsis_w),
          },
          spans,
        ) {
          Ok(value) => value,
        };
      }
      _ => {
        match self.layout_spans(
          current_visual_line,
          SpanLayout {
            font_size: crate::FontSize(font_size),
            start_opt,
            text_direction,
            width_opt,
            ellipsize,
            ellipsis_w: crate::EllipsisWidth(ellipsis_w),
            direction: LayoutDirection::Forward,
          },
          spans,
        ) {
          Ok(value) => value,
        };

        match ::core::primitive::bool::then_some(current_visual_line.elision != Elision::Full, ())
        {
          ::core::option::Option::Some(()) => {
            let Ok(level) = self.ellipsis_level_between(current_visual_line.ranges.last(), None);
            let Ok(ellipsis_range) = self.ellipsis_vlrange(level);
            current_visual_line.ranges.push(ellipsis_range);
            current_visual_line.w.0 += ellipsis_w;
          }
          ::core::option::Option::None => {}
        }
      }
    }

    match ::core::primitive::bool::then_some(current_visual_line.elision != Elision::Full, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(line_len) = self.max_byte_offset();
        let Ok(range) = self.compute_elided_byte_range(current_visual_line, line_len);

        current_visual_line.elision = match range {
          Some(range_2) => Elision::Known(range_2),
          None => Elision::Pending,
        };
      }
      ::core::option::Option::None => {}
    };

    Ok(())
  }

  fn resolve_alignment(
    &self,
    visual_lines: &[VisualLine],
    width_opt: Option<f32>,
    align: Option<Align>,
  ) -> Result<ResolvedAlignment, console_core_never::Never> {
    let align_2 = match align {
      Some(present) => present,
      None => match self.direction {
        crate::TextDirection::RightToLeft => Align::Right,
        crate::TextDirection::LeftToRight => Align::Left,
      },
    };

    let line_width = match width_opt {
      Some(present) => present,
      None => {
        let mut width: f32 = 0.0;

        for visual_line in visual_lines {
          width = width.max(visual_line.w.0);
        }

        width
      }
    };

    let start_x = match self.direction {
      crate::TextDirection::RightToLeft => line_width,
      crate::TextDirection::LeftToRight => 0.0,
    };

    Ok(ResolvedAlignment {
      align: align_2,
      line_width: crate::LineWidth(line_width),
      start_x: crate::HorizontalPosition(start_x),
    })
  }

  fn place_visual_lines(
    &self,
    visual_lines: &[VisualLine],
    cached_glyph_sets: &mut Vec<Vec<LayoutGlyph>>,
    layout_lines: &mut Vec<LayoutLine>,
    options: GlyphPlacement,
  ) -> Result<(), console_core_never::Never> {
    let Ok(alignment) = self.resolve_alignment(visual_lines, options.width_opt, options.align);
    let Ok(line_count) = console_core_number_conversion::fitted::<_, u32>(visual_lines.len());
    let Ok(last_index) = console_core_number_conversion::index(line_count.saturating_sub(1));

    for (index, visual_line) in visual_lines.iter().enumerate() {
      match visual_line.ranges.is_empty().then_some(()) {
        Some(()) => continue,
        None => {}
      }

      let justification = match (alignment.align == Align::Justified
        && visual_line.spaces > 0
        && index != last_index)
        .then_some(())
      {
        Some(()) => {
          let Ok(spaces) =
            console_core_number_conversion::narrow_f32(f64::from(visual_line.spaces));
          (alignment.line_width.0 - visual_line.w.0) / spaces
        }
        None => 0.0,
      };
      let glyphs = match cached_glyph_sets.pop() {
        Some(recycled) => recycled,
        None => Vec::with_capacity(1),
      };
      let Ok(line) = self.place_visual_line(
        visual_line,
        glyphs,
        &alignment,
        &options,
        crate::UsedWidth(justification),
      );
      layout_lines.push(line);
    }

    Ok(())
  }

  fn place_visual_line(
    &self,
    visual_line: &VisualLine,
    glyphs: Vec<LayoutGlyph>,
    alignment: &ResolvedAlignment,
    options: &GlyphPlacement,
    justification: crate::UsedWidth,
  ) -> Result<LayoutLine, console_core_never::Never> {
    let Ok(order) = self.reorder(&visual_line.ranges);
    let Ok(mut placement) = LinePosition::new(glyphs, alignment.start_x);
    let free_width = (alignment.line_width.0 - visual_line.w.0).max(0.0);
    let correction = match (alignment.align, self.direction) {
      (Align::Left, crate::TextDirection::RightToLeft)
      | (Align::Right, crate::TextDirection::LeftToRight)
      | (Align::End, _) => free_width,
      (Align::Center, _) => free_width / 2.0,
      (Align::Left, crate::TextDirection::LeftToRight)
      | (Align::Right, crate::TextDirection::RightToLeft)
      | (Align::Justified, _) => 0.0,
    };

    placement.x.0 = match self.direction {
      crate::TextDirection::RightToLeft => placement.x.0 - correction,
      crate::TextDirection::LeftToRight => placement.x.0 + correction,
    };

    match options.hinting {
      Hinting::Enabled => placement.x.0 = placement.x.0.round(),
      Hinting::Disabled => {}
    }

    let boundary = match visual_line.elision {
      Elision::Known((0, end)) => Some(end),
      Elision::Known((start, _)) => Some(start),
      Elision::Full | Elision::Pending => None,
    };
    let range_options = RangePlacement {
      options,
      justification,
      boundary,
    };

    match self.direction {
      crate::TextDirection::RightToLeft => {
        for range in order.into_iter().rev() {
          let Ok(()) =
            self.place_ordered_ranges(visual_line, range, &mut placement, &range_options);
        }
      }
      crate::TextDirection::LeftToRight => {
        for range in order {
          let Ok(()) =
            self.place_ordered_ranges(visual_line, range, &mut placement, &range_options);
        }
      }
    }

    let Ok(line_height_opt) = line_height(&placement.glyphs);
    let w = match (alignment.align, self.direction) {
      (Align::Justified, crate::TextDirection::RightToLeft) => {
        alignment.start_x.0 - placement.x.0
      }
      (Align::Justified, crate::TextDirection::LeftToRight) => placement.x.0,
      _ => visual_line.w.0,
    };

    Ok(LayoutLine {
      w,
      max_ascent: placement.max_ascent.0,
      max_descent: placement.max_descent.0,
      line_height_opt,
      glyphs: placement.glyphs,
      decorations: placement.decorations,
    })
  }

  fn place_ordered_ranges(
    &self,
    line: &VisualLine,
    order: Range<u32>,
    placement: &mut LinePosition,
    options: &RangePlacement<'_>,
  ) -> Result<(), console_core_never::Never> {
    let Ok(first) = console_core_number_conversion::index(order.start);
    let Ok(last) = console_core_number_conversion::index(order.end);
    let ranges = match line.ranges.get(first..last) {
      Some(selected) => selected,
      None => return Ok(()),
    };

    for range in ranges {
      let Ok(()) = self.place_range(range, placement, options);
    }

    Ok(())
  }

  fn place_range(
    &self,
    range: &VlRange,
    placement: &mut LinePosition,
    options: &RangePlacement<'_>,
  ) -> Result<(), console_core_never::Never> {
    let (decorations, boundary): (&[(Range<u32>, GlyphDecorationData)], Option<u32>) =
      match (range.span == ELLIPSIS_SPAN).then_some(()) {
        Some(()) => (&[], options.boundary),
        None => {
          let Ok(index) = console_core_number_conversion::index(range.span);
          let span = match self.spans.get(index) {
            Some(shaped) => shaped,
            None => return Ok(()),
          };
          (span.decoration_spans.as_slice(), None)
        }
      };
    let Ok(words) = self.get_span_words(range.span);
    let Ok(start) = console_core_number_conversion::index(range.start.word);
    let Ok(end) = console_core_number_conversion::index(range.end.word);
    let Ok(last) = console_core_number_conversion::index(
      range
        .end
        .word
        .saturating_add(u32::from(range.end.glyph != 0)),
    );
    let Ok(start_glyph) = console_core_number_conversion::index(range.start.glyph);
    let Ok(end_glyph) = console_core_number_conversion::index(range.end.glyph);

    for (index, word) in words.iter().enumerate().take(last).skip(start) {
      let candidates = match (index == start, index == end) {
        (false, false) => Some(word.glyphs.as_slice()),
        (true, false) => word.glyphs.get(start_glyph..),
        (false, true) => word.glyphs.get(..end_glyph),
        (true, true) => word.glyphs.get(start_glyph..end_glyph),
      };
      let selected = match candidates {
        Some(glyphs) => glyphs,
        None => continue,
      };
      let expansion = match word.whitespace {
        crate::Whitespace::Blank => options.justification,
        crate::Whitespace::Text => crate::UsedWidth(0.0),
      };

      for glyph in selected {
        let Ok(mut layout) =
          self.position_glyph(placement, glyph, options.options, expansion, range.level);

        match boundary {
          Some(byte) => {
            layout.start = byte;
            layout.end = byte;
          }
          None => {}
        }

        let Ok(()) = placement.push_glyph(layout, decorations);
      }
    }

    Ok(())
  }

  fn position_glyph(
    &self,
    placement: &mut LinePosition,
    glyph: &ShapeGlyph,
    options: &GlyphPlacement,
    expansion: crate::UsedWidth,
    level: unicode_bidi::Level,
  ) -> Result<LayoutGlyph, console_core_never::Never> {
    let size = glyph
      .metrics_opt
      .map_or(options.font_size.0, |metrics| metrics.font_size);
    let mono_em_width = options.match_mono_width.map(|width| width / size);
    let Ok(glyph_size) = glyph.adjusted_font_size(crate::FontSize(size), mono_em_width);
    let mut advance = glyph_size.mul_add(glyph.x_advance, expansion.0);

    match mono_em_width {
      Some(width) => advance = (advance / width).round() * width,
      None => {}
    }

    match options.hinting {
      Hinting::Enabled => advance = advance.round(),
      Hinting::Disabled => {}
    }

    match self.direction {
      crate::TextDirection::RightToLeft => placement.x.0 -= advance,
      crate::TextDirection::LeftToRight => {}
    }

    let Ok(layout) = glyph.layout(
      crate::FontSize(glyph_size),
      glyph.metrics_opt.map(|metrics| metrics.line_height),
      placement.x,
      placement.y,
      crate::GlyphWidth(advance),
      level,
    );

    match self.direction {
      crate::TextDirection::LeftToRight => placement.x.0 += advance,
      crate::TextDirection::RightToLeft => {}
    }

    placement.y.0 += glyph_size * glyph.y_advance;
    placement.max_ascent.0 = placement.max_ascent.0.max(glyph_size * glyph.ascent);
    placement.max_descent.0 = placement.max_descent.0.max(glyph_size * glyph.descent);
    Ok(layout)
  }

  pub fn layout_to_buffer(
    &self,
    scratch: &mut ShapeBuffer,
    options: BufferLayout,
    layout_lines: &mut Vec<LayoutLine>,
  ) -> Result<(), console_core_never::Never> {
    let Ok(LayoutStorage {
      mut visual_lines,
      mut cached_visual_lines,
      mut cached_glyph_sets,
    }) = scratch.recycle_layout(layout_lines);
    let mut current_visual_line = match cached_visual_lines.pop() {
      Some(recycled) => recycled,
      None => VisualLine::default(),
    };

    match options.wrap {
      Wrap::None => {
        let Ok(()) = self.layout_line(
          &mut current_visual_line,
          LineLayout {
            font_size: crate::FontSize(options.font_size),
            start_opt: None,
            text_direction: self.direction,
            width_opt: options.width_opt,
            ellipsize: options.ellipsize,
          },
          &self.spans,
        );
      }
      Wrap::Glyph | Wrap::Word | Wrap::WordOrGlyph => {
        let Ok(()) = self.wrap_visual_lines(
          &mut current_visual_line,
          &mut visual_lines,
          &mut cached_visual_lines,
          &options,
        );
      }
    }

    match current_visual_line.ranges.is_empty().then_some(()) {
      Some(()) => {
        let Ok(()) = current_visual_line.clear();
        cached_visual_lines.push(current_visual_line);
      }
      None => visual_lines.push(current_visual_line),
    }

    let Ok(()) = self.place_visual_lines(
      &visual_lines,
      &mut cached_glyph_sets,
      layout_lines,
      GlyphPlacement {
        font_size: crate::FontSize(options.font_size),
        width_opt: options.width_opt,
        align: options.align,
        match_mono_width: options.match_mono_width,
        hinting: options.hinting,
      },
    );

    match layout_lines.is_empty().then_some(()) {
      Some(()) => layout_lines.push(LayoutLine {
        w: 0.0,
        max_ascent: 0.0,
        max_descent: 0.0,
        line_height_opt: self.metrics_opt.map(|metrics| metrics.line_height),
        glyphs: Vec::new(),
        decorations: Vec::new(),
      }),
      None => {}
    }

    scratch.visual_lines = visual_lines;
    scratch.visual_lines.append(&mut cached_visual_lines);
    scratch.cached_visual_lines = cached_visual_lines;
    scratch.glyph_sets = cached_glyph_sets;
    Ok(())
  }

  fn wrap_visual_lines(
    &self,
    current: &mut VisualLine,
    completed: &mut Vec<VisualLine>,
    cached: &mut Vec<VisualLine>,
    options: &BufferLayout,
  ) -> Result<(), console_core_never::Never> {
    let limit = match options.ellipsize {
      Ellipsize::Start(height) | Ellipsize::Middle(height) | Ellipsize::End(height) => {
        Some(height)
      }
      Ellipsize::None => None,
    };
    let line_height = crate::LineHeight(
      self
        .metrics_opt
        .map_or(options.font_size, |metrics| metrics.line_height),
    );
    let available = crate::AvailableWidth(options.width_opt.map_or(f32::INFINITY, |width| width));
    let mut lines = WrappingLines {
      current,
      completed,
      cached,
      options,
      line_height,
      limit,
      available,
      total_height: crate::LineHeight(0.0),
      count: 0,
    };
    let Ok(progress) = self.ellipsize_wrapped_line(&mut lines, None);

    match progress {
      WrapProgress::Elided => return Ok(()),
      WrapProgress::Continue => {}
    }

    for (index, span) in self.spans.iter().enumerate() {
      let Ok(span_index) = console_core_number_conversion::fitted::<_, u32>(index);
      let Ok(word_count) = console_core_number_conversion::fitted::<_, u32>(span.words.len());
      let Ok(direction) = span.level.direction();
      let words = (0..word_count).zip(&span.words);
      let (direction_2, start, end) =
        match (crate::TextDirection::from(direction) == self.direction).then_some(()) {
          Some(()) => (LayoutDirection::Forward, 0, word_count),
          None => (LayoutDirection::Backward, word_count, 0),
        };
      let mut wrapping_span = WrappingSpan {
        index: span_index,
        direction: direction_2,
        start: WordGlyphPos {
          word: start,
          glyph: 0,
        },
        width: crate::UsedWidth(0.0),
        width_before_blank: crate::UsedWidth(0.0),
        blanks: crate::BlankCount(0),
      };
      let Ok(progress_2) = match direction_2 {
        LayoutDirection::Forward => self.wrap_words(&mut lines, &mut wrapping_span, span, words),
        LayoutDirection::Backward => {
          self.wrap_words(&mut lines, &mut wrapping_span, span, words.rev())
        }
      };

      match progress_2 {
        WrapProgress::Elided => return Ok(()),
        WrapProgress::Continue => {}
      }

      let Ok(()) = self.append_wrap_range(
        lines.current,
        &wrapping_span,
        WordGlyphPos {
          word: end,
          glyph: 0,
        },
      );
    }

    Ok(())
  }

  fn wrap_words<'a>(
    &self,
    lines: &mut WrappingLines<'_>,
    span: &mut WrappingSpan,
    shaped: &ShapeSpan,
    words: impl Iterator<Item = (u32, &'a ShapeWord)>,
  ) -> Result<WrapProgress, console_core_never::Never> {
    for (index, word) in words {
      let Ok(width) = word.width(lines.options.font_size);
      let fits = lines.current.w.0 + (span.width.0 + width) <= lines.available.0
        || (word.whitespace == crate::Whitespace::Blank
          && lines.current.w.0 + span.width.0 <= lines.available.0);

      match fits.then_some(()) {
        Some(()) => {
          match word.whitespace {
            crate::Whitespace::Blank => {
              span.blanks.0 = span.blanks.0.saturating_add(1);
              span.width_before_blank = span.width;
            }
            crate::Whitespace::Text => {}
          }

          span.width.0 += width;
          continue;
        }
        None => {}
      }

      let long_word = lines.options.wrap == Wrap::WordOrGlyph && width > lines.available.0;
      let Ok(progress) = match (lines.options.wrap, long_word) {
        (Wrap::Glyph, _) | (Wrap::WordOrGlyph, true) => {
          self.wrap_long_word(lines, span, word, crate::WordIndex(index))
        }
        (Wrap::None | Wrap::Word, _) | (Wrap::WordOrGlyph, false) => self.wrap_word_boundary(
          lines,
          span,
          shaped,
          word,
          crate::WordIndex(index),
          crate::UsedWidth(width),
        ),
      };

      match progress {
        WrapProgress::Elided => return Ok(progress),
        WrapProgress::Continue => {}
      }
    }

    Ok(WrapProgress::Continue)
  }

  fn wrap_long_word(
    &self,
    lines: &mut WrappingLines<'_>,
    span: &mut WrappingSpan,
    word: &ShapeWord,
    crate::WordIndex(index): crate::WordIndex,
  ) -> Result<WrapProgress, console_core_never::Never> {
    match (span.width.0 > 0.0 && lines.options.wrap == Wrap::WordOrGlyph).then_some(()) {
      Some(()) => {
        let before_word = match span.direction {
          LayoutDirection::Forward => index,
          LayoutDirection::Backward => index.saturating_add(1),
        };
        let boundary = WordGlyphPos {
          word: before_word,
          glyph: 0,
        };
        let Ok(()) = self.append_wrap_range(lines.current, span, boundary);
        span.start = boundary;
        span.width = crate::UsedWidth(0.0);
        span.blanks = crate::BlankCount(0);
        let Ok(progress) = self.advance_wrap_line(lines, span);

        match progress {
          WrapProgress::Elided => return Ok(progress),
          WrapProgress::Continue => {}
        }
      }
      None => {}
    }

    let Ok(glyph_count) = console_core_number_conversion::fitted::<_, u32>(word.glyphs.len());
    let glyphs = (0..glyph_count).zip(&word.glyphs);
    let word_index = crate::WordIndex(index);

    match span.direction {
      LayoutDirection::Forward => self.wrap_glyphs(lines, span, word_index, glyphs),
      LayoutDirection::Backward => self.wrap_glyphs(lines, span, word_index, glyphs.rev()),
    }
  }

  fn wrap_glyphs<'a>(
    &self,
    lines: &mut WrappingLines<'_>,
    span: &mut WrappingSpan,
    crate::WordIndex(word): crate::WordIndex,
    glyphs: impl Iterator<Item = (u32, &'a ShapeGlyph)>,
  ) -> Result<WrapProgress, console_core_never::Never> {
    for (glyph_index, shaped) in glyphs {
      let Ok(width) = shaped.width(lines.options.font_size);

      match (lines.current.w.0 + (span.width.0 + width) <= lines.available.0).then_some(()) {
        Some(()) => {
          span.width.0 += width;
          continue;
        }
        None => {}
      }

      let glyph = match span.direction {
        LayoutDirection::Forward => glyph_index,
        LayoutDirection::Backward => glyph_index.saturating_add(1),
      };
      let boundary = WordGlyphPos { word, glyph };
      let Ok(()) = self.append_wrap_range(lines.current, span, boundary);
      span.start = boundary;
      span.width = crate::UsedWidth(width);
      span.blanks = crate::BlankCount(0);
      let Ok(progress) = self.advance_wrap_line(lines, span);

      match progress {
        WrapProgress::Elided => return Ok(progress),
        WrapProgress::Continue => {}
      }
    }

    Ok(WrapProgress::Continue)
  }

  fn wrap_word_boundary(
    &self,
    lines: &mut WrappingLines<'_>,
    span: &mut WrappingSpan,
    shaped: &ShapeSpan,
    word: &ShapeWord,
    crate::WordIndex(index): crate::WordIndex,
    word_width: crate::UsedWidth,
  ) -> Result<WrapProgress, console_core_never::Never> {
    let (before_word, previous_index) = match span.direction {
      LayoutDirection::Forward => (index, index.checked_sub(1)),
      LayoutDirection::Backward => (index.saturating_add(1), Some(index.saturating_add(1))),
    };
    let trailing_blank = previous_index.and_then(|previous| {
      let Ok(position) = console_core_number_conversion::index(previous);
      shaped
        .words
        .get(position)
        .filter(|previous_word| previous_word.whitespace == crate::Whitespace::Blank)
    });
    let range_end = match (span.direction, trailing_blank) {
      (LayoutDirection::Forward, Some(_)) => before_word.saturating_sub(1),
      (LayoutDirection::Backward, Some(_)) => before_word.saturating_add(1),
      (LayoutDirection::Forward | LayoutDirection::Backward, None) => before_word,
    };

    match (span.width.0 > 0.0).then_some(()) {
      Some(()) => {
        match trailing_blank {
          Some(_) => {
            span.blanks.0 = span.blanks.0.saturating_sub(1);
            span.width = span.width_before_blank;
          }
          None => {}
        }

        let Ok(()) = self.append_wrap_range(
          lines.current,
          span,
          WordGlyphPos {
            word: range_end,
            glyph: 0,
          },
        );
      }
      None => {}
    }

    let (next_word, next_width) = match (span.direction, word.whitespace) {
      (LayoutDirection::Forward, crate::Whitespace::Blank) => {
        (index.saturating_add(1), crate::UsedWidth(0.0))
      }
      (LayoutDirection::Backward, crate::Whitespace::Blank) => (index, crate::UsedWidth(0.0)),
      (LayoutDirection::Forward | LayoutDirection::Backward, crate::Whitespace::Text) => {
        (before_word, word_width)
      }
    };
    span.start = WordGlyphPos {
      word: next_word,
      glyph: 0,
    };
    span.width = next_width;

    match lines.current.ranges.is_empty().then_some(()) {
      Some(()) => Ok(WrapProgress::Continue),
      None => {
        span.blanks = crate::BlankCount(0);
        let ellipsis_word = match span.direction {
          LayoutDirection::Forward => range_end,
          LayoutDirection::Backward => next_word,
        };
        let next = SpanWordGlyphPos {
          span: span.index,
          word: ellipsis_word,
          glyph: 0,
        };
        let Ok(()) = lines.advance();
        self.ellipsize_wrapped_line(lines, Some(next))
      }
    }
  }

  fn append_wrap_range(
    &self,
    line: &mut VisualLine,
    span: &WrappingSpan,
    end: WordGlyphPos,
  ) -> Result<(), console_core_never::Never> {
    let (first, last) = match span.direction {
      LayoutDirection::Forward => (span.start, end),
      LayoutDirection::Backward => (end, span.start),
    };
    self.add_to_visual_line(line, span.index, first, last, span.width, span.blanks)
  }

  fn advance_wrap_line(
    &self,
    lines: &mut WrappingLines<'_>,
    span: &WrappingSpan,
  ) -> Result<WrapProgress, console_core_never::Never> {
    let Ok(()) = lines.advance();
    let Ok(next) = SpanWordGlyphPos::with_wordglyph(span.index, span.start);
    self.ellipsize_wrapped_line(lines, Some(next))
  }

  fn ellipsize_wrapped_line(
    &self,
    lines: &mut WrappingLines<'_>,
    start: Option<SpanWordGlyphPos>,
  ) -> Result<WrapProgress, console_core_never::Never> {
    let last_line = match lines.limit {
      Some(EllipsizeHeightLimit::Lines(maximum)) => maximum == lines.count.saturating_add(1),
      Some(EllipsizeHeightLimit::Height(maximum)) => {
        lines.total_height.0 + lines.line_height.0 * 2.0 > maximum
      }
      None => false,
    };

    match last_line.then_some(()) {
      Some(()) => {
        let Ok(()) = self.layout_line(
          lines.current,
          LineLayout {
            font_size: crate::FontSize(lines.options.font_size),
            start_opt: start,
            text_direction: self.direction,
            width_opt: lines.options.width_opt,
            ellipsize: lines.options.ellipsize,
          },
          &self.spans,
        );
        Ok(WrapProgress::Elided)
      }
      None => Ok(WrapProgress::Continue),
    }
  }
}

#[derive(Debug, Clone, Copy)]
enum WrapProgress {
  Continue,
  Elided,
}

struct WrappingLines<'a> {
  current: &'a mut VisualLine,
  completed: &'a mut Vec<VisualLine>,
  cached: &'a mut Vec<VisualLine>,
  options: &'a BufferLayout,
  line_height: crate::LineHeight,
  limit: Option<EllipsizeHeightLimit>,
  available: crate::AvailableWidth,
  total_height: crate::LineHeight,
  count: u32,
}

impl WrappingLines<'_> {
  fn advance(&mut self) -> Result<(), console_core_never::Never> {
    let next = match self.cached.pop() {
      Some(recycled) => recycled,
      None => VisualLine::default(),
    };
    self.completed.push(mem::replace(self.current, next));
    self.count = self.count.saturating_add(1);
    self.total_height.0 += self.line_height.0;
    Ok(())
  }
}

struct WrappingSpan {
  index: u32,
  direction: LayoutDirection,
  start: WordGlyphPos,
  width: crate::UsedWidth,
  width_before_blank: crate::UsedWidth,
  blanks: crate::BlankCount,
}

#[derive(Debug)]
struct RunShaping {
  start_run: u32,
  end_run: crate::RunEnd,
  span_direction: crate::TextDirection,
}

#[derive(Clone, Copy, Debug)]
struct FallbackShaping {
  start_run: u32,
  end_run: crate::RunEnd,
  span_direction: crate::TextDirection,
}

#[derive(Debug)]
pub struct WordShaping {
  pub word_range: Range<u32>,
  pub level: unicode_bidi::Level,
  pub whitespace: crate::Whitespace,
  pub shaping: Shaping,
}

#[derive(Debug)]
pub struct SpanShaping {
  pub span_range: Range<u32>,
  pub line_direction: crate::TextDirection,
  pub level: unicode_bidi::Level,
  pub shaping: Shaping,
}

#[derive(Debug)]
struct GlyphFitting {
  font_size: crate::FontSize,
  start: SpanWordGlyphPos,
  span_index: u32,
  word_idx: crate::WordIndex,
  direction: LayoutDirection,
  congruence: crate::RunAlignment,
  currently_used_width: crate::UsedWidth,
  total_available_width: crate::AvailableWidth,
  traversal: crate::SearchDirection,
}

#[derive(Debug)]
struct RemainingContent {
  font_size: crate::FontSize,
  span_index: u32,
  word_idx: crate::WordIndex,
  word_count: crate::WordCount,
  starting_word_index: crate::FirstWordIndex,
  direction: LayoutDirection,
  congruence: crate::RunAlignment,
  start_span: crate::FirstSpanIndex,
  span_count: crate::SpanCount,
  threshold: crate::WidthThreshold,
}

enum SpanProgress {
  Continue,
  Complete,
}

struct EllipsisScan<'a> {
  options: SpanLayout,
  start: SpanWordGlyphPos,
  spans: &'a [ShapeSpan],
  span_count: u32,
}

struct EllipsisSpan<'a> {
  index: u32,
  words: &'a [ShapeWord],
  count: crate::WordCount,
  congruence: crate::RunAlignment,
  traversal: crate::SearchDirection,
  start: Option<SpanWordGlyphPos>,
  end: WordGlyphPos,
}

#[derive(Debug)]
struct SpanLayout {
  font_size: crate::FontSize,
  start_opt: Option<SpanWordGlyphPos>,
  text_direction: crate::TextDirection,
  width_opt: Option<f32>,
  ellipsize: Ellipsize,
  ellipsis_w: crate::EllipsisWidth,
  direction: LayoutDirection,
}

#[derive(Debug)]
struct MiddleLayout {
  font_size: crate::FontSize,
  start_opt: Option<SpanWordGlyphPos>,
  text_direction: crate::TextDirection,
  width: crate::LineWidth,
  ellipsize: Ellipsize,
  ellipsis_w: crate::EllipsisWidth,
}

#[derive(Debug)]
struct LineLayout {
  font_size: crate::FontSize,
  start_opt: Option<SpanWordGlyphPos>,
  text_direction: crate::TextDirection,
  width_opt: Option<f32>,
  ellipsize: Ellipsize,
}

#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the public buffer layout request takes its font size in fractional layout pixels"
  )
)]
pub struct BufferLayout {
  pub font_size: f32,
  pub width_opt: Option<f32>,
  pub wrap: Wrap,
  pub ellipsize: Ellipsize,
  pub align: Option<Align>,
  pub match_mono_width: Option<f32>,
  pub hinting: Hinting,
}

#[derive(Debug)]
struct GlyphPlacement {
  font_size: crate::FontSize,
  width_opt: Option<f32>,
  align: Option<Align>,
  match_mono_width: Option<f32>,
  hinting: Hinting,
}

struct RangePlacement<'a> {
  options: &'a GlyphPlacement,
  justification: crate::UsedWidth,
  boundary: Option<u32>,
}

#[derive(Debug)]
struct LinePosition {
  glyphs: Vec<LayoutGlyph>,
  decorations: Vec<DecorationSpan>,
  x: crate::HorizontalPosition,
  y: crate::VerticalPosition,
  max_ascent: crate::LineHeight,
  max_descent: crate::LineHeight,
}

impl LinePosition {
  fn new(
    glyphs: Vec<LayoutGlyph>,
    x: crate::HorizontalPosition,
  ) -> Result<Self, console_core_never::Never> {
    Ok(Self {
      glyphs,
      decorations: Vec::new(),
      x,
      y: crate::VerticalPosition(0.0),
      max_ascent: crate::LineHeight(0.0),
      max_descent: crate::LineHeight(0.0),
    })
  }

  fn push_glyph(
    &mut self,
    glyph: LayoutGlyph,
    decorations: &[(Range<u32>, GlyphDecorationData)],
  ) -> Result<(), console_core_never::Never> {
    let Ok(position) = console_core_number_conversion::fitted::<_, u32>(
      decorations.partition_point(|(range, _)| range.end <= glyph.start),
    );
    let Ok(boundary) = console_core_number_conversion::index(position);
    let decoration = decorations
      .get(boundary)
      .filter(|(range, _)| glyph.start >= range.start);
    let Ok(index) = console_core_number_conversion::fitted::<_, u32>(self.glyphs.len());
    let end = index.saturating_add(1);

    match (self.decorations.last_mut(), decoration) {
      (Some(last), Some((_, data))) => match (last.data == *data).then_some(()) {
        Some(()) => last.glyph_range.end = end,
        None => self.decorations.push(DecorationSpan {
          glyph_range: index..end,
          data: data.clone(),
          color_opt: glyph.color_opt,
          font_size: glyph.font_size,
        }),
      },
      (None, Some((_, data))) => self.decorations.push(DecorationSpan {
        glyph_range: index..end,
        data: data.clone(),
        color_opt: glyph.color_opt,
        font_size: glyph.font_size,
      }),
      (Some(_), None) | (None, None) => {}
    }

    self.glyphs.push(glyph);
    Ok(())
  }
}

fn line_height(glyphs: &[LayoutGlyph]) -> Result<Option<f32>, console_core_never::Never> {
  Ok(
    glyphs
      .iter()
      .filter_map(|glyph| glyph.line_height_opt)
      .reduce(f32::max),
  )
}

#[derive(Debug)]
struct ResolvedAlignment {
  align: Align,
  line_width: crate::LineWidth,
  start_x: crate::HorizontalPosition,
}
