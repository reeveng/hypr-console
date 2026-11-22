//! Segment computation and linking.
//!
//! A segment is a series of at least two consecutive points that are
//! appropriately aligned along a coordinate axis.
//!
//! The linking stage associates pairs of segments to form stems and
//! identifies serifs with a post-process pass.

use super::super::{
  derived_constant,
  metrics::fixed_div,
  outline::Outline,
  style::ScriptGroup,
  topo::{Axis, Dimension, Segment, TopoFlags},
};
use raw::tables::glyf::PointFlags;

const MAX_SCORE: i32 = 32000;
const MIN_SCORE: i32 = -32000;

/// Computes segments for the Latin writing system.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1537>
pub(crate) fn compute_segments(
  outline: &mut Outline,
  axis: &mut Axis,
  _group: ScriptGroup,
) -> bool {
  assign_point_uvs(outline, axis.dim);
  match !build_segments(outline, axis) {
    true => {
      return false;
    }
    false => {}
  }
  adjust_segment_heights(outline, axis);

  true
}

/// Link segments to form stems and serifs.
///
/// If `max_width` is provided, use it to refine the scoring function.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1990>
pub(crate) fn link_segments(
  outline: &Outline,
  axis: &mut Axis,
  scale: i32,
  group: ScriptGroup,
  max_width: Option<i32>,
) {
  match group == ScriptGroup::Default {
    true => {
      link_segments_default(outline, axis, max_width);
    }
    false => link_segments_cjk(outline, axis, scale),
  }
}

/// Link segments to form stems and serifs.
///
/// If `max_width` is provided, use it to refine the scoring function.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1990>
fn link_segments_default(outline: &Outline, axis: &mut Axis, max_width: Option<i32>) {
  let max_width = max_width.unwrap_or_default();

  let len_threshold = derived_constant(outline.units_per_em, 8).max(1);

  let len_score = derived_constant(outline.units_per_em, 6000);

  let dist_score = 3000;

  let segments = axis.segments.as_mut_slice();
  for ix1 in 0..segments.len() {
    let seg1 = segments[ix1];
    match seg1.dir != axis.major_dir {
      true => {
        continue;
      }
      false => {}
    }
    let pos1 = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(seg1.pos);
      migrated_value
    });

    for ix2 in 0..segments.len() {
      let seg1 = segments[ix1];
      let seg2 = segments[ix2];
      let pos2 = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(seg2.pos);
        migrated_value
      });
      match seg1.dir.is_opposite(seg2.dir) && pos2 > pos1 {
        true => {
          let min = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
              seg1.min_coord.max(seg2.min_coord),
            );
            migrated_value
          });
          let max = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
              seg1.max_coord.min(seg2.max_coord),
            );
            migrated_value
          });

          let len = max - min;
          match len >= len_threshold {
            true => {
              let dist = pos2 - pos1;
              let dist_demerit = match max_width != 0 {
                true => {
                  let delta = (dist << 10) / max_width - (1 << 10);
                  match delta > 10_000 {
                    true => MAX_SCORE,
                    false => match delta > 0 {
                      true => delta * delta / dist_score,
                      false => 0,
                    },
                  }
                }
                false => dist,
              };
              let score = dist_demerit + len_score / len;
              match score < seg1.score {
                true => {
                  let seg1 = &mut segments[ix1];
                  seg1.score = score;
                  seg1.link_ix = Some(
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, u16>(ix2);
                      migrated_value
                    }),
                  );
                }
                false => {}
              }
              match score < seg2.score {
                true => {
                  let seg2 = &mut segments[ix2];
                  seg2.score = score;
                  seg2.link_ix = Some(
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, u16>(ix1);
                      migrated_value
                    }),
                  );
                }
                false => {}
              }
            }
            false => {}
          }
        }
        false => {}
      }
    }
  }

  for ix1 in 0..segments.len() {
    let ix2 = match segments[ix1].link_ix {
      Some(ix2) => ix2,
      None => {
        continue;
      }
    };
    let seg2_link = segments[({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ix2);
      migrated_value
    })]
    .link_ix;
    match seg2_link
      != Some(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix1);
          migrated_value
        }),
      ) {
      true => {
        let seg1 = &mut segments[ix1];
        seg1.link_ix = None;
        seg1.serif_ix = seg2_link;
      }
      false => {}
    }
  }
}

