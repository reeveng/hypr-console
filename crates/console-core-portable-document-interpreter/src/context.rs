//! What one interpretation of a page carries from operator to operator: the graphics state and
//! the ones saved beneath it, the path being built, the clips and the box they leave, and the
//! caches the page shares with every other page of the document.
//!
//! The state in force is held apart from the saved ones, so there is always one and asking for
//! it cannot fail; a restore with nothing saved keeps the one there is, as a reader does with an
//! unbalanced `Q`.
//!
//! Content that draws content -- a form, a tiling pattern, a Type 3 glyph, a soft mask -- is
//! bounded twice. The depth is counted on the context and refused past
//! `MAX_NESTED_INTERPRETATION_DEPTH`, and patterns and glyphs, which a device paints on its own
//! stack, stop at the tighter `MAX_PAINT_NESTING`. A depth alone does not bound the work, though:
//! a glyph showing ten glyphs of its own font sixteen deep is 10^16 paints, which is a file that
//! hangs rather than one that overflows. So every paint below the top level spends one of
//! `MAX_NESTED_PAINTS` from a budget the page starts afresh, and is skipped once it is spent.
//! The budget lives on the thread because the paints a device makes on the interpreter's behalf
//! come back through a fresh context and nothing else is shared between them.

use crate::cache::{Cache, CacheKey};
use crate::color::ColorSpace;
use crate::convert::convert_transform;
use crate::font::{Font, StandardFont};
use crate::interpret::state::{ClipType, State, TextStateFont};
use crate::ocg::OcgState;
use crate::util::{Nearly, Nearness, fast_bounding_box};
use crate::{ClipPath, Device, FillRule, InterpreterSettings, Nesting, PaintBudget, StrokeProps};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_portable_document_syntax::content::ops::Transform;
use console_core_portable_document_syntax::object::{Dict, Name, ObjectIdentifier};
use console_core_portable_document_syntax::page::Resources;
use console_core_portable_document_syntax::xref::XRef;
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) const MAX_NESTED_INTERPRETATION_DEPTH: u32 = 50;
pub(crate) const MAX_PAINT_NESTING: u32 = 16;
pub(crate) const MAX_NESTED_PAINTS: u32 = 50_000;

std::thread_local! {
    static NESTED_PAINTS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

pub(crate) fn take_nested_paint(depth: u32) -> Result<PaintBudget, Never> {
  match ::core::primitive::bool::then_some(depth < 2, ()) {
    ::core::option::Option::Some(()) => return Ok(PaintBudget::Granted),
    ::core::option::Option::None => {}
  }

  Ok(NESTED_PAINTS.with(|spent| {
    let used = spent.get();

    match ::core::primitive::bool::then_some(used >= MAX_NESTED_PAINTS, ()) {
      ::core::option::Option::Some(()) => PaintBudget::Spent,
      ::core::option::Option::None => {
        spent.set(used.saturating_add(1));

        PaintBudget::Granted
      }
    }
  }))
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "nested device callbacks may retain document cache handles beyond a parent context's borrow"
  )
)]
pub struct InterpreterCache<'a> {
  font_cache: Rc<RefCell<HashMap<u128, Option<Font<'a>>>>>,
  pub(crate) object_cache: Cache,
}

impl Default for InterpreterCache<'_> {
  fn default() -> Self {
    let Ok(cache) = Self::new();

    cache
  }
}

impl InterpreterCache<'_> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "each document starts one cache allocation whose leases follow deferred rendering callbacks"
    )
  )]
  pub fn new() -> Result<Self, Never> {
    let Ok(object_cache) = Cache::new();

    Ok(Self {
      font_cache: Rc::new(RefCell::new(HashMap::new())),
      object_cache,
    })
  }
}

pub struct Context<'a> {
  current: State<'a>,
  saved: Vec<State<'a>>,
  pub(crate) path: BezPath,
  pub(crate) sub_path_start: Point,
  pub(crate) last_point: Point,
  pub(crate) clip: Option<FillRule>,
  root_transforms: Vec<Affine>,
  bbox: Vec<Rect>,
  pub(crate) settings: InterpreterSettings,
  pub(crate) interpreter_cache: InterpreterCache<'a>,
  pub(crate) xref: &'a XRef,
  pub(crate) ocg_state: OcgState,
  nesting_depth: u32,
}

impl<'a> Context<'a> {
  pub fn new(
    initial_transform: Affine,
    bbox: Rect,
    cache: &InterpreterCache<'a>,
    xref: &'a XRef,
    settings: InterpreterSettings,
  ) -> Result<Self, Never> {
    let Ok(state) = State::new(initial_transform);

    NESTED_PAINTS.with(|spent| spent.set(0));

    Self::new_with(initial_transform, bbox, cache, xref, settings, state, 0)
  }

