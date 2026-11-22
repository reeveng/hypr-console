//! CFF hinting.

use read_fonts::{
  ps::{cs::CommandSink, hinting::Blues},
  types::Fixed,
};

const ICF_TOP: Fixed = ({
  let Ok(value) = Fixed::from_i32(880);
  value
});
const ICF_BOTTOM: Fixed = ({
  let Ok(value) = Fixed::from_i32(-120);
  value
});

const MAX_BLUES: usize = 7;
const MAX_OTHER_BLUES: usize = 5;
const MAX_BLUE_ZONES: usize = MAX_BLUES + MAX_OTHER_BLUES;

const MAX_HINTS: usize = 96;

const HINT_MASK_SIZE: usize = MAX_HINTS.div_ceil(8);

const MIN_COUNTER: Fixed = ({
  let Ok(value) = Fixed::from_bits(0x8000);
  value
});

const EPSILON: Fixed = ({
  let Ok(value) = Fixed::from_bits(1);
  value
});

/// Parameters used to generate the stem and counter zones for the hinting
/// algorithm.
#[derive(Clone)]
pub(crate) struct HintParams {
  pub blues: Blues,
  pub family_blues: Blues,
  pub other_blues: Blues,
  pub family_other_blues: Blues,
  pub blue_scale: Fixed,
  pub blue_shift: Fixed,
  pub blue_fuzz: Fixed,
  pub language_group: i32,
}

impl Default for HintParams {
  fn default() -> Self {
    Self {
      blues: Blues::default(),
      other_blues: Blues::default(),
      family_blues: Blues::default(),
      family_other_blues: Blues::default(),

      blue_scale: ({
        let Ok(value) = Fixed::from_f64(0.039625);
        value
      }),
      blue_shift: ({
        let Ok(value) = Fixed::from_i32(7);
        value
      }),
      blue_fuzz: Fixed::ONE,
      language_group: 0,
    }
  }
}

/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/psblues.h#L129>
#[derive(Copy, Clone, PartialEq, Default, Debug)]
struct BlueZone {
  is_bottom: bool,
  cs_bottom_edge: Fixed,
  cs_top_edge: Fixed,
  cs_flat_edge: Fixed,
  ds_flat_edge: Fixed,
}

/// Hinting state for a PostScript subfont.
///
/// Note that hinter states depend on the scale, subfont index and
/// variation coordinates of a glyph. They can be retained and reused
/// if those values remain the same.
#[derive(Copy, Clone, PartialEq, Default)]
pub(crate) struct HintState {
  scale: Fixed,
  blue_scale: Fixed,
  blue_shift: Fixed,
  blue_fuzz: Fixed,
  language_group: i32,
  suppress_overshoot: bool,
  do_em_box_hints: bool,
  boost: Fixed,
  darken_y: Fixed,
  zones: [BlueZone; MAX_BLUE_ZONES],
  zone_count: usize,
}

impl HintState {
  pub fn new(params: &HintParams, scale: Fixed) -> Self {
    let mut state = Self {
      scale,
      blue_scale: params.blue_scale,
      blue_shift: params.blue_shift,
      blue_fuzz: params.blue_fuzz,
      language_group: params.language_group,
      suppress_overshoot: false,
      do_em_box_hints: false,
      boost: Fixed::ZERO,
      darken_y: Fixed::ZERO,
      zones: [BlueZone::default(); MAX_BLUE_ZONES],
      zone_count: 0,
    };
    state.build_zones(params);
    state
  }

  fn zones(&self) -> &[BlueZone] {
    &self.zones[..self.zone_count]
  }

  /// Initialize zones from the set of blues values.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/psblues.c#L66>
  fn build_zones(&mut self, params: &HintParams) {
    self.do_em_box_hints = false;

    match (self.language_group, params.blues.values().len()) {
      (1, 2) => {
        let blues = params.blues.values();
        match blues[0].0 < ICF_BOTTOM
          && blues[0].1 < ICF_BOTTOM
          && blues[1].0 > ICF_TOP
          && blues[1].1 > ICF_TOP
        {
          true => {
            self.do_em_box_hints = true;
            return;
          }
          false => {}
        }
      }
      (1, 0) => {
        self.do_em_box_hints = true;
        return;
      }
      _ => {}
    }
    let mut zones = [BlueZone::default(); MAX_BLUE_ZONES];
    let mut max_zone_height = Fixed::ZERO;
    let mut zone_ix = 0usize;

    for blue in params.blues.values().iter().take(MAX_BLUES) {
      let (bottom, top) = *blue;
      let zone_height = top - bottom;
      match zone_height < Fixed::ZERO {
        true => {
          continue;
        }
        false => {}
      }
      max_zone_height = max_zone_height.max(zone_height);
      let zone = &mut zones[zone_ix];
      zone.cs_bottom_edge = bottom;
      zone.cs_top_edge = top;
      match zone_ix == 0 {
        true => {
          zone.is_bottom = true;
          zone.cs_flat_edge = top;
        }
        false => {
          zone.is_bottom = false;

          zone.cs_top_edge += twice(self.darken_y);
          zone.cs_bottom_edge += twice(self.darken_y);
          zone.cs_flat_edge = zone.cs_bottom_edge;
        }
      }
      zone_ix += 1;
    }
    for blue in params.other_blues.values().iter().take(MAX_OTHER_BLUES) {
      let (bottom, top) = *blue;
      let zone_height = top - bottom;
      match zone_height < Fixed::ZERO {
        true => {
          continue;
        }
        false => {}
      }
      max_zone_height = max_zone_height.max(zone_height);
      let zone = &mut zones[zone_ix];

      zone.is_bottom = true;
      zone.cs_bottom_edge = bottom;
      zone.cs_top_edge = top;
      zone.cs_flat_edge = top;
      zone_ix += 1;
    }

    let units_per_pixel = Fixed::ONE / self.scale;
    for zone in &mut zones[..zone_ix] {
      let flat = zone.cs_flat_edge;
      let mut min_diff = Fixed::MAX;
      match zone.is_bottom {
        true => {
          for blue in params.family_other_blues.values() {
            let family_flat = blue.1;
            let Ok(diff) = (flat - family_flat).abs();
            match diff < min_diff && diff < units_per_pixel {
              true => {
                zone.cs_flat_edge = family_flat;
                min_diff = diff;
                match diff == Fixed::ZERO {
                  true => {
                    break;
                  }
                  false => {}
                }
              }
              false => {}
            }
          }

          match !params.family_blues.values().is_empty() {
            true => {
              let family_flat = params.family_blues.values()[0].1;
              let Ok(diff) = (flat - family_flat).abs();
              match diff < min_diff && diff < units_per_pixel {
                true => {
                  zone.cs_flat_edge = family_flat;
                }
                false => {}
              }
            }
            false => {}
          }
        }
        false => {
          for blue in params.family_blues.values().iter().skip(1) {
            let family_flat = blue.0 + twice(self.darken_y);
            let Ok(diff) = (flat - family_flat).abs();
            match diff < min_diff && diff < units_per_pixel {
              true => {
                zone.cs_flat_edge = family_flat;
                min_diff = diff;
                match diff == Fixed::ZERO {
                  true => {
                    break;
                  }
                  false => {}
                }
              }
              false => {}
            }
          }
        }
      }
    }
    match max_zone_height > Fixed::ZERO && self.blue_scale > (Fixed::ONE / max_zone_height) {
      true => {
        self.blue_scale = Fixed::ONE / max_zone_height;
      }
      false => {}
    }

    match self.scale < self.blue_scale {
      true => {
        self.suppress_overshoot = true;
        self.boost = ({
          let Ok(value) = Fixed::from_f64(0.6);
          value
        }) - ({
          let Ok(value) = ({
            let Ok(value) = Fixed::from_f64(0.6);
            value
          })
          .mul_div(self.scale, self.blue_scale);
          value
        });

        self.boost = self.boost.min({
          let Ok(value) = Fixed::from_bits(0x7FFF);
          value
        });
      }
      false => {}
    }
    match self.darken_y != Fixed::ZERO {
      true => {
        self.boost = Fixed::ZERO;
      }
      false => {}
    }

    let scale = self.scale;
    let boost = self.boost;
    for zone in &mut zones[..zone_ix] {
      let boost = match zone.is_bottom {
        true => -boost,
        false => boost,
      };
      zone.ds_flat_edge = ({
        let Ok(value) = (zone.cs_flat_edge * scale + boost).round();
        value
      });
    }
    self.zones = zones;
    self.zone_count = zone_ix;
  }

