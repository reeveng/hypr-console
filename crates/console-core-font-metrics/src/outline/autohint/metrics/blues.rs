//! Latin blue values.

use super::{
  super::{
    super::{OutlineGlyphCollection, unscaled::UnscaledOutlineBuf},
    shape::{ShapedCluster, Shaper},
    style::{ScriptGroup, StyleClass},
  },
  ScaledWidth,
};
use crate::{FontRef, MetadataProvider};
use alloc::vec::Vec;
use raw::TableProvider;
use raw::types::F2Dot14;

/// Maximum number of blue values.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afblue.h#L328>
const MAX_BLUES: usize = 8;

const BLUE_STRING_MAX_LEN: usize = 51;

/// Defines the zone(s) that are associated with a blue value.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(transparent)]
pub struct BlueZones(u16);

impl BlueZones {
  pub const NONE: Self = Self(0);
  pub const TOP: Self = Self(1 << 1);
  pub const SUB_TOP: Self = Self(1 << 2);
  pub const NEUTRAL: Self = Self(1 << 3);
  pub const ADJUSTMENT: Self = Self(1 << 4);
  pub const X_HEIGHT: Self = Self(1 << 5);
  pub const LONG: Self = Self(1 << 6);
  pub const HORIZONTAL: Self = Self(1 << 2);
  pub const RIGHT: Self = Self::TOP;

  pub const fn contains(self, other: Self) -> bool {
    self.0 & other.0 == other.0
  }

  #[must_use]
  pub(crate) const fn union(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }

  pub(crate) fn is_top_like(self) -> bool {
    self & (Self::TOP | Self::SUB_TOP) != Self::NONE
  }

  pub fn is_top(self) -> bool {
    self.contains(Self::TOP)
  }

  pub fn is_sub_top(self) -> bool {
    self.contains(Self::SUB_TOP)
  }

  pub fn is_neutral(self) -> bool {
    self.contains(Self::NEUTRAL)
  }

  pub fn is_x_height(self) -> bool {
    self.contains(Self::X_HEIGHT)
  }

  pub(crate) fn is_long(self) -> bool {
    self.contains(Self::LONG)
  }

  pub(crate) fn is_horizontal(self) -> bool {
    self.contains(Self::HORIZONTAL)
  }

  pub(crate) fn is_right(self) -> bool {
    self.contains(Self::RIGHT)
  }

  #[must_use]
  pub fn retain_top_like_or_neutral(self) -> Self {
    self & (Self::TOP | Self::SUB_TOP | Self::NEUTRAL)
  }
}

impl core::ops::Not for BlueZones {
  type Output = Self;

  fn not(self) -> Self::Output {
    Self(!self.0)
  }
}

impl core::ops::BitOr for BlueZones {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    Self(self.0 | rhs.0)
  }
}

impl core::ops::BitOrAssign for BlueZones {
  fn bitor_assign(&mut self, rhs: Self) {
    self.0 |= rhs.0;
  }
}

impl core::ops::BitAnd for BlueZones {
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    Self(self.0 & rhs.0)
  }
}

impl core::ops::BitAndAssign for BlueZones {
  fn bitand_assign(&mut self, rhs: Self) {
    self.0 &= rhs.0;
  }
}

/// An unscaled alignment zone.

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct UnscaledBlue {
  /// Position of the blue.
  pub position: i32,
  /// Overshoot value of the blue.
  pub overshoot: i32,
  /// Maximum extent of outlines used to compute this blue.
  pub ascender: i32,
  /// Minimum extent of outlines used to compute this blue.
  pub descender: i32,
  /// Active zones for this blue.
  pub zones: BlueZones,
}

pub(crate) type UnscaledBlues = Vec<UnscaledBlue>;

/// A scaled alignment zone.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct ScaledBlue {
  /// Scaled position of the blue.
  pub position: ScaledWidth,
  /// Scaled overshoot for the blue.
  pub overshoot: ScaledWidth,
  /// Active zones for this blue.
  pub zones: BlueZones,
  /// True if the blue is active.
  pub is_active: bool,
}

pub(crate) type ScaledBlues = Vec<ScaledBlue>;

