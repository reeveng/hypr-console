//! Table validation, caching and access.

use super::{super::once::Once, FontBlob, FontSource};
use crate::{
  FontRead, FontReadWithArgs, ReadError, TableProvider, TopLevelTable, tables, types::Tag,
};
use alloc::{boxed::Box, sync::Arc};
use console_core_never::Never;
use core::sync::atomic::{AtomicU8, Ordering};

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the same OpenType tags define the cache, its lookups and the public raw accessors"
  )
)]
macro_rules! font_tables {
  ($($tag:literal => $entry:ident, $accessor:ident;)*) => {
    #[derive(Clone, Default, Debug)]
    struct PerTableData<T> {
      $($entry: T,)*
    }

    impl<T> PerTableData<T> {
      fn init_all(&mut self, f: impl Fn(Tag, &mut T)) -> Result<(), Never> {
        $({
          let Ok(tag) = Tag::new($tag);
          f(tag, &mut self.$entry);
        })*

        Ok(())
      }

      fn entry(&self, tag: Tag) -> Result<Option<&T>, Never> {
        let Ok(bytes) = tag.to_be_bytes();

        Ok(match &bytes {
          $($tag => Some(&self.$entry),)*
          _ => None,
        })
      }
    }

    #[cfg_attr(
      dylint_lib = "explicit002_infallible_result",
      expect(explicit002_infallible_result, reason = "the public raw table accessors retain their optional byte-slice contract and forward to a typed table lookup")
    )]
    impl FontTables {
      $(
        #[must_use]
        pub fn $accessor(&self) -> Option<&'_ [u8]> {
          let Ok(tag) = Tag::new($tag);
          let Ok(state) = self.table_state(tag);

          state.map(|state| state.data)
        }
      )*
    }
  };
}

font_tables! {
  b"ankr" => ankr, ankr_data;
  b"avar" => avar, avar_data;
  b"BASE" => base, base_data;
  b"CFF " => cff, cff_data;
  b"CFF2" => cff2, cff2_data;
  b"CBDT" => cbdt, cbdt_data;
  b"CBLC" => cblc, cblc_data;
  b"COLR" => colr, colr_data;
  b"CPAL" => cpal, cpal_data;
  b"cmap" => cmap, cmap_data;
  b"cvar" => cvar, cvar_data;
  b"cvt " => cvt, cvt_data;
  b"DSIG" => dsig, dsig_data;
  b"EBDT" => ebdt, ebdt_data;
  b"EBLC" => eblc, eblc_data;
  b"feat" => feat, feat_data;
  b"fpgm" => fpgm, fpgm_data;
  b"fvar" => fvar, fvar_data;
  b"gasp" => gasp, gasp_data;
  b"GDEF" => gdef, gdef_data;
  b"glyf" => glyf, glyf_data;
  b"GPOS" => gpos, gpos_data;
  b"GSUB" => gsub, gsub_data;
  b"gvar" => gvar, gvar_data;
  b"hdmx" => hdmx, hdmx_data;
  b"head" => head, head_data;
  b"hhea" => hhea, hhea_data;
  b"hmtx" => hmtx, hmtx_data;
  b"HVAR" => hvar, hvar_data;
  b"IFT " => ift, ift_data;
  b"IFTX" => iftx, iftx_data;
  b"JSTF" => jstf, jstf_data;
  b"kern" => kern, kern_data;
  b"kerx" => kerx, kerx_data;
  b"loca" => loca, loca_data;
  b"ltag" => ltag, ltag_data;
  b"MATH" => math, math_data;
  b"maxp" => maxp, maxp_data;
  b"meta" => meta, meta_data;
  b"morx" => morx, morx_data;
  b"MVAR" => mvar, mvar_data;
  b"name" => name, name_data;
  b"OS/2" => os2, os2_data;
  b"post" => post, post_data;
  b"prep" => prep, prep_data;
  b"sbix" => sbix, sbix_data;
  b"STAT" => stat, stat_data;
  b"SVG " => svg, svg_data;
  b"trak" => trak, trak_data;
  b"VARC" => varc, varc_data;
  b"vhea" => vhea, vhea_data;
  b"vmtx" => vmtx, vmtx_data;
  b"VORG" => vorg, vorg_data;
  b"VVAR" => vvar, vvar_data;
}