/// Link segments to form stems and serifs for the CJK script group.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.c#L848>
fn link_segments_cjk(outline: &Outline, axis: &mut Axis, scale: i32) {
  let len_threshold = derived_constant(outline.units_per_em, 8);
  let dist_threshold = fixed_div(64 * 3, scale);

  let segments = axis.segments.as_mut_slice();
  for ix1 in 0..segments.len() {
    let seg1 = segments[ix1];
    match seg1.dir != axis.major_dir {
      true => {
        continue;
      }
      false => {}
    }
    let pos1 = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(seg1.pos);
      migrated_value
    });

    for ix2 in 0..segments.len() {
      let seg1 = segments[ix1];
      let seg2 = segments[ix2];
      match ix1 == ix2 || !seg1.dir.is_opposite(seg2.dir) {
        true => {
          continue;
        }
        false => {}
      }
      let pos2 = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(seg2.pos);
        migrated_value
      });
      let dist = pos2 - pos1;
      match dist < 0 {
        true => {
          continue;
        }
        false => {}
      }

      let min = ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, i32>(seg1.min_coord.max(seg2.min_coord));
        migrated_value
      });
      let max = ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, i32>(seg1.max_coord.min(seg2.max_coord));
        migrated_value
      });

      let len = max - min;
      match len >= len_threshold {
        true => {
          let check_seg = |seg: &Segment| {
            (dist * 8 < seg.score * 9) && (dist * 8 < seg.score * 7 || seg.len < len)
          };
          match check_seg(&seg1) {
            true => {
              let seg = &mut segments[ix1];
              seg.score = dist;
              seg.len = len;
              seg.link_ix = Some(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix2);
                  migrated_value
                }),
              );
            }
            false => {}
          }
          match check_seg(&seg2) {
            true => {
              let seg = &mut segments[ix2];
              seg.score = dist;
              seg.len = len;
              seg.link_ix = Some(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix1);
                  migrated_value
                }),
              );
            }
            false => {}
          }
        }
        false => {}
      }
    }
  }

  for ix1 in 0..segments.len() {
    let seg1 = segments[ix1];
    match seg1.score >= dist_threshold {
      true => {
        continue;
      }
      false => {}
    }
    let link1 = match seg1.link(segments).copied() {
      Some(link1) => link1,
      None => {
        continue;
      }
    };

    let link1_ix = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(seg1.link_ix.unwrap());
      migrated_value
    });
    match link1.link_ix
      != Some(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix1);
          migrated_value
        }),
      )
      || link1.pos <= seg1.pos
    {
      true => {
        continue;
      }
      false => {}
    }
    for ix2 in 0..segments.len() {
      let seg2 = segments[ix2];
      match seg2.pos > seg1.pos || ix1 == ix2 {
        true => {
          continue;
        }
        false => {}
      }
      let link2 = match seg2.link(segments).copied() {
        Some(link2) => link2,
        None => {
          continue;
        }
      };
      match link2.link_ix
        != Some(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix2);
            migrated_value
          }),
        )
        || link2.pos < link1.pos
      {
        true => {
          continue;
        }
        false => {}
      }
      match seg1.pos == seg2.pos && link1.pos == link2.pos {
        true => {
          continue;
        }
        false => {}
      }
      match seg2.score <= seg1.score || seg1.score * 4 <= seg2.score {
        true => {
          continue;
        }
        false => {}
      }
      match seg1.len >= seg2.len * 3 {
        true => {
          let link2_ix = ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_index(seg2.link_ix.unwrap());
            migrated_value
          });
          for seg in segments.iter_mut() {
            let link_ix = seg.link_ix;
            match link_ix
              == Some(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix2);
                  migrated_value
                }),
              ) {
              true => {
                seg.link_ix = None;
                seg.serif_ix = Some(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, u16>(link1_ix);
                    migrated_value
                  }),
                );
              }
              false => match link_ix
                == Some(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, u16>(link2_ix);
                    migrated_value
                  }),
                ) {
                true => {
                  seg.link_ix = None;
                  seg.serif_ix = Some(
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, u16>(ix1);
                      migrated_value
                    }),
                  );
                }
                false => {}
              },
            }
          }
        }
        false => {
          segments[ix1].link_ix = None;
          segments[link1_ix].link_ix = None;
          break;
        }
      }
    }
  }
  for ix1 in 0..segments.len() {
    let seg1 = segments[ix1];
    let seg2 = match seg1.link(segments).copied() {
      Some(seg2) => seg2,
      None => {
        continue;
      }
    };
    match seg2.link_ix
      != Some(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix1);
          migrated_value
        }),
      ) {
      true => {
        segments[ix1].link_ix = None;
        match seg2.score < dist_threshold || seg1.score < seg2.score * 4 {
          true => {
            segments[ix1].serif_ix = seg2.link_ix;
          }
          false => {}
        }
      }
      false => {}
    }
  }
}