/// Compute unscaled blues values for each axis.
pub(crate) fn compute_unscaled_blues(
  shaper: &Shaper,
  coords: &[F2Dot14],
  style: &StyleClass,
) -> [UnscaledBlues; 2] {
  match style.script.group {
    ScriptGroup::Default => [
      Default::default(),
      compute_default_blues(shaper, coords, style),
    ],
    ScriptGroup::Cjk => compute_cjk_blues(shaper, coords, style),

    ScriptGroup::Indic => Default::default(),
  }
}

/// Compute unscaled blue values for the default script set.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L314>
fn compute_default_blues(
  shaper: &Shaper,
  coords: &[F2Dot14],
  style: &StyleClass,
) -> UnscaledBlues {
  let mut blues = UnscaledBlues::new();
  let (mut outline_buf, mut flats, mut rounds) = buffers();
  let (glyphs, units_per_em) = things_all_blues_need(shaper.font());
  let flat_threshold = units_per_em / 14;
  let mut cluster_shaper = shaper.cluster_shaper(style);
  let mut shaped_cluster = ShapedCluster::default();

  for (blue_str, blue_zones) in style.script.blues {
    let mut ascender = i32::MIN;
    let mut descender = i32::MAX;
    let mut n_flats = 0;
    let mut n_rounds = 0;
    for cluster in blue_str.split(' ') {
      let mut best_y_extremum = match blue_zones.is_top() {
        true => i32::MIN,
        false => i32::MAX,
      };
      let mut best_is_round = false;
      cluster_shaper.shape(cluster, &mut shaped_cluster);
      for (glyph, y_offset) in shaped_cluster
        .iter()
        .filter(|g| {
          ({
            let Ok(value) = g.id.to_u32();
            value
          }) != 0
        })
        .filter_map(|g| Some(((glyphs.get(g.id))?, g.y_offset)))
      {
        outline_buf.clear();
        match glyph.draw_unscaled(coords, None, &mut outline_buf).is_err() {
          true => {
            continue;
          }
          false => {}
        }
        let outline = outline_buf.as_ref();

        match outline.points.len() <= 2 {
          true => {
            continue;
          }
          false => {}
        }
        let mut best_y: Option<i16> = None;

        let best_contour_and_point = match blue_zones.is_top_like() {
          true => {
            outline.find_last_contour(|point| match best_y.is_none() || Some(point.y) > best_y {
              true => {
                best_y = Some(point.y);
                ascender = ascender.max(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(point.y);
                    migrated_value
                  }) + y_offset,
                );
                true
              }
              false => {
                descender = descender.min(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(point.y);
                    migrated_value
                  }) + y_offset,
                );
                false
              }
            })
          }
          false => {
            outline.find_last_contour(|point| match best_y.is_none() || Some(point.y) < best_y {
              true => {
                best_y = Some(point.y);
                descender = descender.min(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(point.y);
                    migrated_value
                  }) + y_offset,
                );
                true
              }
              false => {
                ascender = ascender.max(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(point.y);
                    migrated_value
                  }) + y_offset,
                );
                false
              }
            })
          }
        };
        let Some((best_contour_range, best_point_ix)) = best_contour_and_point else {
          continue;
        };
        let best_contour = &outline.points[best_contour_range];

        let mut best_y = ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i32>(best_y.unwrap());
          migrated_value
        });
        let best_x = ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i32>(best_contour[best_point_ix].x);
          migrated_value
        });

        let [mut on_point_first, mut on_point_last] =
          match best_contour[best_point_ix].is_on_curve() {
            true => [Some(best_point_ix); 2],
            false => [None; 2],
          };
        let mut segment_first = best_point_ix;
        let mut segment_last = best_point_ix;

        for (ix, prev) in cycle_backward(best_contour, best_point_ix) {
          let dist = (({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(prev.y);
            migrated_value
          }) - best_y)
            .abs();

          match dist > 5
            && ((({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(prev.x);
              migrated_value
            }) - best_x)
              .abs()
              <= (20 * dist))
          {
            true => {
              break;
            }
            false => {}
          }
          segment_first = ix;
          match prev.is_on_curve() {
            true => {
              on_point_first = Some(ix);
              match on_point_last.is_none() {
                true => {
                  on_point_last = Some(ix);
                }
                false => {}
              }
            }
            false => {}
          }
        }
        let mut next_ix = 0;
        for (ix, next) in cycle_forward(best_contour, best_point_ix) {
          next_ix = ix;
          let dist = (({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(next.y);
            migrated_value
          }) - best_y)
            .abs();

          match dist > 5
            && ((({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(next.x);
              migrated_value
            }) - best_x)
              .abs()
              <= (20 * dist))
          {
            true => {
              break;
            }
            false => {}
          }
          segment_last = ix;
          match next.is_on_curve() {
            true => {
              on_point_last = Some(ix);
              match on_point_first.is_none() {
                true => {
                  on_point_first = Some(ix);
                }
                false => {}
              }
            }
            false => {}
          }
        }
        match blue_zones.is_long() {
          true => {
            let length_threshold = units_per_em / 25;
            let dist = (({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(best_contour[segment_last].x);
              migrated_value
            }) - ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(best_contour[segment_first].x);
              migrated_value
            }))
            .abs();
            match dist < length_threshold
              && satisfies_min_long_segment_len(
                segment_first,
                segment_last,
                best_contour.len() - 1,
              ) {
              true => {
                let height_threshold = units_per_em / 4;

                let mut prev_ix = best_point_ix;
                for (ix, prev) in cycle_backward(best_contour, best_point_ix) {
                  match ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(prev.x);
                    migrated_value
                  }) != best_x
                  {
                    true => {
                      prev_ix = ix;
                      break;
                    }
                    false => {}
                  }
                }

                match prev_ix == best_point_ix {
                  true => {
                    continue;
                  }
                  false => {}
                }
                let is_ltr = ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, i32>(best_contour[prev_ix].x);
                  migrated_value
                }) < best_x;
                let mut first = segment_last;
                let mut last = first;
                let mut p_first = None;
                let mut p_last = None;
                let mut hit = false;
                loop {
                  match !hit {
                    true => {
                      first = last;

                      match best_contour[first].is_on_curve() {
                        true => {
                          p_first = Some(first);
                          p_last = Some(first);
                        }
                        false => {
                          p_first = None;
                          p_last = None;
                        }
                      }
                      hit = true;
                    }
                    false => {}
                  }
                  match last < best_contour.len() - 1 {
                    true => {
                      last += 1;
                    }
                    false => {
                      last = 0;
                    }
                  }
                  match (best_y
                    - ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(best_contour[first].y);
                      migrated_value
                    }))
                  .abs()
                    > height_threshold
                  {
                    true => {
                      hit = false;
                      continue;
                    }
                    false => {}
                  }
                  let dist = (({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(best_contour[last].y);
                    migrated_value
                  }) - ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(best_contour[first].y);
                    migrated_value
                  }))
                  .abs();
                  match dist > 5
                    && (({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(best_contour[last].x);
                      migrated_value
                    }) - ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>(best_contour[first].x);
                      migrated_value
                    }))
                    .abs()
                      <= 20 * dist
                  {
                    true => {
                      hit = false;
                      match last == segment_first {
                        true => {
                          break;
                        }
                        false => {}
                      }
                      continue;
                    }
                    false => {}
                  }
                  match best_contour[last].is_on_curve() {
                    true => {
                      p_last = Some(last);
                      match p_first.is_none() {
                        true => {
                          p_first = Some(last);
                        }
                        false => {}
                      }
                    }
                    false => {}
                  }
                  let first_x = ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(best_contour[first].x);
                    migrated_value
                  });
                  let last_x = ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(best_contour[last].x);
                    migrated_value
                  });
                  let is_cur_ltr = first_x < last_x;
                  let dx = (last_x - first_x).abs();
                  match is_cur_ltr == is_ltr && dx >= length_threshold {
                    true => {
                      loop {
                        match last < best_contour.len() - 1 {
                          true => {
                            last += 1;
                          }
                          false => {
                            last = 0;
                          }
                        }
                        let dy = (({
                          let Ok(migrated_value) =
                            console_core_number_conversion::wrapped::<_, i32>(
                              best_contour[last].y,
                            );
                          migrated_value
                        }) - ({
                          let Ok(migrated_value) =
                            console_core_number_conversion::wrapped::<_, i32>(
                              best_contour[first].y,
                            );
                          migrated_value
                        }))
                        .abs();
                        match dy > 5
                          && (({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, i32>(
                                best_contour[next_ix].x,
                              );
                            migrated_value
                          }) - ({
                            let Ok(migrated_value) =
                              console_core_number_conversion::wrapped::<_, i32>(
                                best_contour[first].x,
                              );
                            migrated_value
                          }))
                          .abs()
                            <= 20 * dist
                        {
                          true => {
                            match last > 0 {
                              true => {
                                last -= 1;
                              }
                              false => {
                                last = best_contour.len() - 1;
                              }
                            }
                            break;
                          }
                          false => {}
                        }
                        p_last = Some(last);
                        match best_contour[last].is_on_curve() {
                          true => {
                            p_last = Some(last);
                            match p_first.is_none() {
                              true => {
                                p_first = Some(last);
                              }
                              false => {}
                            }
                          }
                          false => {}
                        }
                        match last == segment_first {
                          true => {
                            break;
                          }
                          false => {}
                        }
                      }
                      best_y = ({
                        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
                          best_contour[first].y,
                        );
                        migrated_value
                      });
                      segment_first = first;
                      segment_last = last;
                      on_point_first = p_first;
                      on_point_last = p_last;
                      break;
                    }
                    false => {}
                  }
                  match last == segment_first {
                    true => {
                      break;
                    }
                    false => {}
                  }
                }
              }
              false => {}
            }
          }
          false => {}
        }
        best_y += y_offset;

        let is_round = match (on_point_first, on_point_last) {
          (Some(first), Some(last))
            if (({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(best_contour[last].x);
              migrated_value
            }) - ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(best_contour[first].x);
              migrated_value
            }))
            .abs()
              > flat_threshold =>
          {
            false
          }
          _ => {
            !best_contour[segment_first].is_on_curve()
              || !best_contour[segment_last].is_on_curve()
          }
        };
        match is_round && blue_zones.is_neutral() {
          true => {
            continue;
          }
          false => {}
        }

        match blue_zones.is_top() {
          true => match best_y > best_y_extremum {
            true => {
              best_y_extremum = best_y;
              best_is_round = is_round;
            }
            false => {}
          },
          false => match best_y < best_y_extremum {
            true => {
              best_y_extremum = best_y;
              best_is_round = is_round;
            }
            false => {}
          },
        }
      }
      match best_y_extremum != i32::MIN && best_y_extremum != i32::MAX {
        true => match best_is_round {
          true => {
            rounds[n_rounds] = best_y_extremum;
            n_rounds += 1;
          }
          false => {
            flats[n_flats] = best_y_extremum;
            n_flats += 1;
          }
        },
        false => {}
      }
    }
    match n_flats == 0 && n_rounds == 0 {
      true => {
        continue;
      }
      false => {}
    }
    rounds[..n_rounds].sort_unstable();
    flats[..n_flats].sort_unstable();
    let (mut blue_ref, mut blue_shoot) = match n_flats == 0 {
      true => {
        let val = rounds[n_rounds / 2];
        (val, val)
      }
      false => match n_rounds == 0 {
        true => {
          let val = flats[n_flats / 2];
          (val, val)
        }
        false => (flats[n_flats / 2], rounds[n_rounds / 2]),
      },
    };
    match blue_shoot != blue_ref {
      true => {
        let over_ref = blue_shoot > blue_ref;
        match blue_zones.is_top_like() ^ over_ref {
          true => {
            let val = (blue_shoot + blue_ref) / 2;
            blue_ref = val;
            blue_shoot = val;
          }
          false => {}
        }
      }
      false => {}
    }
    let mut blue = UnscaledBlue {
      position: blue_ref,
      overshoot: blue_shoot,
      ascender,
      descender,
      zones: blue_zones.retain_top_like_or_neutral(),
    };
    match blue_zones.is_x_height() {
      true => {
        blue.zones |= BlueZones::ADJUSTMENT;
      }
      false => {}
    }
    blues.push(blue);
  }

  let mut sorted_indices: [usize; MAX_BLUES] = core::array::from_fn(|ix| ix);
  let blue_values = blues.as_mut_slice();
  let len = blue_values.len();
  match len == 0 {
    true => {
      return blues;
    }
    false => {}
  }

  for i in 1..len {
    for j in (1..=i).rev() {
      let first = &blue_values[sorted_indices[j - 1]];
      let second = &blue_values[sorted_indices[j]];
      let a = match first.zones.is_top_like() {
        true => first.position,
        false => first.overshoot,
      };
      let b = match second.zones.is_top_like() {
        true => second.position,
        false => second.overshoot,
      };
      match b >= a {
        true => {
          break;
        }
        false => {}
      }
      sorted_indices.swap(j, j - 1);
    }
  }

  for i in 0..len - 1 {
    let index1 = sorted_indices[i];
    let index2 = sorted_indices[i + 1];
    let first = &blue_values[index1];
    let second = &blue_values[index2];
    let a = match first.zones.is_top_like() {
      true => first.overshoot,
      false => first.position,
    };
    let b = match second.zones.is_top_like() {
      true => second.overshoot,
      false => second.position,
    };
    match a > b {
      true => match first.zones.is_top_like() {
        true => {
          blue_values[index1].overshoot = b;
        }
        false => {
          blue_values[index1].position = b;
        }
      },
      false => {}
    }
  }
  blues
}

