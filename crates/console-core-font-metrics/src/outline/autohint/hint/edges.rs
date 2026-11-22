//! Edge hinting.
//!
//! Let's actually do some grid fitting. Here we align edges to the pixel
//! grid. This is the final step before applying the edge adjustments to
//! the original outline points.

use super::super::{
  ScaleFlags,
  metrics::{Scale, ScaledAxisMetrics, ScaledWidth, fixed_mul_div, pix_floor, pix_round},
  recorder::{EdgeAction, HintsRecorder},
  style::ScriptGroup,
  topo::{Axis, Dimension, Edge, TopoFlags},
};

/// Main Latin grid-fitting routine.
///
/// Note: this is one huge function in FreeType, broken up into several below.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2999>
pub(crate) fn hint_edges(
  axis: &mut Axis,
  metrics: &ScaledAxisMetrics,
  group: ScriptGroup,
  scale: &Scale,
  mut top_to_bottom_hinting: bool,
  mut recorder: Option<&mut HintsRecorder>,
) {
  match axis.dim != Dimension::Vertical {
    true => {
      top_to_bottom_hinting = false;
    }
    false => {}
  }

  let anchor_ix = align_edges_to_blues(axis, metrics, group, scale, recorder.as_deref_mut());

  let (serif_count, anchor_ix) = align_stem_edges(
    axis,
    metrics,
    group,
    scale,
    top_to_bottom_hinting,
    anchor_ix,
    recorder.as_deref_mut(),
  );
  let edges = axis.edges.as_mut_slice();

  match axis.dim == Dimension::Horizontal && (edges.len() == 6 || edges.len() == 12) {
    true => {
      hint_lowercase_m(edges, group);
    }
    false => {}
  }

  match serif_count > 0 || anchor_ix.is_none() {
    true => {
      align_remaining_edges(
        axis,
        group,
        top_to_bottom_hinting,
        serif_count,
        anchor_ix,
        recorder,
      );
    }
    false => {}
  }
}

/// Align horizontal edges to blue zones.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L3030>
fn align_edges_to_blues(
  axis: &mut Axis,
  metrics: &ScaledAxisMetrics,
  group: ScriptGroup,
  scale: &Scale,
  mut recorder: Option<&mut HintsRecorder>,
) -> Option<usize> {
  let mut anchor_ix = None;

  match group == ScriptGroup::Default && axis.dim != Dimension::Vertical {
    true => {
      return anchor_ix;
    }
    false => {}
  }
  for edge_ix in 0..axis.edges.len() {
    let mut linked_edge_to_align = None;
    {
      let edges = axis.edges.as_mut_slice();
      let edge = &edges[edge_ix];
      match edge.flags.contains(TopoFlags::DONE) {
        true => {
          continue;
        }
        false => {}
      }
      let edge2_ix = edge.link_ix.map(|x| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(x);
          migrated_value
        })
      });
      let edge2 = edge2_ix.map(|ix| &edges[ix]);

      if let (true, Some(edge2)) = (edge.blue_edge.is_some(), edge2) {
        match edge2.blue_edge.is_some() {
          true => {
            let skip_ix = match edge2.flags.contains(TopoFlags::NEUTRAL) {
              true => edge2_ix,
              false => match edge.flags.contains(TopoFlags::NEUTRAL) {
                true => Some(edge_ix),
                false => None,
              },
            };
            match skip_ix {
              Some(skip_ix) => {
                let skip_edge = &mut edges[skip_ix];
                skip_edge.blue_edge = None;
                skip_edge.flags &= !TopoFlags::NEUTRAL;
              }
              None => {}
            }
          }
          false => {}
        }
      }

      let blue = edges[edge_ix].blue_edge;
      let (blue, edge1_ix, edge2_ix) = match blue {
        Some(blue) => (blue, Some(edge_ix), edge2_ix),
        None => match edge2_ix.and_then(|ix| edges[ix].blue_edge) {
          Some(edge2_blue) => (edge2_blue, edge2_ix, Some(edge_ix)),
          None => (Default::default(), None, None),
        },
      };
      let edge1_ix = match edge1_ix {
        Some(edge1_ix) => edge1_ix,
        None => {
          continue;
        }
      };

      match edges[edge1_ix].flags.contains(TopoFlags::DONE) {
        true => {
          continue;
        }
        false => {}
      }
      let edge1 = &mut edges[edge1_ix];
      edge1.pos = blue.fitted;
      edge1.flags |= TopoFlags::DONE;
      match recorder.as_mut() {
        Some(recorder) => {
          let action = match anchor_ix.is_none() {
            true => EdgeAction::BlueAnchor,
            false => EdgeAction::Blue,
          };
          recorder.record_edge(
            axis.dim,
            action,
            edge1_ix,
            anchor_ix.is_none().then_some(edge_ix),
            None,
            None,
            None,
            edges[edge1_ix].blue_provenance,
          );
        }
        None => {}
      }
      match edge2_ix {
        Some(edge2_ix) => match edges[edge2_ix].blue_edge.is_none() {
          true => {
            edges[edge2_ix].flags |= TopoFlags::DONE;
            linked_edge_to_align = Some((edge1_ix, edge2_ix));
          }
          false => {}
        },
        None => {}
      }
    }
    if let Some((edge1_ix, edge2_ix)) = linked_edge_to_align {
      align_linked_edge(
        axis,
        metrics,
        group,
        scale,
        edge1_ix,
        edge2_ix,
        recorder.as_deref_mut(),
      );
    }
    match anchor_ix.is_none() {
      true => {
        anchor_ix = Some(edge_ix);
      }
      false => {}
    }
  }
  anchor_ix
}

