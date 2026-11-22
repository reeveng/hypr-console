use console_core_font_hinting::{
  FontProgram, HintInstance, HintOutline, Target,
  raw::{
    FontRef, TableProvider,
    tables::glyf::PointFlags,
    types::{F26Dot6, Fixed, GlyphId, Point},
  },
};

#[test]
fn bytecode_hints_caller_owned_points_without_a_scaler() -> Result<(), Box<dyn std::error::Error>>
{
  let font = FontRef::new(font_test_data::SIMPLE_GLYF)?;
  let program = FontProgram::new(&font)?;
  let head = font.head()?;
  let Ok(units_per_em) = head.units_per_em();
  const SCALE_NUMERATOR: i32 = 16 * 64;
  let Ok(numerator) = Fixed::from_bits(SCALE_NUMERATOR);
  let Ok(denominator) = Fixed::from_bits(i32::from(units_per_em));
  let mut instance = HintInstance::default();
  instance.reconfigure(
    &program,
    console_core_font_hinting::Scale {
      factor: numerator / denominator,
      ppem: 16,
    },
    Target::Mono,
    &[],
  )?;

  let unscaled = [Point::default(); 5];
  let mut scaled = [Point::default(); 5];
  let mut original = scaled;
  let mut flags = [PointFlags::default(); 5];
  let mut phantom = [Point::default(); 4];
  let Ok(mut buffers) = HintBuffers::from_program(&program);
  let Ok(glyph_id) = GlyphId::new(1);

  instance.hint(
    &program,
    &mut HintOutline {
      glyph_id,
      unscaled: &unscaled,
      scaled: &mut scaled,
      original_scaled: &mut original,
      flags: &mut flags,
      contours: &[0],
      phantom: &mut phantom,
      bytecode: &[0xb1, 0, 64, 0x38],
      stack: &mut buffers.stack,
      cvt: &mut buffers.cvt,
      storage: &mut buffers.storage,
      twilight_scaled: &mut buffers.twilight,
      twilight_original_scaled: &mut buffers.twilight_original,
      twilight_flags: &mut buffers.twilight_flags,
      glyph_kind: console_core_font_hinting::GlyphKind::Simple,
      coords: &[],
    },
    console_core_font_hinting::Validation::Pedantic,
  )?;

  let [first, untouched @ ..] = scaled;
  let [_, original_untouched @ ..] = original;
  assert_eq!(
    first,
    Point {
      x: F26Dot6::ONE,
      y: F26Dot6::ZERO
    }
  );
  assert_eq!(untouched, original_untouched);
  assert_eq!(phantom, original_untouched);
  Ok(())
}

struct HintBuffers {
  stack: Vec<i32>,
  cvt: Vec<i32>,
  storage: Vec<i32>,
  twilight: Vec<Point<F26Dot6>>,
  twilight_original: Vec<Point<F26Dot6>>,
  twilight_flags: Vec<PointFlags>,
}

impl HintBuffers {
  fn from_program(program: &FontProgram) -> Result<Self, console_core_never::Never> {
    let Ok(stack_count) = program.max_stack_elements();
    let Ok(cvt_count) = program.cvt_count();
    let Ok(storage_count) = program.storage_count();
    let Ok(twilight_count) = program.twilight_count();
    let twilight: Vec<_> = (0..twilight_count).map(|_| Point::default()).collect();

    Ok(Self {
      stack: (0..stack_count).map(|_| 0).collect(),
      cvt: (0..cvt_count).map(|_| 0).collect(),
      storage: (0..storage_count).map(|_| 0).collect(),
      twilight_original: twilight.clone(),
      twilight,
      twilight_flags: (0..twilight_count).map(|_| PointFlags::default()).collect(),
    })
  }
}
