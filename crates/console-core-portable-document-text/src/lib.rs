//! Extract glyphs from the interpreter, then arrange them into lines and reading order.
//!
//! Glyph boxes are in page view space, measured in points with the origin at the top left,
//! the page rotation applied, and the vertical axis increasing downwards. The three PageText
//! vectors align by glyph and describe its box, line, and preceding word gap. Search ranges count
//! glyphs, including text expanded from a single PDF glyph, and line breaks count as a space.
//!
//! Consecutive glyphs form segments on the same baseline. Nearly touching segments merge, then
//! overlapping segments stack into blocks. A block to the left of the highest remaining block
//! reads first when the two overlap vertically. Right-to-left runs reverse within each line.
//! Combining signs may sit apart from their bases without introducing a word gap.
//!
//! The interpreter still owns document loading and rendering faults. Missing pages produce no
//! text. Callers that isolate interpreter panics can keep doing so around extraction.

use console_core_size::Size;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, nearest_f32_from_f64};
use console_core_portable_document_rasterizer::console_core_portable_document_interpreter::console_core_character_maps::BfString;
use console_core_portable_document_rasterizer::console_core_portable_document_interpreter::font::Glyph;
use console_core_portable_document_rasterizer::console_core_portable_document_interpreter::{
  BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings,
  Paint, PathDrawMode, SoftMask, TransformExt, WritingMode, interpret_page,
};
use console_core_portable_document_rasterizer::console_core_portable_document_syntax::Pdf;
use console_core_portable_document_rasterizer::console_core_portable_document_syntax::page::PageCoordinates;
use kurbo::{Affine, BezPath, Rect, Shape};