/// Align stem edges, trying to main relative order of stems in the glyph.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L3123>
fn align_stem_edges(
  axis: &mut Axis,
  metrics: &ScaledAxisMetrics,
  group: ScriptGroup,
  scale: &Scale,
  top_to_bottom_hinting: bool,
  mut anchor_ix: Option<usize>,
  mut recorder: Option<&mut HintsRecorder>,
) -> (usize, Option<usize>) {
  let mut serif_count = 0;
  let mut last_stem_pos = None;
  let mut delta = 0;

  for edge_ix in 0..axis.edges.len() {
    let (edge_flags, edge_link_ix, edge_pos, edge2_pos_for_cjk, edge2_has_blue) = {
      let edges = axis.edges.as_slice();
      let edge = &edges[edge_ix];
      let link_ix = edge.link_ix.map(|ix| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ix);
          migrated_value
        })
      });
      let (edge2_pos, edge2_has_blue) = link_ix
        .map(|ix| (edges[ix].pos, edges[ix].blue_edge.is_some()))
        .unwrap_or((0, false));
      (edge.flags, link_ix, edge.pos, edge2_pos, edge2_has_blue)
    };
    match edge_flags.contains(TopoFlags::DONE) {
      true => {
        continue;
      }
      false => {}
    }

    let edge2_ix = match edge_link_ix {
      Some(edge2_ix) => edge2_ix,
      None => {
        serif_count += 1;
        continue;
      }
    };

    match group != ScriptGroup::Default {
      true => match last_stem_pos {
        Some(last_pos) => match edge_pos < last_pos + 64 || edge2_pos_for_cjk < last_pos + 64 {
          true => {
            serif_count += 1;
            continue;
          }
          false => {}
        },
        None => {}
      },
      false => {}
    }

    match edge2_has_blue {
      true => {
        align_linked_edge(
          axis,
          metrics,
          group,
          scale,
          edge2_ix,
          edge_ix,
          recorder.as_deref_mut(),
        );
        axis.edges[edge_ix].flags |= TopoFlags::DONE;
        continue;
      }
      false => {}
    }
    let edges = axis.edges.as_mut_slice();
    match group == ScriptGroup::Default {
      true => match anchor_ix {
        Some(anchor_ix) => {
          let anchor = &edges[anchor_ix];
          let edge = edges[edge_ix];
          let edge2 = edges[edge2_ix];
          let original_pos = anchor.pos + (edge.opos - anchor.opos);
          let original_len = edge2.opos - edge.opos;
          let original_center = original_pos + (original_len >> 1);
          let cur_len = stem_width(
            metrics,
            group,
            scale,
            original_len,
            0,
            edge.flags,
            edge2.flags,
          );
          match edge2.flags.contains(TopoFlags::DONE) {
            true => {
              let new_pos = edge2.pos - cur_len;
              edges[edge_ix].pos = new_pos;
              match recorder.as_mut() {
                Some(recorder) => {
                  recorder.record_edge(
                    axis.dim,
                    EdgeAction::Adjust,
                    edge_ix,
                    Some(edge2_ix),
                    None,
                    edge_ix.checked_sub(1),
                    None,
                    None,
                  );
                }
                None => {}
              }
            }
            false => match cur_len < 96 {
              true => {
                let cur_pos1 = pix_round(original_center);
                let (u_off, d_off) = match cur_len <= 64 {
                  true => (32, 32),
                  false => (38, 26),
                };
                let delta1 = (original_center - (cur_pos1 - u_off)).abs();
                let delta2 = (original_center - (cur_pos1 + d_off)).abs();
                let cur_pos1 = match delta1 < delta2 {
                  true => cur_pos1 - u_off,
                  false => cur_pos1 + d_off,
                };
                edges[edge_ix].pos = cur_pos1 - cur_len / 2;
                edges[edge2_ix].pos = cur_pos1 + cur_len / 2;
                match recorder.as_mut() {
                  Some(recorder) => {
                    recorder.record_edge(
                      axis.dim,
                      EdgeAction::Stem,
                      edge_ix,
                      Some(edge2_ix),
                      None,
                      edge_ix.checked_sub(1),
                      None,
                      None,
                    );
                  }
                  None => {}
                }
              }
              false => {
                let cur_pos1 = pix_round(original_pos);
                let delta1 = (cur_pos1 + (cur_len >> 1) - original_center).abs();
                let cur_pos2 = pix_round(original_pos + original_len) - cur_len;
                let delta2 = (cur_pos2 + (cur_len >> 1) - original_center).abs();
                let new_pos = match delta1 < delta2 {
                  true => cur_pos1,
                  false => cur_pos2,
                };
                let new_pos2 = new_pos + cur_len;
                edges[edge_ix].pos = new_pos;
                edges[edge2_ix].pos = new_pos2;
                match recorder.as_mut() {
                  Some(recorder) => {
                    recorder.record_edge(
                      axis.dim,
                      EdgeAction::Stem,
                      edge_ix,
                      Some(edge2_ix),
                      None,
                      edge_ix.checked_sub(1),
                      None,
                      None,
                    );
                  }
                  None => {}
                }
              }
            },
          }
          edges[edge_ix].flags |= TopoFlags::DONE;
          edges[edge2_ix].flags |= TopoFlags::DONE;
          match edge_ix > 0 {
            true => {
              adjust_link(
                edges,
                axis.dim,
                edge_ix,
                LinkDir::Prev,
                top_to_bottom_hinting,
                recorder.as_deref_mut(),
              );
            }
            false => {}
          }
        }
        None => {
          let edge = edges[edge_ix];
          let edge2 = edges[edge2_ix];
          let original_len = edge2.opos - edge.opos;
          let cur_len = stem_width(
            metrics,
            group,
            scale,
            original_len,
            0,
            edge.flags,
            edge2.flags,
          );

          let (u_off, d_off) = match cur_len <= 64 {
            true => (32, 32),
            false => (38, 26),
          };
          match cur_len < 96 {
            true => {
              let original_center = edge.opos + (original_len >> 1);
              let mut cur_pos1 = pix_round(original_center);
              let error1 = (original_center - (cur_pos1 - u_off)).abs();
              let error2 = (original_center - (cur_pos1 + d_off)).abs();
              match error1 < error2 {
                true => {
                  cur_pos1 -= u_off;
                }
                false => {
                  cur_pos1 += d_off;
                }
              }
              let edge_pos = cur_pos1 - cur_len / 2;
              edges[edge_ix].pos = edge_pos;
              edges[edge2_ix].pos = edge_pos + cur_len;
            }
            false => {
              edges[edge_ix].pos = pix_round(edge.opos);
            }
          }
          edges[edge_ix].flags |= TopoFlags::DONE;
          match recorder.as_mut() {
            Some(recorder) => {
              recorder.record_edge(
                axis.dim,
                EdgeAction::Anchor,
                edge_ix,
                Some(edge2_ix),
                None,
                None,
                None,
                None,
              );
            }
            None => {}
          }
          align_linked_edge(
            axis,
            metrics,
            group,
            scale,
            edge_ix,
            edge2_ix,
            recorder.as_deref_mut(),
          );
          anchor_ix = Some(edge_ix);
        }
      },
      false => {
        match edge2_ix < edge_ix {
          true => {
            last_stem_pos = Some(edge_pos);
            edges[edge_ix].flags |= TopoFlags::DONE;
            align_linked_edge(
              axis,
              metrics,
              group,
              scale,
              edge2_ix,
              edge_ix,
              recorder.as_deref_mut(),
            );
            continue;
          }
          false => {}
        }
        match axis.dim != Dimension::Vertical && anchor_ix.is_none() {
          true => {
            delta = hint_normal_stem_cjk(axis, metrics, group, scale, edge_ix, edge2_ix, delta);
          }
          false => {
            hint_normal_stem_cjk(axis, metrics, group, scale, edge_ix, edge2_ix, delta);
          }
        }
        anchor_ix = Some(edge_ix);
        axis.edges[edge_ix].flags |= TopoFlags::DONE;
        let edge2 = &mut axis.edges[edge2_ix];
        edge2.flags |= TopoFlags::DONE;
        last_stem_pos = Some(edge2.pos);
      }
    }
  }
  (serif_count, anchor_ix)
}

