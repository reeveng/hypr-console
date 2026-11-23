use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index};
use console_core_paths::SaturateCast;

use crate::blitter::SpanBlitter;
use crate::edge::{Edge, LineEdge};
use crate::edge_builder::{BasicEdgeBuilder, ShiftedIntRect};
use crate::fixed_point::{FDot16, fdot6, fdot16};
use crate::geom::{IntRectExt, ScreenIntRect};
use crate::{ContainsAnswer, FillRule, IntRect, LengthU32, Path, FloatRectangle, UpdateAnswer};

pub fn fill_path(
  path: &Path,
  fill_rule: FillRule,
  clip: &ScreenIntRect,
  blitter: &mut dyn SpanBlitter,
) -> Result<(), Never> {
  let Ok(bounds) = path.bounds();
  let Ok(rounded) = conservative_round_to_int(&bounds);
  let bounds_2 = match rounded {
    Some(bounds_2) => bounds_2,
    None => return Ok(()),
  };
  fill_path_impl(
    path,
    fill_rule,
    clip,
    Scan {
      bounds: bounds_2,
      shift: 0,
    },
    blitter,
  )
}

fn conservative_round_to_int(src: &FloatRectangle) -> Result<Option<IntRect>, Never> {
  let bias = 0.5 + 1.5 / f64::from(fdot6::ONE);
  let Ok(left) = src.left();
  let Ok(top) = src.top();
  let Ok(right) = src.right();
  let Ok(bottom) = src.bottom();

  IntRect::from_ltrb(
    i32::saturate_from((f64::from(left) - bias).ceil()),
    i32::saturate_from((f64::from(top) - bias).ceil()),
    i32::saturate_from((f64::from(right) + bias).floor()),
    i32::saturate_from((f64::from(bottom) + bias).floor()),
  )
}

pub(super) struct Scan {
  pub bounds: IntRect,
  pub shift: i32,
}

