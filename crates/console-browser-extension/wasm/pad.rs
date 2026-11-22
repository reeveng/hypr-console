//! The pad's label and navigation algorithms. The browser supplies DOM geometry.

use std::cell::RefCell;

type Geometry = [f64; 6];

thread_local! {
  static RECTS: RefCell<Vec<Geometry>> = const { RefCell::new(Vec::new()) };
}

/// Encodes one label as two bits per arrow, preceded by a set sentinel bit.
/// Short labels come first. No label is the prefix of a different label.
#[unsafe(no_mangle)]
pub extern "C" fn hint_label(total: u32, index: u32) -> u64 {
  if index >= total {
    return 0;
  }
  let total = u64::from(total);
  let index = u64::from(index);
  let mut capacity = 4;
  while capacity < total {
    capacity *= 4;
  }
  let short = if capacity > 4 {
    ((capacity - total) / 3).min(capacity / 4)
  } else {
    0
  };

  if index < short {
    capacity / 4 + index
  } else {
    capacity + 4 * short + index - short
  }
}

/// Resizes the browser's scratch buffer. Views must be recreated after this
/// call. Each row holds left, top, width, height, label width and label height.
#[unsafe(no_mangle)]
pub extern "C" fn geometry_buffer(count: u32) -> *mut Geometry {
  RECTS.with_borrow_mut(|rects| {
    rects.resize(count as usize, [0.0; 6]);
    rects.as_mut_ptr()
  })
}

#[unsafe(no_mangle)]
pub extern "C" fn place_hints(viewport_width: f64, viewport_height: f64) {
  RECTS.with_borrow_mut(|rects| {
    place(rects, viewport_width, viewport_height);
  });
}

fn place(rects: &mut [Geometry], viewport_width: f64, viewport_height: f64) {
  for index in 0..rects.len() {
    let (placed, remaining) = rects.split_at_mut(index);
    let Some((rect, _)) = remaining.split_first_mut() else {
      break;
    };
    let [left, top, width, height, label_width, label_height] = *rect;
    let fits = height >= label_height + 8.0;
    let right = left + width;
    let bottom = top + height;
    let above = top - label_height + if fits { 9.0 } else { 4.0 };
    let corners = if fits {
      [
        (left - 3.0, above),
        (right - label_width + 3.0, above),
        (left - 3.0, bottom - 9.0),
        (left - 3.0, top + 2.0),
        (left - label_width - 3.0, top),
      ]
    } else {
      [
        (left - 3.0, above),
        (left - 3.0, top - 2.0),
        (left - label_width - 3.0, top - 2.0),
        (right - label_width + 3.0, above),
        (left - 3.0, bottom - 2.0),
      ]
    };
    let spacing = label_width + 4.0;
    let edge = (1..)
      .map(|step| f64::from(step) * spacing)
      .take_while(|distance| *distance < width)
      .map(|distance| (left - 3.0 + distance, above));
    let mut candidates = corners.into_iter().chain(edge).map(|(x, y)| {
      (
        x.min(viewport_width - label_width - 2.0).max(2.0),
        y.min(viewport_height - label_height - 2.0).max(2.0),
      )
    });
    if let Some(first) = candidates.next() {
      let unoccupied = |&(x, y): &(f64, f64)| {
        !placed
          .iter()
          .any(|&[other_x, other_y, _, _, other_width, other_height]| {
            x < other_x + other_width + 2.0
              && other_x < x + label_width + 2.0
              && y < other_y + other_height + 2.0
              && other_y < y + label_height + 2.0
          })
      };
      let (x, y) = if unoccupied(&first) {
        first
      } else {
        candidates.find(unoccupied).unwrap_or(first)
      };
      let [out_x, out_y, ..] = rect;
      *out_x = x;
      *out_y = y;
    }
  }
}

/// Returns the index of the nearest aligned element, or -1 for scrolling.
#[unsafe(no_mangle)]
pub extern "C" fn walk_toward(
  from_x: f64,
  from_y: f64,
  axis_x: f64,
  axis_y: f64,
  standing: i32,
) -> i32 {
  RECTS.with_borrow(|rects| walk(rects, from_x, from_y, axis_x, axis_y, standing))
}

fn walk(
  rects: &[Geometry],
  from_x: f64,
  from_y: f64,
  axis_x: f64,
  axis_y: f64,
  standing: i32,
) -> i32 {
  let mut best = -1;
  let mut cost = f64::INFINITY;
  for (index, &[left, top, width, height, ..]) in rects.iter().enumerate() {
    let Ok(index) = i32::try_from(index) else {
      break;
    };
    if index == standing {
      continue;
    }
    let delta_x = left + width / 2.0 - from_x;
    let delta_y = top + height / 2.0 - from_y;
    let along = delta_x * axis_x + delta_y * axis_y;
    let across = (delta_x * axis_y + delta_y * axis_x).abs();
    let asked = along + across * 3.0;
    if along >= 4.0 && asked < cost {
      cost = asked;
      best = index;
    }
  }
  best
}