/// Given inclusive indices and a contour length, returns true if the segment
/// is of sufficient size to test for bumps when detecting "long" Hebrew
/// alignment zones.
fn satisfies_min_long_segment_len(first_ix: usize, last_ix: usize, contour_last: usize) -> bool {
  let inclusive_diff = match first_ix <= last_ix {
    true => last_ix - first_ix,
    false => contour_last - first_ix + 1 + last_ix,
  };

  inclusive_diff + 2 <= contour_last
}

/// Compute unscaled blue values for the CJK script set.
///
/// Note: unlike the default code above, this produces two sets of blues,
/// one for horizontal zones and one for vertical zones, respectively. The
/// horizontal set is currently not generated because this has been
/// disabled in FreeType but the code remains because we may want to revisit
/// in the future.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.c#L277>
fn compute_cjk_blues(
  shaper: &Shaper,
  coords: &[F2Dot14],
  style: &StyleClass,
) -> [UnscaledBlues; 2] {
  let mut blues = [UnscaledBlues::new(), UnscaledBlues::new()];
  let (mut outline_buf, mut flats, mut fills) = buffers();
  let (glyphs, _) = things_all_blues_need(shaper.font());
  let mut cluster_shaper = shaper.cluster_shaper(style);
  let mut shaped_cluster = ShapedCluster::default();

  for (blue_str, blue_zones) in style.script.blues {
    let is_horizontal = blue_zones.is_horizontal();

    match is_horizontal {
      true => {
        continue;
      }
      false => {}
    }
    let is_right = blue_zones.is_right();
    let is_top = blue_zones.is_top();
    let blues = &mut blues[usize::from(!is_horizontal)];
    match blues.len() >= MAX_BLUES {
      true => {
        continue;
      }
      false => {}
    }
    let mut n_flats = 0;
    let mut n_fills = 0;
    let mut is_fill = true;
    for cluster in blue_str.split(' ') {
      match cluster == "|" {
        true => {
          is_fill = false;
          continue;
        }
        false => {}
      }
      cluster_shaper.shape(cluster, &mut shaped_cluster);
      for glyph in shaped_cluster
        .iter()
        .filter(|g| {
          ({
            let Ok(value) = g.id.to_u32();
            value
          }) != 0
        })
        .filter_map(|g| (glyphs.get(g.id)))
      {
        outline_buf.clear();
        match glyph.draw_unscaled(coords, None, &mut outline_buf).is_err() {
          true => {
            continue;
          }
          false => {}
        }
        let outline = outline_buf.as_ref();

        match outline.points.len() <= 2 {
          true => {
            continue;
          }
          false => {}
        }

        let best_pos = outline
          .points
          .iter()
          .map(|p| match is_horizontal {
            true => p.x,
            false => p.y,
          })
          .reduce(
            match (is_horizontal && is_right) || (!is_horizontal && is_top) {
              true => |a: i16, c: i16| a.max(c),
              false => |a: i16, c: i16| a.min(c),
            },
          )
          .unwrap();
        match is_fill {
          true => {
            fills[n_fills] = best_pos;
            n_fills += 1;
          }
          false => {
            flats[n_flats] = best_pos;
            n_flats += 1;
          }
        }
      }
    }
    match n_flats == 0 && n_fills == 0 {
      true => {
        continue;
      }
      false => {}
    }

    fills[..n_fills].sort_unstable();
    flats[..n_flats].sort_unstable();
    let (mut blue_ref, mut blue_shoot) = match n_flats == 0 {
      true => {
        let value = ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i32>(fills[n_fills / 2]);
          migrated_value
        });
        (value, value)
      }
      false => match n_fills == 0 {
        true => {
          let value = ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(flats[n_flats / 2]);
            migrated_value
          });
          (value, value)
        }
        false => (
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(fills[n_fills / 2]);
            migrated_value
          }),
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(flats[n_flats / 2]);
            migrated_value
          }),
        ),
      },
    };

    match blue_shoot != blue_ref {
      true => {
        let under_ref = blue_shoot < blue_ref;
        match blue_zones.is_top() ^ under_ref {
          true => {
            blue_ref = (blue_shoot + blue_ref) / 2;
            blue_shoot = blue_ref;
          }
          false => {}
        }
      }
      false => {}
    }
    blues.push(UnscaledBlue {
      position: blue_ref,
      overshoot: blue_shoot,
      ascender: 0,
      descender: 0,
      zones: *blue_zones & BlueZones::TOP,
    });
  }
  blues
}