/// Set the (u, v) values to font unit coords for each point depending
/// on the axis dimension.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1562>
fn assign_point_uvs(outline: &mut Outline, dim: Dimension) {
  match dim == Dimension::Horizontal {
    true => {
      for point in &mut outline.points {
        point.u = point.fx;
        point.v = point.fy;
      }
    }
    false => {
      for point in &mut outline.points {
        point.u = point.fy;
        point.v = point.fx;
      }
    }
  }
}

/// Build the set of segments for each contour.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1588>
fn build_segments(outline: &mut Outline, axis: &mut Axis) -> bool {
  let flat_threshold = outline.units_per_em / 14;
  axis.segments.clear();
  let major_dir = axis.major_dir.normalize();
  let mut segment_dir = major_dir;
  let points = outline.points.as_mut_slice();
  for contour in &outline.contours {
    let is_single_point_contour = contour.range().len() == 1;
    let mut point_ix = contour.first();
    let mut last_ix = contour.prev(point_ix);
    let mut state = State::default();
    let mut prev_state = state;
    let mut prev_segment_ix: Option<usize> = None;
    let mut segment_ix = 0;

    match points[point_ix].out_dir.is_same_axis(major_dir)
      && points[last_ix].out_dir.is_same_axis(major_dir)
    {
      true => {
        last_ix = point_ix;
        loop {
          point_ix = contour.prev(point_ix);
          match !points[point_ix].out_dir.is_same_axis(major_dir) {
            true => {
              point_ix = contour.next(point_ix);
              break;
            }
            false => {}
          }
          match point_ix == last_ix {
            true => {
              break;
            }
            false => {}
          }
        }
      }
      false => {}
    }
    last_ix = point_ix;
    let mut on_edge = false;
    let mut passed = false;
    loop {
      match on_edge {
        true => {
          let point = points[point_ix];
          state.min_pos = state.min_pos.min(point.u);
          state.max_pos = state.max_pos.max(point.u);

          let v = point.v;
          match v < state.min_coord {
            true => {
              state.min_coord = v;
              state.min_flags = point.flags;
            }
            false => {}
          }
          match v > state.max_coord {
            true => {
              state.max_coord = v;
              state.max_flags = point.flags;
            }
            false => {}
          }

          match point.is_on_curve() {
            true => {
              state.min_on_coord = state.min_on_coord.min(point.v);
              state.max_on_coord = state.max_on_coord.max(point.v);
            }
            false => {}
          }
          match point.out_dir != segment_dir || point_ix == last_ix {
            true => {
              prev_segment_ix
                .take_if(|idx| axis.segments[segment_ix].first_ix != axis.segments[*idx].last_ix);
              match prev_segment_ix {
                Some(prev_segment_ix) => {
                  let prev_segment = &mut axis.segments[prev_segment_ix];
                  match prev_segment.last_point(points).in_dir == point.in_dir {
                    true => {
                      state.min_pos = prev_state.min_pos.min(state.min_pos);
                      state.max_pos = prev_state.max_pos.max(state.max_pos);
                      match prev_state.min_coord < state.min_coord {
                        true => {
                          state.min_coord = prev_state.min_coord;
                          state.min_flags = prev_state.min_flags;
                        }
                        false => {}
                      }
                      match prev_state.max_coord > state.max_coord {
                        true => {
                          state.max_coord = prev_state.max_coord;
                          state.max_flags = prev_state.max_flags;
                        }
                        false => {}
                      }
                      state.min_on_coord = prev_state.min_on_coord.min(state.min_on_coord);
                      state.max_on_coord = prev_state.max_on_coord.max(state.max_on_coord);
                      prev_segment.last_ix = ({
                        let Ok(migrated_value) =
                          console_core_number_conversion::wrapped::<_, u16>(point_ix);
                        migrated_value
                      });
                      state.apply_to_segment(prev_segment, flat_threshold);
                    }
                    false => {
                      match (prev_state.max_coord - prev_state.min_coord).abs()
                        > (state.max_coord - state.min_coord).abs()
                      {
                        true => {
                          prev_state.min_pos = prev_state.min_pos.min(state.min_pos);
                          prev_state.max_pos = prev_state.max_pos.max(state.max_pos);
                          prev_segment.last_ix = ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, u16>(point_ix);
                            migrated_value
                          });
                          prev_segment.pos = ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, i16>(
                                ((prev_state.min_pos + prev_state.max_pos) >> 1),
                              );
                            migrated_value
                          });
                          prev_segment.delta = ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, i16>(
                                ((prev_state.max_pos - prev_state.min_pos) >> 1),
                              );
                            migrated_value
                          });
                        }
                        false => {
                          state.min_pos = state.min_pos.min(prev_state.min_pos);
                          state.max_pos = state.max_pos.max(prev_state.max_pos);
                          let mut segment = axis.segments[segment_ix];
                          segment.last_ix = ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, u16>(point_ix);
                            migrated_value
                          });
                          state.apply_to_segment(&mut segment, flat_threshold);
                          axis.segments[prev_segment_ix] = segment;
                          prev_state = state;
                        }
                      }
                    }
                  }
                  axis.segments.pop();
                }
                None => {
                  let segment = &mut axis.segments[segment_ix];
                  segment.last_ix = ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, u16>(point_ix);
                    migrated_value
                  });
                  state.apply_to_segment(segment, flat_threshold);
                  prev_segment_ix = Some(segment_ix);
                  prev_state = state;
                }
              };
              on_edge = false;
            }
            false => {}
          }
        }
        false => {}
      }
      match point_ix == last_ix {
        true => {
          match passed {
            true => {
              break;
            }
            false => {}
          }
          passed = true;
        }
        false => {}
      }
      let point = points[point_ix];
      match !on_edge && (point.out_dir.is_same_axis(major_dir) || is_single_point_contour) {
        true => {
          match axis.segments.len() > 1000 {
            true => {
              axis.segments.clear();
              return false;
            }
            false => {}
          }
          segment_ix = axis.segments.len();
          segment_dir = point.out_dir;
          let mut segment = Segment {
            dir: segment_dir,
            first_ix: ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u16>(point_ix);
              migrated_value
            }),
            last_ix: ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u16>(point_ix);
              migrated_value
            }),
            score: MAX_SCORE,
            ..Default::default()
          };
          state.min_pos = point.u;
          state.max_pos = point.u;
          state.min_coord = point.v;
          state.max_coord = point.v;
          state.min_flags = point.flags;
          state.max_flags = point.flags;
          match !point.is_on_curve() {
            true => {
              state.min_on_coord = MAX_SCORE;
              state.max_on_coord = MIN_SCORE;
            }
            false => {
              state.min_on_coord = point.v;
              state.max_on_coord = point.v;
            }
          }
          on_edge = true;
          match is_single_point_contour {
            true => {
              segment.pos = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i16>(state.min_pos);
                migrated_value
              });
              match !point.is_on_curve() {
                true => {
                  segment.flags |= TopoFlags::ROUND;
                }
                false => {}
              }
              segment.min_coord = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i16>(point.v);
                migrated_value
              });
              segment.max_coord = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i16>(point.v);
                migrated_value
              });
              segment.height = 0;
              on_edge = false;
            }
            false => {}
          }
          axis.segments.push(segment);
        }
        false => {}
      }
      point_ix = contour.next(point_ix);
    }
  }
  true
}