  pub(crate) fn new_with(
    initial_transform: Affine,
    bbox: Rect,
    cache: &InterpreterCache<'a>,
    xref: &'a XRef,
    settings: InterpreterSettings,
    state: State<'a>,
    nesting_depth: u32,
  ) -> Result<Self, Never> {
    let Ok(root) = xref.root_id();
    let Ok(catalog) = xref.get::<Dict<'_>>(root);
    let mut ocg_state = match catalog {
      Some(catalog_2) => {
        let Ok(state_2) = OcgState::from_catalog(&catalog_2);

        state_2
      }
      None => OcgState::default(),
    };

    for (number, generation, visibility) in settings.ocg_overrides.iter() {
      let Ok(group) = ObjectIdentifier::new(*number, *generation);
      let Ok(()) = ocg_state.set_visibility(group, *visibility);
    }

    Ok(Self {
      current: state,
      saved: Vec::new(),
      settings,
      xref,
      root_transforms: vec![initial_transform],
      last_point: Point::default(),
      sub_path_start: Point::default(),
      clip: None,
      bbox: vec![bbox],
      path: BezPath::new(),
      interpreter_cache: cache.clone(),
      ocg_state,
      nesting_depth,
    })
  }

  pub(crate) fn save_state(&mut self) -> Result<(), Never> {
    self.saved.push(self.current.clone());

    Ok(())
  }

  pub(crate) fn bbox(&self) -> Result<Rect, Never> {
    Ok(match self.bbox.last() {
      Some(bbox) => *bbox,
      None => {
        warn!("failed to get a bbox");

        Rect::new(0.0, 0.0, 1.0, 1.0)
      }
    })
  }

  fn push_bbox(&mut self, bbox: Rect) -> Result<(), Never> {
    let Ok(own) = self.bbox();

    self.bbox.push(own.intersect(bbox));

    Ok(())
  }

  pub(crate) fn push_clip_path(
    &mut self,
    clip_path: BezPath,
    fill: FillRule,
    device: &mut impl Device<'a>,
  ) -> Result<(), Never> {
    let Ok(rect) = path_as_rect(&clip_path);

    match rect {
      Some(clip_rect) => {
        let Ok(covered) = self.covered_by(clip_rect);

        match covered {
          Covering::Whole => {
            self.current.clips.push(ClipType::Dummy);

            return Ok(());
          }
          Covering::Partial => {}
        }
      }
      None => {}
    }

    let bbox = clip_path.bounding_box();

    device.push_clip_path(&ClipPath {
      path: clip_path,
      fill,
    });

    let Ok(()) = self.push_bbox(bbox);

    self.current.clips.push(ClipType::Real);

    Ok(())
  }

  fn covered_by(&self, clip_rect: Rect) -> Result<Covering, Never> {
    let Ok(own) = self.bbox();
    let Ok(left) = own.min_x().nearly(clip_rect.min_x());
    let Ok(bottom) = own.min_y().nearly(clip_rect.min_y());
    let Ok(right) = own.max_x().nearly(clip_rect.max_x());
    let Ok(top) = own.max_y().nearly(clip_rect.max_y());

    Ok(match (left, bottom, right, top) {
      (
        Nearly::Equal | Nearly::Above,
        Nearly::Equal | Nearly::Above,
        Nearly::Equal | Nearly::Below,
        Nearly::Equal | Nearly::Below,
      ) => Covering::Whole,
      _ => Covering::Partial,
    })
  }

  pub(crate) fn pop_clip_path(&mut self, device: &mut impl Device<'a>) -> Result<(), Never> {
    match self.current.clips.pop() {
      Some(ClipType::Real) => {
        device.pop_clip_path();

        let _popped = self.bbox.pop();
      }
      Some(ClipType::Dummy) | None => {}
    }

    Ok(())
  }

  pub(crate) fn push_root_transform(&mut self) -> Result<(), Never> {
    self.root_transforms.push(self.current.ctm);

    Ok(())
  }

  pub(crate) fn pop_root_transform(&mut self) -> Result<(), Never> {
    let _popped = self.root_transforms.pop();

    Ok(())
  }

  pub(crate) fn root_transform(&self) -> Result<Affine, Never> {
    Ok(match self.root_transforms.last() {
      Some(transform) => *transform,
      None => Affine::IDENTITY,
    })
  }

  pub(crate) fn restore_state(&mut self, device: &mut impl Device<'a>) -> Result<(), Never> {
    match self.saved.pop() {
      Some(restored) => {
        let Ok(extra) = fitted::<_, u32>(
          self
            .current
            .clips
            .len()
            .saturating_sub(restored.clips.len()),
        );

        for _clip in 0..extra {
          let Ok(()) = self.pop_clip_path(device);
        }

        self.current = restored;
      }
      None => {}
    }

    device.set_soft_mask(self.current.graphics_state.soft_mask.clone());

    Ok(())
  }