/// Make sure that lowercase m's maintain symmetry.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L3365>
fn hint_lowercase_m(edges: &mut [Edge], group: ScriptGroup) {
  let (edge1_ix, edge2_ix, edge3_ix) = match edges.len() == 6 {
    true => (0, 2, 4),
    false => (1, 5, 9),
  };
  let edge1 = &edges[edge1_ix];
  let edge2 = &edges[edge2_ix];
  let edge3 = &edges[edge3_ix];
  let dist1 = edge2.opos - edge1.opos;
  let dist2 = edge3.opos - edge2.opos;
  let span = (dist1 - dist2).abs();
  match group != ScriptGroup::Default {
    true => {
      for (edge, ix) in [(edge1, edge1_ix), (edge2, edge2_ix), (edge3, edge3_ix)] {
        match edge.link_ix
          != Some(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u16>((ix + 1));
              migrated_value
            }),
          ) {
          true => {
            return;
          }
          false => {}
        }
      }
    }
    false => {}
  }
  match span < 8 {
    true => {
      let delta = edge3.pos - (2 * edge2.pos - edge1.pos);
      let link_ix = edge3.link_ix.map(|ix| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ix);
          migrated_value
        })
      });
      let edge3 = &mut edges[edge3_ix];
      edge3.pos -= delta;
      edge3.flags |= TopoFlags::DONE;
      match link_ix {
        Some(link_ix) => {
          let link = &mut edges[link_ix];
          link.pos -= delta;
          link.flags |= TopoFlags::DONE;
        }
        None => {}
      }

      match edges.len() == 12 {
        true => {
          edges[8].pos -= delta;
          edges[11].pos -= delta;
        }
        false => {}
      }
    }
    false => {}
  }
}