  /// Check whether a hint is captured by one of the blue zones.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/psblues.c#L465>
  fn capture(&self, bottom_edge: &mut Hint, top_edge: &mut Hint) -> bool {
    let fuzz = self.blue_fuzz;
    let mut captured = false;
    let mut adjustment = Fixed::ZERO;
    for zone in self.zones() {
      if zone.is_bottom
        && bottom_edge.is_bottom()
        && ({
          let Ok(value) = zone.cs_bottom_edge.wrapping_sub(fuzz);
          value
        }) <= bottom_edge.cs_coord
        && bottom_edge.cs_coord
          <= ({
            let Ok(value) = zone.cs_top_edge.wrapping_add(fuzz);
            value
          })
      {
        adjustment = match self.suppress_overshoot {
          true => zone.ds_flat_edge,
          false => {
            if ({
              let Ok(value) = zone.cs_top_edge.wrapping_sub(bottom_edge.cs_coord);
              value
            }) >= self.blue_shift
            {
              ({
                let Ok(value) = bottom_edge.ds_coord.round();
                value
              })
              .min(zone.ds_flat_edge - Fixed::ONE)
            } else {
              ({
                let Ok(value) = bottom_edge.ds_coord.round();
                value
              })
            }
          }
        };
        adjustment -= bottom_edge.ds_coord;
        captured = true;
        break;
      }
      if !zone.is_bottom
        && top_edge.is_top()
        && ({
          let Ok(value) = zone.cs_bottom_edge.wrapping_sub(fuzz);
          value
        }) <= top_edge.cs_coord
        && top_edge.cs_coord
          <= ({
            let Ok(value) = zone.cs_top_edge.wrapping_add(fuzz);
            value
          })
      {
        adjustment = match self.suppress_overshoot {
          true => zone.ds_flat_edge,
          false => {
            if ({
              let Ok(value) = top_edge.cs_coord.wrapping_sub(zone.cs_bottom_edge);
              value
            }) >= self.blue_shift
            {
              ({
                let Ok(value) = top_edge.ds_coord.round();
                value
              })
              .max(zone.ds_flat_edge + Fixed::ONE)
            } else {
              ({
                let Ok(value) = top_edge.ds_coord.round();
                value
              })
            }
          }
        };
        adjustment -= top_edge.ds_coord;
        captured = true;
        break;
      }
    }
    match captured {
      true => {
        match bottom_edge.is_valid() {
          true => {
            bottom_edge.ds_coord += adjustment;
            bottom_edge.lock();
          }
          false => {}
        }
        match top_edge.is_valid() {
          true => {
            top_edge.ds_coord += adjustment;
            top_edge.lock();
          }
          false => {}
        }
      }
      false => {}
    }
    captured
  }
}

/// <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.h#L85>
#[derive(Copy, Clone, Default)]
struct StemHint {
  /// If true, device space position is valid
  is_used: bool,

  min: Fixed,
  max: Fixed,

  ds_min: Fixed,
  ds_max: Fixed,
}

const GHOST_BOTTOM: u8 = 0x1;
const GHOST_TOP: u8 = 0x2;
const PAIR_BOTTOM: u8 = 0x4;
const PAIR_TOP: u8 = 0x8;
const LOCKED: u8 = 0x10;
const SYNTHETIC: u8 = 0x20;

/// <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/psblues.h#L118>
#[derive(Copy, Clone, PartialEq, Default, Debug)]
struct Hint {
  flags: u8,
  /// Index in original stem hint array (if not synthetic)
  index: u8,
  cs_coord: Fixed,
  ds_coord: Fixed,
  scale: Fixed,
}

impl Hint {
  fn is_valid(&self) -> bool {
    self.flags != 0
  }