#[derive(Clone, Debug, PartialEq)]
pub struct TextGlyph {
  pub text: String,
  pub rect: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageText {
  pub glyphs: Vec<TextGlyph>,
  pub line_of: Vec<u32>,
  pub space_before: Vec<GlyphSpacing>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Points names the page-view coordinate unit and keeps its established single-precision representation"
  )
)]
pub struct Points(pub f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPoint {
  pub x: Points,
  pub y: Points,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphSpacing {
  Adjacent,
  WordBreak,
}

impl From<bool> for GlyphSpacing {
  fn from(word_break: bool) -> Self {
    match word_break.then_some(()) {
      Some(()) => Self::WordBreak,
      None => Self::Adjacent,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAvailability {
  Empty,
  ContainsGlyphs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseSensitivity {
  Sensitive,
  Insensitive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScope {
  Anywhere,
  WholeWords,
}

struct SearchStream {
  characters: Vec<char>,
  owners: Vec<u32>,
}

impl PageText {
  pub fn is_empty(&self) -> Result<TextAvailability, Never> {
    Ok(match self.glyphs.as_slice() {
      [] => TextAvailability::Empty,
      [_, ..] => TextAvailability::ContainsGlyphs,
    })
  }

  fn search_gap(&self, at: u32) -> Result<GlyphSpacing, Never> {
    let previous = match at.checked_sub(1) {
      Some(previous) => previous,
      None => return Ok(GlyphSpacing::Adjacent),
    };
    let Ok(place) = index(at);
    let Ok(before) = index(previous);

    Ok(match self.space_before.get(place) {
      Some(GlyphSpacing::WordBreak) => GlyphSpacing::WordBreak,
      Some(GlyphSpacing::Adjacent) | None => {
        GlyphSpacing::from(self.line_of.get(place) != self.line_of.get(before))
      }
    })
  }

  pub fn text_of(&self, range: Range<u32>) -> Result<String, Never> {
    let Ok(start) = index(range.start);
    let Ok(count) = index(range.end.saturating_sub(range.start));
    let mut output = String::new();
    let mut previous_line = None;

    for ((glyph, line), spacing) in self
      .glyphs
      .iter()
      .zip(&self.line_of)
      .zip(&self.space_before)
      .skip(start)
      .take(count)
    {
      match previous_line {
        Some(previous) => match line.cmp(previous) {
          Ordering::Less | Ordering::Greater => output.push('\n'),
          Ordering::Equal => match spacing {
            GlyphSpacing::WordBreak => output.push(' '),
            GlyphSpacing::Adjacent => {}
          },
        },
        None => {}
      }

      output.push_str(&glyph.text);
      previous_line = Some(line);
    }

    Ok(output)
  }

  pub fn plain_text(&self) -> Result<String, Never> {
    let Ok(count) = fitted::<_, u32>(self.glyphs.len());
    self.text_of(0..count)
  }

  pub fn find(&self, needle: &str) -> Result<Vec<Range<u32>>, Never> {
    self.find_opts(needle, CaseSensitivity::Insensitive, SearchScope::Anywhere)
  }

  pub fn find_opts(
    &self,
    needle: &str,
    case: CaseSensitivity,
    scope: SearchScope,
  ) -> Result<Vec<Range<u32>>, Never> {
    let Ok(folded) = fold(needle, case);
    let words: Vec<_> = folded.split_whitespace().collect();
    let pattern: Vec<_> = words.join(" ").chars().collect();

    match pattern.as_slice() {
      [] => return Ok(Vec::new()),
      [_, ..] => {}
    }

    let Ok(stream) = self.search_stream(case);
    stream.find(&pattern, scope)
  }

  pub fn find_with(
    &self,
    matcher: impl Fn(&[char]) -> Vec<Range<u32>>,
  ) -> Result<Vec<Range<u32>>, Never> {
    let Ok(stream) = self.search_stream(CaseSensitivity::Sensitive);

    Ok(
      matcher(&stream.characters)
        .into_iter()
        .filter_map(|span| {
          let Ok(covered) = stream.glyph_range(span);
          covered
        })
        .collect(),
    )
  }

  fn search_stream(&self, case: CaseSensitivity) -> Result<SearchStream, Never> {
    let mut stream = SearchStream {
      characters: Vec::new(),
      owners: Vec::new(),
    };

    for (glyph, position) in self.glyphs.iter().zip(0_u32..) {
      let Ok(gap) = self.search_gap(position);

      match (
        gap,
        stream.characters.last().filter(|letter| **letter != ' '),
      ) {
        (GlyphSpacing::WordBreak, Some(_letter)) => {
          stream.characters.push(' ');
          stream.owners.push(position);
        }
        (GlyphSpacing::WordBreak, None) | (GlyphSpacing::Adjacent, _) => {}
      }

      let Ok(text) = fold(&glyph.text, case);

      for letter in text.chars() {
        let character = match letter.is_whitespace().then_some(()) {
          Some(()) => ' ',
          None => letter,
        };
        stream.characters.push(character);
        stream.owners.push(position);
      }
    }

    Ok(stream)
  }

  pub fn nearest(&self, point: ViewPoint) -> Result<Option<u32>, Never> {
    let closest = self
      .glyphs
      .iter()
      .zip(0_u32..)
      .map(|(glyph, position)| {
        let [left, top, right, bottom] = glyph.rect;
        let Ok(dx) = distance_outside(point.x, [left, right]);
        let Ok(dy) = distance_outside(point.y, [top, bottom]);

        (position, dx.0 + dy.0 * 3.0)
      })
      .min_by(|a, b| a.1.total_cmp(&b.1));

    Ok(closest.map(|(position, _distance)| position))
  }

  pub fn line_rects(&self, range: Range<u32>) -> Result<Vec<[f32; 4]>, Never> {
    let Ok(start) = index(range.start);
    let Ok(count) = index(range.end.saturating_sub(range.start));
    let mut out: Vec<(u32, [f32; 4])> = Vec::new();

    for (glyph, line) in self
      .glyphs
      .iter()
      .zip(&self.line_of)
      .skip(start)
      .take(count)
    {
      match out.last_mut().filter(|(previous, _)| previous == line) {
        Some((_, rect)) => {
          let Ok(()) = union(rect, &glyph.rect);
        }
        None => out.push((*line, glyph.rect)),
      }
    }

    Ok(out.into_iter().map(|(_, rect)| rect).collect())
  }
}

impl SearchStream {
  fn find(&self, needle: &[char], scope: SearchScope) -> Result<Vec<Range<u32>>, Never> {
    let Ok(count) = fitted::<_, u32>(self.characters.len());
    let Ok(length) = fitted::<_, u32>(needle.len());
    let mut cursor = 0_u32;
    let mut matches = Vec::new();

    for _ in 0..count {
      let end = cursor.saturating_add(length);

      match end.cmp(&count) {
        Ordering::Greater => break,
        Ordering::Equal | Ordering::Less => {}
      }

      let Ok(covered) = self.matched(cursor..end, needle, scope);

      match covered {
        Some(span) => {
          matches.push(span);
          cursor = end;
        }
        None => cursor = cursor.saturating_add(1),
      }
    }

    Ok(matches)
  }

  fn matched(
    &self,
    span: Range<u32>,
    needle: &[char],
    scope: SearchScope,
  ) -> Result<Option<Range<u32>>, Never> {
    let Ok(start) = index(span.start);
    let Ok(end) = index(span.end);
    let window = self.characters.get(start..end);
    let equal = window.filter(|window_2| *window_2 == needle);

    match equal {
      None => Ok(None),
      Some(_window) => match scope {
        SearchScope::Anywhere => self.glyph_range(span),
        SearchScope::WholeWords => {
          let before = start
            .checked_sub(1)
            .and_then(|slot| self.characters.get(slot));
          let after = self.characters.get(end);
          let word = |letter: Option<&char>| letter.is_some_and(|one| one.is_alphanumeric());

          match (word(before) || word(after)).then_some(()) {
            Some(()) => Ok(None),
            None => self.glyph_range(span),
          }
        }
      },
    }
  }

  fn glyph_range(&self, span: Range<u32>) -> Result<Option<Range<u32>>, Never> {
    let Ok(start) = index(span.start);
    let Ok(end) = index(span.end);
    let owners = self.owners.get(start..end);

    Ok(match owners {
      Some([first, middle @ ..]) => {
        let last = match middle.last() {
          Some(last) => last,
          None => first,
        };
        Some(*first..last.saturating_add(1))
      }
      Some([]) | None => None,
    })
  }
}

fn fold(text: &str, case: CaseSensitivity) -> Result<String, Never> {
  Ok(match case {
    CaseSensitivity::Sensitive => text.to_string(),
    CaseSensitivity::Insensitive => text.to_lowercase(),
  })
}

fn distance_outside(point: Points, limits: [f32; 2]) -> Result<Points, Never> {
  let [lower, upper] = limits;

  Ok(Points(match point.0.partial_cmp(&lower) {
    Some(Ordering::Less) => lower - point.0,
    Some(Ordering::Equal | Ordering::Greater) | None => match point.0.partial_cmp(&upper) {
      Some(Ordering::Greater) => point.0 - upper,
      Some(Ordering::Less | Ordering::Equal) | None => 0.0,
    },
  }))
}

#[derive(Clone, Copy, Debug)]
enum Direction {
  Right,
  Down,
  Left,
  Up,
}

impl Direction {
  const FLOWS: [Self; 4] = [Self::Right, Self::Down, Self::Left, Self::Up];

  fn of(glyph: &Glyph<'_>, transform: Affine) -> Result<Self, Never> {
    let [a, b, c, d, ..] = transform.as_coeffs();
    let writing = match glyph {
      Glyph::Outline(outline) => {
        let Ok(mode) = outline.is_vertical();
        mode
      }
      Glyph::Type3(_) => WritingMode::Horizontal,
    };
    let (dx, dy) = match writing {
      WritingMode::Vertical => (-c, -d),
      WritingMode::Horizontal => (a, b),
    };

    Ok(match dx.abs().partial_cmp(&dy.abs()) {
      Some(Ordering::Equal | Ordering::Greater) => match dx.partial_cmp(&0.0) {
        Some(Ordering::Equal | Ordering::Greater) => Direction::Right,
        Some(Ordering::Less) | None => Direction::Left,
      },
      Some(Ordering::Less) | None => match dy.partial_cmp(&0.0) {
        Some(Ordering::Greater) => Direction::Down,
        Some(Ordering::Less | Ordering::Equal) | None => Direction::Up,
      },
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "FontAdvance carries the advance in font space and preserves double precision through glyph transforms"
  )
)]
struct FontAdvance(f64);

enum GlyphBounds {
  Plausible,
  Invalid,
}

impl From<bool> for GlyphBounds {
  fn from(plausible: bool) -> Self {
    match plausible.then_some(()) {
      Some(()) => Self::Plausible,
      None => Self::Invalid,
    }
  }
}

enum Diagnostics {
  Quiet,
  RecordGlyphs,
}

impl Diagnostics {
  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    expect(
      explicit026_env_read_once,
      reason = "PDF text extraction owns the legacy PDFCRAFT_TEXT_DEBUG opt-in and resolves it before giving the setting to its glyph device"
    )
  )]
  fn from_environment() -> Result<Self, Never> {
    Ok(match std::env::var_os("PDFCRAFT_TEXT_DEBUG") {
      Some(_enabled) => Self::RecordGlyphs,
      None => Self::Quiet,
    })
  }

  fn glyph(
    &self,
    glyph: &Glyph<'_>,
    text: Option<&str>,
    transforms: &GlyphTransforms,
  ) -> Result<(), Never> {
    match self {
      Self::Quiet => {}
      Self::RecordGlyphs => {
        let kind = match glyph {
          Glyph::Type3(_) => "t3",
          Glyph::Outline(_) => "ol",
        };

        match text {
          Some(shown) => console_core_logging::warn!(
            "{kind} {shown:?} transform={:?} glyph_transform={:?}",
            transforms.view.as_coeffs(),
            transforms.glyph.as_coeffs(),
          ),
          None => console_core_logging::warn!(
            "NOUNICODE {kind} {:?}",
            (transforms.view * transforms.glyph).as_coeffs()
          ),
        }
      }
    }

    Ok(())
  }
}

struct GlyphTransforms {
  view: Affine,
  glyph: Affine,
}

struct TextDevice {
  glyphs: [Vec<TextGlyph>; 4],
  diagnostics: Diagnostics,
}

impl<'a> Device<'a> for TextDevice {
  fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}

  fn set_blend_mode(&mut self, _: BlendMode) {}

  fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}

  fn push_clip_path(&mut self, _: &ClipPath) {}

  fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}

  fn draw_glyph(
    &mut self,
    glyph: &Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    _: &Paint<'a>,
    _: &GlyphDrawMode,
  ) {
    let Ok(()) = self.collect(
      glyph,
      GlyphTransforms {
        view: transform,
        glyph: glyph_transform,
      },
    );
  }

  fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}

  fn pop_clip_path(&mut self) {}

  fn pop_transparency_group(&mut self) {}
}

impl TextDevice {
  fn collect(&mut self, glyph: &Glyph<'_>, transforms: GlyphTransforms) -> Result<(), Never> {
    let Ok(unicode) = glyph.as_unicode();
    let text = match unicode {
      Some(BfString::Char(character)) => character.to_string(),
      Some(BfString::String(text)) => text,
      None => return self.diagnostics.glyph(glyph, None, &transforms),
    };

    match text.chars().find(|character| !character.is_control()) {
      Some(_visible) => {}
      None => return Ok(()),
    }

    let Ok(()) = self.diagnostics.glyph(glyph, Some(&text), &transforms);
    let view = transforms.view * transforms.glyph;
    let Ok(direction) = Direction::of(glyph, view);
    let Ok(advance) = advance(glyph);
    let em = Rect::new(0.0, -200.0, advance.0, 800.0);
    let bounds = (view * em.to_path(0.1)).bounding_box();

    match GlyphBounds::from(
      [bounds.x0, bounds.y0, bounds.x1, bounds.y1]
        .into_iter()
        .all(f64::is_finite)
        && bounds.width() <= 10_000.0
        && bounds.height() <= 10_000.0,
    ) {
      GlyphBounds::Plausible => {}
      GlyphBounds::Invalid => return Ok(()),
    }

    let [right, down, left, up] = &mut self.glyphs;
    let flow = match direction {
      Direction::Right => right,
      Direction::Down => down,
      Direction::Left => left,
      Direction::Up => up,
    };

    emit_glyphs(flow, &text, bounds)
  }
}

fn advance(glyph: &Glyph<'_>) -> Result<FontAdvance, Never> {
  Ok(FontAdvance(match glyph {
    Glyph::Outline(outline) => {
      let Ok(width) = outline.advance_width();

      match width.filter(|width_2| *width_2 > 0.0) {
        Some(width_3) => f64::from(width_3),
        None => {
          let Ok(path) = outline.outline();
          path.bounding_box().width().max(500.0)
        }
      }
    }
    Glyph::Type3(type3) => {
      let Ok(width) = type3.advance_width();

      match width.filter(|width_4| width_4.is_finite() && *width_4 > 0.0) {
        Some(width_5) => f64::from(width_5),
        None => 600.0,
      }
    }
  }))
}

fn emit_glyphs(flow: &mut Vec<TextGlyph>, text: &str, bounds: Rect) -> Result<(), Never> {
  let Ok(count) = fitted::<_, u32>(text.chars().count());
  let width = bounds.width() / f64::from(count.max(1));
  let Ok(top) = nearest_f32_from_f64(bounds.y0);
  let Ok(bottom) = nearest_f32_from_f64(bounds.y1);

  for (character, position) in text.chars().zip(0_u32..) {
    let start = bounds.x0 + width * f64::from(position);
    let Ok(left) = nearest_f32_from_f64(start);
    let Ok(right) = nearest_f32_from_f64(start + width);
    flow.push(TextGlyph {
      text: character.to_string(),
      rect: [left, top, right, bottom],
    });
  }

  Ok(())
}

pub fn extract_page(
  pdf: &Pdf,
  page: u32,
  settings: &InterpreterSettings,
) -> Result<Option<PageText>, Never> {
  let Ok(pages) = pdf.pages();
  let Ok(at) = index(page);
  let selected = match pages.get(at) {
    Some(selected) => selected,
    None => return Ok(None),
  };
  let Ok((wide, tall)) = selected.render_dimensions();
  let extent = Size {
    width: Points(wide),
    height: Points(tall),
  };
  let Ok(cache) = InterpreterCache::new();
  let Ok(initial_transform) = selected.initial_transform(PageCoordinates::TopLeft);
  let Ok(xref) = selected.xref();
  let Ok(mut context) = Context::new(
    initial_transform.to_kurbo(),
    Rect::new(0.0, 0.0, f64::from(wide), f64::from(tall)),
    &cache,
    xref,
    settings.clone(),
  );
  let Ok(diagnostics) = Diagnostics::from_environment();
  let mut device = TextDevice {
    glyphs: Default::default(),
    diagnostics,
  };
  let Ok(()) = interpret_page(selected, &mut context, &mut device);
  let Ok(text) = join_flows(device.glyphs, extent);

  Ok(Some(text))
}

fn join_flows(glyphs: [Vec<TextGlyph>; 4], extent: Size<Points>) -> Result<PageText, Never> {
  let mut flows = Vec::new();

  for (direction, mut glyphs_2) in Direction::FLOWS.into_iter().zip(glyphs) {
    match glyphs_2.as_slice() {
      [] => continue,
      [_, ..] => {}
    }

    let top = glyphs_2
      .iter()
      .map(|glyph| {
        let [_, top, ..] = glyph.rect;
        top
      })
      .fold(f32::INFINITY, f32::min);

    for glyph in &mut glyphs_2 {
      let Ok(rect) = to_upright(glyph.rect, direction, extent);
      glyph.rect = rect;
    }

    let Ok(mut text) = layout(glyphs_2);

    for glyph in &mut text.glyphs {
      let Ok(rect) = from_upright(glyph.rect, direction, extent);
      glyph.rect = rect;
    }

    flows.push((top, text));
  }

  flows.sort_by(|a, b| a.0.total_cmp(&b.0));
  let mut text = PageText::default();

  for (_, flow) in flows {
    let offset = match text.line_of.last() {
      Some(line) => line.saturating_add(1),
      None => 0,
    };
    text.line_of.extend(
      flow
        .line_of
        .into_iter()
        .map(|line| line.saturating_add(offset)),
    );
    text.glyphs.extend(flow.glyphs);
    text.space_before.extend(flow.space_before);
  }

  Ok(text)
}

fn to_upright(
  rect: [f32; 4],
  direction: Direction,
  extent: Size<Points>,
) -> Result<[f32; 4], Never> {
  let [x0, y0, x1, y1] = rect;
  let w = extent.width.0;
  let h = extent.height.0;

  Ok(match direction {
    Direction::Down => [y0, w - x1, y1, w - x0],
    Direction::Left => [w - x1, h - y1, w - x0, h - y0],
    Direction::Up => [h - y1, x0, h - y0, x1],
    Direction::Right => rect,
  })
}

fn from_upright(
  rect: [f32; 4],
  direction: Direction,
  extent: Size<Points>,
) -> Result<[f32; 4], Never> {
  let [x0, y0, x1, y1] = rect;
  let w = extent.width.0;
  let h = extent.height.0;

  Ok(match direction {
    Direction::Down => [w - y1, x0, w - y0, x1],
    Direction::Left => [w - x1, h - y1, w - x0, h - y0],
    Direction::Up => [y0, h - x1, y1, h - x0],
    Direction::Right => rect,
  })
}

#[derive(Clone, Copy)]
enum BidiDirection {
  LeftToRight,
  RightToLeft,
}

fn bidi_direction(text: &str) -> Result<BidiDirection, Never> {
  Ok(match text.chars().find(|character| {
    matches!(
      u32::from(*character),
      0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF
    )
  }) {
    Some(_) => BidiDirection::RightToLeft,
    None => BidiDirection::LeftToRight,
  })
}

enum ScriptSpacing {
  EastAsian,
  Clustered,
  Other,
}

fn script_spacing(character: char) -> Result<ScriptSpacing, Never> {
  Ok(match u32::from(character) {
    0x2E80..=0x9FFF
    | 0xAC00..=0xD7AF
    | 0xF900..=0xFAFF
    | 0xFF00..=0xFFEF
    | 0x20000..=0x3134F
    | 0x1100..=0x11FF => ScriptSpacing::EastAsian,
    0x0300..=0x036F | 0x0900..=0x0DFF | 0x0E00..=0x0EFF | 0x1000..=0x109F | 0x1780..=0x17FF => {
      ScriptSpacing::Clustered
    }
    _ => ScriptSpacing::Other,
  })
}

struct ReadingGlyph {
  glyph: TextGlyph,
  sequence: u32,
  spacing: GlyphSpacing,
}

struct Segment {
  glyphs: Vec<ReadingGlyph>,
  bounds: [f32; 4],
  height: Points,
}

struct Block {
  segments: Vec<Segment>,
  bounds: [f32; 4],
}

enum SegmentRelation {
  Mergeable,
  Separate,
}

impl From<bool> for SegmentRelation {
  fn from(mergeable: bool) -> Self {
    match mergeable.then_some(()) {
      Some(()) => Self::Mergeable,
      None => Self::Separate,
    }
  }
}

fn union(a: &mut [f32; 4], b: &[f32; 4]) -> Result<(), Never> {
  let [left, top, right, bottom] = a;
  let [other_left, other_top, other_right, other_bottom] = *b;

  *left = left.min(other_left);
  *top = top.min(other_top);
  *right = right.max(other_right);
  *bottom = bottom.max(other_bottom);

  Ok(())
}

impl TextGlyph {
  fn height(&self) -> Result<Points, Never> {
    let [_, top, _, bottom] = self.rect;
    Ok(Points((bottom - top).max(0.1)))
  }

  fn middle(&self) -> Result<Points, Never> {
    let [_, top, _, bottom] = self.rect;
    Ok(Points((top + bottom) / 2.0))
  }
}

pub fn layout(glyphs: Vec<TextGlyph>) -> Result<PageText, Never> {
  let Ok(distinct) = without_copies(glyphs);
  let Ok(consecutive) = segment(distinct);
  let Ok(segs) = merge_neighbours(consecutive);
  let Ok(blocks) = block_segments(segs);
  let Ok(ordered) = reading_order(blocks);
  let mut page = PageText::default();
  let mut line = 0_u32;

  for block in ordered {
    for mut segment in block.segments {
      let Ok(()) = mark_spacing(&mut segment.glyphs, segment.height);
      let Ok(()) = logical_order(&mut segment.glyphs);

      for (glyph, position) in segment.glyphs.into_iter().zip(0_u32..) {
        let spacing = match position {
          0 => GlyphSpacing::Adjacent,
          1.. => glyph.spacing,
        };
        page.glyphs.push(glyph.glyph);
        page.line_of.push(line);
        page.space_before.push(spacing);
      }

      line = line.saturating_add(1);
    }
  }

  Ok(page)
}

fn without_copies(glyphs: Vec<TextGlyph>) -> Result<Vec<ReadingGlyph>, Never> {
  let mut previous = VecDeque::with_capacity(4);
  let mut keep = Vec::with_capacity(glyphs.len());

  for glyph in &glyphs {
    let [x, y, ..] = glyph.rect;
    let Ok(height) = glyph.height();
    let duplicate = previous.iter().rev().flatten().any(|older: &&TextGlyph| {
      let [px, py, ..] = older.rect;
      older.text == glyph.text
        && (px - x).abs() < height.0 * 0.2
        && (py - y).abs() < height.0 * 0.2
    });

    match previous.len() {
      4 => {
        previous.pop_front();
      }
      _other => {}
    }

    let next = match duplicate.then_some(()) {
      Some(()) => None,
      None => Some(glyph),
    };
    keep.push(next.is_some());
    previous.push_back(next);
  }

  Ok(
    glyphs
      .into_iter()
      .zip(0_u32..)
      .zip(keep)
      .filter_map(|((glyph, sequence), original)| {
        original.then_some(ReadingGlyph {
          glyph,
          sequence,
          spacing: GlyphSpacing::Adjacent,
        })
      })
      .collect(),
  )
}

fn segment(glyphs: Vec<ReadingGlyph>) -> Result<VecDeque<Segment>, Never> {
  let mut segments: VecDeque<Segment> = VecDeque::new();

  for glyph in glyphs {
    let joined = segments.back_mut().filter(|previous| {
      let Ok(relation) = previous.continuous(&glyph.glyph);

      matches!(relation, SegmentRelation::Mergeable)
    });
    let Ok(height) = glyph.glyph.height();
    let bounds = glyph.glyph.rect;

    match joined {
      Some(segment) => {
        segment.glyphs.push(glyph);
        let Ok(()) = union(&mut segment.bounds, &bounds);
        segment.height.0 = segment.height.0.max(height.0);
      }
      None => segments.push_back(Segment {
        glyphs: vec![glyph],
        bounds,
        height,
      }),
    }
  }

  Ok(segments)
}

impl Segment {
  fn continuous(&self, glyph: &TextGlyph) -> Result<SegmentRelation, Never> {
    let previous = match self.glyphs.last() {
      Some(previous) => &previous.glyph,
      None => return Ok(SegmentRelation::Separate),
    };
    let Ok(height) = glyph.height();
    let Ok(previous_height) = previous.height();
    let Ok(center) = glyph.middle();
    let Ok(previous_center) = previous.middle();
    let width = height.0.min(previous_height.0);
    let [left, ..] = glyph.rect;
    let [previous_left, _, previous_right, _] = previous.rect;
    let gap = left - previous_right;

    Ok(SegmentRelation::from(
      (center.0 - previous_center.0).abs() < width * 0.5
        && gap < width * 3.0
        && left > previous_left - width * 2.0,
    ))
  }

  fn relation(&self, other: &Self) -> Result<SegmentRelation, Never> {
    let height = self.height.0.min(other.height.0);
    let [left, top, right, bottom] = self.bounds;
    let [other_left, other_top, other_right, other_bottom] = other.bounds;
    let gap = (other_left - right).max(left - other_right);
    let center = (top + bottom) / 2.0;
    let other_center = (other_top + other_bottom) / 2.0;

    Ok(SegmentRelation::from(
      (center - other_center).abs() < height * 0.3
        && (self.height.0 / other.height.0 - 1.0).abs() < 0.35
        && gap < height * 1.2,
    ))
  }

  fn merge(&mut self, other: Self) -> Result<(), Never> {
    self.glyphs.extend(other.glyphs);
    let Ok(()) = union(&mut self.bounds, &other.bounds);
    self.height.0 = self.height.0.max(other.height.0);

    Ok(())
  }
}

fn first_merge(segments: &VecDeque<Segment>) -> Result<Option<(u32, u32)>, Never> {
  for (segment, left) in segments.iter().zip(0_u32..) {
    'inner_for_2: for (other, right) in segments.iter().zip(0_u32..) {
      match left.cmp(&right) {
        Ordering::Equal => continue 'inner_for_2,
        Ordering::Less | Ordering::Greater => {}
      }

      let Ok(relation) = segment.relation(other);

      match relation {
        SegmentRelation::Mergeable => return Ok(Some((left, right))),
        SegmentRelation::Separate => {}
      }
    }
  }

  Ok(None)
}

fn merge_neighbours(mut segments: VecDeque<Segment>) -> Result<Vec<Segment>, Never> {
  let Ok(count) = fitted::<_, u32>(segments.len());

  for _ in 1..count {
    let Ok(pair) = first_merge(&segments);
    let (first, second) = match pair {
      Some(pair_2) => pair_2,
      None => break,
    };
    let Ok(remove_at) = index(second);
    let removed = match segments.remove(remove_at) {
      Some(removed) => removed,
      None => break,
    };
    let adjusted = match second.cmp(&first) {
      Ordering::Less => first.saturating_sub(1),
      Ordering::Equal | Ordering::Greater => first,
    };
    let Ok(keep_at) = index(adjusted);

    match segments.get_mut(keep_at) {
      Some(segment) => {
        let Ok(()) = segment.merge(removed);
      }
      None => break,
    }
  }

  for segment in &mut segments {
    segment.glyphs.sort_by(|a, b| {
      let [left, ..] = a.glyph.rect;
      let [right, ..] = b.glyph.rect;
      left.total_cmp(&right)
    });
  }

  Ok(segments.into_iter().collect())
}

impl Block {
  fn accepts(&self, segment: &Segment) -> Result<SegmentRelation, Never> {
    let last = match self.segments.last() {
      Some(last) => last,
      None => return Ok(SegmentRelation::Separate),
    };
    let [seg_left, seg_top, seg_right, ..] = segment.bounds;
    let [left, _, right, ..] = self.bounds;
    let [_, _, _, last_bottom] = last.bounds;
    let vertical_gap = seg_top - last_bottom;
    let overlap = seg_right.min(right) - seg_left.max(left);
    let narrowest = (seg_right - seg_left).min(right - left).max(1.0);

    Ok(SegmentRelation::from(
      vertical_gap > -last.height.0 * 0.5
        && vertical_gap < last.height.0.max(segment.height.0) * 1.1
        && overlap > narrowest * 0.3
        && (last.height.0 / segment.height.0 - 1.0).abs() < 0.6,
    ))
  }
}

fn first_block<'a>(
  blocks: &'a mut VecDeque<Block>,
  segment: &Segment,
) -> Result<Option<&'a mut Block>, Never> {
  for block in blocks {
    let Ok(relation) = block.accepts(segment);

    match relation {
      SegmentRelation::Mergeable => return Ok(Some(block)),
      SegmentRelation::Separate => {}
    }
  }

  Ok(None)
}