#[derive(Default)]
struct BlobTableEntry {
  flag: AtomicU8,
  start: u32,
  end: u32,
}

struct BlobTables {
  blob: FontBlob,
  tables: Box<PerTableData<BlobTableEntry>>,
}

impl BlobTables {
  fn table_state(&self, tag: Tag) -> Result<Option<TableState<'_>>, Never> {
    let Ok(entry) = self.tables.entry(tag);
    let entry_3 = match entry {
      Some(entry_2) => entry_2,
      None => return Ok(None),
    };
    let Ok(range) =
      console_core_number_conversion::indices(u64::from(entry_3.start)..u64::from(entry_3.end));

    Ok(self.blob.get(range).map(|data| TableState {
      flag: &entry_3.flag,
      data,
    }))
  }
}

type TableFunctionEntry = Once<Option<(AtomicU8, FontBlob)>>;

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "cloned font sources share a provider callback and its once-initialized table bytes and validation flags"
  )
)]
pub struct FontTableFunction {
  table_fn: Arc<dyn Fn(Tag) -> Option<FontBlob> + Send + Sync>,
  tables: Arc<PerTableData<TableFunctionEntry>>,
}

impl FontTableFunction {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the published constructor accepts an existing provider callback and initializes the table cache shared by clones"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `FontTableFunction::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new(table_fn: Arc<dyn Fn(Tag) -> Option<FontBlob> + Send + Sync>) -> Self {
    Self {
      table_fn,
      tables: Arc::new(PerTableData::default()),
    }
  }
}

impl FontTableFunction {
  fn table_state(&self, tag: Tag) -> Result<Option<TableState<'_>>, Never> {
    let Ok(entry) = self.tables.entry(tag);
    let entry_3 = match entry {
      Some(entry_2) => entry_2,
      None => return Ok(None),
    };

    Ok(
      entry_3
        .get_or_init(|| (self.table_fn)(tag).map(|blob| (AtomicU8::new(0), blob)))
        .as_ref()
        .map(|(flag, data)| TableState {
          flag,
          data: data.as_ref(),
        }),
    )
  }
}

enum TableSource {
  None,
  Blob(BlobTables),
  Function(FontTableFunction),
}

#[derive(Copy, Clone)]
struct TableState<'a> {
  flag: &'a AtomicU8,
  data: &'a [u8],
}

impl<'a> TableState<'a> {
  const UNCHECKED: u8 = 0;
  const VALID: u8 = 1;
  const INVALID: u8 = 2;