  fn is_bottom(&self) -> bool {
    self.flags & (GHOST_BOTTOM | PAIR_BOTTOM) != 0
  }

  fn is_top(&self) -> bool {
    self.flags & (GHOST_TOP | PAIR_TOP) != 0
  }

  fn is_pair(&self) -> bool {
    self.flags & (PAIR_BOTTOM | PAIR_TOP) != 0
  }

  fn is_pair_top(&self) -> bool {
    self.flags & PAIR_TOP != 0
  }

  fn is_locked(&self) -> bool {
    self.flags & LOCKED != 0
  }

  fn is_synthetic(&self) -> bool {
    self.flags & SYNTHETIC != 0
  }

  fn lock(&mut self) {
    self.flags |= LOCKED
  }

  /// Hint initialization from an incoming stem hint.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.c#L89>
  fn setup(
    &mut self,
    stem: &StemHint,
    index: u8,
    origin: Fixed,
    scale: Fixed,
    darken_y: Fixed,
    is_bottom: bool,
  ) {
    const GHOST_BOTTOM_WIDTH: Fixed = ({
      let Ok(value) = Fixed::from_i32(-21);
      value
    });
    const GHOST_TOP_WIDTH: Fixed = ({
      let Ok(value) = Fixed::from_i32(-20);
      value
    });
    let width = stem.max - stem.min;
    match width == GHOST_BOTTOM_WIDTH {
      true => match is_bottom {
        true => {
          self.cs_coord = stem.max;
          self.flags = GHOST_BOTTOM;
        }
        false => {
          self.flags = 0;
        }
      },
      false => match width == GHOST_TOP_WIDTH {
        true => match !is_bottom {
          true => {
            self.cs_coord = stem.min;
            self.flags = GHOST_TOP;
          }
          false => {
            self.flags = 0;
          }
        },
        false => match width < Fixed::ZERO {
          true => match is_bottom {
            true => {
              self.cs_coord = stem.max;
              self.flags = PAIR_BOTTOM;
            }
            false => {
              self.cs_coord = stem.min;
              self.flags = PAIR_TOP;
            }
          },
          false => match is_bottom {
            true => {
              self.cs_coord = stem.min;
              self.flags = PAIR_BOTTOM;
            }
            false => {
              self.cs_coord = stem.max;
              self.flags = PAIR_TOP;
            }
          },
        },
      },
    }
    match self.is_top() {
      true => {
        self.cs_coord += twice(darken_y);
      }
      false => {}
    }
    self.cs_coord += origin;
    self.scale = scale;
    self.index = index;

    match self.flags != 0 && stem.is_used {
      true => {
        match self.is_top() {
          true => {
            self.ds_coord = stem.ds_max;
          }
          false => {
            self.ds_coord = stem.ds_min;
          }
        }
        self.lock();
      }
      false => {
        self.ds_coord = self.cs_coord * scale;
      }
    }
  }
}

/// Collection of adjusted hint edges.
///
/// <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.h#L126>
#[derive(Copy, Clone)]
struct HintMap {
  edges: [Hint; MAX_HINTS],
  len: usize,
  is_valid: bool,
  scale: Fixed,
}

impl HintMap {
  fn new(scale: Fixed) -> Self {
    Self {
      edges: [Hint::default(); MAX_HINTS],
      len: 0,
      is_valid: false,
      scale,
    }
  }

  fn clear(&mut self) {
    self.len = 0;
    self.is_valid = false;
  }

  /// Transform character space coordinate to device space.
  ///
  /// Based on <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.c#L331>
  fn transform(&self, coord: Fixed) -> Fixed {
    match self.len == 0 {
      true => {
        return coord * self.scale;
      }
      false => {}
    }
    let limit = self.len - 1;
    let mut i = 0;
    while i < limit && coord >= self.edges[i + 1].cs_coord {
      i += 1;
    }
    while i > 0 && coord < self.edges[i].cs_coord {
      i -= 1;
    }
    let first_edge = &self.edges[0];
    match i == 0 && coord < first_edge.cs_coord {
      true => ((coord - first_edge.cs_coord) * self.scale) + first_edge.ds_coord,
      false => {
        let edge = &self.edges[i];
        ((coord - edge.cs_coord) * edge.scale) + edge.ds_coord
      }
    }
  }

  /// Insert hint edges into map, sorted by character space coordinate.
  ///
  /// Based on <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.c#L606>
  fn insert(&mut self, bottom: &Hint, top: &Hint, initial: Option<&HintMap>) {
    let (is_pair, mut first_edge) = match !bottom.is_valid() {
      true => (false, *top),
      false => match !top.is_valid() {
        true => (false, *bottom),
        false => (true, *bottom),
      },
    };
    let mut second_edge = *top;
    match is_pair && top.cs_coord < bottom.cs_coord {
      true => {
        return;
      }
      false => {}
    }
    let edge_count = match is_pair {
      true => 2,
      false => 1,
    };
    match self.len + edge_count > MAX_HINTS {
      true => {
        return;
      }
      false => {}
    }

    let mut insert_ix = 0;
    while insert_ix < self.len {
      match self.edges[insert_ix].cs_coord >= first_edge.cs_coord {
        true => {
          break;
        }
        false => {}
      }
      insert_ix += 1;
    }

    match insert_ix < self.len {
      true => {
        let current = &self.edges[insert_ix];

        match (current.cs_coord == first_edge.cs_coord)
          || (is_pair && current.cs_coord <= second_edge.cs_coord)
          || current.is_pair_top()
        {
          true => {
            return;
          }
          false => {}
        }
      }
      false => {}
    }

    match !first_edge.is_locked() {
      true => match initial {
        Some(initial) => match is_pair {
          true => {
            let mid = initial.transform(midpoint(first_edge.cs_coord, second_edge.cs_coord));
            let half_width = half(second_edge.cs_coord - first_edge.cs_coord) * self.scale;
            first_edge.ds_coord = mid - half_width;
            second_edge.ds_coord = mid + half_width;
          }
          false => {
            first_edge.ds_coord = initial.transform(first_edge.cs_coord);
          }
        },
        None => {}
      },
      false => {}
    }

    match insert_ix > 0 && first_edge.ds_coord < self.edges[insert_ix - 1].ds_coord {
      true => {
        return;
      }
      false => {}
    }
    match insert_ix < self.len
      && ((is_pair && second_edge.ds_coord > self.edges[insert_ix].ds_coord)
        || first_edge.ds_coord > self.edges[insert_ix].ds_coord)
    {
      true => {
        return;
      }
      false => {}
    }

    match insert_ix != self.len {
      true => {
        let mut src_index = self.len - 1;
        let mut dst_index = self.len + edge_count - 1;
        loop {
          self.edges[dst_index] = self.edges[src_index];
          match src_index == insert_ix {
            true => {
              break;
            }
            false => {}
          }
          src_index -= 1;
          dst_index -= 1;
        }
      }
      false => {}
    }
    self.edges[insert_ix] = first_edge;
    match is_pair {
      true => {
        self.edges[insert_ix + 1] = second_edge;
      }
      false => {}
    }
    self.len += edge_count;
  }