fn block_segments(mut segments: Vec<Segment>) -> Result<VecDeque<Block>, Never> {
  segments.sort_by(|a, b| {
    let [_, top, ..] = a.bounds;
    let [_, other_top, ..] = b.bounds;
    top.total_cmp(&other_top)
  });
  let mut blocks = VecDeque::new();

  for segment in segments {
    let bounds = segment.bounds;
    let Ok(found) = first_block(&mut blocks, &segment);

    match found {
      Some(block) => {
        block.segments.push(segment);
        let Ok(()) = union(&mut block.bounds, &bounds);
      }
      None => blocks.push_back(Block {
        segments: vec![segment],
        bounds,
      }),
    }
  }

  Ok(blocks)
}

fn reading_order(mut blocks: VecDeque<Block>) -> Result<Vec<Block>, Never> {
  let Ok(count) = fitted::<_, u32>(blocks.len());
  let mut ordered = Vec::new();

  for _ in 0..count {
    let Ok(next) = next_block(&blocks);
    let slot = match next {
      Some(slot) => slot,
      None => break,
    };
    let Ok(at) = index(slot);

    match blocks.remove(at) {
      Some(block) => ordered.push(block),
      None => break,
    }
  }

  Ok(ordered)
}

fn next_block(blocks: &VecDeque<Block>) -> Result<Option<u32>, Never> {
  let top = blocks.iter().zip(0_u32..).min_by(|(a, _), (b, _)| {
    let [_, first, ..] = a.bounds;
    let [_, second, ..] = b.bounds;
    first.total_cmp(&second)
  });
  let (top_block, position) = match top {
    Some(found) => found,
    None => return Ok(None),
  };
  let [left, top_2, _, bottom] = top_block.bounds;
  let earlier = blocks
    .iter()
    .zip(0_u32..)
    .filter(|(candidate, _)| {
      let [_, candidate_top, candidate_right, candidate_bottom] = candidate.bounds;
      let overlap = candidate_bottom.min(bottom) - candidate_top.max(top_2);

      candidate_right <= left + 1.0
        && overlap > 0.5 * (candidate_bottom - candidate_top).min(bottom - top_2)
    })
    .min_by(|(candidate, _), (other, _)| {
      let [candidate_left, ..] = candidate.bounds;
      let [other_left, ..] = other.bounds;
      candidate_left.total_cmp(&other_left)
    });

  Ok(Some(match earlier {
    Some((_block, slot)) => slot,
    None => position,
  }))
}