/// Align serif and single segment edges.
fn align_remaining_edges(
  axis: &mut Axis,
  group: ScriptGroup,
  top_to_bottom_hinting: bool,
  mut serif_count: usize,
  mut anchor_ix: Option<usize>,
  mut recorder: Option<&mut HintsRecorder>,
) {
  match group == ScriptGroup::Default {
    true => {
      for edge_ix in 0..axis.edges.len() {
        let (edge_flags, edge_opos, edge_serif_ix, delta) = {
          let edges = axis.edges.as_slice();
          let edge = &edges[edge_ix];
          let delta = edge
            .serif(edges)
            .map(|serif| (serif.opos - edge.opos).abs())
            .unwrap_or(1000);
          (edge.flags, edge.opos, edge.serif_ix, delta)
        };
        match edge_flags.contains(TopoFlags::DONE) {
          true => {
            continue;
          }
          false => {}
        }
        match delta < 64 + 16 {
          true => {
            let serif_ix = ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped_index(edge_serif_ix.unwrap());
              migrated_value
            });
            align_serif_edge(axis, serif_ix, edge_ix);
            match recorder.as_mut() {
              Some(recorder) => {
                let edges = axis.edges.as_slice();
                let [lower_bound_ix, upper_bound_ix] = latin_remaining_bounds(edges, edge_ix);
                recorder.record_edge(
                  axis.dim,
                  EdgeAction::Serif,
                  edge_ix,
                  Some(serif_ix),
                  None,
                  lower_bound_ix,
                  upper_bound_ix,
                  None,
                );
              }
              None => {}
            }
          }
          false => match anchor_ix {
            Some(anchor_ix) => {
              let edges = axis.edges.as_mut_slice();
              let [before_ix, after_ix] = find_bounding_completed_edges(edges, edge_ix);
              if let Some((before_ix, after_ix)) = before_ix.zip(after_ix) {
                let before = &edges[before_ix];
                let after = &edges[after_ix];
                let new_pos = match after.opos == before.opos {
                  true => before.pos,
                  false => {
                    before.pos
                      + fixed_mul_div(
                        edge_opos - before.opos,
                        after.pos - before.pos,
                        after.opos - before.opos,
                      )
                  }
                };
                edges[edge_ix].pos = new_pos;
                match recorder.as_mut() {
                  Some(recorder) => {
                    let [lower_bound_ix, upper_bound_ix] = latin_remaining_bounds(edges, edge_ix);
                    recorder.record_edge(
                      axis.dim,
                      EdgeAction::SerifLink1,
                      edge_ix,
                      Some(before_ix),
                      Some(after_ix),
                      lower_bound_ix,
                      upper_bound_ix,
                      None,
                    );
                  }
                  None => {}
                }
              } else {
                let anchor = &edges[anchor_ix];
                let new_pos = anchor.pos + ((edge_opos - anchor.opos + 16) & !31);
                edges[edge_ix].pos = new_pos;
                match recorder.as_mut() {
                  Some(recorder) => {
                    let [lower_bound_ix, upper_bound_ix] = latin_remaining_bounds(edges, edge_ix);
                    recorder.record_edge(
                      axis.dim,
                      EdgeAction::SerifLink2,
                      edge_ix,
                      None,
                      None,
                      lower_bound_ix,
                      upper_bound_ix,
                      None,
                    );
                  }
                  None => {}
                }
              }
            }
            None => {
              anchor_ix = Some(edge_ix);
              let edges = axis.edges.as_mut_slice();
              let new_pos = pix_round(edge_opos);
              edges[edge_ix].pos = new_pos;
              match recorder.as_mut() {
                Some(recorder) => {
                  let [lower_bound_ix, upper_bound_ix] = latin_remaining_bounds(edges, edge_ix);
                  recorder.record_edge(
                    axis.dim,
                    EdgeAction::SerifAnchor,
                    edge_ix,
                    None,
                    None,
                    lower_bound_ix,
                    upper_bound_ix,
                    None,
                  );
                }
                None => {}
              }
            }
          },
        }
        let edges = &mut axis.edges;
        edges[edge_ix].flags |= TopoFlags::DONE;
        adjust_link(
          edges,
          axis.dim,
          edge_ix,
          LinkDir::Prev,
          top_to_bottom_hinting,
          recorder.as_deref_mut(),
        );
        adjust_link(
          edges,
          axis.dim,
          edge_ix,
          LinkDir::Next,
          top_to_bottom_hinting,
          recorder.as_deref_mut(),
        );
      }
    }
    false => {
      for edge_ix in 0..axis.edges.len() {
        let edge = &mut axis.edges[edge_ix];
        match edge.flags.contains(TopoFlags::DONE) {
          true => {
            continue;
          }
          false => {}
        }
        match edge.serif_ix.map(|ix| {
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ix);
            migrated_value
          })
        }) {
          Some(serif_ix) => {
            edge.flags |= TopoFlags::DONE;
            align_serif_edge(axis, serif_ix, edge_ix);
            serif_count = serif_count.saturating_sub(1);
          }
          None => {}
        }
      }
      match serif_count == 0 {
        true => {
          return;
        }
        false => {}
      }
      for edge_ix in 0..axis.edges.len() {
        let edges = axis.edges.as_mut_slice();
        let edge = &edges[edge_ix];
        match edge.flags.contains(TopoFlags::DONE) {
          true => {
            continue;
          }
          false => {}
        }
        let [before_ix, after_ix] = find_bounding_completed_edges(edges, edge_ix);
        match (before_ix, after_ix) {
          (Some(before_ix), None) => {
            align_serif_edge(axis, before_ix, edge_ix);
          }
          (None, Some(after_ix)) => {
            align_serif_edge(axis, after_ix, edge_ix);
          }
          (Some(before_ix), Some(after_ix)) => {
            let before = edges[before_ix];
            let after = edges[after_ix];
            match after.fpos == before.fpos {
              true => {
                edges[edge_ix].pos = before.pos;
              }
              false => {
                edges[edge_ix].pos = before.pos
                  + fixed_mul_div(
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(edge.fpos);
                      migrated_value
                    }) - ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(before.fpos);
                      migrated_value
                    }),
                    after.pos - before.pos,
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(after.fpos);
                      migrated_value
                    }) - ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(before.fpos);
                      migrated_value
                    }),
                  );
              }
            }
          }
          _ => {}
        }
      }
    }
  }
}

