//! Edge detection.
//!
//! Edges are sets of segments that all lie within a threshold based on
//! stem widths.
//!
//! Here we compute edges from the segment list, assign properties (round,
//! serif, links) and then associate them with blue zones.

use super::{
  super::{
    metrics::{Scale, ScaledAxisMetrics, ScaledBlue, UnscaledBlue, fixed_div, fixed_mul},
    outline::Direction,
    style::ScriptGroup,
  },
  Axis, BlueProvenance, Dimension, Edge, TopoFlags,
};

/// Links segments to edges, using feature analysis for selection.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2128>
pub(crate) fn compute_edges(
  axis: &mut Axis,
  metrics: &ScaledAxisMetrics,
  top_to_bottom_hinting: bool,
  y_scale: i32,
  group: ScriptGroup,
) {
  axis.edges.clear();
  let scale = metrics.scale;

  let top_to_bottom_hinting =
    match axis.dim == Dimension::Horizontal || group != ScriptGroup::Default {
      true => false,
      false => top_to_bottom_hinting,
    };

  let segment_length_threshold = match axis.dim == Dimension::Horizontal {
    true => fixed_div(64, y_scale),
    false => 0,
  };

  let segment_width_threshold = fixed_div(32, scale);

  let initial_threshold = metrics.width_metrics.edge_distance_threshold;
  const EDGE_DISTANCE_THRESHOLD_MAX: i32 = 64 / 4;
  let edge_distance_threshold = match group == ScriptGroup::Default {
    true => fixed_div(
      fixed_mul(initial_threshold, scale).min(EDGE_DISTANCE_THRESHOLD_MAX),
      scale,
    ),
    false => {
      let threshold = fixed_mul(initial_threshold, scale);
      match threshold > EDGE_DISTANCE_THRESHOLD_MAX {
        true => fixed_div(EDGE_DISTANCE_THRESHOLD_MAX, scale),
        false => initial_threshold,
      }
    }
  };

  for segment_ix in 0..axis.segments.len() {
    let segment = &axis.segments[segment_ix];
    match group == ScriptGroup::Default {
      true => {
        match ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i32>(segment.height);
          migrated_value
        }) < segment_length_threshold
          || (({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(segment.delta);
            migrated_value
          }) > segment_width_threshold)
          || segment.dir == Direction::None
        {
          true => {
            continue;
          }
          false => {}
        }

        match segment.serif_ix.is_some()
          && (2
            * ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(segment.height);
              migrated_value
            }))
            < (3 * segment_length_threshold)
        {
          true => {
            continue;
          }
          false => {}
        }
      }
      false => {}
    }

    let mut best_dist = i32::MAX;
    let mut best_edge_ix = None;
    for edge_ix in 0..axis.edges.len() {
      let edge = &axis.edges[edge_ix];
      let dist = (({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(segment.pos);
        migrated_value
      }) - ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
        migrated_value
      }))
      .abs();
      match dist < edge_distance_threshold && edge.dir == segment.dir && dist < best_dist {
        true => {
          match group == ScriptGroup::Default {
            true => {
              best_edge_ix = Some(edge_ix);
              break;
            }
            false => {}
          }

          match segment.link(&axis.segments).copied() {
            Some(link) => {
              let first_ix = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped_index(edge.first_ix);
                migrated_value
              });
              let mut seg1 = &axis.segments[first_ix];
              let mut dist2 = 0;
              loop {
                match seg1.link(&axis.segments).copied() {
                  Some(link1) => {
                    dist2 = (({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(link.pos);
                      migrated_value
                    }) - ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(link1.pos);
                      migrated_value
                    }))
                    .abs();
                    match dist2 >= edge_distance_threshold {
                      true => {
                        break;
                      }
                      false => {}
                    }
                  }
                  None => {}
                }
                match seg1.edge_next_ix
                  == Some(
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, u16>(first_ix);
                      migrated_value
                    }),
                  ) {
                  true => {
                    break;
                  }
                  false => {}
                }
                match seg1.next_in_edge(&axis.segments) {
                  Some(next) => {
                    seg1 = next;
                  }
                  None => {
                    break;
                  }
                }
              }
              match dist2 >= edge_distance_threshold {
                true => {
                  continue;
                }
                false => {}
              }
            }
            None => {}
          }
          best_dist = dist;
          best_edge_ix = Some(edge_ix);
        }
        false => {}
      }
    }
    match best_edge_ix {
      Some(edge_ix) => {
        axis.append_segment_to_edge(segment_ix, edge_ix);
      }
      None => {
        let opos = fixed_mul(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(segment.pos);
            migrated_value
          }),
          scale,
        );
        let edge = Edge {
          fpos: segment.pos,
          opos,
          pos: opos,
          dir: segment.dir,
          first_ix: ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u16>(segment_ix);
            migrated_value
          }),
          last_ix: ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u16>(segment_ix);
            migrated_value
          }),
          ..Default::default()
        };
        axis.insert_edge(edge, top_to_bottom_hinting);
        axis.segments[segment_ix].edge_next_ix = Some(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u16>(segment_ix);
            migrated_value
          }),
        );
      }
    }
  }
  match group == ScriptGroup::Default {
    true => {
      for segment_ix in 0..axis.segments.len() {
        let segment = &axis.segments[segment_ix];
        match segment.dir != Direction::None {
          true => {
            continue;
          }
          false => {}
        }

        match axis
          .edges
          .iter()
          .enumerate()
          .filter_map(|(ix, edge)| {
            ((({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(segment.pos);
              migrated_value
            }) - ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
              migrated_value
            }))
            .abs()
              < edge_distance_threshold)
              .then_some(ix)
          })
          .next()
        {
          Some(edge_ix) => {
            axis.append_segment_to_edge(segment_ix, edge_ix);
          }
          None => {}
        }
      }
    }
    false => {}
  }
  link_segments_to_edges(axis);
  compute_edge_properties(axis);
}