fn logical_order(glyphs: &mut [ReadingGlyph]) -> Result<(), Never> {
  let Ok(length) = fitted::<_, u32>(glyphs.len());
  let mut cursor = 0_u32;

  for _ in 0..length {
    let Ok(at) = index(cursor);
    let rest = match glyphs.get_mut(at..) {
      Some(rest @ [_, ..]) => rest,
      Some([]) | None => break,
    };
    let Ok(first) = rest
      .first()
      .map(|glyph| bidi_direction(&glyph.glyph.text))
      .transpose();
    let width = match first {
      Some(BidiDirection::RightToLeft) => {
        let Ok(run) = rtl_run(rest);
        let Ok(limit) = index(run);

        match rest.get_mut(..limit) {
          Some(run_2) => run_2.reverse(),
          None => {}
        }

        run
      }
      Some(BidiDirection::LeftToRight) | None => 1,
    };

    cursor = cursor.saturating_add(width);
  }

  Ok(())
}

fn rtl_run(glyphs: &[ReadingGlyph]) -> Result<u32, Never> {
  let mut rest = glyphs.iter().peekable();
  let Ok(length) = fitted::<_, u32>(glyphs.len());
  let mut width = 0_u32;

  for _ in 0..length {
    let glyph = match rest.next() {
      Some(glyph) => glyph,
      None => break,
    };
    let Ok(direction) = bidi_direction(&glyph.glyph.text);

    let continuing = match direction {
      BidiDirection::RightToLeft => true,
      BidiDirection::LeftToRight => {
        glyph.glyph.text.trim().is_empty()
          && rest.peek().is_some_and(|next| {
            matches!(
              bidi_direction(&next.glyph.text),
              Ok(BidiDirection::RightToLeft)
            )
          })
      }
    };

    match continuing.then_some(()) {
      Some(()) => width = width.saturating_add(1),
      None => break,
    }
  }

  Ok(width)
}