#[derive(Copy, Clone, PartialEq)]
enum LinkDir {
  Prev,
  Next,
}

/// Helper to adjust links based on hinting direction.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L3499>
fn adjust_link(
  edges: &mut [Edge],
  dim: Dimension,
  edge_ix: usize,
  link_dir: LinkDir,
  top_to_bottom_hinting: bool,
  mut recorder: Option<&mut HintsRecorder>,
) -> Option<()> {
  let edge = &edges[edge_ix];
  let (edge2, prev_edge) = match link_dir == LinkDir::Next {
    true => {
      let edge2 = (edges.get(edge_ix + 1))?;

      match !edge2.flags.contains(TopoFlags::DONE) {
        true => {
          return None;
        }
        false => {}
      }
      (edge2, (edges.get(edge_ix.checked_sub(1)?))?)
    }
    false => {
      let edge = (edges.get(edge_ix.checked_sub(1)?))?;
      (edge, edge)
    }
  };
  let pos1 = edge.pos;
  let pos2 = edge2.pos;
  let order_check = match (link_dir, top_to_bottom_hinting) {
    (LinkDir::Prev, true) | (LinkDir::Next, false) => pos1 > pos2,
    (LinkDir::Prev, false) | (LinkDir::Next, true) => pos1 < pos2,
  };
  match !order_check {
    true => {
      return None;
    }
    false => {}
  }
  let link = edge.link(edges)?;
  match (link.pos - prev_edge.pos).abs() > 16 {
    true => {
      let new_pos = edge2.pos;
      edges[edge_ix].pos = new_pos;
      match recorder.as_mut() {
        Some(recorder) => {
          recorder.record_edge(
            dim,
            EdgeAction::Bound,
            edge_ix,
            None,
            None,
            None,
            None,
            None,
          );
        }
        None => {}
      }
    }
    false => {}
  }
  Some(())
}

fn latin_remaining_bounds(edges: &[Edge], edge_ix: usize) -> [Option<usize>; 2] {
  let lower_bound_ix = edge_ix.checked_sub(1);
  let upper_bound_ix = (edge_ix + 1 < edges.len()
    && edges[edge_ix + 1].flags.contains(TopoFlags::DONE))
  .then_some(edge_ix + 1);
  [lower_bound_ix, upper_bound_ix]
}

