use crate::{
  GlyphExtents, GlyphInfo, GlyphPosition, Tag,
  hb::{
    face::{BasicFontMetrics, Scale},
    tables::TableRanges,
  },
};
use read_fonts::{
  FontRef, TableProvider,
  tables::{
    glyf::Glyf,
    gvar::Gvar,
    hmtx::{Hmtx, LongMetric},
    hvar::Hvar,
    loca::Loca,
    mvar::Mvar,
    vmtx::Vmtx,
    vorg::Vorg,
    vvar::Vvar,
  },
  types::{BoundingBox, F2Dot14, Fixed, GlyphId, Point},
};

#[derive(Clone)]
pub struct GlyphMetrics<'a> {
  _hmtx: Option<Hmtx<'a>>,
  h_metrics: &'a [LongMetric],
  hvar: Option<Hvar<'a>>,
  vmtx: Option<Vmtx<'a>>,
  vvar: Option<Vvar<'a>>,
  vorg: Option<Vorg<'a>>,
  glyf: Option<GlyfTables<'a>>,
  mvar: Option<Mvar<'a>>,
  num_glyphs: u32,
  upem: u16,
  ascent: i16,
  descent: i16,
}

#[derive(Clone)]
struct GlyfTables<'a> {
  loca: Loca<'a>,
  glyf: Glyf<'a>,
  gvar: Option<Gvar<'a>>,
}

impl<'a> GlyphMetrics<'a> {
  pub(crate) fn new(font: &FontRef<'a>, table_ranges: &TableRanges) -> Self {
    let num_glyphs = table_ranges.num_glyphs;
    let upem = table_ranges.units_per_em;
    let hmtx = table_ranges
      .hmtx
      .resolve_data(font)
      .and_then(|data| Hmtx::read(data, table_ranges.num_h_metrics).ok());
    let h_metrics = hmtx
      .as_ref()
      .map(|hmtx| {
        ({
          let Ok(value) = hmtx.h_metrics();
          value
        })
      })
      .unwrap_or_default();
    let hvar = table_ranges.hvar.resolve_table(font);
    let vmtx = table_ranges
      .vmtx
      .resolve_data(font)
      .and_then(|data| Vmtx::read(data, table_ranges.num_v_metrics).ok());
    let vvar = table_ranges.vvar.resolve_table(font);
    let vorg = table_ranges.vorg.resolve_table(font);
    let loca = table_ranges
      .loca
      .resolve_data(font)
      .and_then(|data| Loca::read(data, table_ranges.loca_format).ok());
    let glyf = table_ranges.glyf.resolve_table(font);
    let glyf = if let Some((loca, glyf)) = loca.zip(glyf) {
      let gvar = table_ranges.gvar.resolve_table(font);
      Some(GlyfTables { loca, glyf, gvar })
    } else {
      None
    };
    let mvar = table_ranges.mvar.resolve_table(font);
    let ascent = table_ranges.ascent;
    let descent = table_ranges.descent;
    Self {
      _hmtx: hmtx,
      h_metrics: (h_metrics),
      hvar,
      vmtx,
      vvar,
      vorg,
      glyf,
      mvar,
      num_glyphs,
      upem,
      ascent,
      descent,
    }
  }

  pub(crate) fn from_tables(font: &impl TableProvider<'a>, metrics: &BasicFontMetrics) -> Self {
    let hmtx = font.hmtx().ok();
    let h_metrics = hmtx
      .as_ref()
      .map(|hmtx| {
        ({
          let Ok(value) = hmtx.h_metrics();
          value
        })
      })
      .unwrap_or_default();
    let hvar = font.hvar().ok();
    let vmtx = font.vmtx().ok();
    let vvar = font.vvar().ok();
    let vorg = font.vorg().ok();
    let loca = font.loca(None).ok();
    let glyf = font.glyf().ok();
    let glyf = if let Some((loca, glyf)) = loca.zip(glyf) {
      let gvar = font.gvar().ok();
      Some(GlyfTables { loca, glyf, gvar })
    } else {
      None
    };
    let mvar = font.mvar().ok();
    Self {
      _hmtx: hmtx,
      h_metrics: (h_metrics),
      hvar,
      vmtx,
      vvar,
      vorg,
      glyf,
      mvar,
      num_glyphs: metrics.num_glyphs,
      upem: metrics.units_per_em,
      ascent: metrics.ascent,
      descent: metrics.descent,
    }
  }