  #[cfg_attr(
    dylint_lib = "explicit069_lock_discipline",
    expect(
      explicit069_lock_discipline,
      reason = "the Release stores publish the validation decision and the Acquire load reads it before choosing the cached path for immutable table bytes"
    )
  )]
  fn try_load<T>(
    &self,
    sanitize_fn: impl Fn(&'a [u8]) -> Result<T, ReadError>,
    load_fn: impl Fn(&'a [u8]) -> Result<T, ReadError>,
  ) -> Result<T, ReadError> {
    match self.flag.load(Ordering::Acquire) {
      Self::UNCHECKED => match sanitize_fn(self.data) {
        Ok(table) => {
          self.flag.store(Self::VALID, Ordering::Release);
          Ok(table)
        }
        Err(_) => {
          self.flag.store(Self::INVALID, Ordering::Release);
          Err(ReadError::ValidationError)
        }
      },
      Self::VALID => load_fn(self.data),

      _ => Err(ReadError::ValidationError),
    }
  }
}

pub struct FontTables(TableSource);

impl FontTables {
  pub fn new(source: impl Into<FontSource>, index: u32) -> Result<Self, ReadError> {
    let source_2 =
      <_ as ::core::convert::Into<crate::model::font::source::FontSource>>::into(source);

    match source_2 {
      FontSource::Blob(blob) => {
        let font_ref = crate::FontRef::from_index(&blob, index)?;
        let mut tables = PerTableData::default();
        let Ok(table_records) = ({
          let Ok(explicit_value) = font_ref.table_directory();
          explicit_value
        })
        .table_records();
        let Ok(()) = tables.init_all(|tag, entry: &mut BlobTableEntry| {
          entry.start = u32::MAX;
          let ::core::result::Result::Ok(index_3) = ::console_core_number_conversion::fixed_index(
            match table_records.binary_search_by_key(&tag, |record| {
              let Ok(table_tag) = record.tag();
              table_tag
            }) {
              Ok(index_2) => index_2,
              Err(_insertion_position) => return,
            },
          );
          let record =
            match table_records.get(match ::console_core_number_conversion::index(index_3) {
              ::core::result::Result::Ok(native_position) => native_position,
            }) {
              Some(record) => record,
              None => return,
            };
          let Ok(start) = record.offset();
          let Ok(length) = record.length();
          let end = match start.checked_add(length) {
            Some(end) => end,
            None => {
              return;
            }
          };
          let Ok(range) = console_core_number_conversion::indices(start..end);

          match ::core::primitive::bool::then_some(blob.get(range).is_none(), ()) {
            ::core::option::Option::Some(()) => {
              return;
            }
            ::core::option::Option::None => {}
          }

          *entry = BlobTableEntry {
            flag: AtomicU8::new(0),
            start,
            end,
          };
        });
        Ok(Self(TableSource::Blob(BlobTables {
          blob,
          tables: Box::new(tables),
        })))
      }
      FontSource::TableFunction(func) => Ok(Self(TableSource::Function(func))),
    }
  }
}

impl FontTables {
  fn table_state(&self, tag: Tag) -> Result<Option<TableState<'_>>, Never> {
    match &self.0 {
      TableSource::None => Ok(None),
      TableSource::Blob(blob) => blob.table_state(tag),
      TableSource::Function(function) => function.table_state(tag),
    }
  }

  fn load_table<'a, T: TopLevelTable + FontRead<'a>>(&'a self) -> Result<T, ReadError> {
    self.load_table_with_tag(T::TAG)
  }

  fn load_table_with_tag<'a, T: FontRead<'a>>(&'a self, tag: Tag) -> Result<T, ReadError> {
    let Ok(state) = self.table_state(tag);
    let state_2 = state.ok_or(ReadError::TableIsMissing(tag))?;
    let load = |data: &'a [u8]| FontRead::read(crate::FontData::from(data));
    state_2.try_load(load, load)
  }

  fn load_table_with_args<'a, T: TopLevelTable + FontReadWithArgs<'a>>(
    &'a self,
    args: &T::Arguments,
  ) -> Result<T, ReadError> {
    let Ok(state) = self.table_state(T::TAG);
    let state_2 = state.ok_or(ReadError::TableIsMissing(T::TAG))?;
    let load =
      |data: &'a [u8]| FontReadWithArgs::read_with_args(crate::FontData::from(data), args);
    state_2.try_load(load, load)
  }
}

pub(super) static EMPTY_FONT_TABLES: FontTables = FontTables(TableSource::None);