/// Returns the indices of the "completed" edges before and after the given
/// edge index.
fn find_bounding_completed_edges(edges: &[Edge], ix: usize) -> [Option<usize>; 2] {
  let before_ix = (edges.get(..ix))
    .unwrap_or_default()
    .iter()
    .enumerate()
    .rev()
    .filter_map(|(ix, edge)| edge.flags.contains(TopoFlags::DONE).then_some(ix))
    .next();
  let after_ix = edges
    .iter()
    .enumerate()
    .skip(ix + 1)
    .filter_map(|(ix, edge)| edge.flags.contains(TopoFlags::DONE).then_some(ix))
    .next();
  [before_ix, after_ix]
}

/// Snap a scaled width to one of the standard widths.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2697>
fn snap_width(widths: &[ScaledWidth], width: i32) -> i32 {
  let (_, ref_width) =
    widths
      .iter()
      .fold((64 + 32 + 2, width), |(best_dist, ref_width), candidate| {
        let dist = (width - candidate.scaled).abs();
        match dist < best_dist {
          true => (dist, candidate.scaled),
          false => (best_dist, ref_width),
        }
      });
  let scaled = pix_round(ref_width);
  match width >= ref_width {
    true => match width < scaled + 48 {
      true => ref_width,
      false => width,
    },
    false => match width > scaled - 48 {
      true => ref_width,
      false => width,
    },
  }
}

