use core::fmt;

use unicode_segmentation::{ClusterKind, UnicodeSegmentation};

use crate::{
  Affinity, Align, Attrs, AttrsList, BidiParagraphs, BorrowedWithFontSystem, BufferLine, Color,
  Cursor, DecorationSpan, Ellipsize, FontSystem, Hinting, LayoutCursor, LayoutGlyph, LayoutLine,
  LineEnding, LineIter, Renderer, Scroll, ShapeLine, Shaping, Wrap, render_decoration,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DirtyFlags(u8);

impl DirtyFlags {
  const EMPTY: Self = Self(0);
  const RELAYOUT: Self = Self(1);
  const TAB_SHAPE: Self = Self(2);
  const TEXT_SET: Self = Self(4);
  const SCROLL: Self = Self(8);

  const fn contains(self, wanted: Self) -> Result<crate::Presence, console_core_never::Never> {
    Ok(match wanted.0 & !self.0 {
      0 => crate::Presence::Present,
      _ => crate::Presence::Absent,
    })
  }
}

impl core::ops::BitOr for DirtyFlags {
  type Output = Self;

  fn bitor(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }
}

impl core::ops::BitOrAssign for DirtyFlags {
  fn bitor_assign(&mut self, other: Self) {
    self.0 |= other.0;
  }
}

#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "public layout run geometry is measured in fractional layout pixels"
  )
)]
pub struct LayoutRun<'a> {
  pub line_i: u32,

  pub text: &'a str,

  pub direction: crate::TextDirection,

  pub glyphs: &'a [LayoutGlyph],

  pub decorations: &'a [DecorationSpan],

  pub line_y: f32,

  pub line_top: f32,

  pub line_height: f32,

  pub line_w: f32,
}

impl LayoutRun<'_> {
  pub fn highlight(
    &self,
    cursor_start: Cursor,
    cursor_end: Cursor,
  ) -> Result<impl Iterator<Item = (f32, f32)>, console_core_never::Never> {
    let mut results = Vec::new();
    let mut selected = None;

    for glyph in self.glyphs {
      let Ok(start) = console_core_number_conversion::index(glyph.start);
      let Ok(end) = console_core_number_conversion::index(glyph.end);
      let cluster = match self.text.get(start..end) {
        Some(cluster) => cluster,
        None => continue,
      };
      let Ok(graphemes) = cluster.grapheme_indices(ClusterKind::Extended);
      let Ok(total) =
        console_core_number_conversion::fitted::<_, u32>(graphemes.clone().count().max(1));
      let Ok(count) = console_core_number_conversion::narrow_f32(f64::from(total));
      let width = glyph.w / count;
      let mut x = glyph.x;

      for (position, grapheme) in graphemes {
        let Ok(offset) = console_core_number_conversion::fitted::<_, u32>(position);
        let Ok(length) = console_core_number_conversion::fitted::<_, u32>(grapheme.len());
        let span_start = glyph.start.saturating_add(offset);
        let span_end = span_start.saturating_add(length);
        let included = (cursor_start.line != self.line_i || span_end > cursor_start.index)
          && (cursor_end.line != self.line_i || span_start < cursor_end.index);

        match (included, selected.take()) {
          (true, Some((left, right))) => {
            selected = Some((f32::min(left, x), f32::max(right, x + width)))
          }
          (true, None) => selected = Some((x, x + width)),
          (false, range) => {
            let Ok(()) = append_highlight(&mut results, range);
          }
        }

        x += width;
      }
    }

    let Ok(()) = append_highlight(&mut results, selected);
    Ok(results.into_iter())
  }

  pub fn cursor_position(
    &self,
    cursor: &Cursor,
  ) -> Result<Option<f32>, console_core_never::Never> {
    {
      let Ok(cursor_glyph) = self.cursor_glyph(cursor);
      let (glyph_idx, glyph_offset) = match cursor_glyph {
        Some(value) => value,
        None => return Ok(None),
      };
      let x = self
        .glyphs
        .get(match console_core_number_conversion::index(glyph_idx) {
          Ok(position) => position,
        })
        .map_or_else(
          || {
            self.glyphs.last().map_or(0.0, |glyph| {
              match ::core::primitive::bool::then_some(
                glyph.level.direction() == Ok(unicode_bidi::Direction::RightToLeft),
                (),
              ) {
                ::core::option::Option::Some(()) => glyph.x,
                ::core::option::Option::None => glyph.x + glyph.w,
              }
            })
          },
          |glyph| match ::core::primitive::bool::then_some(
            glyph.level.direction() == Ok(unicode_bidi::Direction::RightToLeft),
            (),
          ) {
            ::core::option::Option::Some(()) => glyph.x + glyph.w - glyph_offset,
            ::core::option::Option::None => glyph.x + glyph_offset,
          },
        );
      Ok(Some(x))
    }
  }

  pub fn cursor_glyph(
    &self,
    cursor: &Cursor,
  ) -> Result<Option<(u32, f32)>, console_core_never::Never> {
    match (cursor.line == self.line_i).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    for (position, glyph) in self.glyphs.iter().enumerate() {
      let Ok(glyph_i) = console_core_number_conversion::fitted::<_, u32>(position);

      match (
        cursor.index == glyph.start,
        glyph.start < cursor.index && cursor.index < glyph.end,
      ) {
        (true, _) => return Ok(Some((glyph_i, 0.0))),
        (false, true) => {
          let Ok(offset) = self.glyph_cursor_offset(glyph, crate::ByteIndex(cursor.index));
          return Ok(offset.map(|offset_2| (glyph_i, offset_2)));
        }
        (false, false) => {}
      }
    }

    match self
      .glyphs
      .iter()
      .enumerate()
      .find(|(_, glyph)| cursor.index == glyph.end)
    {
      Some((position, glyph)) => {
        let Ok(glyph_i) = console_core_number_conversion::fitted::<_, u32>(position);
        Ok(Some((glyph_i, glyph.w)))
      }
      None => Ok(self.glyphs.is_empty().then_some((0, 0.0))),
    }
  }

  fn glyph_cursor_offset(
    &self,
    glyph: &LayoutGlyph,
    crate::ByteIndex(cursor): crate::ByteIndex,
  ) -> Result<Option<f32>, console_core_never::Never> {
    let Ok(start) = console_core_number_conversion::index(glyph.start);
    let Ok(end) = console_core_number_conversion::index(glyph.end);
    let cluster = match self.text.get(start..end) {
      Some(cluster) => cluster,
      None => return Ok(None),
    };
    let Ok(graphemes) = cluster.grapheme_indices(ClusterKind::Extended);
    let mut before: i32 = 0;
    let mut total: i32 = 0;

    for (position, _) in graphemes {
      let Ok(offset) = console_core_number_conversion::fitted::<_, u32>(position);

      match (glyph.start.saturating_add(offset) < cursor).then_some(()) {
        Some(()) => before = before.saturating_add(1),
        None => {}
      }

      total = total.saturating_add(1);
    }

    let Ok(prefix) = console_core_number_conversion::narrow_f32(f64::from(before));
    let Ok(divisor) = console_core_number_conversion::narrow_f32(f64::from(total));
    Ok(Some(glyph.w * prefix / divisor))
  }

  fn hit_x(
    &self,
    crate::HorizontalPosition(x): crate::HorizontalPosition,
  ) -> Result<Option<Cursor>, console_core_never::Never> {
    let first = self.glyphs.first().filter(|glyph| match self.direction {
      crate::TextDirection::RightToLeft => x > glyph.x,
      crate::TextDirection::LeftToRight => x < 0.0,
    });

    for glyph in self.glyphs {
      match (x >= glyph.x && x <= glyph.x + glyph.w).then_some(()) {
        Some(()) => return self.hit_glyph(glyph, crate::HorizontalPosition(x)),
        None => {}
      }
    }

    let (position, affinity) = match first {
      Some(glyph) => (glyph.start, Affinity::After),
      None => {
        let end = self.glyphs.iter().map(|glyph| glyph.end).max();
        (end.map_or(0, |position| position), Affinity::Before)
      }
    };
    let Ok(cursor) = Cursor::new_with_affinity(self.line_i, crate::ByteIndex(position), affinity);

    Ok(Some(cursor))
  }

  fn hit_glyph(
    &self,
    glyph: &LayoutGlyph,
    crate::HorizontalPosition(x): crate::HorizontalPosition,
  ) -> Result<Option<Cursor>, console_core_never::Never> {
    let Ok(start) = console_core_number_conversion::index(glyph.start);
    let Ok(end) = console_core_number_conversion::index(glyph.end);
    let cluster = match self.text.get(start..end) {
      Some(cluster) => cluster,
      None => return Ok(None),
    };
    let Ok(graphemes) = cluster.grapheme_indices(ClusterKind::Extended);
    let Ok(total) = console_core_number_conversion::fitted::<_, u32>(graphemes.clone().count());
    let Ok(count) = console_core_number_conversion::narrow_f32(f64::from(total));
    let width = glyph.w / count;
    let rtl = glyph.level.direction() == Ok(unicode_bidi::Direction::RightToLeft);
    let mut left = glyph.x;

    for (position, grapheme) in graphemes {
      match (x >= left && x <= left + width).then_some(()) {
        Some(()) => {
          let Ok(offset) = console_core_number_conversion::fitted::<_, u32>(position);
          let Ok(length) = console_core_number_conversion::fitted::<_, u32>(grapheme.len());
          let edge = (x >= left + width / 2.0) != rtl;
          let (index, affinity) = match edge.then_some(()) {
            Some(()) => (
              glyph.start.saturating_add(offset.saturating_add(length)),
              Affinity::Before,
            ),
            None => (glyph.start.saturating_add(offset), Affinity::After),
          };
          let Ok(cursor) =
            Cursor::new_with_affinity(self.line_i, crate::ByteIndex(index), affinity);
          return Ok(Some(cursor));
        }
        None => left += width,
      }
    }

    let edge = (x >= glyph.x + glyph.w / 2.0) != rtl;
    let (index, affinity) = match edge.then_some(()) {
      Some(()) => {
        let Ok(length) = console_core_number_conversion::fitted::<_, u32>(cluster.len());
        (glyph.start.saturating_add(length), Affinity::Before)
      }
      None => (glyph.start, Affinity::After),
    };
    let Ok(cursor) = Cursor::new_with_affinity(self.line_i, crate::ByteIndex(index), affinity);

    Ok(Some(cursor))
  }

  pub const fn cursor_from_glyph_left(
    &self,
    glyph: &LayoutGlyph,
  ) -> Result<Cursor, console_core_never::Never> {
    {
      Ok(match self.direction {
        crate::TextDirection::RightToLeft => {
          let Ok(cursor) =
            Cursor::new_with_affinity(self.line_i, crate::ByteIndex(glyph.end), Affinity::Before);

          cursor
        }
        crate::TextDirection::LeftToRight => {
          let Ok(cursor) = Cursor::new_with_affinity(
            self.line_i,
            crate::ByteIndex(glyph.start),
            Affinity::After,
          );

          cursor
        }
      })
    }
  }

  pub const fn cursor_from_glyph_right(
    &self,
    glyph: &LayoutGlyph,
  ) -> Result<Cursor, console_core_never::Never> {
    {
      Ok(match self.direction {
        crate::TextDirection::RightToLeft => {
          let Ok(cursor) = Cursor::new_with_affinity(
            self.line_i,
            crate::ByteIndex(glyph.start),
            Affinity::After,
          );

          cursor
        }
        crate::TextDirection::LeftToRight => {
          let Ok(cursor) =
            Cursor::new_with_affinity(self.line_i, crate::ByteIndex(glyph.end), Affinity::Before);

          cursor
        }
      })
    }
  }
}

