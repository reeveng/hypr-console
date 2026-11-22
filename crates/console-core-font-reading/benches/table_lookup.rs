//! Benchmark that simulates loading tables required for outline extraction.

use console_core_font_reading::{
  FontRef, ReadError, TableProvider,
  model::Font,
  tables::{
    cff::Cff, cff2::Cff2, cvar::Cvar, glyf::Glyf, gvar::Gvar, hdmx::Hdmx, hmtx::Hmtx, hvar::Hvar,
    loca::Loca, maxp::Maxp,
  },
};
use core::hint::black_box;
use criterion::Criterion;
use console_core_never::Never;
use font_types::BigEndian;

const TEST_FONTS: &[(&str, &[u8])] = &[
  ("glyf", font_test_data::TINOS_SUBSET),
  ("glyf+gvar", font_test_data::AMSTELVAR_AVAR2_A),
  ("cff", font_test_data::NOTO_SERIF_DISPLAY_TRIMMED),
  ("cff2", font_test_data::CANTARELL_VF_TRIMMED),
];

fn main() -> Result<(), ReadError> {
  let mut criterion = Criterion::default().configure_from_args();

  for (name, font_data) in TEST_FONTS {
    let tables = FontRef::from_index(font_data, 0)?;
    let label = format!("FontRef {name} outline tables");
    benchmark_outline_tables(&mut criterion, &label, &tables)?;
  }

  for (name, font_data) in TEST_FONTS {
    let font = Font::new(*font_data, 0)?;
    let tables = font.tables();
    let label = format!("Font {name} outline tables");
    benchmark_outline_tables(&mut criterion, &label, &tables)?;
  }

  criterion.final_summary();

  Ok(())
}

fn benchmark_outline_tables<'a>(
  criterion: &mut Criterion,
  label: &str,
  tables: &impl TableProvider<'a>,
) -> Result<(), ReadError> {
  let _validated_fixture = OutlineTables::new(tables)?;
  let _registered_case = criterion.bench_function(label, |bencher| {
    bencher.iter(|| black_box(OutlineTables::new(tables)))
  });

  Ok(())
}

#[expect(
  dead_code,
  reason = "the benchmark measures constructing complete table sets, so their fields are retained without being read"
)]
struct GlyfTables<'a> {
  loca: Loca<'a>,
  glyf: Glyf<'a>,
  gvar: Option<Gvar<'a>>,
  cvt: Option<&'a [BigEndian<i16>]>,
  cvar: Option<Cvar<'a>>,
  hdmx: Option<Hdmx<'a>>,
}

#[expect(
  dead_code,
  reason = "the benchmark measures constructing complete table sets, so their fields are retained without being read"
)]
enum OutlineFormatTables<'a> {
  None,
  Glyf(GlyfTables<'a>),
  Cff(Cff<'a>),
  Cff2(Cff2<'a>),
}

#[expect(
  dead_code,
  reason = "the benchmark measures constructing complete table sets, so their fields are retained without being read"
)]
struct OutlineTables<'a> {
  maxp: Maxp<'a>,
  hmtx: Hmtx<'a>,
  hvar: Option<Hvar<'a>>,
  format: OutlineFormatTables<'a>,
}

impl<'a> OutlineTables<'a> {
  #[inline(never)]
  fn new(tables: &impl TableProvider<'a>) -> Result<Self, ReadError> {
    let maxp = tables.maxp()?;
    let hmtx = tables.hmtx()?;
    let Ok(hvar) = optional_table(tables.hvar());
    let format = match tables.glyf() {
      Ok(glyf) => {
        let loca = tables.loca(None)?;
        let Ok(gvar) = optional_table(tables.gvar());
        let Ok(cvt) = optional_table(tables.cvt());
        let Ok(cvar) = optional_table(tables.cvar());
        let Ok(hdmx) = optional_table(tables.hdmx());

        OutlineFormatTables::Glyf(GlyfTables {
          loca,
          glyf,
          gvar,
          cvt,
          cvar,
          hdmx,
        })
      }
      Err(_unavailable_glyph_outlines) => match tables.cff2() {
        Ok(cff2) => OutlineFormatTables::Cff2(cff2),
        Err(_unavailable_cff2_table) => match tables.cff() {
          Ok(cff) => OutlineFormatTables::Cff(cff),
          Err(_unavailable_cff_table) => OutlineFormatTables::None,
        },
      },
    };

    Ok(Self { maxp, hmtx, hvar, format })
  }
}

fn optional_table<T>(read: Result<T, ReadError>) -> Result<Option<T>, Never> {
  Ok(match read {
    Ok(table) => Some(table),
    Err(_unavailable_optional_table) => None,
  })
}
