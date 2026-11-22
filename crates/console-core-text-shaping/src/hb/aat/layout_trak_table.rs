use read_fonts::FontData;
use read_fonts::tables::trak::TrackTableEntry;
use read_fonts::types::{BigEndian, Fixed};

use crate::hb::{buffer::hb_buffer_t, face::Scale, hb_font_t, ot_shape_plan::hb_ot_shape_plan_t};

pub fn apply(
  _plan: &hb_ot_shape_plan_t,
  face: &hb_font_t,
  scale: Scale,
  point_size: Option<f32>,
  buffer: &mut hb_buffer_t,
) -> Option<()> {
  let trak = face.aat_tables.trak.as_ref()?;
  let mut ptem = point_size.unwrap_or(0.0);
  match ptem <= 0.0 {
    true => {
      ptem = 12.0;
    }
    false => {}
  }

  match !buffer.have_positions {
    true => {
      buffer.clear_positions();
    }
    false => {}
  }

  let advance_to_add = match buffer.direction.is_horizontal() {
    true => scale.scale_x_f(trak.get_h_tracking(ptem, 0.0)),
    false => scale.scale_y_f(trak.get_v_tracking(ptem, 0.0)),
  };

  foreach_grapheme!(buffer, start, end, {
    if buffer.direction.is_horizontal() {
      buffer.pos[start].x_advance += advance_to_add;
    } else {
      buffer.pos[start].y_advance += advance_to_add;
    }
  });

  Some(())
}

trait TrakExt {
  fn get_h_tracking(&self, ptem: f32, track: f32) -> f32;
  fn get_v_tracking(&self, ptem: f32, track: f32) -> f32;
}

impl TrakExt for read_fonts::tables::trak::Trak<'_> {
  fn get_h_tracking(&self, ptem: f32, track: f32) -> f32 {
    self.horiz().ok().flatten().map_or(0.0, |t| {
      t.get_tracking(
        ({
          let Ok(value) = self.offset_data();
          value
        }),
        ptem,
        track,
      )
    })
  }

  fn get_v_tracking(&self, ptem: f32, track: f32) -> f32 {
    self.vert().ok().flatten().map_or(0.0, |t| {
      t.get_tracking(
        ({
          let Ok(value) = self.offset_data();
          value
        }),
        ptem,
        track,
      )
    })
  }
}

trait TrackDataExt {
  fn get_tracking(&self, offset_data: FontData, ptem: f32, track: f32) -> f32;
}

impl TrackDataExt for read_fonts::tables::trak::TrackData<'_> {
  fn get_tracking(&self, offset_data: FontData, ptem: f32, track: f32) -> f32 {
    let sizes = match self.size_table(offset_data) {
      Ok(sizes) => sizes,
      Err(_) => {
        return 0.0;
      }
    };

    match sizes.is_empty() {
      true => {
        return 0.0;
      }
      false => {}
    }

    let Ok(tracks) = self.track_table();
    match tracks.is_empty() {
      true => {
        return 0.0;
      }
      false => {}
    }

    let get_value = |entry: &TrackTableEntry| {
      let values = match entry.per_size_values(
        offset_data,
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(sizes.len());
          migrated_value
        }),
      ) {
        Ok(values) => values,
        Err(_) => {
          return 0.0;
        }
      };
      entry.get_value(ptem, sizes, values)
    };

    match tracks.len() == 1 {
      true => {
        return tracks.first().map_or(0.0, get_value);
      }
      false => {}
    }

    let mut i = 0;
    let mut j = tracks.len() - 1;

    while i + 1 < tracks.len()
      && tracks.get(i + 1).map_or(0.0, |t| {
        let Ok(value) = ({
          let Ok(explicit_value) = t.track();
          explicit_value
        })
        .to_f32();
        value
      }) <= track
    {
      i += 1;
    }
    while j > 0
      && tracks.get(j - 1).map_or(0.0, |t| {
        let Ok(value) = ({
          let Ok(explicit_value) = t.track();
          explicit_value
        })
        .to_f32();
        value
      }) >= track
    {
      j -= 1;
    }

    match i == j {
      true => {
        return tracks.get(i).map_or(0.0, get_value);
      }
      false => {}
    }

    let t0 = tracks.get(i).map_or(0.0, |t| {
      let Ok(t_track) = t.track();
      let Ok(value) = t_track.to_f32();
      value
    });
    let t1 = tracks.get(j).map_or(0.0, |t| {
      let Ok(t_track) = t.track();
      let Ok(value) = t_track.to_f32();
      value
    });
    let interp = match (t1 - t0).abs() < f32::EPSILON {
      true => 0.0,
      false => (track - t0) / (t1 - t0),
    };

    let a = tracks.get(i).map_or(0.0, get_value);
    let b = tracks.get(j).map_or(0.0, get_value);
    a + interp * (b - a)
  }
}

trait TrackEntryExt {
  fn get_value(&self, ptem: f32, sizes: &[BigEndian<Fixed>], values: &[BigEndian<i16>]) -> f32;
}

impl TrackEntryExt for TrackTableEntry {
  fn get_value(&self, ptem: f32, sizes: &[BigEndian<Fixed>], values: &[BigEndian<i16>]) -> f32 {
    let n = sizes.len().min(values.len());
    match n == 0 {
      true => {
        return 0.0;
      }
      false => {}
    }

    for i in 0..n {
      let size_pt = sizes.get(i).map_or(0.0, |f| {
        let Ok(value) = ({
          let Ok(value) = f.get();
          value
        })
        .to_f32();
        value
      });
      match size_pt >= ptem {
        true => {
          match i == 0 {
            true => {
              return values
                .first()
                .map(|v| {
                  f32::from(
                    ({
                      let Ok(value) = v.get();
                      value
                    }),
                  )
                })
                .unwrap_or_default();
            }
            false => {}
          }

          let s0 = sizes.get(i - 1).map_or(0.0, |f| {
            let Ok(value) = ({
              let Ok(value) = f.get();
              value
            })
            .to_f32();
            value
          });
          let s1 = size_pt;
          let v0 = values
            .get(i - 1)
            .map(|v| {
              f32::from(
                ({
                  let Ok(value) = v.get();
                  value
                }),
              )
            })
            .unwrap_or_default();
          let v1 = values
            .get(i)
            .map(|v| {
              f32::from(
                ({
                  let Ok(value) = v.get();
                  value
                }),
              )
            })
            .unwrap_or_default();

          match (s1 - s0).abs() < f32::EPSILON {
            true => {
              return (v0 + v1) * 0.5;
            }
            false => {}
          }

          let t = (ptem - s0) / (s1 - s0);
          return v0 + t * (v1 - v0);
        }
        false => {}
      }
    }

    values
      .get(n - 1)
      .map(|v| {
        f32::from(
          ({
            let Ok(value) = v.get();
            value
          }),
        )
      })
      .unwrap_or_default()
  }
}