fn mark_spacing(glyphs: &mut [ReadingGlyph], height: Points) -> Result<(), Never> {
  let Ok(threshold) = gap_threshold(glyphs, height);
  let mut entries = glyphs.iter_mut();
  let mut previous = match entries.next() {
    Some(first) => first,
    None => return Ok(()),
  };

  for current in entries {
    let Ok(continuous) = spacing_between(&previous.glyph, &current.glyph, threshold, height);

    match continuous {
      GlyphSpacing::WordBreak => match current.sequence.cmp(&previous.sequence) {
        Ordering::Greater | Ordering::Equal => current.spacing = GlyphSpacing::WordBreak,
        Ordering::Less => previous.spacing = GlyphSpacing::WordBreak,
      },
      GlyphSpacing::Adjacent => {}
    }

    previous = current;
  }

  Ok(())
}

fn gap_between(left: &TextGlyph, right: &TextGlyph) -> Result<Points, Never> {
  let [_, _, left_right, _] = left.rect;
  let [right_left, ..] = right.rect;

  Ok(Points(right_left - left_right))
}

fn gap_threshold(glyphs: &[ReadingGlyph], height: Points) -> Result<Points, Never> {
  let mut gaps: Vec<_> = glyphs
    .windows(2)
    .filter_map(|pair| match pair {
      [left, right] => {
        let Ok(gap) = gap_between(&left.glyph, &right.glyph);
        Some(gap)
      }
      _unknown_length => None,
    })
    .collect();
  gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
  let typical = match gaps.get(gaps.len().wrapping_div(3)) {
    Some(distance) => distance.0.max(0.0),
    None => 0.0,
  };

  Ok(Points((typical + height.0 * 0.15).max(height.0 * 0.15)))
}