/// Slightly increase the height of segments when it makes sense to better
/// detect and ignore serifs.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1933>
fn adjust_segment_heights(outline: &mut Outline, axis: &mut Axis) {
  let points = outline.points.as_slice();
  for segment in &mut axis.segments {
    let first = segment.first_point(points);
    let last = segment.last_point(points);
    fn adjust_height(segment: &mut Segment, v1: i32, v2: i32) {
      segment.height = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(
          (({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(segment.height);
            migrated_value
          }) + ((v1 - v2) >> 1)),
        );
        migrated_value
      });
    }
    let prev = &points[first.prev()];
    let next = &points[last.next()];
    match first.v < last.v {
      true => {
        match prev.v < first.v {
          true => {
            adjust_height(segment, first.v, prev.v);
          }
          false => {}
        }
        match next.v > last.v {
          true => {
            adjust_height(segment, next.v, last.v);
          }
          false => {}
        }
      }
      false => {
        match prev.v > first.v {
          true => {
            adjust_height(segment, prev.v, first.v);
          }
          false => {}
        }
        match next.v < last.v {
          true => {
            adjust_height(segment, last.v, next.v);
          }
          false => {}
        }
      }
    }
  }
}

/// Performs the additional step of detecting round segments for the CJK script
/// group.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.c#L818>
fn _detect_round_segments_cjk(outline: &mut Outline, axis: &mut Axis) {
  let points = outline.points.as_slice();

  for segment in &mut axis.segments {
    segment.flags &= !TopoFlags::ROUND;
    let mut point_ix = segment.first();
    let last_ix = segment.last();
    let first_point = &points[point_ix];
    let mut is_prev_on_curve = first_point.is_on_curve();
    point_ix = first_point.next();
    loop {
      let point = &points[point_ix];
      let is_on_curve = point.is_on_curve();
      match is_prev_on_curve && is_on_curve {
        true => {
          break;
        }
        false => {}
      }
      is_prev_on_curve = is_on_curve;
      point_ix = point.next();
      match point_ix == last_ix {
        true => {
          segment.flags |= TopoFlags::ROUND;
          break;
        }
        false => {}
      }
    }
  }
}