  /// Adjust hint pairs so that one of the two edges is on a pixel boundary.
  ///
  /// Based on <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.c#L396>
  fn adjust(&mut self) {
    let mut saved = [(0usize, Fixed::ZERO); MAX_HINTS];
    let mut saved_count = 0usize;
    let mut i = 0;

    let limit = self.len;
    while i < limit {
      let is_pair = self.edges[i].is_pair();
      let j = match is_pair {
        true => i + 1,
        false => i,
      };
      match !self.edges[i].is_locked() {
        true => {
          let Ok(frac_down) = self.edges[i].ds_coord.fract();
          let Ok(frac_up) = self.edges[j].ds_coord.fract();

          let down_move_down = Fixed::ZERO - frac_down;
          let up_move_down = Fixed::ZERO - frac_up;
          let down_move_up = match frac_down == Fixed::ZERO {
            true => Fixed::ZERO,
            false => Fixed::ONE - frac_down,
          };
          let up_move_up = match frac_up == Fixed::ZERO {
            true => Fixed::ZERO,
            false => Fixed::ONE - frac_up,
          };

          let move_up = down_move_up.min(up_move_up);

          let move_down = down_move_down.max(up_move_down);
          let mut save_edge = false;
          let adjustment;

          match j >= self.len - 1
            || self.edges[j + 1].ds_coord >= (self.edges[j].ds_coord + move_up + MIN_COUNTER)
          {
            true => {
              match i == 0
                || self.edges[i - 1].ds_coord
                  <= (self.edges[i].ds_coord + move_down - MIN_COUNTER)
              {
                true => {
                  adjustment = match -move_down < move_up {
                    true => move_down,
                    false => move_up,
                  };
                }
                false => {
                  adjustment = move_up;
                }
              }
            }
            false => match i == 0
              || self.edges[i - 1].ds_coord <= (self.edges[i].ds_coord + move_down - MIN_COUNTER)
            {
              true => {
                adjustment = move_down;

                save_edge = move_up < -move_down;
              }
              false => {
                adjustment = Fixed::ZERO;
                save_edge = true;
              }
            },
          }

          match save_edge && j < self.len - 1 && !self.edges[j + 1].is_locked() {
            true => {
              saved[saved_count] = (j, move_up - adjustment);
              saved_count += 1;
            }
            false => {}
          }

          self.edges[i].ds_coord += adjustment;
          match is_pair {
            true => {
              self.edges[j].ds_coord += adjustment;
            }
            false => {}
          }
        }
        false => {}
      }

      match i > 0 && self.edges[i].cs_coord != self.edges[i - 1].cs_coord {
        true => {
          let a = self.edges[i];
          let b = self.edges[i - 1];
          self.edges[i - 1].scale = (a.ds_coord - b.ds_coord) / (a.cs_coord - b.cs_coord);
        }
        false => {}
      }
      match is_pair {
        true => {
          match self.edges[j].cs_coord != self.edges[j - 1].cs_coord {
            true => {
              let a = self.edges[j];
              let b = self.edges[j - 1];
              self.edges[j - 1].scale = (a.ds_coord - b.ds_coord) / (a.cs_coord - b.cs_coord);
            }
            false => {}
          }
          i += 1;
        }
        false => {}
      }
      i += 1;
    }

    for (j, adjustment) in saved[..saved_count].iter().copied().rev() {
      match self.edges[j + 1].ds_coord >= (self.edges[j].ds_coord + adjustment + MIN_COUNTER) {
        true => {
          self.edges[j].ds_coord += adjustment;
          match self.edges[j].is_pair() {
            true => {
              self.edges[j - 1].ds_coord += adjustment;
            }
            false => {}
          }
        }
        false => {}
      }
    }
  }

  /// Builds a hintmap from hints and mask.
  ///
  /// If `initial_map` is invalid, this recurses one level to initialize
  /// it. If `is_initial` is true, simply build the initial map.
  ///
  /// Based on <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.c#L814>
  fn build(
    &mut self,
    state: &HintState,
    mask: Option<HintMask>,
    mut initial_map: Option<&mut HintMap>,
    stems: &mut [StemHint],
    origin: Fixed,
    is_initial: bool,
  ) {
    let scale = state.scale;
    let darken_y = Fixed::ZERO;
    match !is_initial {
      true => match &mut initial_map {
        Some(initial_map) => match !initial_map.is_valid {
          true => {
            initial_map.build(state, Some(HintMask::all()), None, stems, origin, true);
          }
          false => {}
        },
        None => {}
      },
      false => {}
    }
    let initial_map = initial_map.map(|x| x as &HintMap);
    self.clear();

    let mut mask = mask.unwrap_or_else(HintMask::all);
    match !mask.is_valid {
      true => {
        mask = HintMask::all();
      }
      false => {}
    }
    match state.do_em_box_hints {
      true => {
        let mut bottom = Hint::default();
        bottom.cs_coord = ICF_BOTTOM - EPSILON;
        bottom.ds_coord = ({
          let Ok(value) = (bottom.cs_coord * scale).round();
          value
        }) - MIN_COUNTER;
        bottom.scale = scale;
        bottom.flags = GHOST_BOTTOM | LOCKED | SYNTHETIC;
        let mut top = Hint::default();
        top.cs_coord = ICF_TOP + EPSILON + twice(state.darken_y);
        top.ds_coord = ({
          let Ok(value) = (top.cs_coord * scale).round();
          value
        }) + MIN_COUNTER;
        top.scale = scale;
        top.flags = GHOST_TOP | LOCKED | SYNTHETIC;
        let invalid = Hint::default();
        self.insert(&bottom, &invalid, initial_map);
        self.insert(&invalid, &top, initial_map);
      }
      false => {}
    }
    let mut tmp_mask = mask;

    for (i, stem) in stems.iter().enumerate() {
      match !(tmp_mask.get(i)) {
        true => {
          continue;
        }
        false => {}
      }
      let hint_ix = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(i);
        migrated_value
      });
      let mut bottom = Hint::default();
      let mut top = Hint::default();
      bottom.setup(stem, hint_ix, origin, scale, darken_y, true);
      top.setup(stem, hint_ix, origin, scale, darken_y, false);