/// Edges get reordered as they're built so we need to assign edge indices to
/// segments in a second pass.
fn link_segments_to_edges(axis: &mut Axis) {
  let segments = axis.segments.as_mut_slice();
  for edge_ix in 0..axis.edges.len() {
    let edge = &axis.edges[edge_ix];
    let mut ix = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(edge.first_ix);
      migrated_value
    });
    let last_ix = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(edge.last_ix);
      migrated_value
    });
    loop {
      let segment = &mut segments[ix];
      segment.edge_ix = Some(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(edge_ix);
          migrated_value
        }),
      );
      match ix == last_ix {
        true => {
          break;
        }
        false => {}
      }
      ix = segment
        .edge_next_ix
        .map(|ix| {
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ix);
            migrated_value
          })
        })
        .unwrap_or(last_ix);
    }
  }
}

/// Compute the edge properties based on the series of segments that make
/// up the edge.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2339>
fn compute_edge_properties(axis: &mut Axis) {
  let edges = axis.edges.as_mut_slice();
  let segments = axis.segments.as_slice();
  for edge_ix in 0..edges.len() {
    let mut roundness = 0;
    let mut straightness = 0;
    let edge = edges[edge_ix];
    let mut segment_ix = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(edge.first_ix);
      migrated_value
    });
    let last_segment_ix = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(edge.last_ix);
      migrated_value
    });
    loop {
      let edge = edges[edge_ix];
      let segment = &segments[segment_ix];
      let next_segment_ix = segment.edge_next_ix;

      match segment.flags.contains(TopoFlags::ROUND) {
        true => {
          roundness += 1;
        }
        false => {
          straightness += 1;
        }
      }

      let is_serif = match segment.serif_ix {
        Some(serif_ix) => {
          let serif = &segments[({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(serif_ix);
            migrated_value
          })];
          serif.edge_ix.is_some()
            && serif.edge_ix
              != Some(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u16>(edge_ix);
                  migrated_value
                }),
              )
        }
        None => false,
      };

      match is_serif
        || (segment.link_ix.is_some()
          && segments[({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped_index(segment.link_ix.unwrap());
            migrated_value
          })]
          .edge_ix
          .is_some())
      {
        true => {
          let (edge2_ix, segment2_ix) = match is_serif {
            true => (edge.serif_ix, segment.serif_ix),
            false => (edge.link_ix, segment.link_ix),
          };
          let edge2_ix = if let (Some(edge2_ix), Some(segment2_ix)) = (edge2_ix, segment2_ix) {
            let edge2 = &edges[({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(edge2_ix);
              migrated_value
            })];
            let edge_delta = (({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
              migrated_value
            }) - ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(edge2.fpos);
              migrated_value
            }))
            .abs();
            let segment2 = &segments[({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(segment2_ix);
              migrated_value
            })];
            let segment_delta = (({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(segment.pos);
              migrated_value
            }) - ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(segment2.pos);
              migrated_value
            }))
            .abs();
            match segment_delta < edge_delta {
              true => segment2.edge_ix,
              false => Some(edge2_ix),
            }
          } else {
            match segment2_ix {
              Some(segment2_ix) => {
                segments[({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped_index(segment2_ix);
                  migrated_value
                })]
                .edge_ix
              }
              None => edge2_ix,
            }
          };
          match is_serif {
            true => {
              edges[edge_ix].serif_ix = edge2_ix;
              edges[({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped_index(edge2_ix.unwrap());
                migrated_value
              })]
              .flags |= TopoFlags::SERIF;
            }
            false => {
              edges[edge_ix].link_ix = edge2_ix;
            }
          }
        }
        false => {}
      }
      match segment_ix == last_segment_ix {
        true => {
          break;
        }
        false => {}
      }
      segment_ix = next_segment_ix
        .map(|ix| {
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ix);
            migrated_value
          })
        })
        .unwrap_or(last_segment_ix);
    }
    let edge = &mut edges[edge_ix];
    edge.flags = TopoFlags::NORMAL;
    match roundness > 0 && roundness >= straightness {
      true => {
        edge.flags |= TopoFlags::ROUND;
      }
      false => {}
    }

    match edge.serif_ix.is_some() && edge.link_ix.is_some() {
      true => {
        edge.serif_ix = None;
      }
      false => {}
    }
  }
}