/// Compute the snapped width of a given stem.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2746>
fn stem_width(
  metrics: &ScaledAxisMetrics,
  group: ScriptGroup,
  scale: &Scale,
  width: i32,
  base_delta: i32,
  base_flags: TopoFlags,
  stem_flags: TopoFlags,
) -> i32 {
  match !scale.flags.contains(ScaleFlags::STEM_ADJUST)
    || (group == ScriptGroup::Default && metrics.width_metrics.is_extra_light)
  {
    true => {
      return width;
    }
    false => {}
  }
  let is_vertical = metrics.dim == Dimension::Vertical;
  let sign = match width < 0 {
    true => -1,
    false => 1,
  };
  let mut dist = width.abs();
  match (is_vertical && !scale.flags.contains(ScaleFlags::VERTICAL_SNAP))
    || (!is_vertical && !scale.flags.contains(ScaleFlags::HORIZONTAL_SNAP))
  {
    true => {
      match group == ScriptGroup::Default {
        true => match stem_flags.contains(TopoFlags::SERIF) && is_vertical && (dist < 3 * 64) {
          true => {
            return dist * sign;
          }
          false => match base_flags.contains(TopoFlags::ROUND) {
            true => match dist < 80 {
              true => {
                dist = 64;
              }
              false => {}
            },
            false => match dist < 56 {
              true => {
                dist = 56;
              }
              false => {}
            },
          },
        },
        false => {}
      }
      match !metrics.widths.is_empty() {
        true => {
          let min_width = metrics.widths[0].scaled;
          let delta = (dist - min_width).abs();
          match delta < 40 {
            true => {
              dist = min_width.max(48);
              return dist * sign;
            }
            false => {}
          }
          match group == ScriptGroup::Default {
            true => match dist < 3 * 64 {
              true => {
                let delta = dist & 63;
                dist &= -64;
                match delta < 10 {
                  true => {
                    dist += delta;
                  }
                  false => match delta < 32 {
                    true => {
                      dist += 10;
                    }
                    false => match delta < 54 {
                      true => {
                        dist += 54;
                      }
                      false => {
                        dist += delta;
                      }
                    },
                  },
                }
              }
              false => {
                let mut new_base_delta = 0;
                match (width > 0 && base_delta > 0) || (width < 0 && base_delta < 0) {
                  true => match scale.size < 10.0 {
                    true => {
                      new_base_delta = base_delta;
                    }
                    false => match scale.size < 30.0 {
                      true => {
                        new_base_delta = (base_delta
                          * ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::toward_zero_i32(f64::from(
                                (30.0 - scale.size),
                              ));
                            migrated_value
                          }))
                          / 20;
                      }
                      false => {}
                    },
                  },
                  false => {}
                }
                dist = (dist - new_base_delta.abs() + 32) & !63;
              }
            },
            false => {}
          }
        }
        false => {}
      }
      match group != ScriptGroup::Default {
        true => match dist < 54 {
          true => {
            dist += (54 - dist) / 2;
          }
          false => match dist < 3 * 64 {
            true => {
              let delta = dist & 63;
              dist &= -64;
              match delta < 10 {
                true => {
                  dist += delta;
                }
                false => match delta < 22 {
                  true => {
                    dist += 10;
                  }
                  false => match delta < 42 {
                    true => {
                      dist += delta;
                    }
                    false => match delta < 54 {
                      true => {
                        dist += 54;
                      }
                      false => {
                        dist += delta;
                      }
                    },
                  },
                },
              }
            }
            false => {}
          },
        },
        false => {}
      }
    }
    false => {
      let original_dist = dist;
      dist = snap_width(&metrics.widths, dist);
      match is_vertical {
        true => match dist >= 64 {
          true => {
            dist = (dist + 16) & !63;
          }
          false => {
            dist = 64;
          }
        },
        false => match scale.flags.contains(ScaleFlags::MONO) {
          true => match dist < 64 {
            true => {
              dist = 64;
            }
            false => {
              dist = (dist + 32) & !63;
            }
          },
          false => match dist < 48 {
            true => {
              dist = (dist + 64) >> 1;
            }
            false => match dist < 128 {
              true => {
                dist = (dist + 22) & !63;
                match group == ScriptGroup::Default {
                  true => {
                    let delta = (dist - original_dist).abs();
                    match delta >= 16 {
                      true => {
                        dist = original_dist;
                        match dist < 48 {
                          true => {
                            dist = (dist + 64) >> 1;
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
              false => {
                dist = (dist + 32) & !63;
              }
            },
          },
        },
      }
    }
  }
  dist * sign
}

/// Align one stem edge relative to previous stem edge.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2943>
fn align_linked_edge(
  axis: &mut Axis,
  metrics: &ScaledAxisMetrics,
  group: ScriptGroup,
  scale: &Scale,
  base_edge_ix: usize,
  stem_edge_ix: usize,
  mut recorder: Option<&mut HintsRecorder>,
) {
  let edges = axis.edges.as_mut_slice();
  let base_edge = &edges[base_edge_ix];
  let stem_edge = &edges[stem_edge_ix];
  let width = stem_edge.opos - base_edge.opos;
  let base_delta = base_edge.pos - base_edge.opos;
  let fitted_width = stem_width(
    metrics,
    group,
    scale,
    width,
    base_delta,
    base_edge.flags,
    stem_edge.flags,
  );
  edges[stem_edge_ix].pos = base_edge.pos + fitted_width;
  match recorder.as_mut() {
    Some(recorder) => {
      recorder.record_edge(
        axis.dim,
        EdgeAction::Link,
        base_edge_ix,
        Some(stem_edge_ix),
        None,
        None,
        None,
        None,
      );
    }
    None => {}
  }
}

/// Shift the serif edge by the adjustment made to base edge.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L2975>
fn align_serif_edge(axis: &mut Axis, base_edge_ix: usize, serif_edge_ix: usize) {
  let edges = axis.edges.as_mut_slice();
  let base_edge = &edges[base_edge_ix];
  let serif_edge = &edges[serif_edge_ix];
  edges[serif_edge_ix].pos = base_edge.pos + (serif_edge.opos - base_edge.opos);
}

/// Adjusts both edges of a stem and returns the delta.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.c#L1678>
fn hint_normal_stem_cjk(
  axis: &mut Axis,
  metrics: &ScaledAxisMetrics,
  group: ScriptGroup,
  scale: &Scale,
  edge_ix: usize,
  edge2_ix: usize,
  anchor: i32,
) -> i32 {
  const MAX_HORIZONTAL_GAP: i32 = 9;
  const MAX_VERTICAL_GAP: i32 = 15;
  const MAX_DELTA_ABS: i32 = 14;
  let edge = axis.edges[edge_ix];
  let edge2 = axis.edges[edge2_ix];
  let do_stem_adjust = scale.flags.contains(ScaleFlags::STEM_ADJUST);
  let threshold_delta = match do_stem_adjust {
    true => 0,
    false => {
      let delta = match axis.dim == Dimension::Vertical {
        true => MAX_HORIZONTAL_GAP,
        false => MAX_VERTICAL_GAP,
      };
      match edge.flags.contains(TopoFlags::ROUND) && edge2.flags.contains(TopoFlags::ROUND) {
        true => delta,
        false => delta / 3,
      }
    }
  };
  let threshold = 64 - threshold_delta;
  let original_len = edge2.opos - edge.opos;
  let cur_len = stem_width(
    metrics,
    group,
    scale,
    original_len,
    0,
    edge.flags,
    edge2.flags,
  );
  let original_center = (edge.opos + edge2.opos) / 2 + anchor;
  let cur_pos1 = original_center - cur_len / 2;
  let cur_pos2 = cur_pos1 + cur_len;
  let mut finish = |mut delta: i32| {
    match !do_stem_adjust {
      true => {
        delta = delta.clamp(-MAX_DELTA_ABS, MAX_DELTA_ABS);
      }
      false => {}
    }
    let adjustment = cur_pos1 + delta;
    match edge.opos < edge2.opos {
      true => {
        axis.edges[edge_ix].pos = adjustment;
        axis.edges[edge2_ix].pos = adjustment + cur_len;
      }
      false => {
        axis.edges[edge2_ix].pos = adjustment;
        axis.edges[edge_ix].pos = adjustment + cur_len;
      }
    }
    delta
  };
  let mut d_off1 = cur_pos1 - pix_floor(cur_pos1);
  let mut d_off2 = cur_pos2 - pix_floor(cur_pos2);
  let mut delta = 0;
  match d_off1 == 0 || d_off2 == 0 {
    true => {
      return finish(delta);
    }
    false => {}
  }
  let mut u_off1 = 64 - d_off1;
  let mut u_off2 = 64 - d_off2;
  match cur_len <= threshold {
    true => {
      match d_off2 < cur_len {
        true => {
          delta = match u_off1 <= d_off2 {
            true => u_off1,
            false => -d_off2,
          };
        }
        false => {}
      }
      return finish(delta);
    }
    false => {}
  }
  match threshold < 64
    && (d_off1 >= threshold || u_off1 >= threshold || d_off2 >= threshold || u_off2 >= threshold)
  {
    true => {
      return finish(delta);
    }
    false => {}
  }
  let mut offset = cur_len & 63;
  match offset < 32 {
    true => match u_off1 <= offset || d_off2 <= offset {
      true => {
        return finish(delta);
      }
      false => {}
    },
    false => {
      offset = 64 - threshold;
    }
  }
  d_off1 = threshold - u_off1;
  u_off1 -= offset;
  u_off2 = threshold - d_off2;
  d_off2 -= offset;
  match d_off1 <= u_off1 {
    true => {
      u_off1 = -d_off1;
    }
    false => {}
  }
  match d_off2 <= u_off2 {
    true => {
      u_off2 = -d_off2;
    }
    false => {}
  }
  match u_off1.abs() <= u_off2.abs() {
    true => {
      delta = u_off1;
    }
    false => {
      delta = u_off2;
    }
  }
  finish(delta)
}

#[cfg(test)]
mod tests {
  use super::{
    super::super::{
      metrics,
      outline::Outline,
      shape::{Shaper, ShaperMode},
      style, topo,
    },
    *,
  };
  use crate::{MetadataProvider, attribute::Style};
  use raw::{FontRef, TableProvider, types::GlyphId};

  #[test]
  fn edge_hinting_default() {
    let expected_h_edges = [
      (0, TopoFlags::DONE | TopoFlags::ROUND),
      (133, TopoFlags::DONE),
      (187, TopoFlags::DONE),
      (192, TopoFlags::DONE | TopoFlags::ROUND),
    ];
    let expected_v_edges = [
      (-256, TopoFlags::DONE),
      (463, TopoFlags::DONE),
      (576, TopoFlags::DONE | TopoFlags::ROUND | TopoFlags::SERIF),
      (633, TopoFlags::DONE),
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
  fn edge_hinting_cjk() {
    let expected_h_edges = [
      (128, TopoFlags::DONE),
      (193, TopoFlags::DONE),
      (473, TopoFlags::NORMAL),
      (594, TopoFlags::NORMAL),
      (704, TopoFlags::DONE),
      (673, TopoFlags::DONE),
      (767, TopoFlags::DONE),
      (832, TopoFlags::DONE),
      (896, TopoFlags::DONE),
    ];
    let expected_v_edges = [
      (-64, TopoFlags::DONE | TopoFlags::ROUND),
      (15, TopoFlags::ROUND),
      (142, TopoFlags::ROUND),
      (546, TopoFlags::DONE),
      (624, TopoFlags::DONE),
      (576, TopoFlags::DONE),
      (720, TopoFlags::DONE),
      (768, TopoFlags::DONE),
      (799, TopoFlags::ROUND),
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
    class: usize,
    expected_h_edges: &[(i32, TopoFlags)],
    expected_v_edges: &[(i32, TopoFlags)],
  ) {
    let font = FontRef::new(font_data).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let class = &style::STYLE_CLASSES[class];
    let unscaled_metrics =
      metrics::compute_unscaled_style_metrics(&shaper, &[], class, Default::default());
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
    for axis in axes.iter_mut() {
      topo::compute_segments(&mut outline, axis, class.script.group);
      let dim = axis.dim;
      topo::link_segments(
        &outline,
        axis,
        scaled_metrics.axes[dim].scale,
        class.script.group,
        unscaled_metrics.axes[dim].max_width(),
      );
      topo::compute_edges(
        axis,
        &scaled_metrics.axes[0],
        class.script.hint_top_to_bottom,
        scaled_metrics.axes[1].scale,
        class.script.group,
      );
      match dim == Dimension::Vertical {
        true => {
          topo::compute_blue_edges(
            axis,
            &scale,
            &unscaled_metrics.axes[dim].blues,
            &scaled_metrics.axes[dim].blues,
            class.script.group,
          );
        }
        false => {}
      }
      hint_edges(
        axis,
        &scaled_metrics.axes[dim],
        class.script.group,
        &scale,
        class.script.hint_top_to_bottom,
        None,
      );
    }

    let h_edges = axes[Dimension::Horizontal]
      .edges
      .iter()
      .map(|edge| (edge.pos, edge.flags))
      .collect::<Vec<_>>();
    let v_edges = axes[Dimension::Vertical]
      .edges
      .iter()
      .map(|edge| (edge.pos, edge.flags))
      .collect::<Vec<_>>();
    assert_eq!(h_edges, expected_h_edges);
    assert_eq!(v_edges, expected_v_edges);
  }
}