  pub(crate) fn get(&self) -> Result<&State<'a>, Never> {
    Ok(&self.current)
  }

  pub(crate) fn get_mut(&mut self) -> Result<&mut State<'a>, Never> {
    Ok(&mut self.current)
  }

  pub(crate) fn pre_concat_transform(&mut self, transform: Transform) -> Result<(), Never> {
    let Ok(affine) = convert_transform(transform);

    self.pre_concat_affine(affine)
  }

  pub(crate) fn pre_concat_affine(&mut self, transform: Affine) -> Result<(), Never> {
    self.current.ctm *= transform;

    Ok(())
  }

  pub(crate) fn get_color_space(
    &self,
    resources: &Resources<'_>,
    name: &Name<'_>,
  ) -> Result<Option<ColorSpace>, Never> {
    let Ok(found) = resources.get_color_space(name);
    let object = match found {
      Some(object) => object,
      None => return Ok(None),
    };
    let cache = &self.interpreter_cache.object_cache;

    cache.get_or_insert_with(object.cache_key(), || {
      let Ok(color_space) = ColorSpace::new(object.clone(), cache);

      color_space
    })
  }

  pub(crate) fn stroke_props(&self) -> Result<StrokeProps, Never> {
    Ok(self.current.graphics_state.stroke_props.clone())
  }

  pub(crate) fn num_states(&self) -> Result<u32, Never> {
    fitted::<_, u32>(self.saved.len().saturating_add(1))
  }

  pub(crate) fn nesting_depth(&self) -> Result<u32, Never> {
    Ok(self.nesting_depth)
  }

  pub(crate) fn begin_nested_interpretation(&mut self) -> Result<Nesting, Never> {
    match ::core::primitive::bool::then_some(
      self.nesting_depth >= MAX_NESTED_INTERPRETATION_DEPTH,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        warn!("interpreter nesting depth exceeded");

        return Ok(Nesting::TooDeep);
      }
      ::core::option::Option::None => {}
    }

    let Ok(budget) = take_nested_paint(self.nesting_depth.saturating_add(1));

    match budget {
      PaintBudget::Granted => {
        self.nesting_depth = self.nesting_depth.saturating_add(1);

        Ok(Nesting::Entered)
      }
      PaintBudget::Spent => {
        warn!("nested paint budget exceeded");

        Ok(Nesting::TooDeep)
      }
    }
  }

  pub(crate) fn end_nested_interpretation(&mut self) -> Result<(), Never> {
    self.nesting_depth = self.nesting_depth.saturating_sub(1);

    Ok(())
  }

  pub(crate) fn resolve_font(
    &mut self,
    font_dict: &Dict<'a>,
  ) -> Result<Option<TextStateFont<'a>>, Never> {
    let cache_key = font_dict.cache_key();
    let fonts = &self.interpreter_cache.font_cache;
    let cached = match fonts.try_borrow() {
      Ok(table) => table.get(&cache_key).cloned(),
      Err(_font_table_already_mutably_borrowed) => None,
    };
    let resolved = match cached {
      Some(resolved) => resolved,
      None => {
        let Ok(font) = Font::new(
          font_dict,
          &self.settings.font_resolver,
          &self.settings.cmap_resolver,
        );

        match fonts.try_borrow_mut() {
          Ok(mut table) => {
            table.insert(cache_key, font.clone());
          }
          Err(cache_error) => warn!(
            "font cache unavailable, using uncached face: {}",
            cache_error
          ),
        }

        font
      }
    };

    Ok(match resolved {
      Some(resolved_2) => Some(TextStateFont::Font(resolved_2)),
      None => {
        let Ok(fallback) =
          Font::new_standard(StandardFont::Helvetica, &self.settings.font_resolver);

        fallback.map(TextStateFont::Fallback)
      }
    })
  }
}

enum Covering {
  Whole,
  Partial,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reached {
  Yes,
  No,
}

pub(crate) fn path_as_rect(path: &BezPath) -> Result<Option<Rect>, Never> {
  let elements = path.elements();

  match elements.len() {
    5 => {}
    _ => return Ok(None),
  }

  let mut points = Vec::with_capacity(4);

  for (position, element) in elements.iter().enumerate() {
    match (element, position) {
      (PathEl::MoveTo(point), 0) | (PathEl::LineTo(point), _) => points.push(*point),
      (PathEl::MoveTo(_), _) | (PathEl::QuadTo(_, _), _) | (PathEl::CurveTo(_, _, _), _) => {
        return Ok(None);
      }
      (PathEl::ClosePath, _) => {}
    }
  }

  let Ok(bbox) = fast_bounding_box(path);
  let Ok(low_low) = touched(&points, Point::new(bbox.min_x(), bbox.min_y()));
  let Ok(low_high) = touched(&points, Point::new(bbox.min_x(), bbox.max_y()));
  let Ok(high_low) = touched(&points, Point::new(bbox.max_x(), bbox.min_y()));
  let Ok(high_high) = touched(&points, Point::new(bbox.max_x(), bbox.max_y()));

  Ok(match (low_low, low_high, high_low, high_high) {
    (Reached::Yes, Reached::Yes, Reached::Yes, Reached::Yes) => Some(bbox),
    _ => None,
  })
}

fn touched(points: &[Point], corner: Point) -> Result<Reached, Never> {
  let mut reached = Reached::No;

  for point in points {
    let Ok(across) = point.x.nearly(corner.x);
    let Ok(down) = point.y.nearly(corner.y);

    match (across, down) {
      (Nearly::Equal, Nearly::Equal) => reached = Reached::Yes,
      _ => {}
    }
  }

  Ok(reached)
}