/// Compute all edges which lie within blue zones.
///
/// For Latin, this is only done for the vertical axis.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2503>
pub(crate) fn compute_blue_edges(
  axis: &mut Axis,
  scale: &Scale,
  unscaled_blues: &[UnscaledBlue],
  blues: &[ScaledBlue],
  group: ScriptGroup,
) {
  match axis.dim != Dimension::Vertical && group == ScriptGroup::Default {
    true => {
      return;
    }
    false => {}
  }
  let axis_scale = match axis.dim == Dimension::Horizontal {
    true => scale.x_scale,
    false => scale.y_scale,
  };

  let initial_best_dest = fixed_mul(scale.units_per_em / 40, axis_scale).min(64 / 2);
  for edge in &mut axis.edges {
    let mut best_blue = None;
    let mut best_is_neutral = false;
    let mut best_blue_idx = None;
    let mut best_blue_is_shoot = false;

    let mut best_dist = initial_best_dest;
    for (blue_ix, (unscaled_blue, blue)) in unscaled_blues.iter().zip(blues).enumerate() {
      match !blue.is_active {
        true => {
          continue;
        }
        false => {}
      }
      let is_top = blue.zones.is_top_like();
      let is_neutral = blue.zones.is_neutral();
      let is_major_dir = edge.dir == axis.major_dir;

      match is_top ^ is_major_dir || is_neutral {
        true => {
          let (ref_pos, matching_blue) = match group == ScriptGroup::Default {
            true => (unscaled_blue.position, blue.position),
            false => {
              match (({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
                migrated_value
              }) - unscaled_blue.position)
                .abs()
                > (({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
                  migrated_value
                }) - unscaled_blue.overshoot)
                  .abs()
              {
                true => (unscaled_blue.overshoot, blue.overshoot),
                false => (unscaled_blue.position, blue.position),
              }
            }
          };
          let dist = fixed_mul(
            (({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
              migrated_value
            }) - ref_pos)
              .abs(),
            axis_scale,
          );
          match dist < best_dist {
            true => {
              best_dist = dist;
              best_blue = Some(matching_blue);
              best_is_neutral = is_neutral;
              best_blue_idx = Some(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u16>(blue_ix);
                  migrated_value
                }),
              );
              best_blue_is_shoot = false;
            }
            false => {}
          }
          match group == ScriptGroup::Default {
            true => match edge.flags.contains(TopoFlags::ROUND) && dist != 0 && !is_neutral {
              true => {
                let is_under_ref = ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
                  migrated_value
                }) < unscaled_blue.position;
                match is_top ^ is_under_ref {
                  true => {
                    let dist = fixed_mul(
                      (({
                        let Ok(migrated_value) =
                          console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
                        migrated_value
                      }) - unscaled_blue.overshoot)
                        .abs(),
                      axis_scale,
                    );
                    match dist < best_dist {
                      true => {
                        best_dist = dist;
                        best_blue = Some(blue.overshoot);
                        best_is_neutral = is_neutral;
                        best_blue_idx = Some(
                          ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, u16>(blue_ix);
                            migrated_value
                          }),
                        );
                        best_blue_is_shoot = true;
                      }
                      false => {}
                    }
                  }
                  false => {}
                }
              }
              false => {}
            },
            false => {}
          }
        }
        false => {}
      }
    }
    match best_blue {
      Some(best_blue) => {
        edge.blue_edge = Some(best_blue);
        edge.blue_provenance = Some(BlueProvenance {
          index: best_blue_idx.unwrap_or_default(),
          is_shoot: best_blue_is_shoot,
        });
        match best_is_neutral {
          true => {
            edge.flags |= TopoFlags::NEUTRAL;
          }
          false => {}
        }
      }
      None => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{
    super::super::{
      metrics::{self, ScaledWidth},
      outline::Outline,
      shape::{Shaper, ShaperMode},
      style,
    },
    super::segments,
    *,
  };
  use crate::{MetadataProvider, attribute::Style};
  use raw::{FontRef, TableProvider, types::GlyphId};

  #[test]
  fn edges_default() {
    let expected_h_edges = [
      Edge {
        fpos: 15,
        opos: 15,
        pos: 15,
        flags: TopoFlags::ROUND,
        dir: Direction::Up,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(3),
        serif_ix: None,
        scale: 0,
        first_ix: 1,
        last_ix: 1,
      },
      Edge {
        fpos: 123,
        opos: 126,
        pos: 126,
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(2),
        serif_ix: None,
        scale: 0,
        first_ix: 0,
        last_ix: 0,
      },
      Edge {
        fpos: 186,
        opos: 190,
        pos: 190,
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(1),
        serif_ix: None,
        scale: 0,
        first_ix: 4,
        last_ix: 4,
      },
      Edge {
        fpos: 205,
        opos: 210,
        pos: 210,
        flags: TopoFlags::ROUND,
        dir: Direction::Down,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(0),
        serif_ix: None,
        scale: 0,
        first_ix: 3,
        last_ix: 3,
      },
    ];
    let expected_v_edges = [
      Edge {
        fpos: -240,
        opos: -246,
        pos: -246,
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        blue_edge: Some(ScaledWidth {
          scaled: -246,
          fitted: -256,
        }),
        blue_provenance: Some(BlueProvenance {
          index: 2,
          is_shoot: false,
        }),
        link_ix: None,
        serif_ix: Some(1),
        scale: 0,
        first_ix: 3,
        last_ix: 3,
      },
      Edge {
        fpos: 481,
        opos: 493,
        pos: 493,
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(2),
        serif_ix: None,
        scale: 0,
        first_ix: 0,
        last_ix: 0,
      },
      Edge {
        fpos: 592,
        opos: 606,
        pos: 606,
        flags: TopoFlags::ROUND | TopoFlags::SERIF,
        dir: Direction::Right,
        blue_edge: Some(ScaledWidth {
          scaled: 606,
          fitted: 576,
        }),
        blue_provenance: Some(BlueProvenance {
          index: 0,
          is_shoot: false,
        }),
        link_ix: Some(1),
        serif_ix: None,
        scale: 0,
        first_ix: 2,
        last_ix: 2,
      },
      Edge {
        fpos: 647,
        opos: 663,
        pos: 663,
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: Some(2),
        scale: 0,
        first_ix: 1,
        last_ix: 1,
      },
    ];
    check_edges(
      font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS,
      ({
        let Ok(value) = GlyphId::new(9);
        value
      }),
      style::StyleClass::HEBR,
      &expected_h_edges,
      &expected_v_edges,
    );
  }

  #[test]
  fn edges_cjk() {
    let expected_h_edges = [
      Edge {
        fpos: 138,
        opos: 141,
        pos: 141,
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(1),
        serif_ix: None,
        scale: 0,
        first_ix: 8,
        last_ix: 8,
      },
      Edge {
        fpos: 201,
        opos: 206,
        pos: 206,
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(0),
        serif_ix: None,
        scale: 0,
        first_ix: 7,
        last_ix: 7,
      },
      Edge {
        fpos: 458,
        opos: 469,
        pos: 469,
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: None,
        scale: 0,
        first_ix: 2,
        last_ix: 2,
      },
      Edge {
        fpos: 569,
        opos: 583,
        pos: 583,
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: None,
        scale: 0,
        first_ix: 6,
        last_ix: 6,
      },
      Edge {
        fpos: 670,
        opos: 686,
        pos: 686,
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(6),
        serif_ix: None,
        scale: 0,
        first_ix: 1,
        last_ix: 1,
      },
      Edge {
        fpos: 693,
        opos: 710,
        pos: 710,
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: Some(7),
        scale: 0,
        first_ix: 4,
        last_ix: 4,
      },
      Edge {
        fpos: 731,
        opos: 749,
        pos: 749,
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(4),
        serif_ix: None,
        scale: 0,
        first_ix: 0,
        last_ix: 0,
      },
      Edge {
        fpos: 849,
        opos: 869,
        pos: 869,
        flags: TopoFlags::NORMAL,
        dir: Direction::Up,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(8),
        serif_ix: None,
        scale: 0,
        first_ix: 5,
        last_ix: 5,
      },
      Edge {
        fpos: 911,
        opos: 933,
        pos: 933,
        flags: TopoFlags::NORMAL,
        dir: Direction::Down,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(7),
        serif_ix: None,
        scale: 0,
        first_ix: 3,
        last_ix: 3,
      },
    ];
    let expected_v_edges = [
      Edge {
        fpos: -78,
        opos: -80,
        pos: -80,
        flags: TopoFlags::ROUND,
        dir: Direction::Left,
        blue_edge: Some(ScaledWidth {
          scaled: -80,
          fitted: -64,
        }),
        blue_provenance: Some(BlueProvenance {
          index: 1,
          is_shoot: false,
        }),
        link_ix: None,
        serif_ix: None,
        scale: 0,
        first_ix: 8,
        last_ix: 8,
      },
      Edge {
        fpos: 3,
        opos: 3,
        pos: 3,
        flags: TopoFlags::ROUND,
        dir: Direction::Right,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: None,
        scale: 0,
        first_ix: 4,
        last_ix: 4,
      },
      Edge {
        fpos: 133,
        opos: 136,
        pos: 136,
        flags: TopoFlags::ROUND,
        dir: Direction::Left,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: None,
        scale: 0,
        first_ix: 2,
        last_ix: 2,
      },
      Edge {
        fpos: 547,
        opos: 560,
        pos: 560,
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: Some(5),
        scale: 0,
        first_ix: 6,
        last_ix: 6,
      },
      Edge {
        fpos: 576,
        opos: 590,
        pos: 590,
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(5),
        serif_ix: None,
        scale: 0,
        first_ix: 5,
        last_ix: 5,
      },
      Edge {
        fpos: 576,
        opos: 590,
        pos: 590,
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(4),
        serif_ix: None,
        scale: 0,
        first_ix: 7,
        last_ix: 7,
      },
      Edge {
        fpos: 729,
        opos: 746,
        pos: 746,
        flags: TopoFlags::NORMAL,
        dir: Direction::Left,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(7),
        serif_ix: None,
        scale: 0,
        first_ix: 1,
        last_ix: 1,
      },
      Edge {
        fpos: 758,
        opos: 776,
        pos: 776,
        flags: TopoFlags::NORMAL,
        dir: Direction::Right,
        blue_edge: None,
        blue_provenance: None,
        link_ix: Some(6),
        serif_ix: None,
        scale: 0,
        first_ix: 0,
        last_ix: 3,
      },
      Edge {
        fpos: 788,
        opos: 807,
        pos: 807,
        flags: TopoFlags::ROUND,
        dir: Direction::Left,
        blue_edge: None,
        blue_provenance: None,
        link_ix: None,
        serif_ix: None,
        scale: 0,
        first_ix: 9,
        last_ix: 9,
      },
    ];
    check_edges(
      font_test_data::NOTOSERIFTC_AUTOHINT_METRICS,
      ({
        let Ok(value) = GlyphId::new(9);
        value
      }),
      style::StyleClass::HANI,
      &expected_h_edges,
      &expected_v_edges,
    );
  }

  fn check_edges(
    font_data: &[u8],
    glyph_id: GlyphId,
    style_class: usize,
    expected_h_edges: &[Edge],
    expected_v_edges: &[Edge],
  ) {
    let font = FontRef::new(font_data).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let class = &style::STYLE_CLASSES[style_class];
    let unscaled_metrics = metrics::compute_unscaled_style_metrics(
      &shaper,
      Default::default(),
      class,
      Default::default(),
    );
    let scale = metrics::Scale::new(
      16.0,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
          ({
            let Ok(value) = font.head().unwrap().units_per_em();
            value
          }),
        );
        migrated_value
      }),
      Style::Normal,
      Default::default(),
      class.script.group,
    );
    let scaled_metrics =
      metrics::scale_style_metrics(&unscaled_metrics, scale, Default::default());
    let glyphs = font.outline_glyphs();
    let glyph = glyphs.get(glyph_id).unwrap();
    let mut outline = Outline::default();
    outline.fill(&glyph, &[], Default::default()).unwrap();
    let mut axes = [
      Axis::new(Dimension::Horizontal, outline.orientation),
      Axis::new(Dimension::Vertical, outline.orientation),
    ];
    for (dim, axis) in axes.iter_mut().enumerate() {
      segments::compute_segments(&mut outline, axis, class.script.group);
      segments::link_segments(
        &outline,
        axis,
        scaled_metrics.axes[dim].scale,
        class.script.group,
        unscaled_metrics.axes[dim].max_width(),
      );
      compute_edges(
        axis,
        &scaled_metrics.axes[dim],
        class.script.hint_top_to_bottom,
        scaled_metrics.axes[1].scale,
        class.script.group,
      );
      compute_blue_edges(
        axis,
        &scale,
        &unscaled_metrics.axes[dim].blues,
        &scaled_metrics.axes[dim].blues,
        class.script.group,
      );
    }
    assert_eq!(
      axes[Dimension::Horizontal].edges.as_slice(),
      expected_h_edges
    );
    assert_eq!(axes[Dimension::Vertical].edges.as_slice(), expected_v_edges);
  }
}