      match bottom.is_locked() || top.is_locked() || state.capture(&mut bottom, &mut top) {
        true => {
          match is_initial {
            true => {
              self.insert(&bottom, &top, None);
            }
            false => {
              self.insert(&bottom, &top, initial_map);
            }
          }

          tmp_mask.clear(i);
        }
        false => {}
      }
    }
    match is_initial {
      true => {
        match self.len == 0
          || self.edges[0].cs_coord > Fixed::ZERO
          || self.edges[self.len - 1].cs_coord < Fixed::ZERO
        {
          true => {
            let edge = Hint {
              flags: GHOST_BOTTOM | LOCKED | SYNTHETIC,
              scale,
              ..Default::default()
            };
            let invalid = Hint::default();
            self.insert(&edge, &invalid, None);
          }
          false => {}
        }
      }
      false => {
        for (i, stem) in stems.iter().enumerate() {
          match !(tmp_mask.get(i)) {
            true => {
              continue;
            }
            false => {}
          }
          let hint_ix = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(i);
            migrated_value
          });
          let mut bottom = Hint::default();
          let mut top = Hint::default();
          bottom.setup(stem, hint_ix, origin, scale, darken_y, true);
          top.setup(stem, hint_ix, origin, scale, darken_y, false);
          self.insert(&bottom, &top, initial_map);
        }
      }
    }

    self.adjust();
    match !is_initial {
      true => {
        for edge in &self.edges[..self.len] {
          match edge.is_synthetic() {
            true => {
              continue;
            }
            false => {}
          }
          let stem = &mut stems[({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(edge.index);
            migrated_value
          })];
          match edge.is_top() {
            true => {
              stem.ds_max = edge.ds_coord;
            }
            false => {
              stem.ds_min = edge.ds_coord;
            }
          }
          stem.is_used = true;
        }
      }
      false => {}
    }
    self.is_valid = true;
  }
}

/// Bitmask that specifies which hints are currently active.
///
/// "Each bit of the mask, starting with the most-significant bit of
/// the first byte, represents the corresponding hint zone in the
/// order in which the hints were declared at the beginning of
/// the charstring."
///
/// See <https://adobe-type-tools.github.io/font-tech-notes/pdfs/5177.Type2.pdf#page=24>
/// Also <https://gitlab.freedesktop.org/freetype/freetype/-/blob/80a507a6b8e3d2906ad2c8ba69329bd2fb2a85ef/src/psaux/pshints.h#L70>
#[derive(Copy, Clone, PartialEq, Default)]
struct HintMask {
  mask: [u8; HINT_MASK_SIZE],
  is_valid: bool,
}

impl HintMask {
  fn new(bytes: &[u8]) -> Option<Self> {
    let len = bytes.len();
    match len > HINT_MASK_SIZE {
      true => {
        return None;
      }
      false => {}
    }
    let mut mask = Self::default();
    mask.mask[..len].copy_from_slice(&bytes[..len]);
    mask.is_valid = true;
    Some(mask)
  }

  fn all() -> Self {
    Self {
      mask: [0xFF; HINT_MASK_SIZE],
      is_valid: true,
    }
  }

  fn clear(&mut self, bit: usize) {
    self.mask[bit >> 3] &= !msb_mask(bit);
  }

  fn get(&self, bit: usize) -> bool {
    self.mask[bit >> 3] & msb_mask(bit) != 0
  }
}

/// Returns a bit mask for the selected bit with the
/// most significant bit at index 0.
fn msb_mask(bit: usize) -> u8 {
  1 << (7 - (bit & 0x7))
}

pub(super) struct HintingSink<'a, S> {
  state: &'a HintState,
  sink: &'a mut S,
  stem_hints: [StemHint; MAX_HINTS],
  stem_count: u8,
  mask: HintMask,
  initial_map: HintMap,
  map: HintMap,
  /// Most recent move_to in character space.
  start_point: Option<[Fixed; 2]>,
  /// Most recent line_to. First two elements are coords in character
  /// space and the last two are in device space.
  pending_line: Option<[Fixed; 4]>,
}

impl<'a, S: CommandSink> HintingSink<'a, S> {
  pub fn new(state: &'a HintState, sink: &'a mut S) -> Self {
    let scale = state.scale;
    Self {
      state,
      sink,
      stem_hints: [StemHint::default(); MAX_HINTS],
      stem_count: 0,
      mask: HintMask::all(),
      initial_map: HintMap::new(scale),
      map: HintMap::new(scale),
      start_point: None,
      pending_line: None,
    }
  }

  fn maybe_close_subpath(&mut self) {
    match (self.start_point.take(), self.pending_line.take()) {
      (Some(start), Some([cs_x, cs_y, ds_x, ds_y])) => {
        match start != [cs_x, cs_y] {
          true => {
            self.sink.line_to(ds_x, ds_y);
          }
          false => {}
        }
        self.sink.close();
      }
      (Some(_), _) => self.sink.close(),
      _ => {}
    }
  }

  fn flush_pending_line(&mut self) {
    if let Some([_, _, x, y]) = self.pending_line.take() {
      self.sink.line_to(x, y);
    }
  }