fn append_highlight(
  results: &mut Vec<(f32, f32)>,
  range: Option<(f32, f32)>,
) -> Result<(), console_core_never::Never> {
  match range {
    Some((start, end)) => {
      let width = end - start;

      match (width > 0.0).then_some(()) {
        Some(()) => results.push((start, width)),
        None => {}
      }
    }
    None => {}
  }

  Ok(())
}

fn rich_segments<'r, 's>(
  spans: impl IntoIterator<Item = (&'s str, Attrs<'r>)>,
) -> Result<(String, Vec<(Attrs<'r>, core::ops::Range<u32>)>), console_core_never::Never> {
  let mut text = String::new();
  let mut attributes = Vec::new();

  for (segment, attrs) in spans {
    let Ok(start) = console_core_number_conversion::fitted::<_, u32>(text.len());
    text.push_str(segment);
    let Ok(end) = console_core_number_conversion::fitted::<_, u32>(text.len());
    attributes.push((attrs, start..end));
  }

  Ok((text, attributes))
}

fn append_rich_segment(
  content: &mut String,
  attrs_list: &mut AttrsList,
  segment: &str,
  attrs: &Attrs,
) -> Result<(), console_core_never::Never> {
  let Ok(start) = console_core_number_conversion::fitted::<_, u32>(content.len());
  content.push_str(segment);
  let Ok(end) = console_core_number_conversion::fitted::<_, u32>(content.len());
  let Ok(defaults) = attrs_list.defaults();

  match (*attrs != defaults).then_some(()) {
    Some(()) => attrs_list.add_span(start..end, attrs),
    None => Ok(()),
  }
}

fn advance_placed_line(
  count: &mut u32,
  placed: crate::Presence,
) -> Result<(), console_core_never::Never> {
  match placed {
    crate::Presence::Present => *count = count.saturating_add(1),
    crate::Presence::Absent => {}
  }

  Ok(())
}

#[derive(Debug)]
pub struct LayoutRunIter<'b> {
  lines: &'b [BufferLine],
  height_opt: Option<f32>,
  line_height: crate::LineHeight,
  scroll: crate::ScrollOffset,
  line_i: u32,
  layout_i: u32,
  line_top: crate::VerticalPosition,
}