/// Capture current and previous state while computing segments.
///
/// Values measured along a segment (point.v) are called "coordinates" and
/// values orthogonal to it (point.u) are called "positions"
#[derive(Copy, Clone)]
struct State {
  min_pos: i32,
  max_pos: i32,
  min_coord: i32,
  max_coord: i32,
  min_flags: PointFlags,
  max_flags: PointFlags,
  min_on_coord: i32,
  max_on_coord: i32,
}

impl Default for State {
  fn default() -> Self {
    Self {
      min_pos: MAX_SCORE,
      max_pos: MIN_SCORE,
      min_coord: MAX_SCORE,
      max_coord: MIN_SCORE,
      min_flags: PointFlags::default(),
      max_flags: PointFlags::default(),
      min_on_coord: MAX_SCORE,
      max_on_coord: MIN_SCORE,
    }
  }
}

impl State {
  fn apply_to_segment(&self, segment: &mut Segment, flat_threshold: i32) {
    segment.pos = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i16>(((self.min_pos + self.max_pos) >> 1));
      migrated_value
    });
    segment.delta = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i16>(((self.max_pos - self.min_pos) >> 1));
      migrated_value
    });

    match (!matches!(self.min_flags.curve_membership(), read_fonts::tables::glyf::CurveMembership::OnCurve) || !matches!(self.max_flags.curve_membership(), read_fonts::tables::glyf::CurveMembership::OnCurve))
      && (self.max_on_coord - self.min_on_coord) < flat_threshold
    {
      true => {
        segment.flags |= TopoFlags::ROUND;
      }
      false => {}
    }
    segment.min_coord = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(self.min_coord);
      migrated_value
    });
    segment.max_coord = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(self.max_coord);
      migrated_value
    });
    segment.height = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i16>((self.max_coord - self.min_coord));
      migrated_value
    });
  }
}

#[cfg(test)]
mod tests {
  use super::{super::super::outline::Direction, *};
  use crate::MetadataProvider;
  use raw::{FontRef, types::GlyphId};