fn spacing_between(
  left: &TextGlyph,
  right: &TextGlyph,
  threshold: Points,
  height: Points,
) -> Result<GlyphSpacing, Never> {
  let Ok(gap) = gap_between(left, right);
  let east_asian = |text: &str| {
    text
      .chars()
      .any(|ch| matches!(script_spacing(ch), Ok(ScriptSpacing::EastAsian)))
  };
  let combining = |text: &str| {
    text
      .chars()
      .any(|ch| matches!(script_spacing(ch), Ok(ScriptSpacing::Clustered)))
  };

  Ok(GlyphSpacing::from(
    gap.0 > threshold.0
      && !(east_asian(&left.text) && east_asian(&right.text) && gap.0 < height.0 * 0.5)
      && !((combining(&left.text) || combining(&right.text)) && gap.0 < height.0 * 0.6)
      && !left.text.trim().is_empty()
      && !right.text.trim().is_empty(),
  ))
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_number_conversion::fixed_index;
  use std::fmt::Write as _;

  struct VerticalTextFixture<'a> {
    encoding: &'a str,
    cmap: &'a str,
    operators: &'a str,
  }

  fn vertical_text_pdf(
    VerticalTextFixture { encoding, cmap, operators }: VerticalTextFixture<'_>,
  ) -> Result<Vec<u8>, std::fmt::Error> {
    let objects: [&str; 8] = [
      "<< /Type /Catalog /Pages 2 0 R >>",
      "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
      "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
        /Resources << /Font << /F1 4 0 R /H1 5 0 R >> >> /Contents 8 0 R >>",
      &format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /TestVertical /Encoding /{encoding} \
          /DescendantFonts [6 0 R] /ToUnicode 7 0 R >>"
      ),
      "<< /Type /Font /Subtype /Type0 /BaseFont /TestVertical /Encoding /Identity-H \
        /DescendantFonts [6 0 R] /ToUnicode 7 0 R >>",
      "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /TestVertical \
        /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
        /DW 1000 /DW2 [880 -1000] /CIDToGIDMap /Identity >>",
      &format!("<< /Length {} >>\nstream\n{cmap}\nendstream", cmap.len()),
      &format!(
        "<< /Length {} >>\nstream\n{operators}\nendstream",
        operators.len()
      ),
    ];
    let Ok(count) = fixed_index(objects.len());
    let size = count.saturating_add(1);
    let mut pdf = String::from("%PDF-1.7\n");
    let mut offsets: Vec<u64> = Vec::with_capacity(objects.len());

    for (number, object) in (1_u32..).zip(objects) {
      let Ok(offset) = fixed_index(pdf.len());
      offsets.push(offset);
      writeln!(pdf, "{number} 0 obj\n{object}\nendobj")?;
    }

    let Ok(table) = fixed_index(pdf.len());
    writeln!(pdf, "xref\n0 {size}\n0000000000 65535 f ")?;

    for offset in offsets {
      writeln!(pdf, "{offset:010} 00000 n ")?;
    }

    writeln!(
      pdf,
      "trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{table}\n%%EOF"
    )?;

    Ok(pdf.into_bytes())
  }

  #[test]
  fn identity_v_columns_are_searchable_in_reading_order() -> Result<(), Box<dyn std::error::Error>>
  {
    for encoding in ["Identity-V", "UniJIS-UCS2-V"] {
      let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /TestUnicode def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange 5 beginbfchar <0001> <65E5> <0002> <672C> <0003> <8A9E> <0004> <6587> <0005> <7AE0> endbfchar endcmap CMapName currentdict /CMap defineresource pop end end";
      let mapping = match encoding {
        "Identity-V" => cmap.to_owned(),
        _ => {
          let mut mapping = cmap.to_owned();

          for (code, unicode) in [
            ("0001", "65E5"),
            ("0002", "672C"),
            ("0003", "8A9E"),
            ("0004", "6587"),
            ("0005", "7AE0"),
          ] {
            mapping = mapping.replace(
              &format!("<{code}> <{unicode}>"),
              &format!("<{unicode}> <{unicode}>"),
            );
          }

          mapping
        }
      };
      let (first, second) = match encoding {
        "Identity-V" => ("000100020003", "00040005"),
        _ => ("65E5672C8A9E", "65877AE0"),
      };
      let operators = format!(
        "BT /H1 20 Tf 1 0 0 1 40 280 Tm <{first}> Tj /F1 20 Tf 1 0 0 1 200 250 Tm <{first}> Tj 1 0 0 1 170 250 Tm <{second}> Tj ET"
      );
      let bytes = vertical_text_pdf(VerticalTextFixture {
        encoding,
        cmap: &mapping,
        operators: &operators,
      })?;
      let pdf = Pdf::new(bytes).map_err(|fault| std::io::Error::other(format!("{fault:?}")))?;
      let Ok(extracted) = extract_page(&pdf, 0, &InterpreterSettings::default());
      let text = extracted.ok_or_else(|| std::io::Error::other("missing vertical text page"))?;
      let Ok(plain) = text.plain_text();
      let Ok(first_word) = text.find("日本語");
      let Ok(second_word) = text.find("文章");
      assert_eq!(plain, "日本語\n日本語\n文章");
      assert_eq!(first_word, vec![0..3, 3..6]);
      assert_eq!(second_word, vec![6..8]);
      let Ok(rects) = text.line_rects(3..6);
      let [left, top, right, bottom] = match rects.as_slice() {
        [only] => *only,
        _unexpected => {
          return Err(Box::new(std::io::Error::other(
            "expected one highlight per line",
          )));
        }
      };
      assert!(
        bottom - top > right - left,
        "a vertical search highlight: {rects:?}"
      );

      let right_column = text
        .glyphs
        .get(3)
        .ok_or_else(|| std::io::Error::other("missing first column"))?;
      let left_column = text
        .glyphs
        .get(6)
        .ok_or_else(|| std::io::Error::other("missing second column"))?;
      let [first_left, ..] = right_column.rect;
      let [second_left, ..] = left_column.rect;
      assert!(
        first_left > second_left,
        "columns read right to left in page view coordinates"
      );
    }

    Ok(())
  }

  #[test]
  fn upright_mapping_round_trips() {
    let rect = [10.0, 20.0, 30.0, 25.0];
    let extent = Size {
      width: Points(300.0),
      height: Points(200.0),
    };

    for direction in Direction::FLOWS {
      let Ok(upright) = to_upright(rect, direction, extent);
      let Ok(restored) = from_upright(upright, direction, extent);
      assert_eq!(restored, rect, "{direction:?}");
    }

    let Ok(vertical) = to_upright([290.0, 0.0, 300.0, 10.0], Direction::Down, extent);
    assert_eq!(vertical, [0.0, 0.0, 10.0, 10.0]);
  }

  fn glyph(text: &str, edge: [f32; 3]) -> Result<TextGlyph, Never> {
    let [left, top, right] = edge;

    Ok(TextGlyph {
      text: text.to_owned(),
      rect: [left, top, right, top + 10.0],
    })
  }

  #[test]
  fn lines_words_and_search() {
    let mut glyphs = Vec::new();

    for (word, origin) in [
      (
        "Hello",
        ViewPoint {
          x: Points(10.0),
          y: Points(10.0),
        },
      ),
      (
        "world",
        ViewPoint {
          x: Points(50.0),
          y: Points(10.0),
        },
      ),
      (
        "Next",
        ViewPoint {
          x: Points(10.0),
          y: Points(30.0),
        },
      ),
    ] {
      let Ok(()) = write_word(&mut glyphs, word, origin, Points(6.0));
    }

    let Ok(text) = layout(glyphs);
    let Ok(empty) = PageText::default().is_empty();
    let Ok(present) = text.is_empty();
    let Ok(plain) = text.plain_text();
    let Ok(across_lines) = text.find("WORLD next");
    let Ok(whole_words) =
      text.find_opts("world", CaseSensitivity::Sensitive, SearchScope::WholeWords);
    let Ok(case_mismatch) =
      text.find_opts("WORLD", CaseSensitivity::Sensitive, SearchScope::WholeWords);
    let Ok(word_part) = text.find_opts(
      "orld",
      CaseSensitivity::Insensitive,
      SearchScope::WholeWords,
    );
    let Ok(letter) = text.find("o");
    let Ok(highlights) = text.line_rects(3..12);
    let Ok(nearest) = text.nearest(ViewPoint {
      x: Points(52.0),
      y: Points(14.0),
    });
    assert_eq!(empty, TextAvailability::Empty);
    assert_eq!(present, TextAvailability::ContainsGlyphs);
    assert_eq!(plain, "Hello world\nNext");
    assert_eq!(across_lines, vec![5..14]);
    assert_eq!(whole_words, vec![5..10]);
    assert!(case_mismatch.is_empty());
    assert!(word_part.is_empty());
    assert_eq!(letter, vec![4..5, 6..7]);
    assert_eq!(highlights.len(), 2);
    assert_eq!(nearest, Some(5));
  }

  fn write_word(
    glyphs: &mut Vec<TextGlyph>,
    word: &str,
    origin: ViewPoint,
    advance: Points,
  ) -> Result<(), Never> {
    for (character, position) in word.chars().zip(0_u32..) {
      let Ok(steps) = nearest_f32_from_f64(f64::from(position));
      let left = origin.x.0 + steps * advance.0;
      let Ok(glyph) = glyph(&character.to_string(), [left, origin.y.0, left + 6.0]);
      glyphs.push(glyph);
    }

    Ok(())
  }

  #[test]
  fn fake_bold_duplicates_are_removed() {
    let mut glyphs = Vec::new();

    for (character, position) in "Bold".chars().zip(0_u32..) {
      let Ok(steps) = nearest_f32_from_f64(f64::from(position));
      let left = 10.0 + steps * 7.0;
      let word = character.to_string();
      let Ok(first) = glyph(&word, [left, 10.0, left + 6.0]);
      let Ok(copy) = glyph(&word, [left + 0.3, 10.0, left + 6.3]);
      glyphs.push(first);
      glyphs.push(copy);
    }

    let Ok(laid) = layout(glyphs);
    let Ok(plain) = laid.plain_text();
    assert_eq!(plain, "Bold");
  }

  #[test]
  fn two_columns_read_left_then_right_even_if_drawn_interleaved() {
    let mut glyphs = Vec::new();

    for (word, x, y) in [
      ("Left1", 10.0, 10.0),
      ("Right1", 200.0, 10.0),
      ("Left2", 10.0, 24.0),
      ("Right2", 200.0, 24.0),
    ] {
      let origin = ViewPoint {
        x: Points(x),
        y: Points(y),
      };
      let Ok(()) = write_word(&mut glyphs, word, origin, Points(6.0));
    }

    let Ok(text) = layout(glyphs);
    let Ok(plain) = text.plain_text();
    assert_eq!(plain, "Left1\nLeft2\nRight1\nRight2");
  }

  #[test]
  fn tracked_text_keeps_letters_together() {
    let mut glyphs = Vec::new();
    let first = ViewPoint {
      x: Points(10.0),
      y: Points(10.0),
    };
    let second = ViewPoint {
      x: Points(10.0 + 7.0 * 9.0 + 5.0),
      y: Points(10.0),
    };
    let Ok(()) = write_word(&mut glyphs, "CHAPTER", first, Points(9.0));
    let Ok(()) = write_word(&mut glyphs, "FOUR", second, Points(9.0));
    let Ok(text) = layout(glyphs);
    let Ok(plain) = text.plain_text();
    assert_eq!(plain, "CHAPTER FOUR");
  }

  #[test]
  fn rtl_runs_come_out_in_logical_order_and_cjk_has_no_spaces() {
    let origin = ViewPoint {
      x: Points(10.0),
      y: Points(10.0),
    };

    for (visual, advance, expected) in [("םולש", 6.0, "שלום"), ("每个字", 9.0, "每个字")]
    {
      let mut glyphs = Vec::new();
      let Ok(()) = write_word(&mut glyphs, visual, origin, Points(advance));
      let Ok(text) = layout(glyphs);
      let Ok(plain) = text.plain_text();
      assert_eq!(plain, expected);
    }
  }
}