impl<'a> TableProvider<'a> for &'a FontTables {
  fn data_for_tag(&self, _tag: Tag) -> Result<Option<crate::FontData<'a>>, ReadError> {
    Ok(None)
  }

  fn head(&self) -> Result<tables::head::Head<'a>, ReadError> {
    self.load_table()
  }

  fn name(&self) -> Result<tables::name::Name<'a>, ReadError> {
    self.load_table()
  }

  fn hhea(&self) -> Result<tables::hhea::Hhea<'a>, ReadError> {
    self.load_table()
  }

  fn vhea(&self) -> Result<tables::vhea::Vhea<'a>, ReadError> {
    self.load_table()
  }

  fn hmtx(&self) -> Result<tables::hmtx::Hmtx<'a>, ReadError> {
    let number_of_h_metrics = self.hhea().map(|hhea| {
      let Ok(count) = hhea.number_of_h_metrics();
      count
    })?;
    self.load_table_with_args(&number_of_h_metrics)
  }

  fn hdmx(&self) -> Result<tables::hdmx::Hdmx<'a>, ReadError> {
    let num_glyphs = self.maxp().map(|maxp| {
      let Ok(count) = maxp.num_glyphs();
      count
    })?;
    self.load_table_with_args(&num_glyphs)
  }

  fn vmtx(&self) -> Result<tables::vmtx::Vmtx<'a>, ReadError> {
    let number_of_v_metrics = self.vhea().map(|vhea| {
      let Ok(count) = vhea.number_of_long_ver_metrics();
      count
    })?;
    self.load_table_with_args(&number_of_v_metrics)
  }

  fn vorg(&self) -> Result<tables::vorg::Vorg<'a>, ReadError> {
    self.load_table()
  }

  fn fvar(&self) -> Result<tables::fvar::Fvar<'a>, ReadError> {
    self.load_table()
  }

  fn avar(&self) -> Result<tables::avar::Avar<'a>, ReadError> {
    self.load_table()
  }

  fn hvar(&self) -> Result<tables::hvar::Hvar<'a>, ReadError> {
    self.load_table()
  }

  fn vvar(&self) -> Result<tables::vvar::Vvar<'a>, ReadError> {
    self.load_table()
  }

  fn mvar(&self) -> Result<tables::mvar::Mvar<'a>, ReadError> {
    self.load_table()
  }

  fn maxp(&self) -> Result<tables::maxp::Maxp<'a>, ReadError> {
    self.load_table()
  }

  fn os2(&self) -> Result<tables::os2::Os2<'a>, ReadError> {
    self.load_table()
  }

  fn post(&self) -> Result<tables::post::Post<'a>, ReadError> {
    self.load_table()
  }

  fn gasp(&self) -> Result<tables::gasp::Gasp<'a>, ReadError> {
    self.load_table()
  }

  fn loca(
    &self,
    format: impl Into<Option<tables::loca::OffsetFormat>>,
  ) -> Result<tables::loca::Loca<'a>, ReadError> {
    use tables::loca::OffsetFormat;

    let format_3 = match <_ as ::core::convert::Into<
      ::core::option::Option<crate::tables::loca::OffsetFormat>,
    >>::into(format)
    {
      Some(format_2) => format_2,
      None => {
        let head = self.head()?;

        match head.index_to_loc_format() {
          Ok(1) => OffsetFormat::Long,
          Ok(_) => OffsetFormat::Short,
        }
      }
    };

    self.load_table_with_args(&format_3)
  }

  fn glyf(&self) -> Result<tables::glyf::Glyf<'a>, ReadError> {
    self.load_table()
  }

  fn gvar(&self) -> Result<tables::gvar::Gvar<'a>, ReadError> {
    self.load_table()
  }

  fn cvt(&self) -> Result<&'a [types::BigEndian<i16>], ReadError> {
    let Ok(value_1) = Tag::new(b"cvt ");

    let cvt_data = self.cvt_data().ok_or(ReadError::TableIsMissing(value_1))?;
    let Ok(table_data) = crate::FontData::new(cvt_data);
    table_data.read_array(
      0..({
        let Ok(explicit_value) = table_data.len();
        explicit_value
      }),
    )
  }

  fn cvar(&self) -> Result<tables::cvar::Cvar<'a>, ReadError> {
    self.load_table()
  }

  fn cff(&self) -> Result<tables::cff::Cff<'a>, ReadError> {
    self.load_table()
  }

  fn cff2(&self) -> Result<tables::cff2::Cff2<'a>, ReadError> {
    self.load_table()
  }

  fn cmap(&self) -> Result<tables::cmap::Cmap<'a>, ReadError> {
    self.load_table()
  }

  fn gdef(&self) -> Result<tables::gdef::Gdef<'a>, ReadError> {
    self.load_table()
  }

  fn gpos(&self) -> Result<tables::gpos::Gpos<'a>, ReadError> {
    self.load_table()
  }

  fn gsub(&self) -> Result<tables::gsub::Gsub<'a>, ReadError> {
    self.load_table()
  }

  fn feat(&self) -> Result<tables::feat::Feat<'a>, ReadError> {
    self.load_table()
  }

  fn ltag(&self) -> Result<tables::ltag::Ltag<'a>, ReadError> {
    self.load_table()
  }

  fn ankr(&self) -> Result<tables::ankr::Ankr<'a>, ReadError> {
    self.load_table()
  }

  fn trak(&self) -> Result<tables::trak::Trak<'a>, ReadError> {
    self.load_table()
  }

  fn morx(&self) -> Result<tables::morx::Morx<'a>, ReadError> {
    self.load_table()
  }

  fn kerx(&self) -> Result<tables::kerx::Kerx<'a>, ReadError> {
    self.load_table()
  }

  fn kern(&self) -> Result<tables::kern::Kern<'a>, ReadError> {
    self.load_table()
  }

  fn colr(&self) -> Result<tables::colr::Colr<'a>, ReadError> {
    self.load_table()
  }

  fn cpal(&self) -> Result<tables::cpal::Cpal<'a>, ReadError> {
    self.load_table()
  }

  fn cblc(&self) -> Result<tables::cblc::Cblc<'a>, ReadError> {
    self.load_table()
  }

  fn cbdt(&self) -> Result<tables::cbdt::Cbdt<'a>, ReadError> {
    self.load_table()
  }

  fn eblc(&self) -> Result<tables::eblc::Eblc<'a>, ReadError> {
    self.load_table()
  }

  fn ebdt(&self) -> Result<tables::ebdt::Ebdt<'a>, ReadError> {
    self.load_table()
  }

  fn sbix(&self) -> Result<tables::sbix::Sbix<'a>, ReadError> {
    let num_glyphs = self.maxp().map(|maxp| {
      let Ok(count) = maxp.num_glyphs();
      count
    })?;
    self.load_table_with_args(&num_glyphs)
  }

  fn stat(&self) -> Result<tables::stat::Stat<'a>, ReadError> {
    self.load_table()
  }

  fn svg(&self) -> Result<tables::svg::Svg<'a>, ReadError> {
    self.load_table()
  }

  fn varc(&self) -> Result<tables::varc::Varc<'a>, ReadError> {
    self.load_table()
  }

  #[cfg(feature = "ift")]
  fn ift(&self) -> Result<tables::ift::Ift<'a>, ReadError> {
    let Ok(value_2) = Tag::new(b"IFT ");

    self.load_table_with_tag(value_2)
  }

  #[cfg(feature = "ift")]
  fn iftx(&self) -> Result<tables::ift::Ift<'a>, ReadError> {
    let Ok(value_3) = Tag::new(b"IFTX");

    self.load_table_with_tag(value_3)
  }

  fn meta(&self) -> Result<tables::meta::MetadataTable<'a>, ReadError> {
    self.load_table()
  }

  fn base(&self) -> Result<tables::base::Base<'a>, ReadError> {
    self.load_table()
  }

  fn dsig(&self) -> Result<tables::dsig::Dsig<'a>, ReadError> {
    self.load_table()
  }
}