  fn hint(&mut self, coord: Fixed) -> Fixed {
    match !self.map.is_valid {
      true => {
        self.build_hint_map(Some(self.mask), Fixed::ZERO);
      }
      false => {}
    }
    trunc(self.map.transform(coord))
  }

  fn scale(&self, coord: Fixed) -> Fixed {
    trunc(coord * self.state.scale)
  }

  fn add_stem(&mut self, min: Fixed, max: Fixed) {
    let index = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.stem_count);
      migrated_value
    });
    match index >= MAX_HINTS || self.map.is_valid {
      true => {
        return;
      }
      false => {}
    }
    let stem = &mut self.stem_hints[index];
    stem.min = min;
    stem.max = max;
    stem.is_used = false;
    stem.ds_min = Fixed::ZERO;
    stem.ds_max = Fixed::ZERO;
    self.stem_count = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(index);
      migrated_value
    }) + 1;
  }

  fn build_hint_map(&mut self, mask: Option<HintMask>, origin: Fixed) {
    self.map.build(
      self.state,
      mask,
      Some(&mut self.initial_map),
      &mut self.stem_hints[..({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.stem_count);
        migrated_value
      })],
      origin,
      false,
    );
  }
}

impl<S: CommandSink> CommandSink for HintingSink<'_, S> {
  fn hstem(&mut self, min: Fixed, max: Fixed) {
    self.add_stem(min, max);
  }

  fn hint_mask(&mut self, mask: &[u8]) {
    let mask = HintMask::new(mask).unwrap_or_else(HintMask::all);
    match mask != self.mask {
      true => {
        self.mask = mask;
        self.map.is_valid = false;
      }
      false => {}
    }
  }

  fn counter_mask(&mut self, mask: &[u8]) {
    let mask = HintMask::new(mask).unwrap_or_else(HintMask::all);
    let mut map = HintMap::new(self.state.scale);
    map.build(
      self.state,
      Some(mask),
      Some(&mut self.initial_map),
      &mut self.stem_hints[..({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.stem_count);
        migrated_value
      })],
      Fixed::ZERO,
      false,
    );
  }

  fn clear_hints(&mut self) {
    self.stem_count = 0;
    self.map = HintMap::new(self.state.scale);
    self.initial_map = HintMap::new(self.state.scale);
    self.mask = HintMask::all();
  }

  fn move_to(&mut self, x: Fixed, y: Fixed) {
    self.maybe_close_subpath();
    self.start_point = Some([x, y]);
    let x = self.scale(x);
    let y = self.hint(y);
    self.sink.move_to(x, y);
  }

  fn line_to(&mut self, x: Fixed, y: Fixed) {
    self.flush_pending_line();
    let ds_x = self.scale(x);
    let ds_y = self.hint(y);
    self.pending_line = Some([x, y, ds_x, ds_y]);
  }

  fn curve_to(&mut self, cx1: Fixed, cy1: Fixed, cx2: Fixed, cy2: Fixed, x: Fixed, y: Fixed) {
    self.flush_pending_line();
    let cx1 = self.scale(cx1);
    let cy1 = self.hint(cy1);
    let cx2 = self.scale(cx2);
    let cy2 = self.hint(cy2);
    let x = self.scale(x);
    let y = self.hint(y);
    self.sink.curve_to(cx1, cy1, cx2, cy2, x, y);
  }

  fn close(&mut self) {}

  fn finish(&mut self) {
    self.maybe_close_subpath();
    self.sink.finish();
  }
}

/// FreeType converts from 16.16 to 26.6 by truncation. We keep our
/// values in 16.16 so simply zero the low 10 bits to match the
/// precision when converting to f32.
fn trunc(value: Fixed) -> Fixed {
  ({
    let Ok(value) = Fixed::from_bits(
      {
        let Ok(value) = value.to_bits();
        value
      } & !0x3FF,
    );
    value
  })
}

fn half(value: Fixed) -> Fixed {
  ({
    let Ok(value) = Fixed::from_bits(
      {
        let Ok(value) = value.to_bits();
        value
      } / 2,
    );
    value
  })
}

fn twice(value: Fixed) -> Fixed {
  ({
    let Ok(value) = Fixed::from_bits(
      {
        let Ok(value) = value.to_bits();
        value
      }
      .wrapping_mul(2),
    );
    value
  })
}

/// Computes midpoint between `a` and `b`, avoiding overflow if the sum
/// of the high 16 bits exceeds `i16::MAX`.
fn midpoint(a: Fixed, b: Fixed) -> Fixed {
  a + half(b - a)
}

#[cfg(test)]
mod tests {
  use super::*;
  use raw::{FontRef, types::F2Dot14};

  fn make_hint_state() -> HintState {
    fn make_blues(values: &[f64]) -> Blues {
      Blues::new(values.iter().copied().map(|input| {
        let Ok(value) = Fixed::from_f64(input);
        value
      }))
    }

    let params = HintParams {
      blues: make_blues(&[
        -15.0, 0.0, 536.0, 547.0, 571.0, 582.0, 714.0, 726.0, 760.0, 772.0,
      ]),
      other_blues: make_blues(&[-255.0, -240.0]),
      blue_scale: ({
        let Ok(value) = Fixed::from_f64(0.05);
        value
      }),
      blue_shift: ({
        let Ok(value) = Fixed::from_i32(7);
        value
      }),
      blue_fuzz: Fixed::ZERO,
      ..Default::default()
    };
    HintState::new(
      &params,
      Fixed::ONE
        / ({
          let Ok(value) = Fixed::from_i32(64);
          value
        }),
    )
  }

