use console_core_text_layout::{Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};

#[test]
fn wrap_word_fallback() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(mut font_system) = FontSystem::new_with_locale_and_db(String::from("en-US"), {
    let Ok(adapted) = fontdb::Database::new();
    adapted
  });
  let font = match std::fs::read("fonts/Inter-Regular.ttf") {
    Ok(value) => value,
    Err(error) => return Err(Box::new(error)),
  };
  let Ok(db_mut) = font_system.db_mut();
  let Ok(()) = db_mut.load_font_data(font);
  let Ok(metrics) = Metrics::new(14.0, console_core_text_layout::LineHeight(20.0));

  let Ok(mut buffer) = Buffer::new(&mut font_system, metrics);

  let Ok(mut layout) = buffer.borrow_with(&mut font_system);

  let Ok(()) = layout.set_wrap(Wrap::WordOrGlyph);

  let Ok(()) = layout.set_size(Some(50.0), Some(1000.0));

  let Ok(attrs) = Attrs::new();
  let Ok(family) = attrs.family(console_core_text_layout::Family::Name("Inter"));

  let Ok(()) = layout.set_text("Lorem ipsum dolor sit amet, qui minim labore adipisicing minim sint cillum sint consectetur cupidatat.", &family, Shaping::Advanced, None);

  let Ok(layout_runs) = layout.layout_runs();
  let measured_size = layout_runs.fold(0.0f32, |width, run| width.max(run.line_w));

  let Ok(size) = layout.size();
  assert!(
    measured_size
      <= (match size.0 {
        Some(value) => value,
        None => 0.0,
      }),
    "Measured width is larger than buffer width\n{} <= {}",
    measured_size,
    (match match layout.size() {
      Ok(value) => value,
    }
    .0
    {
      Some(value) => value,
      None => 0.0,
    })
  );
  Ok(())
}