#[cfg(test)]
mod tests {
  use crate::FontRef;

  use super::*;

  #[test]
  fn missing_tables_are_missing() -> ::core::result::Result<(), crate::read::ReadError> {
    let tables = &match FontTables::new(font_test_data::AHEM, 0) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(read_error),
    };
    assert!(matches!(tables.cbdt(), Err(ReadError::TableIsMissing(_))));
    assert!(matches!(tables.cff(), Err(ReadError::TableIsMissing(_))));
    assert!(matches!(tables.dsig(), Err(ReadError::TableIsMissing(_))));
    ::core::result::Result::Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the fixture passes an external callback to the public FontTableFunction constructor"
    )
  )]
  fn table_function_matches_font_ref() -> ::core::result::Result<(), crate::test_support::Failure>
  {
    let font =
      crate::test_support::fixture_value(FontRef::new(font_test_data::AHEM), "fixture value")?;
    let font_copy = font.clone();
    let table_fn = FontTableFunction::new(Arc::new(move |tag| {
      let table = match font_copy.data_for_tag(tag) {
        Ok(table) => table,
        Err(_unreadable_table) => return None,
      };
      table.map(|data| {
        let Ok(bytes) = data.as_bytes();
        <crate::model::font::blob::FontBlob as ::core::convert::From<
          ::std::vec::Vec<::core::primitive::u8>,
        >>::from(Vec::from(bytes))
      })
    }));
    let fixture_value =
      crate::test_support::fixture_value(FontTables::new(table_fn, 0), "fixture value")?;
    let tables = &fixture_value;

    for (data, tag) in [
      (tables.gasp_data(), b"gasp"),
      (tables.glyf_data(), b"glyf"),
      (tables.hmtx_data(), b"hmtx"),
    ] {
      let font_ref_data = {
        let font_ref_table = crate::test_support::fixture_value(
          font.data_for_tag({
            let Ok(explicit_value) = Tag::new(tag);
            explicit_value
          }),
          "fixture value",
        )?;
        font_ref_table
      }
      .map(|d| {
        let Ok(bytes) = d.as_bytes();
        bytes
      });
      assert_eq!(data, font_ref_data);
    }

    Ok(())
  }
}