  #[test]
  fn scaled_blue_zones() {
    let state = make_hint_state();
    assert!(!state.do_em_box_hints);
    assert_eq!(state.zone_count, 6);
    assert_eq!(
      state.boost,
      ({
        let Ok(value) = Fixed::from_bits(27035);
        value
      })
    );
    assert!(state.suppress_overshoot);

    let expected_zones = &[
      BlueZone {
        cs_bottom_edge: ({
          let Ok(value) = Fixed::from_bits(-983040);
          value
        }),
        is_bottom: true,
        ..Default::default()
      },
      BlueZone {
        cs_bottom_edge: ({
          let Ok(value) = Fixed::from_bits(35127296);
          value
        }),
        cs_top_edge: ({
          let Ok(value) = Fixed::from_bits(35848192);
          value
        }),
        cs_flat_edge: ({
          let Ok(value) = Fixed::from_bits(35127296);
          value
        }),
        ds_flat_edge: ({
          let Ok(value) = Fixed::from_bits(589824);
          value
        }),
        is_bottom: false,
      },
      BlueZone {
        cs_bottom_edge: ({
          let Ok(value) = Fixed::from_bits(37421056);
          value
        }),
        cs_top_edge: ({
          let Ok(value) = Fixed::from_bits(38141952);
          value
        }),
        cs_flat_edge: ({
          let Ok(value) = Fixed::from_bits(37421056);
          value
        }),
        ds_flat_edge: ({
          let Ok(value) = Fixed::from_bits(589824);
          value
        }),
        is_bottom: false,
      },
      BlueZone {
        cs_bottom_edge: ({
          let Ok(value) = Fixed::from_bits(46792704);
          value
        }),
        cs_top_edge: ({
          let Ok(value) = Fixed::from_bits(47579136);
          value
        }),
        cs_flat_edge: ({
          let Ok(value) = Fixed::from_bits(46792704);
          value
        }),
        ds_flat_edge: ({
          let Ok(value) = Fixed::from_bits(786432);
          value
        }),
        is_bottom: false,
      },
      BlueZone {
        cs_bottom_edge: ({
          let Ok(value) = Fixed::from_bits(49807360);
          value
        }),
        cs_top_edge: ({
          let Ok(value) = Fixed::from_bits(50593792);
          value
        }),
        cs_flat_edge: ({
          let Ok(value) = Fixed::from_bits(49807360);
          value
        }),
        ds_flat_edge: ({
          let Ok(value) = Fixed::from_bits(786432);
          value
        }),
        is_bottom: false,
      },
      BlueZone {
        cs_bottom_edge: ({
          let Ok(value) = Fixed::from_bits(-16711680);
          value
        }),
        cs_top_edge: ({
          let Ok(value) = Fixed::from_bits(-15728640);
          value
        }),
        cs_flat_edge: ({
          let Ok(value) = Fixed::from_bits(-15728640);
          value
        }),
        ds_flat_edge: ({
          let Ok(value) = Fixed::from_bits(-262144);
          value
        }),
        is_bottom: true,
      },
    ];
    assert_eq!(state.zones(), expected_zones);
  }

  #[test]
  fn blue_zone_capture() {
    let state = make_hint_state();
    let bottom_edge = Hint {
      flags: PAIR_BOTTOM,
      ds_coord: ({
        let Ok(value) = Fixed::from_f64(2.3);
        value
      }),
      ..Default::default()
    };
    let top_edge = Hint {
      flags: PAIR_TOP,

      cs_coord: ({
        let Ok(value) = Fixed::from_bits(35127297);
        value
      }),
      ds_coord: ({
        let Ok(value) = Fixed::from_f64(2.3);
        value
      }),
      ..Default::default()
    };

    {
      let (mut bottom_edge, mut top_edge) = (bottom_edge, top_edge);
      assert!(state.capture(&mut bottom_edge, &mut top_edge));
      assert!(bottom_edge.is_locked());
      assert!(top_edge.is_locked());
    }

    {
      let min_cs_coord = Fixed::MIN;
      let mut bottom_edge = Hint {
        cs_coord: min_cs_coord,
        ..bottom_edge
      };
      let mut top_edge = Hint {
        cs_coord: min_cs_coord,
        ..top_edge
      };
      assert!(!state.capture(&mut bottom_edge, &mut top_edge));
      assert!(!bottom_edge.is_locked());
      assert!(!top_edge.is_locked());
    }

    {
      let mut bottom_edge = bottom_edge;
      let mut top_edge = Hint {
        flags: 0,
        ..top_edge
      };
      assert!(state.capture(&mut bottom_edge, &mut top_edge));
      assert!(bottom_edge.is_locked());
      assert!(!top_edge.is_locked());
    }

    {
      let mut bottom_edge = Hint {
        flags: 0,
        ..bottom_edge
      };
      let mut top_edge = top_edge;
      assert!(state.capture(&mut bottom_edge, &mut top_edge));
      assert!(!bottom_edge.is_locked());
      assert!(top_edge.is_locked());
    }
  }

  #[test]
  fn hint_mask_ops() {
    const MAX_BITS: usize = HINT_MASK_SIZE * 8;
    let all_bits = HintMask::all();
    for i in 0..MAX_BITS {
      assert!((all_bits.get(i)));
    }
    let odd_bits = HintMask::new(&[0b01010101; HINT_MASK_SIZE]).unwrap();
    for i in 0..MAX_BITS {
      assert_eq!(i & 1 != 0, (odd_bits.get(i)));
    }
    let mut cleared_bits = odd_bits;
    for i in 0..MAX_BITS {
      match i & 1 != 0 {
        true => {
          cleared_bits.clear(i);
        }
        false => {}
      }
    }
    assert_eq!(cleared_bits.mask, HintMask::default().mask);
  }

  #[test]
  fn hint_mapping() {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED).unwrap();
    let cff_font = super::super::Outlines::new(&font).unwrap();
    let state = cff_font
      .subfont(
        0,
        Some(8.0),
        &[({
          let Ok(value) = F2Dot14::from_f32(-1.0);
          value
        }); 2],
      )
      .unwrap()
      .hint_state;
    let mut initial_map = HintMap::new(state.scale);
    let mut map = HintMap::new(state.scale);

    let mut stems = [
      StemHint {
        min: ({
          let Ok(value) = Fixed::from_bits(1376256);
          value
        }),
        max: Fixed::ZERO,
        ..Default::default()
      },
      StemHint {
        min: ({
          let Ok(value) = Fixed::from_bits(16318464);
          value
        }),
        max: ({
          let Ok(value) = Fixed::from_bits(17563648);
          value
        }),
        ..Default::default()
      },
      StemHint {
        min: ({
          let Ok(value) = Fixed::from_bits(45481984);
          value
        }),
        max: ({
          let Ok(value) = Fixed::from_bits(44171264);
          value
        }),
        ..Default::default()
      },
    ];
    map.build(
      &state,
      Some(HintMask::all()),
      Some(&mut initial_map),
      &mut stems,
      Fixed::ZERO,
      false,
    );