#[inline(always)]
fn buffers<T: Copy + Default>() -> (
  UnscaledOutlineBuf,
  [T; BLUE_STRING_MAX_LEN],
  [T; BLUE_STRING_MAX_LEN],
) {
  (
    UnscaledOutlineBuf::new(),
    [T::default(); BLUE_STRING_MAX_LEN],
    [T::default(); BLUE_STRING_MAX_LEN],
  )
}

/// A thneed is something everyone needs
#[inline(always)]
fn things_all_blues_need<'a>(font: &FontRef<'a>) -> (OutlineGlyphCollection<'a>, i32) {
  (
    font.outline_glyphs(),
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
        ({
          let value = font
            .head()
            .map(|head| {
              ({
                let Ok(value) = head.units_per_em();
                value
              })
            })
            .unwrap_or_default();
          value
        }),
      );
      migrated_value
    }),
  )
}

/// Iterator that begins at `start + 1` and cycles through all items
/// of the slice in forward order, ending with `start`.
pub(super) fn cycle_forward<T>(items: &[T], start: usize) -> impl Iterator<Item = (usize, &T)> {
  let len = items.len();
  let start = start + 1;
  (0..len).map(move |ix| {
    let real_ix = (ix + start) % len;
    (real_ix, &items[real_ix])
  })
}