pub(super) fn fill_path_impl(
  path: &Path,
  fill_rule: FillRule,
  clip_rect: &ScreenIntRect,
  scan: Scan,
  blitter: &mut dyn SpanBlitter,
) -> Result<(), Never> {
  let Ok(shifted) = ShiftedIntRect::new(clip_rect, scan.shift);
  let shifted_clip = match shifted {
    Some(shifted_clip) => shifted_clip,
    None => return Ok(()),
  };
  let comparison = scan.bounds.to_screen_int_rect().map(|bounds| {
    let Ok(contains) = clip_rect.contains(&bounds);
    contains
  });
  let clip = match comparison {
    Some(ContainsAnswer::Yes) => None,
    Some(ContainsAnswer::No) | None => Some(&shifted_clip),
  };
  let Ok(built) = BasicEdgeBuilder::build_edges(path, clip, scan.shift);
  let edges = match built {
    Some(edges) => edges,
    None => return Ok(()),
  };
  let Ok(gathered) = ScanEdges::new(edges);
  let mut edges_2 = match gathered {
    Some(edges_2) => edges_2,
    None => return Ok(()),
  };
  let Ok(shift) = fitted::<i32, u32>(scan.shift);
  let Ok(start_y) = scan.bounds.y();
  let Ok(stop_y) = scan.bounds.bottom();
  let shifted_start = start_y.wrapping_shl(shift);
  let shifted_stop = stop_y.wrapping_shl(shift);
  let Ok(shifted_rect) = shifted_clip.shifted();
  let Ok(top) = shifted_rect.y();
  let Ok(bottom) = shifted_rect.bottom();
  let Ok(clip_top) = fitted::<u32, i32>(top);
  let Ok(clip_bottom) = fitted::<u32, i32>(bottom);
  let (first, stop) = match clip {
    Some(_) => (shifted_start.max(clip_top), shifted_stop.min(clip_bottom)),
    None => (shifted_start, shifted_stop),
  };
  let row_range = match (u32::try_from(first), u32::try_from(stop)) {
    (Ok(start), Ok(end)) => start..end.max(start.saturating_add(1)),
    (Err(_negative_start), _) => return Ok(()),
    (_, Err(_negative_stop)) => return Ok(()),
  };

  let Ok(right_side) = shifted_rect.right();

  for row in row_range {
    let Ok(pending) = edges_2.draw_row(
      RowFill {
        row,
        right_side,
        fill_rule,
      },
      blitter,
    );

    match pending {
      Some(next) => {
        let Ok(row_number) = fitted::<u32, i32>(row.saturating_add(1));
        let Ok(()) = edges_2.activate(next, row_number);
      }
      None => break,
    }
  }

  Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct EdgeId(u32);

const HEAD: EdgeId = EdgeId(0);

struct ScanEdges {
  slots: Vec<Edge>,
}

struct RowFill {
  row: u32,
  right_side: u32,
  fill_rule: FillRule,
}

struct Winding {
  start: u32,
  count: i32,
  mask: i32,
}

impl Winding {
  fn crossed(
    &mut self,
    edge: &LineEdge,
    row: u32,
    blitter: &mut dyn SpanBlitter,
  ) -> Result<(), Never> {
    let Ok(rounded) = fdot16::round_to_i32(edge.x);
    let Ok(x) = console_core_number_conversion::wrapped::<i32, u32>(rounded);

    match self.count & self.mask {
      0 => self.start = x,
      _ => {}
    }

    let Ok(delta) = edge.winding.delta();
    self.count = self.count.wrapping_add(delta);

    match self.count & self.mask {
      0 => draw_span(self.start..x, row, blitter),
      _ => Ok(()),
    }
  }
}

fn draw_span(span: Range<u32>, row: u32, blitter: &mut dyn SpanBlitter) -> Result<(), Never> {
  match LengthU32::new(span.end.wrapping_sub(span.start)) {
    Some(width) => blitter.blit_h(
      crate::geom::PixelPosition {
        x: span.start,
        y: row,
      },
      width,
    ),
    None => Ok(()),
  }
}

impl ScanEdges {
  fn new(mut slots: Vec<Edge>) -> Result<Option<Self>, Never> {
    let Ok(count) = fixed_index(slots.len());
    let last = match u32::try_from(count) {
      Ok(total) => total,
      Err(_too_many_edges_for_the_link_representation) => return Ok(None),
    };

    match last.checked_add(2) {
      Some(_) => {}
      None => return Ok(None),
    }

    slots.sort_by_key(|edge| (edge.first_y, edge.x));

    let mut previous = 0u32;

    for edge in &mut slots {
      edge.prev = Some(previous);
      edge.next = Some(previous.saturating_add(2));
      previous = previous.saturating_add(1);
    }

    slots.insert(
      0,
      Edge::Line(LineEdge {
        next: Some(1),
        x: i32::MIN,
        first_y: i32::MIN,
        ..LineEdge::default()
      }),
    );
    slots.push(Edge::Line(LineEdge {
      prev: Some(last),
      first_y: i32::MAX,
      ..LineEdge::default()
    }));

    Ok(Some(Self { slots }))
  }

  fn edge(&self, id: EdgeId) -> Result<Option<&Edge>, Never> {
    let Ok(position) = index(id.0);

    Ok(self.slots.get(position))
  }

  fn edge_mut(&mut self, id: EdgeId) -> Result<Option<&mut Edge>, Never> {
    let Ok(position) = index(id.0);

    Ok(self.slots.get_mut(position))
  }

  fn draw_row(
    &mut self,
    fill: RowFill,
    blitter: &mut dyn SpanBlitter,
  ) -> Result<Option<EdgeId>, Never> {
    let Ok(head) = self.edge(HEAD);
    let (mut current, mut previous_x) = match head {
      Some(head_2) => (head_2.next.map(EdgeId), head_2.x),
      None => return Ok(None),
    };
    let mut winding = Winding {
      start: 0,
      count: 0,
      mask: match fill.fill_rule {
        FillRule::EvenOdd => 1,
        FillRule::Winding => -1,
      },
    };
    let Ok(row) = fitted::<u32, i32>(fill.row);
    let Ok(capacity) = fixed_index(self.slots.len());

    for _ in 0..capacity {
      let id = match current {
        Some(id) => id,
        None => return Ok(None),
      };
      let Ok(found) = self.edge_mut(id);
      let edge = match found {
        Some(edge) => edge,
        None => return Ok(None),
      };

      match edge.first_y.cmp(&row) {
        Ordering::Greater => break,
        Ordering::Less | Ordering::Equal => {}
      }

      current = edge.next.map(EdgeId);
      let Ok(line) = edge.as_line();
      let Ok(()) = winding.crossed(line, fill.row, blitter);
      let Ok(next) = advanced(edge, row);

      match next {
        Advance::Finished => {
          let Ok(()) = self.detach(id);
        }
        Advance::At(x) => match x.cmp(&previous_x) {
          Ordering::Less => {
            let Ok(()) = self.move_left(id, x);
          }
          Ordering::Equal | Ordering::Greater => previous_x = x,
        },
      }
    }

    match winding.count & winding.mask {
      0 => {}
      _ => {
        let Ok(()) = draw_span(winding.start..fill.right_side, fill.row, blitter);
      }
    }

    Ok(current)
  }

  fn detach(&mut self, id: EdgeId) -> Result<(), Never> {
    let Ok(found) = self.edge(id);
    let edge = match found {
      Some(edge) => edge,
      None => return Ok(()),
    };

    match (edge.prev, edge.next) {
      (Some(previous), Some(next)) => {
        let Ok(()) = self.set_next(EdgeId(previous), EdgeId(next));
        let Ok(()) = self.set_previous(EdgeId(next), EdgeId(previous));
      }
      (None, _) | (_, None) => {}
    }

    Ok(())
  }

  fn set_previous(&mut self, slot: EdgeId, previous: EdgeId) -> Result<(), Never> {
    let Ok(found) = self.edge_mut(slot);

    match found {
      Some(edge) => edge.prev = Some(previous.0),
      None => {}
    }

    Ok(())
  }

  fn set_next(&mut self, slot: EdgeId, next: EdgeId) -> Result<(), Never> {
    let Ok(found) = self.edge_mut(slot);

    match found {
      Some(edge) => edge.next = Some(next.0),
      None => {}
    }

    Ok(())
  }

  fn move_after(&mut self, id: EdgeId, after: EdgeId) -> Result<(), Never> {
    let Ok(slot) = self.edge(after);
    let following = match slot.and_then(|edge| edge.next).map(EdgeId) {
      Some(following) => following,
      None => return Ok(()),
    };

    match (after == id || following == id).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(()) = self.detach(id);
    let Ok(()) = self.set_previous(id, after);
    let Ok(()) = self.set_next(id, following);
    let Ok(()) = self.set_previous(following, id);
    let Ok(()) = self.set_next(after, id);

    Ok(())
  }

  fn previous_with_x(&self, from: EdgeId, x: FDot16) -> Result<Option<EdgeId>, Never> {
    let mut cursor = from;
    let Ok(capacity) = fixed_index(self.slots.len());

    for _ in 0..capacity {
      let Ok(found) = self.edge(cursor);
      let edge = match found {
        Some(edge) => edge,
        None => return Ok(None),
      };

      match (edge.x.cmp(&x), edge.prev) {
        (Ordering::Less | Ordering::Equal, _) | (_, None) => return Ok(Some(cursor)),
        (Ordering::Greater, Some(previous)) => cursor = EdgeId(previous),
      }
    }

    Ok(None)
  }

  fn move_left(&mut self, edge: EdgeId, x: FDot16) -> Result<(), Never> {
    let Ok(found) = self.edge(edge);
    let previous = match found.and_then(|at| at.prev) {
      Some(previous) => EdgeId(previous),
      None => return Ok(()),
    };
    let Ok(destination) = self.previous_with_x(previous, x);

    match destination {
      Some(after) => self.move_after(edge, after),
      None => Ok(()),
    }
  }

  fn activate(&mut self, first: EdgeId, row: i32) -> Result<(), Never> {
    let Ok(found) = self.edge(first);
    let edge = match found {
      Some(edge) => edge,
      None => return Ok(()),
    };
    let previous = match edge.prev {
      Some(previous) => EdgeId(previous),
      None => return Ok(()),
    };
    let Ok(previous_edge) = self.edge(previous);

    match previous_edge.map(|previous_2| (edge.first_y == row, previous_2.x.cmp(&edge.x))) {
      Some((true, Ordering::Greater)) => {}
      Some((false, _) | (_, Ordering::Less | Ordering::Equal)) | None => return Ok(()),
    }

    let Ok(destination) = self.previous_with_x(previous, edge.x);
    let mut after = match destination {
      Some(after) => after,
      None => return Ok(()),
    };
    let mut incoming = first;
    let Ok(capacity) = fixed_index(self.slots.len());

    for _ in 0..capacity {
      let Ok(found_2) = self.edge(incoming);
      let waiting = match found_2.filter(|edge_2| edge_2.first_y == row) {
        Some(waiting) => waiting,
        None => break,
      };
      let next = waiting.next.map(EdgeId);
      let Ok(spot) = self.insertion_from(after, incoming, waiting.x);

      match spot {
        Some(after_2) => {
          let Ok(()) = self.move_after(incoming, after_2);
        }
        None => break,
      }

      after = incoming;
      incoming = match next {
        Some(next_2) => next_2,
        None => break,
      };
    }

    Ok(())
  }

  fn insertion_from(
    &self,
    mut after: EdgeId,
    incoming: EdgeId,
    x: FDot16,
  ) -> Result<Option<EdgeId>, Never> {
    let Ok(capacity) = fixed_index(self.slots.len());

    for _ in 0..capacity {
      let Ok(found) = self.edge(after);
      let candidate = match found.and_then(|edge| edge.next).map(EdgeId) {
        Some(candidate) => candidate,
        None => return Ok(None),
      };
      let Ok(ahead) = self.edge(candidate);

      match ahead.map(|edge| (candidate == incoming, edge.x.cmp(&x))) {
        Some((true, _) | (_, Ordering::Equal | Ordering::Greater)) => return Ok(Some(after)),
        Some((false, Ordering::Less)) => after = candidate,
        None => return Ok(None),
      }
    }

    Ok(None)
  }
}

enum Advance {
  Finished,
  At(FDot16),
}

fn advanced(edge: &mut Edge, row: i32) -> Result<Advance, Never> {
  match edge.last_y.cmp(&row) {
    Ordering::Greater => {
      edge.x = edge.x.wrapping_add(edge.dx);

      Ok(Advance::At(edge.x))
    }
    Ordering::Less => Ok(Advance::Finished),
    Ordering::Equal => {
      let (updated, x) = match edge {
        Edge::Line(_) => return Ok(Advance::Finished),
        Edge::Quadratic(quad) => {
          let Ok(updated) = quad.update();

          (updated, quad.line.x)
        }
        Edge::Cubic(cubic) => {
          let Ok(updated) = cubic.update();

          (updated, cubic.line.x)
        }
      };

      Ok(match updated {
        UpdateAnswer::Yes => Advance::At(x),
        UpdateAnswer::No => Advance::Finished,
      })
    }
  }
}
