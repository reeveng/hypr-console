use console_core_text_layout::{
  Align, Attrs, AttrsList, BidiParagraphs, Buffer, Family, FontSize, FontSystem, Hinting,
  Metrics, ShapeLine, Shaping, Weight, Wrap, fontdb,
};

#[test]
fn stable_wrap() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(family) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("FiraMono"));
  let Ok(weight) = family.weight(Weight::MEDIUM);
  let Ok(attrs) = AttrsList::new(&weight);
  let Ok(mut font_system) = FontSystem::new_with_locale_and_db(String::from("en-US"), {
    let Ok(adapted) = fontdb::Database::new();
    adapted
  });
  let font = match std::fs::read("fonts/FiraMono-Medium.ttf") {
    Ok(value) => value,
    Err(error) => return Err(Box::new(error)),
  };
  let Ok(db_mut) = font_system.db_mut();
  let Ok(()) = db_mut.load_font_data(font);

  let hello_sample = match std::fs::read_to_string("sample/hello.txt") {
    Ok(value) => value,
    Err(error) => return Err(Box::new(error)),
  };
  let cases = [
    "(6)  SomewhatBoringDisplayTransform",
    "",
    " ",
    "  ",
    "   ",
    "       ",
  ]
  .into_iter()
  .chain(match BidiParagraphs::new(&hello_sample) {
    Ok(value) => value,
  });

  for text in cases {
    let Ok(()) = check_variants(&mut font_system, text, &attrs);
  }

  Ok(())
}

fn check_variants(
  fonts: &mut FontSystem,
  text: &str,
  attrs: &AttrsList,
) -> Result<(), console_core_never::Never> {
  let long_suffix = format!("{text}            ");
  let short_suffix = format!("{text}    ");

  for wrap in [Wrap::None, Wrap::Glyph, Wrap::Word, Wrap::WordOrGlyph] {
    for align_opt in [
      None,
      Some(Align::Left),
      Some(Align::Right),
      Some(Align::Center),
      Some(Align::End),
    ] {
      for start_width_opt in [
        None,
        Some(f32::MAX),
        Some(80.0),
        Some(198.2132),
        Some(20.0),
        Some(4.0),
        Some(300.0),
      ] {
        for sample in [text, long_suffix.as_str(), short_suffix.as_str()] {
          let Ok(()) = check_wrap(
            fonts,
            sample,
            attrs,
            WrapCase {
              font_size: FontSize(18.0),
              wrap,
              align_opt,
              start_width_opt,
            },
          );
        }
      }
    }
  }

  Ok(())
}

#[test]
fn wrap_extra_line() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(mut font_system) = FontSystem::new();
  let Ok(metrics) = Metrics::new(14.0, console_core_text_layout::LineHeight(20.0));

  let Ok(mut buffer) = Buffer::new(&mut font_system, metrics);

  let Ok(mut buffer_2) = buffer.borrow_with(&mut font_system);

  let Ok(()) = buffer_2.set_wrap(Wrap::Word);

  let Ok(()) = buffer_2.set_size(Some(50.0), Some(1000.0));

  let Ok(attrs) = Attrs::new();
  let Ok(family) = attrs.family(console_core_text_layout::Family::Name("Inter"));

  let Ok(()) = buffer_2.set_text("Lorem ipsum dolor sit amet, qui minim labore adipisicing\n\nweeewoooo minim sint cillum sint consectetur cupidatat.", &family, Shaping::Advanced, None);

  let Ok(layout_runs) = buffer_2.layout_runs();
  let Ok(empty_lines) = console_core_number_conversion::fitted::<_, u32>(
    layout_runs
      .filter(|x| {
        ::core::matches!(
          ::core::primitive::f32::classify(x.line_w),
          ::core::num::FpCategory::Zero
        )
      })
      .count(),
  );
  let Ok(buffer_2_layout_runs) = buffer_2.layout_runs();
  let Ok(overflow_lines) = console_core_number_conversion::fitted::<_, u32>(
    buffer_2_layout_runs.filter(|x| x.line_w > 50.).count(),
  );

  assert_eq!(empty_lines, 1);
  assert_eq!(overflow_lines, 4);
  Ok(())
}

#[derive(Debug)]
struct WrapCase {
  font_size: FontSize,
  wrap: Wrap,
  align_opt: Option<Align>,
  start_width_opt: Option<f32>,
}

fn check_wrap(
  font_system: &mut FontSystem,
  text: &str,
  attrs: &AttrsList,
  case: WrapCase,
) -> Result<(), console_core_never::Never> {
  let WrapCase {
    font_size,
    wrap,
    align_opt,
    start_width_opt,
  } = case;

  let Ok(line) = ShapeLine::new(font_system, text, attrs, Shaping::Advanced, 8);

  let Ok(layout_unbounded) = line.layout(
    font_size,
    start_width_opt,
    wrap,
    align_opt,
    None,
    Hinting::Disabled,
  );
  let max_width = layout_unbounded.iter().map(|l| l.w).fold(0.0, f32::max);
  let new_limit = match start_width_opt {
    Some(start_width) => f32::min(start_width, max_width),
    None => max_width,
  };

  let Ok(layout_bounded) = line.layout(
    font_size,
    Some(new_limit),
    wrap,
    align_opt,
    None,
    Hinting::Disabled,
  );
  let bounded_max_width = layout_bounded.iter().map(|l| l.w).fold(0.0, f32::max);

  assert_eq!(
    (max_width, layout_unbounded.len()),
    (bounded_max_width, layout_bounded.len()),
    "Wrap \"{wrap:?}\" and align \"{align_opt:?}\" with text: \"{text}\"",
  );

  for (u, b) in layout_unbounded
    .iter()
    .skip(1)
    .zip(layout_bounded.iter().skip(1))
  {
    assert_eq!(
      u.w, b.w,
      "Wrap {wrap:?} and align \"{align_opt:?}\" with text: \"{text}\"",
    );
  }

  Ok(())
}