/// Iterator that begins at `start - 1` and cycles through all items
/// of the slice in reverse order, ending with `start`.
pub(super) fn cycle_backward<T>(items: &[T], start: usize) -> impl Iterator<Item = (usize, &T)> {
  let len = items.len();
  (0..len).rev().map(move |ix| {
    let real_ix = (ix + start) % len;
    (real_ix, &items[real_ix])
  })
}

#[cfg(test)]
mod tests {
  use crate::outline::autohint::metrics::BlueZones;

  use super::{
    super::super::{
      shape::{Shaper, ShaperMode},
      style,
    },
    UnscaledBlue, satisfies_min_long_segment_len,
  };
  use raw::FontRef;

  #[test]
  fn latin_blues() {
    let font = FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let style = &style::STYLE_CLASSES[super::StyleClass::LATN];
    let blues = super::compute_default_blues(&shaper, &[], style);
    let values = blues.as_slice();
    let expected = [
      UnscaledBlue {
        position: 714,
        overshoot: 725,
        ascender: 725,
        descender: -230,
        zones: BlueZones::TOP,
      },
      UnscaledBlue {
        position: 0,
        overshoot: -10,
        ascender: 725,
        descender: -10,
        zones: BlueZones::default(),
      },
      UnscaledBlue {
        position: 760,
        overshoot: 760,
        ascender: 770,
        descender: -240,
        zones: BlueZones::TOP,
      },
      UnscaledBlue {
        position: 536,
        overshoot: 546,
        ascender: 546,
        descender: -10,
        zones: BlueZones::TOP | BlueZones::ADJUSTMENT,
      },
      UnscaledBlue {
        position: 0,
        overshoot: -10,
        ascender: 546,
        descender: -10,
        zones: BlueZones::default(),
      },
      UnscaledBlue {
        position: -240,
        overshoot: -240,
        ascender: 760,
        descender: -240,
        zones: BlueZones::default(),
      },
    ];
    assert_eq!(values, &expected);
  }