impl<'b> LayoutRunIter<'b> {
  pub const fn new(buffer: &'b Buffer) -> Result<Self, console_core_never::Never> {
    {
      let Ok(layout_run_iter) = Self::from_lines(
        buffer.lines.as_slice(),
        buffer.height_opt,
        buffer.metrics.line_height,
        crate::ScrollOffset(buffer.scroll.vertical),
        buffer.scroll.line,
      );

      Ok(layout_run_iter)
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the const iterator constructor accepts the existing scalar line height in layout pixels"
    )
  )]
  pub const fn from_lines(
    lines: &'b [BufferLine],
    height_opt: Option<f32>,
    line_height: f32,
    scroll: crate::ScrollOffset,
    start: u32,
  ) -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        lines,
        height_opt,
        line_height: crate::LineHeight(line_height),
        scroll,
        line_i: start,
        layout_i: 0,
        line_top: crate::VerticalPosition(0.0),
      })
    }
  }
}

impl<'b> Iterator for LayoutRunIter<'b> {
  type Item = LayoutRun<'b>;

  fn next(&mut self) -> Option<Self::Item> {
    for line in self
      .lines
      .iter()
      .skip(match console_core_number_conversion::index(self.line_i) {
        Ok(position) => position,
      })
    {
      let Ok(shape_opt) = line.shape_opt();
      let shape = shape_opt?;
      let Ok(layout_opt) = line.layout_opt();
      let layout = layout_opt?;

      'text_items_13: for layout_line in
        layout
          .iter()
          .skip(match console_core_number_conversion::index(self.layout_i) {
            Ok(position) => position,
          })
      {
        self.layout_i = (self.layout_i).saturating_add(1);

        let line_height = match layout_line.line_height_opt {
          Some(present) => present,
          None => self.line_height.0,
        };
        let line_top = self.line_top.0 - self.scroll.0;
        let glyph_height = layout_line.max_ascent + layout_line.max_descent;
        let centering_offset = (line_height - glyph_height) / 2.0;
        let line_y = line_top + centering_offset + layout_line.max_ascent;

        match self.height_opt {
          Some(height) => {
            match ::core::primitive::bool::then_some(line_y - layout_line.max_ascent > height, ())
            {
              ::core::option::Option::Some(()) => {
                return None;
              }
              ::core::option::Option::None => {}
            }
          }
          None => {}
        }

        self.line_top.0 += line_height;

        match ::core::primitive::bool::then_some(line_y + layout_line.max_descent < 0.0, ()) {
          ::core::option::Option::Some(()) => {
            continue 'text_items_13;
          }
          ::core::option::Option::None => {}
        }

        let Ok(text) = line.text();
        return Some(LayoutRun {
          line_i: self.line_i,
          text,
          direction: shape.direction,
          glyphs: &layout_line.glyphs,
          decorations: &layout_line.decorations,
          line_y,
          line_top,
          line_height,
          line_w: layout_line.w,
        });
      }

      self.line_i = (self.line_i).saturating_add(1);
      self.layout_i = 0;
    }

    None
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "public base font size and line height remain fractional layout pixels for existing renderers"
  )
)]
pub struct Metrics {
  pub font_size: f32,

  pub line_height: f32,
}

impl Metrics {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public const constructor accepts the existing font-size scalar in layout pixels"
    )
  )]
  pub const fn new(
    font_size: f32,
    line_height: crate::LineHeight,
  ) -> Result<Self, console_core_never::Never> {
    let crate::LineHeight(line_height_2) = line_height;

    {
      Ok(Self {
        font_size,
        line_height: line_height_2,
      })
    }
  }

  pub fn relative(
    size: impl Into<crate::FontSize>,
    line_height_scale: crate::LineHeightScale,
  ) -> Result<Self, console_core_never::Never> {
    let crate::FontSize(font_size) = Into::<crate::FontSize>::into(size);
    let crate::LineHeightScale(line_height_scale_2) = line_height_scale;

    {
      Ok(Self {
        font_size,
        line_height: font_size * line_height_scale_2,
      })
    }
  }

  pub fn scale(
    self,
    factor: impl Into<crate::ScaleFactor>,
  ) -> Result<Self, console_core_never::Never> {
    let crate::ScaleFactor(scale) = Into::<crate::ScaleFactor>::into(factor);

    {
      Ok(Self {
        font_size: self.font_size * scale,
        line_height: self.line_height * scale,
      })
    }
  }
}

impl fmt::Display for Metrics {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}px / {}px", self.font_size, self.line_height)
  }
}

#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "Redraw records whether a frame is stale; width, height, and monospace width independently constrain layout, so each absence is valid with either redraw state."
  )
)]
pub struct Buffer {
  pub lines: Vec<BufferLine>,
  metrics: Metrics,
  width_opt: Option<f32>,
  height_opt: Option<f32>,
  scroll: Scroll,

  redraw: crate::Redraw,
  wrap: Wrap,
  ellipsize: Ellipsize,
  monospace_width: Option<f32>,
  tab_width: u16,
  hinting: Hinting,

  dirty: DirtyFlags,
}

impl Clone for Buffer {
  fn clone(&self) -> Self {
    Self {
      lines: self.lines.clone(),
      metrics: self.metrics,
      width_opt: self.width_opt,
      height_opt: self.height_opt,
      scroll: self.scroll,
      redraw: self.redraw,
      wrap: self.wrap,
      ellipsize: self.ellipsize,
      monospace_width: self.monospace_width,
      tab_width: self.tab_width,
      hinting: self.hinting,
      dirty: self.dirty,
    }
  }
}

impl Buffer {
  pub fn new_empty(metrics: Metrics) -> Result<Self, console_core_never::Never> {
    {
      assert_ne!(metrics.line_height, 0.0, "line height cannot be 0");
      Ok(Self {
        lines: Vec::new(),
        metrics,
        width_opt: None,
        height_opt: None,
        scroll: Scroll::default(),
        redraw: crate::Redraw::Current,
        wrap: Wrap::WordOrGlyph,
        ellipsize: Ellipsize::None,
        monospace_width: None,
        tab_width: 8,
        hinting: Hinting::default(),
        dirty: DirtyFlags::EMPTY,
      })
    }
  }

  pub fn new(
    font_system: &mut FontSystem,
    metrics: Metrics,
  ) -> Result<Self, console_core_never::Never> {
    {
      let Ok(mut buffer) = Self::new_empty(metrics);

      let Ok(attrs) = Attrs::new();

      let Ok(()) = buffer.set_text("", &attrs, Shaping::Advanced, None);

      let Ok(()) = buffer.shape_until_scroll(font_system, crate::Pruning::Preserve);

      Ok(buffer)
    }
  }

