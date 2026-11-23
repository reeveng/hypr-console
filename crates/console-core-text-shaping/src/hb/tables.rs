use core::ops::Range;

use read_fonts::{
  FontData, FontRead, FontRef, TableProvider, TopLevelTable,
  tables::{
    ankr::Ankr,
    cmap::{Cmap, CmapSubtable, LegacyEncoding, PlatformId},
    feat::Feat,
    gdef::Gdef,
    glyf::Glyf,
    gpos::Gpos,
    gsub::Gsub,
    gvar::Gvar,
    hmtx::Hmtx,
    hvar::Hvar,
    kern::Kern,
    kerx::Kerx,
    loca::{Loca, OffsetFormat},
    morx::Morx,
    mvar::Mvar,
    trak::Trak,
    vmtx::Vmtx,
    vorg::Vorg,
    vvar::Vvar,
  },
  types::Tag,
};

const WINDOWS_SYMBOL_ENCODING: u16 = 0;
const WINDOWS_UNICODE_BMP_ENCODING: u16 = 1;
const WINDOWS_UNICODE_FULL_ENCODING: u16 = 10;

const UNICODE_1_0_ENCODING: u16 = 0;
const UNICODE_1_1_ENCODING: u16 = 1;
const UNICODE_ISO_ENCODING: u16 = 2;
const UNICODE_2_0_BMP_ENCODING: u16 = 3;
const UNICODE_2_0_FULL_ENCODING: u16 = 4;

const UNICODE_FULL_ENCODING: u16 = 6;

#[derive(Clone)]
pub struct TableRanges {
  pub num_glyphs: u32,
  pub units_per_em: u16,
  pub loca_format: OffsetFormat,
  pub num_v_metrics: u16,
  pub num_h_metrics: u16,
  pub ascent: i16,
  pub descent: i16,
  pub loca: TableRange,
  pub glyf: TableRange,
  pub gvar: TableRange,
  pub hmtx: TableRange,
  pub hvar: TableRange,
  pub vmtx: TableRange,
  pub vvar: TableRange,
  pub vorg: TableRange,
  pub mvar: TableRange,
  pub cmap: TableRange,
  pub cmap_subtable: Option<SelectedCmapSubtable>,
  pub cmap_vs_subtable: Option<u16>,
  pub gdef: TableRange,
  pub gsub: TableRange,
  pub gpos: TableRange,
  pub morx: TableRange,
  pub kerx: TableRange,
  pub ankr: TableRange,
  pub kern: TableRange,
  pub feat: TableRange,
  pub trak: TableRange,
}

#[derive(Copy, Clone)]
pub struct SelectedCmapSubtable {
  pub index: u16,
  pub legacy_encoding: LegacyEncoding,
}