  pub(crate) fn advance_width(&self, gid: impl Into<GlyphId>, coords: &[F2Dot14]) -> Option<i32> {
    let gid = gid.into();
    let mut advance = match self
      .h_metrics
      .get(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index({
            let Ok(value) = gid.to_u32();
            value
          });
          migrated_value
        }),
      )
      .or_else(|| self.h_metrics.last())
      .map(|metric| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
            ({
              let Ok(value) = metric.advance();
              value
            }),
          );
          migrated_value
        })
      }) {
      Some(mut advance) => advance,
      None => {
        return ({
          let Ok(value) = gid.to_u32();
          value
        } < self.num_glyphs)
          .then_some(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(self.upem);
              migrated_value
            }) / 2,
          );
      }
    };
    match !coords.is_empty() {
      true => match self.hvar.as_ref() {
        Some(hvar) => {
          advance += ({
            let Ok(value) = hvar
              .advance_width_delta(gid, coords)
              .unwrap_or_default()
              .to_i32();
            value
          });
        }
        None => match self.phantom_deltas(gid, coords) {
          Some(deltas) => {
            advance += ({
              let Ok(value) = deltas[1].x.to_i32();
              value
            }) - ({
              let Ok(value) = deltas[0].x.to_i32();
              value
            });
          }
          None => {}
        },
      },
      false => {}
    }
    Some(advance)
  }

  pub(crate) fn populate_advance_widths(
    &self,
    infos: &[GlyphInfo],
    pos: &mut [GlyphPosition],
    coords: &[F2Dot14],
    scale: Scale,
  ) {
    for (info, pos) in infos.iter().zip(pos.iter_mut()) {
      pos.x_advance = self
        .h_metrics
        .get(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(info.glyph_id);
            migrated_value
          }),
        )
        .or_else(|| self.h_metrics.last())
        .map(|metric| {
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
              ({
                let Ok(value) = metric.advance();
                value
              }),
            );
            migrated_value
          })
        })
        .or_else(|| {
          (info.glyph_id < self.num_glyphs).then_some(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, i32>(self.upem);
              migrated_value
            }) / 2,
          )
        })
        .unwrap_or_default();
    }
    match !coords.is_empty() {
      true => match self.hvar.as_ref() {
        Some(hvar) => {
          for (info, pos) in infos.iter().zip(pos.iter_mut()) {
            pos.x_advance += ({
              let Ok(value) = hvar
                .advance_width_delta(info.as_glyph(), coords)
                .unwrap_or_default()
                .to_i32();
              value
            });
          }
        }
        None => {
          for (info, pos) in infos.iter().zip(pos.iter_mut()) {
            match self.phantom_deltas(info.as_glyph(), coords) {
              Some(deltas) => {
                pos.x_advance += ({
                  let Ok(value) = deltas[1].x.to_i32();
                  value
                }) - ({
                  let Ok(value) = deltas[0].x.to_i32();
                  value
                });
              }
              None => {}
            }
          }
        }
      },
      false => {}
    }
    for pos in pos.iter_mut() {
      pos.x_advance = scale.scale_x(pos.x_advance);
    }
  }

  pub(crate) fn _left_side_bearing(
    &self,
    gid: impl Into<GlyphId>,
    coords: &[F2Dot14],
  ) -> Option<i32> {
    let gid = gid.into();
    let mut bearing = match self._hmtx.as_ref() {
      Some(hmtx) => {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
            hmtx
              .side_bearing(gid)
              .unwrap_or_default()
              .unwrap_or_default(),
          );
          migrated_value
        })
      }
      None => match self.bounds(gid, coords) {
        Some(extents) => {
          return Some(extents.x_min);
        }
        None => {
          return None;
        }
      },
    };
    match !coords.is_empty() {
      true => match self.hvar.as_ref() {
        Some(hvar) => {
          bearing += ({
            let Ok(value) = hvar.lsb_delta(gid, coords).unwrap_or_default().to_i32();
            value
          });
        }
        None => match self.phantom_deltas(gid, coords) {
          Some(deltas) => {
            bearing += ({
              let Ok(value) = deltas[0].x.to_i32();
              value
            });
          }
          None => {}
        },
      },
      false => {}
    }
    Some(bearing)
  }

  pub(crate) fn advance_height(
    &self,
    gid: impl Into<GlyphId>,
    coords: &[F2Dot14],
  ) -> Option<i32> {
    let gid = gid.into();
    let mut advance = match self
      .vmtx
      .as_ref()
      .and_then(|vmtx| {
        let Ok(value) = vmtx.advance(gid);
        value
      })
      .map(|advance| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(advance);
          migrated_value
        })
      }) {
      Some(mut advance) => advance,
      None => {
        return Some(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(self.ascent);
            migrated_value
          }) - ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(self.descent);
            migrated_value
          }),
        );
      }
    };
    match !coords.is_empty() {
      true => match self.vvar.as_ref() {
        Some(vvar) => {
          advance += ({
            let Ok(value) = vvar
              .advance_height_delta(gid, coords)
              .unwrap_or_default()
              .to_i32();
            value
          });
        }
        None => match self.phantom_deltas(gid, coords) {
          Some(deltas) => {
            advance += ({
              let Ok(value) = deltas[3].y.to_i32();
              value
            }) - ({
              let Ok(value) = deltas[2].y.to_i32();
              value
            });
          }
          None => {}
        },
      },
      false => {}
    }
    Some(advance)
  }

  pub(crate) fn top_side_bearing(
    &self,
    gid: impl Into<GlyphId>,
    coords: &[F2Dot14],
  ) -> Option<i32> {
    let gid = gid.into();
    let mut bearing = match self.vmtx.as_ref() {
      Some(vmtx) => {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
            vmtx
              .side_bearing(gid)
              .unwrap_or_default()
              .unwrap_or_default(),
          );
          migrated_value
        })
      }
      None => {
        return None;
      }
    };
    match !coords.is_empty() {
      true => match self.vvar.as_ref() {
        Some(vvar) => {
          bearing += ({
            let Ok(value) = vvar.tsb_delta(gid, coords).unwrap_or_default().to_i32();
            value
          });
        }
        None => match self.phantom_deltas(gid, coords) {
          Some(deltas) => {
            bearing += ({
              let Ok(value) = deltas[3].y.to_i32();
              value
            });
          }
          None => {}
        },
      },
      false => {}
    }
    Some(bearing)
  }

  pub(crate) fn v_origin(&self, gid: impl Into<GlyphId>, coords: &[F2Dot14]) -> Option<i32> {
    let gid = gid.into();
    let origin = match self.vorg.as_ref() {
      Some(vorg) => {
        let mut origin = ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, i32>(vorg.vertical_origin_y(gid));
          migrated_value
        });
        match !coords.is_empty() {
          true => match self.vvar.as_ref() {
            Some(vvar) => {
              origin += ({
                let Ok(value) = vvar.v_org_delta(gid, coords).unwrap_or_default().to_i32();
                value
              });
            }
            None => {}
          },
          false => {}
        }
        origin
      }
      None => match self.bounds(gid, coords) {
        Some(extents) => {
          let origin = match self.vmtx.is_some() {
            true => {
              let mut origin = Some(extents.y_max);
              let tsb = self.top_side_bearing(gid, coords);
              match tsb {
                Some(tsb) => {
                  origin = Some(origin.unwrap() + tsb);
                }
                None => {
                  origin = None;
                }
              }
              match origin.is_some() && !coords.is_empty() {
                true => match self.vvar.as_ref() {
                  Some(vvar) => {
                    origin = Some(
                      origin.unwrap()
                        + ({
                          let Ok(value) =
                            vvar.v_org_delta(gid, coords).unwrap_or_default().to_i32();
                          value
                        }),
                    );
                  }
                  None => {}
                },
                false => {}
              }
              origin
            }
            false => None,
          };

          match origin {
            Some(origin) => origin,
            None => {
              let mut advance = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(self.ascent);
                migrated_value
              }) - ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(self.descent);
                migrated_value
              });
              match self.mvar.as_ref() {
                Some(mvar) => {
                  advance += {
                    let Ok(value) = mvar
                      .metric_delta(
                        {
                          let Ok(value) = Tag::new(b"hasc");
                          value
                        },
                        coords,
                      )
                      .unwrap_or_default()
                      .to_i32();
                    value
                  } - {
                    let Ok(value) = mvar
                      .metric_delta(
                        {
                          let Ok(value) = Tag::new(b"hdsc");
                          value
                        },
                        coords,
                      )
                      .unwrap_or_default()
                      .to_i32();
                    value
                  };
                }
                None => {}
              }
              let diff = advance - (extents.y_max - extents.y_min);
              extents.y_max + (diff >> 1)
            }
          }
        }
        None => {
          let mut ascent = ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(self.ascent);
            migrated_value
          });
          match self.mvar.as_ref() {
            Some(mvar) => {
              ascent += ({
                let Ok(value) = mvar
                  .metric_delta(
                    {
                      let Ok(value) = Tag::new(b"hasc");
                      value
                    },
                    coords,
                  )
                  .unwrap_or_default()
                  .to_i32();
                value
              });
            }
            None => {}
          }
          ascent
        }
      },
    };
    Some(origin)
  }

  fn bounds(&self, gid: impl Into<GlyphId>, coords: &[F2Dot14]) -> Option<BoundingBox<i32>> {
    let gid = gid.into();
    let glyf = self.glyf.as_ref()?;
    let glyph = glyf.loca.get_glyf(gid, &glyf.glyf).ok()?;
    let glyph = match glyph {
      Some(glyph) => glyph,
      None => {
        return Some(BoundingBox::default());
      }
    };
    match !coords.is_empty() {
      true => {
        return None;
      }
      false => {}
    }
    Some(BoundingBox {
      x_min: ({
        let Ok(x_min) = glyph.x_min();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x_min);
        migrated_value
      }),
      y_min: ({
        let Ok(y_min) = glyph.y_min();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y_min);
        migrated_value
      }),
      x_max: ({
        let Ok(x_max) = glyph.x_max();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x_max);
        migrated_value
      }),
      y_max: ({
        let Ok(y_max) = glyph.y_max();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y_max);
        migrated_value
      }),
    })
  }

  pub(crate) fn extents(
    &self,
    gid: impl Into<GlyphId>,
    coords: &[F2Dot14],
  ) -> Option<GlyphExtents> {
    let gid = gid.into();
    let glyf = self.glyf.as_ref()?;
    let glyph = glyf.loca.get_glyf(gid, &glyf.glyf).ok()?;
    let glyph = match glyph {
      Some(glyph) => glyph,
      None => {
        return Some(GlyphExtents::default());
      }
    };
    match !coords.is_empty() {
      true => {
        return None;
      }
      false => {}
    }
    Some(GlyphExtents {
      x_bearing: ({
        let Ok(x_min) = glyph.x_min();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x_min);
        migrated_value
      }),
      y_bearing: ({
        let Ok(y_max) = glyph.y_max();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y_max);
        migrated_value
      }),
      width: ({
        let Ok(x_max) = glyph.x_max();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x_max);
        migrated_value
      }) - ({
        let Ok(x_min) = glyph.x_min();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x_min);
        migrated_value
      }),
      height: ({
        let Ok(y_min) = glyph.y_min();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y_min);
        migrated_value
      }) - ({
        let Ok(y_max) = glyph.y_max();
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y_max);
        migrated_value
      }),
    })
  }

  fn phantom_deltas(&self, gid: GlyphId, coords: &[F2Dot14]) -> Option<[Point<Fixed>; 4]> {
    let glyf = self.glyf.as_ref()?;
    let gvar = glyf.gvar.as_ref()?;
    gvar
      .phantom_point_deltas(&glyf.glyf, &glyf.loca, coords, gid)
      .ok()?
  }
}