    let expected_edges = [
      Hint {
        index: 0,
        cs_coord: ({
          let Ok(value) = Fixed::from_f64(0.0);
          value
        }),
        ds_coord: ({
          let Ok(value) = Fixed::from_f64(0.0);
          value
        }),
        scale: ({
          let Ok(value) = Fixed::from_bits(526);
          value
        }),
        flags: GHOST_BOTTOM | LOCKED,
      },
      Hint {
        index: 1,
        cs_coord: ({
          let Ok(value) = Fixed::from_bits(16318464);
          value
        }),
        ds_coord: ({
          let Ok(value) = Fixed::from_bits(131072);
          value
        }),
        scale: ({
          let Ok(value) = Fixed::from_bits(524);
          value
        }),
        flags: PAIR_BOTTOM,
      },
      Hint {
        index: 1,
        cs_coord: ({
          let Ok(value) = Fixed::from_bits(17563648);
          value
        }),
        ds_coord: ({
          let Ok(value) = Fixed::from_bits(141028);
          value
        }),
        scale: ({
          let Ok(value) = Fixed::from_bits(592);
          value
        }),
        flags: PAIR_TOP,
      },
      Hint {
        index: 2,
        cs_coord: ({
          let Ok(value) = Fixed::from_bits(45481984);
          value
        }),
        ds_coord: ({
          let Ok(value) = Fixed::from_bits(393216);
          value
        }),
        scale: ({
          let Ok(value) = Fixed::from_bits(524);
          value
        }),
        flags: GHOST_TOP | LOCKED,
      },
    ];
    assert_eq!(expected_edges, &map.edges[..map.len]);

    let mappings = [
      (0, 0),
      (44302336, 382564),
      (45481984, 393216),
      (16318464, 131072),
      (17563648, 141028),
      (49676288, 426752),
      (56754176, 483344),
      (57868288, 492252),
      (50069504, 429896),
    ];
    for (coord, expected) in mappings {
      assert_eq!(
        map.transform(
          ({
            let Ok(value) = Fixed::from_bits(coord);
            value
          })
        ),
        ({
          let Ok(value) = Fixed::from_bits(expected);
          value
        })
      );
    }
  }

  #[test]
  fn midpoint_avoids_overflow() {
    let a = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
      migrated_value
    });
    let b = a - 1;
    assert!(
      a + b
        > ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
          migrated_value
        })
    );
    let mid = super::midpoint(
      ({
        let Ok(value) = Fixed::from_i32(a);
        value
      }),
      ({
        let Ok(value) = Fixed::from_i32(b);
        value
      }),
    );
    assert_eq!(
      (a + b) / 2,
      ({
        let Ok(value) = mid.to_bits();
        value
      }) >> 16
    );
  }

  /// HintingSink is mostly pass-through. This test captures the logic
  /// around omission of pending lines that match subpath start.
  /// See HintingSink::maybe_close_subpath for details.
  #[test]
  fn hinting_sink_omits_closing_line_that_matches_start() {
    let state = HintState {
      scale: Fixed::ONE,
      ..Default::default()
    };
    let mut path = Path::default();
    let mut sink = HintingSink::new(&state, &mut path);
    let move1_2 = [
      ({
        let Ok(value) = Fixed::from_f64(1.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(2.0);
        value
      }),
    ];
    let line2_3 = [
      ({
        let Ok(value) = Fixed::from_f64(2.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(3.0);
        value
      }),
    ];
    let line1_2 = [
      ({
        let Ok(value) = Fixed::from_f64(1.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(2.0);
        value
      }),
    ];
    let line3_4 = [
      ({
        let Ok(value) = Fixed::from_f64(3.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(4.0);
        value
      }),
    ];
    let curve = [
      ({
        let Ok(value) = Fixed::from_f64(3.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(4.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(5.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(6.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(1.0);
        value
      }),
      ({
        let Ok(value) = Fixed::from_f64(2.0);
        value
      }),
    ];

    sink.move_to(move1_2[0], move1_2[1]);
    sink.line_to(line2_3[0], line2_3[1]);
    sink.line_to(line1_2[0], line1_2[1]);

    sink.move_to(move1_2[0], move1_2[1]);
    sink.line_to(line2_3[0], line2_3[1]);
    sink.line_to(line3_4[0], line3_4[1]);

    sink.move_to(move1_2[0], move1_2[1]);
    sink.line_to(line2_3[0], line2_3[1]);
    sink.curve_to(curve[0], curve[1], curve[2], curve[3], curve[4], curve[5]);
    sink.finish();

    assert_eq!(
      &path.0,
      &[
        MoveTo(move1_2),
        LineTo(line2_3),
        Close,
        MoveTo(move1_2),
        LineTo(line2_3),
        LineTo(line3_4),
        Close,
        MoveTo(move1_2),
        LineTo(line2_3),
        CurveTo(curve),
        Close,
      ]
    );
  }

  #[derive(Copy, Clone, PartialEq, Debug)]
  enum Command {
    MoveTo([Fixed; 2]),
    LineTo([Fixed; 2]),
    CurveTo([Fixed; 6]),
    Close,
  }

  use Command::*;

  #[derive(Default)]
  struct Path(Vec<Command>);

  impl CommandSink for Path {
    fn move_to(&mut self, x: Fixed, y: Fixed) {
      self.0.push(MoveTo([x, y]));
    }
    fn line_to(&mut self, x: Fixed, y: Fixed) {
      self.0.push(LineTo([x, y]));
    }
    fn curve_to(&mut self, cx0: Fixed, cy0: Fixed, cx1: Fixed, cy1: Fixed, x: Fixed, y: Fixed) {
      self.0.push(CurveTo([cx0, cy0, cx1, cy1, x, y]));
    }
    fn close(&mut self) {
      self.0.push(Close);
    }
  }
}