impl TableRanges {
  pub fn new(font: &FontRef) -> Self {
    let num_glyphs = font
      .maxp()
      .map(|maxp| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
            ({
              let Ok(value) = maxp.num_glyphs();
              value
            }),
          );
          migrated_value
        })
      })
      .unwrap_or_default();
    let (units_per_em, loca_format) = font.head().map_or((1000, OffsetFormat::Short), |head| {
      (
        {
          let Ok(value) = head.units_per_em();
          value
        },
        match head.index_to_loc_format() {
          Ok(1) => OffsetFormat::Long,
          Ok(_) => OffsetFormat::Short,
        },
      )
    });
    let os2 = font.os2().ok();
    let hhea = font.hhea().ok();
    let (ascent, descent) = match &os2 {
      Some(os2) => {
        let Ok(s_typo_ascender) = os2.s_typo_ascender();
        let Ok(s_typo_descender) = os2.s_typo_descender();

        (s_typo_ascender, s_typo_descender)
      }
      None => match &hhea {
        Some(hhea) => (
          ({
            let Ok(value) = {
              let Ok(value) = hhea.ascender();
              value
            }
            .to_i16();
            value
          }),
          ({
            let Ok(value) = ({
              let Ok(value) = hhea.descender();
              value
            })
            .to_i16();
            value
          }),
        ),
        None => (0, 0),
      },
    };
    let num_h_metrics = hhea
      .map(|hhea| {
        let Ok(count) = hhea.number_of_h_metrics();
        count
      })
      .unwrap_or_default();
    let num_v_metrics = font
      .vhea()
      .map(|vhea| {
        let Ok(count) = vhea.number_of_long_ver_metrics();
        count
      })
      .unwrap_or_default();
    let offset = |tag| TableRange::new(font, tag).unwrap_or_default();
    let loca = offset(Loca::TAG);
    let glyf = offset(Glyf::TAG);
    let gvar = offset(Gvar::TAG);
    let hmtx = offset(Hmtx::TAG);
    let hvar = offset(Hvar::TAG);
    let vmtx = offset(Vmtx::TAG);
    let vvar = offset(Vvar::TAG);
    let vorg = offset(Vorg::TAG);
    let mvar = offset(Mvar::TAG);
    let cmap = offset(Cmap::TAG);
    let cmap_table: Option<Cmap> = cmap.resolve_table(font);
    let cmap_subtable = cmap_table
      .as_ref()
      .and_then(|cmap| find_best_cmap_subtable(cmap))
      .map(|(index, platform, encoding, _)| {
        let Ok(legacy_encoding) = LegacyEncoding::classify(platform, encoding);

        SelectedCmapSubtable {
          index,
          legacy_encoding,
        }
      });
    let cmap_vs_subtable = cmap_table.and_then(|cmap| {
      let data = ({
        let Ok(data) = cmap.offset_data();
        data
      });
      let Ok(encoding_records) = cmap.encoding_records();

      encoding_records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| Some((index, record.subtable((data)).ok()?)))
        .find_map(|(index, subtable)| match subtable {
          CmapSubtable::Format14(_) => Some(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
              migrated_value
            }),
          ),
          _ => None,
        })
    });
    let gdef = offset(Gdef::TAG);
    let gsub = offset(Gsub::TAG);
    let gpos = offset(Gpos::TAG);
    let morx = offset(Morx::TAG);
    let kerx = offset(Kerx::TAG);
    let ankr = offset(Ankr::TAG);
    let kern = offset(Kern::TAG);
    let feat = offset(Feat::TAG);
    let trak = offset(Trak::TAG);
    Self {
      num_glyphs,
      units_per_em,
      loca_format,
      num_v_metrics: (num_v_metrics),
      num_h_metrics: (num_h_metrics),
      ascent,
      descent,
      loca,
      glyf,
      gvar,
      hmtx,
      hvar,
      vmtx,
      vvar,
      vorg,
      mvar,
      cmap,
      cmap_subtable,
      cmap_vs_subtable,
      gdef,
      gsub,
      gpos,
      morx,
      kerx,
      ankr,
      kern,
      feat,
      trak,
    }
  }

  pub(crate) fn from_tables<'a>(font: &impl TableProvider<'a>) -> Self {
    let num_glyphs = font
      .maxp()
      .map(|maxp| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
            ({
              let Ok(value) = maxp.num_glyphs();
              value
            }),
          );
          migrated_value
        })
      })
      .unwrap_or_default();
    let (units_per_em, loca_format) = font.head().map_or((1000, OffsetFormat::Short), |head| {
      (
        {
          let Ok(value) = head.units_per_em();
          value
        },
        match head.index_to_loc_format() {
          Ok(1) => OffsetFormat::Long,
          Ok(_) => OffsetFormat::Short,
        },
      )
    });
    let os2 = font.os2().ok();
    let hhea = font.hhea().ok();
    let (ascent, descent) = match &os2 {
      Some(os2) => {
        let Ok(s_typo_ascender) = os2.s_typo_ascender();
        let Ok(s_typo_descender) = os2.s_typo_descender();

        (s_typo_ascender, s_typo_descender)
      }
      None => match &hhea {
        Some(hhea) => (
          ({
            let Ok(value) = {
              let Ok(value) = hhea.ascender();
              value
            }
            .to_i16();
            value
          }),
          ({
            let Ok(value) = ({
              let Ok(value) = hhea.descender();
              value
            })
            .to_i16();
            value
          }),
        ),
        None => (0, 0),
      },
    };
    let num_h_metrics = hhea
      .map(|hhea| {
        let Ok(count) = hhea.number_of_h_metrics();
        count
      })
      .unwrap_or_default();
    let num_v_metrics = font
      .vhea()
      .map(|vhea| {
        let Ok(count) = vhea.number_of_long_ver_metrics();
        count
      })
      .unwrap_or_default();
    Self {
      num_glyphs,
      units_per_em,
      loca_format,
      num_v_metrics: (num_v_metrics),
      num_h_metrics: (num_h_metrics),
      ascent,
      descent,
      loca: TableRange::default(),
      glyf: TableRange::default(),
      gvar: TableRange::default(),
      hmtx: TableRange::default(),
      hvar: TableRange::default(),
      vmtx: TableRange::default(),
      vvar: TableRange::default(),
      vorg: TableRange::default(),
      mvar: TableRange::default(),
      cmap: TableRange::default(),
      cmap_subtable: None,
      cmap_vs_subtable: None,
      gdef: TableRange::default(),
      gsub: TableRange::default(),
      gpos: TableRange::default(),
      morx: TableRange::default(),
      kerx: TableRange::default(),
      ankr: TableRange::default(),
      kern: TableRange::default(),
      feat: TableRange::default(),
      trak: TableRange::default(),
    }
  }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct TableRange(u32, u32);