  pub fn borrow_with<'a>(
    &'a mut self,
    font_system: &'a mut FontSystem,
  ) -> Result<BorrowedWithFontSystem<'a, Self>, console_core_never::Never> {
    {
      Ok(BorrowedWithFontSystem {
        inner: self,
        font_system,
      })
    }
  }

  fn resolve_dirty(&mut self) -> Result<crate::Redraw, console_core_never::Never> {
    let dirty = self.dirty;

    match (dirty == DirtyFlags::EMPTY).then_some(()) {
      Some(()) => {
        let needs_shaping = self
          .lines
          .iter()
          .any(|line| line.needs_reshaping() == Ok(crate::Reshaping::Needed));
        return Ok(match needs_shaping.then_some(()) {
          Some(()) => {
            self.redraw = crate::Redraw::Needed;
            crate::Redraw::Needed
          }
          None => crate::Redraw::Current,
        });
      }
      None => {}
    }

    match dirty.contains(DirtyFlags::TEXT_SET) {
      Ok(crate::Presence::Present) => {}
      Ok(crate::Presence::Absent) => {
        let Ok(()) = self.refresh_cached_lines(dirty);
      }
    }

    self.redraw = crate::Redraw::Needed;
    self.dirty = DirtyFlags::EMPTY;
    Ok(crate::Redraw::Needed)
  }

  fn refresh_cached_lines(&mut self, dirty: DirtyFlags) -> Result<(), console_core_never::Never> {
    let Ok(tab_shape) = dirty.contains(DirtyFlags::TAB_SHAPE);

    match tab_shape {
      crate::Presence::Present => {
        for line in &mut self.lines {
          let Ok(shape) = line.shape_opt();
          let Ok(text) = line.text();

          match (shape.is_some() && text.contains('\t')).then_some(()) {
            Some(()) => {
              let Ok(()) = line.reset_shaping();
            }
            None => {}
          }
        }
      }
      crate::Presence::Absent => {}
    }

    let Ok(relayout) = dirty.contains(DirtyFlags::RELAYOUT);

    match relayout {
      crate::Presence::Present => {
        for line in &mut self.lines {
          let Ok(shape) = line.shape_opt();

          match shape {
            Some(_) => {
              let Ok(()) = line.reset_layout();
            }
            None => {}
          }
        }
      }
      crate::Presence::Absent => {}
    }

    Ok(())
  }

  pub fn shape_until_cursor(
    &mut self,
    font_system: &mut FontSystem,
    cursor: Cursor,
    pruning: crate::Pruning,
  ) -> Result<(), console_core_never::Never> {
    let Ok(()) = self.shape_until_scroll(font_system, pruning);
    let old_scroll = self.scroll;
    let Ok(location) = self.layout_cursor(font_system, cursor);
    let located = match location {
      Some(located) => located,
      None => return Ok(()),
    };

    match self.reveal_cursor_line(font_system, located) {
      Ok(crate::Presence::Present) => {}
      Ok(crate::Presence::Absent) => return Ok(()),
    }

    match (old_scroll != self.scroll).then_some(()) {
      Some(()) => self.dirty |= DirtyFlags::SCROLL,
      None => {}
    }

    let Ok(()) = self.shape_until_scroll(font_system, pruning);
    let Ok(location_2) = self.layout_cursor(font_system, cursor);

    match location_2 {
      Some(located_2) => self.reveal_cursor_column(font_system, located_2),
      None => Ok(()),
    }
  }

  fn reveal_cursor_line(
    &mut self,
    font_system: &mut FontSystem,
    cursor: LayoutCursor,
  ) -> Result<crate::Presence, console_core_never::Never> {
    let default_height = self.metrics.line_height;
    let Ok(line_layout) = self.line_layout(font_system, cursor.line);
    let lines = match line_layout {
      Some(lines) => lines,
      None => return Ok(crate::Presence::Absent),
    };
    let Ok(index) = console_core_number_conversion::index(cursor.layout);
    let current = match lines.get(index) {
      Some(line) => line,
      None => return Ok(crate::Presence::Absent),
    };
    let mut top = 0.0;

    for line in lines.iter().take(index) {
      top += match line.line_height_opt {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => default_height,
      };
    }

    let bottom = top
      + match current.line_height_opt {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => default_height,
      };
    let above = self.scroll.line > cursor.line
      || (self.scroll.line == cursor.line && self.scroll.vertical > top);

    match above.then_some(()) {
      Some(()) => {
        self.scroll.line = cursor.line;
        self.scroll.vertical = top;
        return Ok(crate::Presence::Present);
      }
      None => {}
    }

    match self.height_opt {
      Some(height) => self.scroll_to_cursor_bottom(
        font_system,
        cursor.line,
        crate::VerticalPosition(bottom),
        crate::LineHeight(height),
      ),
      None => Ok(crate::Presence::Present),
    }
  }

  fn scroll_to_cursor_bottom(
    &mut self,
    font_system: &mut FontSystem,
    mut line: u32,
    crate::VerticalPosition(mut bottom): crate::VerticalPosition,
    crate::LineHeight(height): crate::LineHeight,
  ) -> Result<crate::Presence, console_core_never::Never> {
    match (line <= self.scroll.line).then_some(()) {
      Some(()) => {
        match (bottom > height + self.scroll.vertical).then_some(()) {
          Some(()) => self.scroll.vertical = bottom - height,
          None => {}
        }

        return Ok(crate::Presence::Present);
      }
      None => {}
    }

    let default_height = self.metrics.line_height;

    for _ in 0..line.saturating_sub(self.scroll.line) {
      match (line > self.scroll.line).then_some(()) {
        Some(()) => {}
        None => break,
      }

      line = line.saturating_sub(1);
      let Ok(layout) = self.line_layout(font_system, line);
      let lines = match layout {
        Some(lines) => lines,
        None => return Ok(crate::Presence::Absent),
      };

      for shape in lines {
        bottom += match shape.line_height_opt {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => default_height,
        };
      }

      match (bottom > height + self.scroll.vertical).then_some(()) {
        Some(()) => {
          self.scroll.line = line;
          self.scroll.vertical = bottom - height;
        }
        None => {}
      }
    }

    Ok(crate::Presence::Present)
  }

  fn reveal_cursor_column(
    &mut self,
    font_system: &mut FontSystem,
    cursor: LayoutCursor,
  ) -> Result<(), console_core_never::Never> {
    let Ok(index) = console_core_number_conversion::index(cursor.layout);
    let Ok(glyph_index) = console_core_number_conversion::index(cursor.glyph);
    let Ok(layout) = self.line_layout(font_system, cursor.line);
    let line = match layout.and_then(|lines| lines.get(index)) {
      Some(line) => line,
      None => return Ok(()),
    };
    let (left, right) = line
      .glyphs
      .get(glyph_index)
      .or_else(|| line.glyphs.last())
      .map_or((0.0, 0.0), |glyph| {
        let start = glyph.x;
        let end = glyph.x + glyph.w;
        (start.min(end), start.max(end))
      });

    match (left < self.scroll.horizontal).then_some(()) {
      Some(()) => {
        self.scroll.horizontal = left;
        self.redraw = crate::Redraw::Needed;
      }
      None => {}
    }

    match self
      .width_opt
      .filter(|width| right > self.scroll.horizontal + width)
    {
      Some(width) => {
        self.scroll.horizontal = right - width;
        self.redraw = crate::Redraw::Needed;
      }
      None => {}
    }

    Ok(())
  }

  pub fn shape_until_scroll(
    &mut self,
    font_system: &mut FontSystem,
    pruning: crate::Pruning,
  ) -> Result<(), console_core_never::Never> {
    match self.resolve_dirty() {
      Ok(crate::Redraw::Current) => return Ok(()),
      Ok(crate::Redraw::Needed) => {}
    }

    let Ok(line_count) = console_core_number_conversion::fitted::<_, u32>(self.lines.len());

    match (self.scroll.line >= line_count).then_some(()) {
      Some(()) => {
        self.scroll.line = line_count.saturating_sub(1);
        self.scroll.vertical = 0.0;
      }
      None => {}
    }

    let old_scroll = self.scroll;
    let Ok(step_limit) =
      console_core_number_conversion::fitted::<_, u32>(self.lines.len().saturating_add(1));

    for _ in 0..step_limit {
      let Ok(()) = self.move_scroll_back(font_system);
      let end = self.scroll.vertical
        + match self.height_opt {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => f32::INFINITY,
        };
      let Ok(height) =
        self.shape_visible_lines(font_system, crate::VerticalPosition(end), pruning);
      let total_height = match height {
        Some(crate::LineHeight(height_2)) => height_2,
        None => return Ok(()),
      };

      match (total_height < end && self.scroll.line > 0).then_some(()) {
        Some(()) => self.scroll.vertical -= end - total_height,
        None => break,
      }
    }

    match (old_scroll != self.scroll).then_some(()) {
      Some(()) => self.redraw = crate::Redraw::Needed,
      None => {}
    }

    Ok(())
  }

  fn move_scroll_back(
    &mut self,
    font_system: &mut FontSystem,
  ) -> Result<(), console_core_never::Never> {
    let default_height = self.metrics.line_height;

    for _ in 0..self.scroll.line.saturating_add(1) {
      match (self.scroll.vertical < 0.0, self.scroll.line) {
        (false, _) => break,
        (true, 0) => {
          self.scroll.vertical = 0.0;
          break;
        }
        (true, line) => {
          let previous = line.saturating_sub(1);
          let Ok(layout) = self.line_layout(font_system, previous);
          let height = match layout {
            Some(lines) => lines.iter().fold(0.0, |height, line_2| {
              height
                + match line_2.line_height_opt {
                  ::core::option::Option::Some(option_value) => option_value,
                  ::core::option::Option::None => default_height,
                }
            }),
            None => default_height,
          };
          self.scroll.line = previous;
          self.scroll.vertical += height;
        }
      }
    }

    Ok(())
  }

  fn shape_visible_lines(
    &mut self,
    font_system: &mut FontSystem,
    crate::VerticalPosition(end): crate::VerticalPosition,
    pruning: crate::Pruning,
  ) -> Result<Option<crate::LineHeight>, console_core_never::Never> {
    match pruning {
      crate::Pruning::Discard => {
        let Ok(before) = console_core_number_conversion::index(self.scroll.line);

        for line in self.lines.iter_mut().take(before) {
          let Ok(()) = line.reset_shaping();
        }
      }
      crate::Pruning::Preserve => {}
    }

    let mut total_height = 0.0;
    let default_height = self.metrics.line_height;
    let Ok(length) = console_core_number_conversion::fitted::<_, u32>(self.lines.len());

    for line in self.scroll.line..length {
      match (total_height > end, pruning) {
        (true, crate::Pruning::Preserve) => break,
        (true, crate::Pruning::Discard) => {
          let Ok(index) = console_core_number_conversion::index(line);

          match self.lines.get_mut(index) {
            Some(line_2) => {
              let Ok(()) = line_2.reset_shaping();
            }
            None => {}
          }

          continue;
        }
        (false, _) => {}
      }

      let Ok(layout) = self.line_layout(font_system, line);
      let lines = match layout {
        Some(lines) => lines,
        None => return Ok(None),
      };
      let mut layout_height = 0.0;

      for layout_2 in lines {
        let height = match layout_2.line_height_opt {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => default_height,
        };
        layout_height += height;
        total_height += height;
      }

      match (line == self.scroll.line && layout_height <= self.scroll.vertical).then_some(()) {
        Some(()) => {
          self.scroll.line = self.scroll.line.saturating_add(1);
          self.scroll.vertical -= layout_height;
        }
        None => {}
      }
    }

    Ok(Some(crate::LineHeight(total_height)))
  }

  pub fn layout_cursor(
    &mut self,
    font_system: &mut FontSystem,
    cursor: Cursor,
  ) -> Result<Option<LayoutCursor>, console_core_never::Never> {
    let Ok(line_layout) = self.line_layout(font_system, cursor.line);
    let layout = match line_layout {
      Some(layout) => layout,
      None => return Ok(None),
    };

    for (layout_index, line) in layout.iter().enumerate() {
      let Ok(line_number) = console_core_number_conversion::fitted::<_, u32>(layout_index);

      'inner_for_2: for (glyph_index, glyph) in line.glyphs.iter().enumerate() {
        let Ok(start) =
          Cursor::new_with_affinity(cursor.line, crate::ByteIndex(glyph.start), Affinity::After);
        let Ok(end) =
          Cursor::new_with_affinity(cursor.line, crate::ByteIndex(glyph.end), Affinity::Before);
        let (left, right) = match glyph.level.direction() {
          Ok(unicode_bidi::Direction::LeftToRight) => (start, end),
          Ok(unicode_bidi::Direction::RightToLeft) => (end, start),
        };
        let Ok(index) = console_core_number_conversion::fitted::<_, u32>(glyph_index);
        let placement = match (cursor == left, cursor == right) {
          (true, _) => index,
          (false, true) => index.saturating_add(1),
          (false, false) => continue 'inner_for_2,
        };
        let Ok(located) = LayoutCursor::new(
          cursor.line,
          crate::LayoutIndex(line_number),
          crate::GlyphIndex(placement),
        );
        return Ok(Some(located));
      }
    }

    let Ok(fallback) =
      LayoutCursor::new(cursor.line, crate::LayoutIndex(0), crate::GlyphIndex(0));
    Ok(Some(fallback))
  }

  pub fn line_shape(
    &mut self,
    font_system: &mut FontSystem,
    line_i: u32,
  ) -> Result<Option<&ShapeLine>, console_core_never::Never> {
    {
      let line = match self
        .lines
        .get_mut(match console_core_number_conversion::index(line_i) {
          Ok(position) => position,
        }) {
        Some(value) => value,
        None => return Ok(None),
      };
      let Ok(shape) = line.shape(font_system, self.tab_width);

      Ok(Some(shape))
    }
  }

  pub fn line_layout(
    &mut self,
    font_system: &mut FontSystem,
    line_i: u32,
  ) -> Result<Option<&[LayoutLine]>, console_core_never::Never> {
    {
      let line = match self
        .lines
        .get_mut(match console_core_number_conversion::index(line_i) {
          Ok(position) => position,
        }) {
        Some(value) => value,
        None => return Ok(None),
      };
      let Ok(layout) = line.layout(
        font_system,
        crate::BufferLineLayout {
          font_size: self.metrics.font_size,
          width_opt: self.width_opt,
          wrap: self.wrap,
          ellipsize: self.ellipsize,
          match_mono_width: self.monospace_width,
          tab_width: self.tab_width,
          hinting: self.hinting,
        },
      );

      Ok(Some(layout))
    }
  }

  pub const fn metrics(&self) -> Result<Metrics, console_core_never::Never> {
    Ok(self.metrics)
  }

  pub fn set_metrics(&mut self, metrics: Metrics) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(metrics != self.metrics, ()) {
        ::core::option::Option::Some(()) => {
          assert_ne!(metrics.font_size, 0.0, "font size cannot be 0");
          assert_ne!(metrics.line_height, 0.0, "line height cannot be 0");
          self.metrics = metrics;
          self.dirty |= DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  pub const fn hinting(&self) -> Result<Hinting, console_core_never::Never> {
    Ok(self.hinting)
  }

  pub fn set_hinting(&mut self, hinting: Hinting) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(hinting != self.hinting, ()) {
        ::core::option::Option::Some(()) => {
          self.hinting = hinting;
          self.dirty |= DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  pub const fn wrap(&self) -> Result<Wrap, console_core_never::Never> {
    Ok(self.wrap)
  }

  pub fn set_wrap(&mut self, wrap: Wrap) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(wrap != self.wrap, ()) {
        ::core::option::Option::Some(()) => {
          self.wrap = wrap;
          self.dirty |= DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  pub const fn ellipsize(&self) -> Result<Ellipsize, console_core_never::Never> {
    Ok(self.ellipsize)
  }

  pub fn set_ellipsize(&mut self, ellipsize: Ellipsize) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(ellipsize != self.ellipsize, ()) {
        ::core::option::Option::Some(()) => {
          self.ellipsize = ellipsize;
          self.dirty |= DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  pub const fn monospace_width(&self) -> Result<Option<f32>, console_core_never::Never> {
    Ok(self.monospace_width)
  }

  pub fn set_monospace_width(
    &mut self,
    monospace_width: Option<f32>,
  ) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(monospace_width != self.monospace_width, ()) {
        ::core::option::Option::Some(()) => {
          self.monospace_width = monospace_width;
          self.dirty |= DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  pub const fn tab_width(&self) -> Result<u16, console_core_never::Never> {
    Ok(self.tab_width)
  }

  pub fn set_tab_width(&mut self, tab_width: u16) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(tab_width == 0, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(tab_width != self.tab_width, ()) {
        ::core::option::Option::Some(()) => {
          self.tab_width = tab_width;
          self.dirty |= DirtyFlags::TAB_SHAPE | DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  pub const fn size(&self) -> Result<(Option<f32>, Option<f32>), console_core_never::Never> {
    Ok((self.width_opt, self.height_opt))
  }

  pub fn set_size(
    &mut self,
    width_opt: Option<f32>,
    height_opt: Option<f32>,
  ) -> Result<(), console_core_never::Never> {
    {
      let width_clamped = width_opt.map(|v| v.max(0.0));
      let height_clamped = height_opt.map(|v| v.max(0.0));

      match ::core::primitive::bool::then_some(width_clamped != self.width_opt, ()) {
        ::core::option::Option::Some(()) => {
          self.width_opt = width_clamped;
          self.dirty |= DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(height_clamped != self.height_opt, ()) {
        ::core::option::Option::Some(()) => {
          self.height_opt = height_clamped;
          self.dirty |= DirtyFlags::RELAYOUT;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  pub fn set_metrics_and_size(
    &mut self,
    metrics: Metrics,
    width_opt: Option<f32>,
    height_opt: Option<f32>,
  ) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.set_metrics(metrics);

      let Ok(()) = self.set_size(width_opt, height_opt);

      Ok(())
    }
  }

  pub const fn scroll(&self) -> Result<Scroll, console_core_never::Never> {
    Ok(self.scroll)
  }

  pub fn set_scroll(&mut self, scroll: Scroll) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(scroll != self.scroll, ()) {
        ::core::option::Option::Some(()) => {
          self.scroll = scroll;
          self.dirty |= DirtyFlags::SCROLL;
          self.redraw = crate::Redraw::Needed;
        }
        ::core::option::Option::None => {}
      };

      Ok(())
    }
  }

  fn set_text_impl(
    &mut self,
    text: &str,
    attrs: &Attrs,
    shaping: Shaping,
    alignment: Option<Align>,
  ) -> Result<(), console_core_never::Never> {
    let mut line_count = 0u32;
    let mut last_ending = LineEnding::default();
    let Ok(lines) = LineIter::new(text);

    for (range, ending) in lines {
      let Ok(start) = console_core_number_conversion::index(range.start);
      let Ok(end) = console_core_number_conversion::index(range.end);
      let line_text = match text.get(start..end) {
        Some(slice) => slice,
        None => continue,
      };
      let Ok(()) = self.reset_text_line(line_count, line_text, ending, attrs, shaping);
      last_ending = ending;
      line_count = line_count.saturating_add(1);
    }

    match last_ending {
      LineEnding::None => {}
      LineEnding::Lf | LineEnding::CrLf | LineEnding::Cr | LineEnding::LfCr => {
        let Ok(()) = self.reset_text_line(line_count, "", LineEnding::None, attrs, shaping);
        line_count = line_count.saturating_add(1);
      }
    }

    let Ok(length) = console_core_number_conversion::index(line_count);
    self.lines.truncate(length);

    match alignment {
      Some(align) => {
        for line in &mut self.lines {
          let Ok(_change) = line.set_align(Some(align));
        }
      }
      None => {}
    }

    self.scroll = Scroll::default();
    Ok(())
  }

  fn reset_text_line(
    &mut self,
    line_number: u32,
    value: &str,
    ending: LineEnding,
    attrs: &Attrs,
    shaping: Shaping,
  ) -> Result<(), console_core_never::Never> {
    let Ok(index) = console_core_number_conversion::index(line_number);

    match self.lines.get_mut(index) {
      Some(line) => {
        let Ok(mut reused_text) = line.reclaim_text();
        reused_text.push_str(value);
        let Ok(old_attrs) = line.reclaim_attrs();
        let Ok(reused_attrs) = old_attrs.reset(attrs);
        let Ok(()) = line.reset_new(reused_text, ending, reused_attrs, shaping);
      }
      None => {
        let Ok(attrs_list) = AttrsList::new(attrs);
        let Ok(line) = BufferLine::new(value, ending, attrs_list, shaping);
        self.lines.push(line);
      }
    }

    Ok(())
  }

  pub fn set_text(
    &mut self,
    text: &str,
    attrs: &Attrs,
    shaping: Shaping,
    alignment: Option<Align>,
  ) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.set_text_impl(text, attrs, shaping, alignment);

      self.dirty |= DirtyFlags::TEXT_SET;
      self.redraw = crate::Redraw::Needed;

      Ok(())
    }
  }

  fn set_rich_text_impl<'r, 's, I>(
    &mut self,
    spans: I,
    default_attrs: &Attrs,
    shaping: Shaping,
    alignment: Option<Align>,
  ) -> Result<(), console_core_never::Never>
  where
    I: IntoIterator<Item = (&'s str, Attrs<'r>)>,
  {
    let Ok((text, segments)) = rich_segments(spans);
    let Ok(steps) = console_core_number_conversion::fitted::<_, u32>(
      text.len().saturating_add(segments.len()).saturating_add(1),
    );
    let mut spans_iter = segments.into_iter();
    let mut next_span = spans_iter.next();
    let Ok(paragraphs) = BidiParagraphs::new(&text);
    let mut lines = paragraphs.map(|line| {
      let Ok(start) = console_core_number_conversion::fitted::<_, u32>(
        line.as_ptr().addr().saturating_sub(text.as_ptr().addr()),
      );
      let Ok(length) = console_core_number_conversion::fitted::<_, u32>(line.len());
      start..start.saturating_add(length)
    });
    let mut next_line = lines.next();
    let mut line_count = 0;
    let Ok((mut content, mut attrs_list)) = self.reclaim_rich_line(line_count, default_attrs);

    for _ in 0..steps {
      let (line, attrs, span) = match (&next_line, &next_span) {
        (Some(line), Some((attrs, span))) => (line, attrs, span),
        (None, _) | (_, None) => {
          let Ok(attrs) = AttrsList::new(default_attrs);
          let Ok(placed) = self.store_rich_line(line_count, (String::new(), attrs), shaping);
          let Ok(()) = advance_placed_line(&mut line_count, placed);
          break;
        }
      };
      let start = line.start.max(span.start);
      let end = line.end.min(span.end);

      match (
        start < end,
        content.is_empty() && attrs.metrics_opt.is_some(),
      ) {
        (true, _) => {
          let Ok(range) = console_core_number_conversion::indices(start..end);
          let segment = match text.get(range) {
            Some(segment) => segment,
            None => break,
          };
          let Ok(()) = append_rich_segment(&mut content, &mut attrs_list, segment, attrs);
        }
        (false, true) => {
          let Ok(reset) = attrs_list.reset(attrs);
          attrs_list = reset;
        }
        (false, false) => {}
      }

      match (span.end < line.end).then_some(()) {
        Some(()) => {
          next_span = spans_iter.next();
          continue;
        }
        None => {}
      }

      next_line = lines.next();

      match next_line {
        Some(_) => {
          let Ok((text_buffer, attribute_buffer)) =
            self.reclaim_rich_line(line_count.saturating_add(1), default_attrs);
          let old_text = core::mem::replace(&mut content, text_buffer);
          let old_attrs = core::mem::replace(&mut attrs_list, attribute_buffer);

          match self.store_rich_line(line_count, (old_text, old_attrs), shaping) {
            Ok(crate::Presence::Present) => line_count = line_count.saturating_add(1),
            Ok(crate::Presence::Absent) => break,
          }
        }
        None => {
          let Ok(placed) = self.store_rich_line(line_count, (content, attrs_list), shaping);
          let Ok(()) = advance_placed_line(&mut line_count, placed);
          break;
        }
      }
    }

    let Ok(count) = console_core_number_conversion::index(line_count);
    self.lines.truncate(count);

    for line in &mut self.lines {
      let Ok(_) = line.set_align(alignment);
    }

    self.scroll = Scroll::default();
    Ok(())
  }

  fn reclaim_rich_line(
    &mut self,
    line: u32,
    defaults: &Attrs,
  ) -> Result<(String, AttrsList), console_core_never::Never> {
    let Ok(index) = console_core_number_conversion::index(line);

    match self.lines.get_mut(index) {
      Some(line_2) => {
        let Ok(attrs) = line_2.reclaim_attrs();
        let Ok(reset) = attrs.reset(defaults);
        let Ok(text) = line_2.reclaim_text();
        Ok((text, reset))
      }
      None => {
        let Ok(attrs) = AttrsList::new(defaults);
        Ok((String::new(), attrs))
      }
    }
  }

  fn store_rich_line(
    &mut self,
    line: u32,
    (text, attrs): (String, AttrsList),
    shaping: Shaping,
  ) -> Result<crate::Presence, console_core_never::Never> {
    let Ok(index) = console_core_number_conversion::index(line);
    let append = self.lines.len() == index;

    match (self.lines.get_mut(index), append) {
      (Some(line_2), _) => {
        let Ok(()) = line_2.reset_new(text, LineEnding::default(), attrs, shaping);
      }
      (None, true) => {
        let Ok(line_3) = BufferLine::new(text, LineEnding::default(), attrs, shaping);
        self.lines.push(line_3);
      }
      (None, false) => return Ok(crate::Presence::Absent),
    }

    Ok(crate::Presence::Present)
  }

  pub fn set_rich_text<'r, 's, I>(
    &mut self,
    spans: I,
    default_attrs: &Attrs,
    shaping: Shaping,
    alignment: Option<Align>,
  ) -> Result<(), console_core_never::Never>
  where
    I: IntoIterator<Item = (&'s str, Attrs<'r>)>,
  {
    {
      let Ok(()) = self.set_rich_text_impl(spans, default_attrs, shaping, alignment);

      self.dirty |= DirtyFlags::TEXT_SET;
      self.redraw = crate::Redraw::Needed;

      Ok(())
    }
  }

  pub const fn redraw(&self) -> Result<crate::Redraw, console_core_never::Never> {
    Ok(self.redraw)
  }

  pub fn set_redraw(
    &mut self,
    redrawing: crate::Redraw,
  ) -> Result<(), console_core_never::Never> {
    self.redraw = redrawing;

    Ok(())
  }

  pub fn layout_runs(&self) -> Result<LayoutRunIter<'_>, console_core_never::Never> {
    {
      let Ok(layout_run_iter) = LayoutRunIter::new(self);

      Ok(layout_run_iter)
    }
  }

  pub fn hit(
    &self,
    horizontal: impl Into<crate::HorizontalPosition>,
    crate::VerticalPosition(y): crate::VerticalPosition,
  ) -> Result<Option<Cursor>, console_core_never::Never> {
    let crate::HorizontalPosition(x) = Into::<crate::HorizontalPosition>::into(horizontal);
    let Ok(mut runs) = self.layout_runs();
    let mut pending = runs.next();
    let mut first_run = true;
    let mut selected = None;

    for following in runs.map(Some).chain(core::iter::once(None)) {
      let run = match core::mem::replace(&mut pending, following) {
        Some(run) => run,
        None => return Ok(None),
      };
      let top = run.line_top;
      let above = first_run && y < top;
      let inside = y >= top && y < top + run.line_height;
      let below_last = pending.is_none() && y > run.line_y;

      match (above, inside, below_last) {
        (true, _, _) => {
          first_run = false;
          let Ok(cursor) = Cursor::new(run.line_i, crate::ByteIndex(0));
          selected = Some(cursor);
        }
        (false, true, _) => match run.hit_x(crate::HorizontalPosition(x)) {
          Ok(Some(cursor)) => {
            selected = Some(cursor);
            break;
          }
          Ok(None) => return Ok(None),
        },
        (false, false, true) => {
          let Ok(length) = console_core_number_conversion::fitted::<_, u32>(run.text.len());
          let Ok(cursor) =
            Cursor::new_with_affinity(run.line_i, crate::ByteIndex(length), Affinity::Before);
          selected = Some(cursor);
        }
        (false, false, false) => {}
      }
    }

    console_core_logging::trace!("click({x}, {y})");
    Ok(selected)
  }

  pub fn cursor_position(
    &self,
    cursor: &Cursor,
  ) -> Result<Option<(f32, f32)>, console_core_never::Never> {
    {
      let Ok(layout_runs) = self.layout_runs();

      Ok(
        layout_runs
          .filter(|run| run.line_i == cursor.line)
          .find_map(|run| {
            let Ok(cursor_position) = run.cursor_position(cursor);

            cursor_position.map(|x| (x, run.line_top))
          }),
      )
    }
  }

  pub fn line_direction(
    &self,
    line: u32,
  ) -> Result<Option<crate::TextDirection>, console_core_never::Never> {
    {
      Ok(
        match match self
          .lines
          .get(match console_core_number_conversion::index(line) {
            Ok(position) => position,
          }) {
          Some(value) => value,
          None => return Ok(None),
        }
        .shape_opt()
        {
          Ok(value) => value,
        }
        .map(|shape| shape.direction),
      )
    }
  }

  pub fn draw<State, F>(
    &mut self,
    font_system: &mut FontSystem,
    cache: &mut crate::GlyphCache,
    color: Color,
    state: &mut State,
    callback: F,
  ) -> Result<(), console_core_never::Never>
  where
    F: FnMut(&mut State, i32, i32, u32, u32, Color),
  {
    {
      let Ok(()) = self.shape_until_scroll(font_system, crate::Pruning::Preserve);

      let mut renderer = crate::LegacyRenderer {
        font_system,
        cache,
        state,
        callback,
      };

      let Ok(layout_runs) = self.layout_runs();

      for run in layout_runs {
        for glyph in run.glyphs {
          let Ok(physical_glyph) = glyph.physical((0., run.line_y), 1.0);
          let glyph_color = match glyph.color_opt {
            Some(color_2) => color_2,
            None => color,
          };
          renderer.glyph(physical_glyph, glyph_color);
        }

        let Ok(()) = render_decoration(&mut renderer, &run, color);
      }

      Ok(())
    }
  }

  pub fn render<R: Renderer>(
    &mut self,
    font_system: &mut FontSystem,
    renderer: &mut R,
    color: Color,
  ) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.shape_until_scroll(font_system, crate::Pruning::Preserve);

      let Ok(layout_runs) = self.layout_runs();

      for run in layout_runs {
        for glyph in run.glyphs {
          let Ok(physical_glyph) = glyph.physical((0., run.line_y), 1.0);
          let glyph_color = match glyph.color_opt {
            Some(color_2) => color_2,
            None => color,
          };
          renderer.glyph(physical_glyph, glyph_color);
        }

        let Ok(()) = render_decoration(renderer, &run, color);
      }

      Ok(())
    }
  }
}

impl BorrowedWithFontSystem<'_, Buffer> {
  pub fn shape_until_cursor(
    &mut self,
    cursor: Cursor,
    pruning: crate::Pruning,
  ) -> Result<(), console_core_never::Never> {
    let prune = pruning == crate::Pruning::Discard;

    {
      let Ok(()) = self.inner.shape_until_cursor(
        self.font_system,
        cursor,
        match ::core::primitive::bool::then_some(prune, ()) {
          ::core::option::Option::Some(()) => crate::Pruning::Discard,
          ::core::option::Option::None => crate::Pruning::Preserve,
        },
      );

      Ok(())
    }
  }

  pub fn line_shape(
    &mut self,
    line_i: u32,
  ) -> Result<Option<&ShapeLine>, console_core_never::Never> {
    {
      let Ok(line_shape) = self.inner.line_shape(self.font_system, line_i);

      Ok(line_shape)
    }
  }

  pub fn line_layout(
    &mut self,
    line_i: u32,
  ) -> Result<Option<&[LayoutLine]>, console_core_never::Never> {
    {
      let Ok(line_layout) = self.inner.line_layout(self.font_system, line_i);

      Ok(line_layout)
    }
  }

  pub fn set_metrics(&mut self, metrics: Metrics) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_metrics(metrics);

      Ok(())
    }
  }

  pub fn set_hinting(&mut self, hinting: Hinting) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_hinting(hinting);

      Ok(())
    }
  }

  pub fn set_wrap(&mut self, wrap: Wrap) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_wrap(wrap);

      Ok(())
    }
  }

  pub fn set_ellipsize(&mut self, ellipsize: Ellipsize) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_ellipsize(ellipsize);

      Ok(())
    }
  }

  pub fn set_size(
    &mut self,
    width_opt: Option<f32>,
    height_opt: Option<f32>,
  ) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_size(width_opt, height_opt);

      Ok(())
    }
  }

  pub fn set_metrics_and_size(
    &mut self,
    metrics: Metrics,
    width_opt: Option<f32>,
    height_opt: Option<f32>,
  ) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self
        .inner
        .set_metrics_and_size(metrics, width_opt, height_opt);

      Ok(())
    }
  }

  pub fn set_tab_width(&mut self, tab_width: u16) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_tab_width(tab_width);

      Ok(())
    }
  }

  pub fn set_monospace_width(
    &mut self,
    monospace_width: Option<f32>,
  ) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_monospace_width(monospace_width);

      Ok(())
    }
  }

  pub fn set_text(
    &mut self,
    text: &str,
    attrs: &Attrs,
    shaping: Shaping,
    alignment: Option<Align>,
  ) -> Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.inner.set_text(text, attrs, shaping, alignment);

      Ok(())
    }
  }

  pub fn set_rich_text<'r, 's, I>(
    &mut self,
    spans: I,
    default_attrs: &Attrs,
    shaping: Shaping,
    alignment: Option<Align>,
  ) -> Result<(), console_core_never::Never>
  where
    I: IntoIterator<Item = (&'s str, Attrs<'r>)>,
  {
    {
      let Ok(()) = self
        .inner
        .set_rich_text(spans, default_attrs, shaping, alignment);

      Ok(())
    }
  }

  pub fn shape_until_scroll(
    &mut self,
    pruning: crate::Pruning,
  ) -> Result<(), console_core_never::Never> {
    let prune = pruning == crate::Pruning::Discard;

    {
      let Ok(()) = self.inner.shape_until_scroll(
        self.font_system,
        match ::core::primitive::bool::then_some(prune, ()) {
          ::core::option::Option::Some(()) => crate::Pruning::Discard,
          ::core::option::Option::None => crate::Pruning::Preserve,
        },
      );

      Ok(())
    }
  }

  pub fn layout_runs(&mut self) -> Result<LayoutRunIter<'_>, console_core_never::Never> {
    {
      let Ok(()) = self
        .inner
        .shape_until_scroll(self.font_system, crate::Pruning::Preserve);

      let Ok(layout_runs) = self.inner.layout_runs();

      Ok(layout_runs)
    }
  }

  pub fn hit(
    &mut self,
    x: impl Into<crate::HorizontalPosition>,
    y: crate::VerticalPosition,
  ) -> Result<Option<Cursor>, console_core_never::Never> {
    let crate::VerticalPosition(y_2) = y;

    {
      let Ok(()) = self
        .inner
        .shape_until_scroll(self.font_system, crate::Pruning::Preserve);

      let Ok(hit) = self.inner.hit(x, crate::VerticalPosition(y_2));

      Ok(hit)
    }
  }

  pub fn draw<State, F>(
    &mut self,
    cache: &mut crate::GlyphCache,
    color: Color,
    state: &mut State,
    f: F,
  ) -> Result<(), console_core_never::Never>
  where
    F: FnMut(&mut State, i32, i32, u32, u32, Color),
  {
    {
      let Ok(()) = self.inner.draw(self.font_system, cache, color, state, f);

      Ok(())
    }
  }
}