  #[test]
  fn hebrew_long_blues() {
    let font = FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::Nominal);

    let style = &style::STYLE_CLASSES[super::StyleClass::HEBR];
    let blues = super::compute_default_blues(&shaper, &[], style);
    let values = blues.as_slice();
    assert_eq!(values.len(), 3);
    let expected = [
      UnscaledBlue {
        position: 592,
        overshoot: 592,
        ascender: 647,
        descender: -240,
        zones: BlueZones::TOP,
      },
      UnscaledBlue {
        position: 0,
        overshoot: -9,
        ascender: 647,
        descender: -9,
        zones: BlueZones::default(),
      },
      UnscaledBlue {
        position: -240,
        overshoot: -240,
        ascender: 647,
        descender: -240,
        zones: BlueZones::default(),
      },
    ];
    assert_eq!(values, &expected);
  }

  #[test]
  fn cjk_blues() {
    let font = FontRef::new(font_test_data::NOTOSERIFTC_AUTOHINT_METRICS).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let style = &style::STYLE_CLASSES[super::StyleClass::HANI];
    let blues = super::compute_cjk_blues(&shaper, &[], style);
    let values = blues[1].as_slice();
    let expected = [
      UnscaledBlue {
        position: 837,
        overshoot: 824,
        ascender: 0,
        descender: 0,
        zones: BlueZones::TOP,
      },
      UnscaledBlue {
        position: -78,
        overshoot: -66,
        ascender: 0,
        descender: 0,
        zones: BlueZones::default(),
      },
    ];
    assert_eq!(values, &expected);
  }

  #[test]
  fn c2sc_shaped_blues() {
    let font = FontRef::new(font_test_data::NOTOSERIF_AUTOHINT_SHAPING).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::BestEffort);
    let style = &style::STYLE_CLASSES[super::StyleClass::LATN_C2SC];
    let blues = super::compute_default_blues(&shaper, &[], style);
    let values = blues.as_slice();

    let expected = [
      UnscaledBlue {
        position: 571,
        overshoot: 571,
        ascender: 571,
        descender: 0,
        zones: BlueZones::TOP,
      },
      UnscaledBlue {
        position: 0,
        overshoot: 0,
        ascender: 571,
        descender: 0,
        zones: BlueZones::default(),
      },
    ];
    assert_eq!(values, &expected);
  }

  /// Avoid subtraction overflow raised in
  /// <https://github.com/googlefonts/fontations/issues/1218>
  #[test]
  fn long_segment_len_avoid_overflow() {
    assert!(satisfies_min_long_segment_len(22, 0, 22));
  }

  #[test]
  fn cycle_iter_forward() {
    let items = [0, 1, 2, 3, 4, 5, 6, 7];
    let from_5 = super::cycle_forward(&items, 5)
      .map(|(_, val)| *val)
      .collect::<Vec<_>>();
    assert_eq!(from_5, &[6, 7, 0, 1, 2, 3, 4, 5]);
    let from_last = super::cycle_forward(&items, 7)
      .map(|(_, val)| *val)
      .collect::<Vec<_>>();
    assert_eq!(from_last, &items);

    let _ = super::cycle_forward::<i32>(&[], 5).count();
  }

  #[test]
  fn cycle_iter_backward() {
    let items = [0, 1, 2, 3, 4, 5, 6, 7];
    let from_5 = super::cycle_backward(&items, 5)
      .map(|(_, val)| *val)
      .collect::<Vec<_>>();
    assert_eq!(from_5, &[4, 3, 2, 1, 0, 7, 6, 5]);
    let from_0 = super::cycle_backward(&items, 0)
      .map(|(_, val)| *val)
      .collect::<Vec<_>>();
    assert_eq!(from_0, &[7, 6, 5, 4, 3, 2, 1, 0]);

    let _ = super::cycle_backward::<i32>(&[], 5).count();
  }
}