impl TableRange {
  fn new(font: &FontRef, tag: Tag) -> Option<Self> {
    let records = ({
      let Ok(value) = font.table_directory();
      value
    })
    .table_records();
    ({
      let Ok(value) = records;
      value
    })
    .binary_search_by_key(&tag, |rec| {
      let Ok(value) = rec.tag();
      value
    })
    .ok()
    .and_then(|ix| {
      ({
        let Ok(value) = records;
        value
      })
      .get(ix)
    })
    .map(|rec| {
      let Ok(offset) = rec.offset();
      let Ok(length) = rec.length();
      Self(offset, length)
    })
  }

  pub fn resolve(self) -> Option<Range<usize>> {
    let start = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.0);
      migrated_value
    });
    (start != 0).then_some(
      start
        ..start.wrapping_add(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.1);
            migrated_value
          }),
        ),
    )
  }

  pub fn resolve_data<'a>(self, font: &FontRef<'a>) -> Option<FontData<'a>> {
    {
      let Ok(value) = ({
        let Ok(value) = font.data();
        value
      })
      .slice({
        let range = self.resolve()?;
        let Ok(start) = console_core_number_conversion::fitted::<_, u64>(range.start);
        let Ok(end) = console_core_number_conversion::fitted::<_, u64>(range.end);
        start..end
      });
      value
    }
  }

  pub fn resolve_table<'a, T: FontRead<'a>>(self, font: &FontRef<'a>) -> Option<T> {
    T::read(self.resolve_data(font)?).ok()
  }

  pub fn len(self) -> u32 {
    self.1
  }
}

fn find_best_cmap_subtable<'a>(
  cmap: &Cmap<'a>,
) -> Option<(u16, PlatformId, u16, CmapSubtable<'a>)> {
  find_cmap_subtable(cmap, PlatformId::Windows, WINDOWS_SYMBOL_ENCODING)
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Windows, WINDOWS_UNICODE_FULL_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Unicode, UNICODE_FULL_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Unicode, UNICODE_2_0_FULL_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Windows, WINDOWS_UNICODE_BMP_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Unicode, UNICODE_2_0_BMP_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Unicode, UNICODE_ISO_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Unicode, UNICODE_1_1_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Unicode, UNICODE_1_0_ENCODING))
    .or_else(|| find_cmap_subtable(cmap, PlatformId::Macintosh, 0))
}

fn find_cmap_subtable<'a>(
  cmap: &Cmap<'a>,
  platform_id: PlatformId,
  encoding_id: u16,
) -> Option<(u16, PlatformId, u16, CmapSubtable<'a>)> {
  let offset_data = ({
    let Ok(data) = cmap.offset_data();
    data
  });
  let Ok(encoding_records) = cmap.encoding_records();

  for (index, record) in encoding_records.iter().enumerate() {
    let Ok(record_platform_id) = record.platform_id();

    match record_platform_id != platform_id
      || ({
        let Ok(explicit_value) = record.encoding_id();
        explicit_value
      }) != encoding_id
    {
      true => {
        continue;
      }
      false => {}
    }
    match record.subtable((offset_data)) {
      Ok(subtable) => match subtable {
        CmapSubtable::Format0(_)
        | CmapSubtable::Format4(_)
        | CmapSubtable::Format6(_)
        | CmapSubtable::Format10(_)
        | CmapSubtable::Format12(_)
        | CmapSubtable::Format13(_) => {
          return Some((
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
              migrated_value
            }),
            platform_id,
            encoding_id,
            subtable,
          ));
        }
        _ => {}
      },
      Err(_) => {}
    }
  }
  None
}