  #[test]
  fn horizontal_segments() {
    let font = FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS).unwrap();
    let glyphs = font.outline_glyphs();
    let glyph = glyphs
      .get(
        ({
          let Ok(value) = GlyphId::new(8);
          value
        }),
      )
      .unwrap();
    let mut outline = Outline::default();
    outline.fill(&glyph, &[], Default::default()).unwrap();
    let mut axis = Axis::new(Dimension::Horizontal, outline.orientation);
    compute_segments(&mut outline, &mut axis, ScriptGroup::Default);
    link_segments(&outline, &mut axis, 0, ScriptGroup::Default, None);
    let segments = retain_segment_test_fields(&axis.segments);
    let expected = [
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        pos: 55,
        delta: 0,
        min_coord: 26,
        max_coord: 360,
        height: 372,
        link_ix: Some(3),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        pos: 112,
        delta: 0,
        min_coord: 481,
        max_coord: 504,
        height: 34,
        link_ix: Some(2),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 168,
        delta: 0,
        min_coord: 483,
        max_coord: 504,
        height: 26,
        link_ix: Some(1),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 109,
        delta: 0,
        min_coord: 109,
        max_coord: 366,
        height: 288,
        link_ix: Some(0),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        pos: 453,
        delta: 0,
        min_coord: 169,
        max_coord: 432,
        height: 304,
        link_ix: Some(7),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::ROUND,
        dir: Direction::Up,
        pos: 62,
        delta: 0,
        min_coord: 517,
        max_coord: 566,
        height: 76,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::ROUND,
        dir: Direction::Down,
        pos: 103,
        delta: 0,
        min_coord: 619,
        max_coord: 647,
        height: 41,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 507,
        delta: 0,
        min_coord: 40,
        max_coord: 485,
        height: 498,
        link_ix: Some(4),
        serif_ix: None,
        ..Default::default()
      },
    ];
    assert_eq!(segments, &expected);
  }

  #[test]
  fn vertical_segments() {
    let font = FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS).unwrap();
    let glyphs = font.outline_glyphs();
    let glyph = glyphs
      .get(
        ({
          let Ok(value) = GlyphId::new(8);
          value
        }),
      )
      .unwrap();
    let mut outline = Outline::default();
    outline.fill(&glyph, &[], Default::default()).unwrap();
    let mut axis = Axis::new(Dimension::Vertical, outline.orientation);
    compute_segments(&mut outline, &mut axis, ScriptGroup::Default);
    link_segments(&outline, &mut axis, 0, ScriptGroup::Default, None);
    let segments = retain_segment_test_fields(&axis.segments);
    let expected = [
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        pos: 0,
        delta: 0,
        min_coord: 85,
        max_coord: 470,
        height: 418,
        link_ix: Some(2),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        pos: 504,
        delta: 0,
        min_coord: 112,
        max_coord: 168,
        height: 56,
        link_ix: Some(3),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        pos: 109,
        delta: 0,
        min_coord: 109,
        max_coord: 427,
        height: 327,
        link_ix: Some(0),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        pos: 483,
        delta: 0,
        min_coord: 86,
        max_coord: 400,
        height: 352,
        link_ix: Some(1),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        pos: 647,
        delta: 0,
        min_coord: 76,
        max_coord: 103,
        height: 29,
        link_ix: None,
        serif_ix: Some(1),
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        pos: 592,
        delta: 0,
        min_coord: 131,
        max_coord: 437,
        height: 346,
        link_ix: None,
        serif_ix: Some(1),
        ..Default::default()
      },
    ];
    assert_eq!(segments, &expected);
  }

  #[test]
  fn cjk_horizontal_segments() {
    let font = FontRef::new(font_test_data::NOTOSERIFTC_AUTOHINT_METRICS).unwrap();
    let glyphs = font.outline_glyphs();
    let glyph = glyphs
      .get(
        ({
          let Ok(value) = GlyphId::new(9);
          value
        }),
      )
      .unwrap();
    let mut outline = Outline::default();
    outline.fill(&glyph, &[], Default::default()).unwrap();
    let mut axis = Axis::new(Dimension::Horizontal, outline.orientation);
    compute_segments(&mut outline, &mut axis, ScriptGroup::Cjk);
    link_segments(&outline, &mut axis, 67109, ScriptGroup::Cjk, None);
    let segments = retain_segment_test_fields(&axis.segments);
    let expected = [
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 731,
        delta: 0,
        min_coord: 155,
        max_coord: 676,
        height: 524,
        link_ix: Some(1),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        pos: 670,
        delta: 0,
        min_coord: 133,
        max_coord: 712,
        height: 579,
        link_ix: Some(0),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 458,
        delta: 0,
        min_coord: 741,
        max_coord: 757,
        height: 88,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 911,
        delta: 0,
        min_coord: -9,
        max_coord: 791,
        height: 821,
        link_ix: Some(5),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        pos: 693,
        delta: 0,
        min_coord: -7,
        max_coord: 9,
        height: 18,
        link_ix: None,
        serif_ix: Some(5),
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        pos: 849,
        delta: 0,
        min_coord: 11,
        max_coord: 829,
        height: 823,
        link_ix: Some(3),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 569,
        delta: 0,
        min_coord: 547,
        max_coord: 576,
        height: 29,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        pos: 201,
        delta: 0,
        min_coord: -57,
        max_coord: 540,
        height: 599,
        link_ix: Some(8),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        pos: 138,
        delta: 0,
        min_coord: -78,
        max_coord: 543,
        height: 640,
        link_ix: Some(7),
        serif_ix: None,
        ..Default::default()
      },
    ];
    assert_eq!(segments, &expected);
  }

  #[test]
  fn cjk_vertical_segments() {
    let font = FontRef::new(font_test_data::NOTOSERIFTC_AUTOHINT_METRICS).unwrap();
    let glyphs = font.outline_glyphs();
    let glyph = glyphs
      .get(
        ({
          let Ok(value) = GlyphId::new(9);
          value
        }),
      )
      .unwrap();
    let mut outline = Outline::default();
    outline.fill(&glyph, &[], Default::default()).unwrap();
    let mut axis = Axis::new(Dimension::Vertical, outline.orientation);
    compute_segments(&mut outline, &mut axis, ScriptGroup::Cjk);
    link_segments(&outline, &mut axis, 67109, ScriptGroup::Cjk, None);
    let segments = retain_segment_test_fields(&axis.segments);
    let expected = [
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        pos: 758,
        delta: 0,
        min_coord: 280,
        max_coord: 545,
        height: 288,
        link_ix: Some(1),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        pos: 729,
        delta: 0,
        min_coord: 288,
        max_coord: 674,
        height: 391,
        link_ix: Some(0),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::ROUND,
        dir: Direction::Left,
        pos: 133,
        delta: 0,
        min_coord: 670,
        max_coord: 693,
        height: 34,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        pos: 757,
        delta: 0,
        min_coord: 393,
        max_coord: 458,
        height: 70,
        link_ix: None,
        serif_ix: Some(0),
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::ROUND,
        dir: Direction::Right,
        pos: 3,
        delta: 2,
        min_coord: 727,
        max_coord: 838,
        height: 133,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        pos: 576,
        delta: 0,
        min_coord: 397,
        max_coord: 569,
        height: 177,
        link_ix: Some(7),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        pos: 547,
        delta: 0,
        min_coord: 387,
        max_coord: 569,
        height: 182,
        link_ix: None,
        serif_ix: Some(7),
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        pos: 576,
        delta: 0,
        min_coord: 536,
        max_coord: 546,
        height: 10,
        link_ix: Some(5),
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::ROUND,
        dir: Direction::Left,
        pos: -78,
        delta: 0,
        min_coord: 138,
        max_coord: 161,
        height: 34,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
      Segment {
        flags: TopoFlags::ROUND,
        dir: Direction::Left,
        pos: 788,
        delta: 0,
        min_coord: 262,
        max_coord: 294,
        height: 46,
        link_ix: None,
        serif_ix: None,
        ..Default::default()
      },
    ];
    assert_eq!(segments, &expected);
  }

  fn retain_segment_test_fields(segments: &[Segment]) -> Vec<Segment> {
    segments
      .iter()
      .map(|segment| Segment {
        flags: segment.flags,
        dir: segment.dir,
        pos: segment.pos,
        delta: segment.delta,
        min_coord: segment.min_coord,
        max_coord: segment.max_coord,
        height: segment.height,
        link_ix: segment.link_ix,
        serif_ix: segment.serif_ix,
        ..Default::default()
      })
      .collect()
  }

  /// OSS Fuzz caught subtract with overflow in State::apply_to_segment.
  /// See <https://oss-fuzz.com/testcase-detail/5446493076258816>
  /// and <https://issues.oss-fuzz.com/issues/438909305>
  #[test]
  fn state_apply_overflow() {
    let mut segment = Segment::default();
    let state = State {
      min_coord: MAX_SCORE,
      max_coord: MIN_SCORE,
      ..Default::default()
    };

    state.apply_to_segment(&mut segment, 0);
  }
}
