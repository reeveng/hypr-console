use common::DrawTestCfg;
use console_core_text_layout::Attrs;
use fontdb::Family;

mod common;

use console_core_geometry::Size;

#[test]
fn test_hebrew_word_rendering() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("a_hebrew_word");
  let Ok(font_size) = draw_test_cfg.font_size(36., console_core_text_layout::LineHeight(40.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("בדיקה");
  let Ok(canvas) = text.canvas(Size {
    width: 120,
    height: 60,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_hebrew_paragraph_rendering() -> Result<(), Box<dyn std::error::Error>> {
  let paragraph = "השועל החום המהיר קופץ מעל הכלב העצלן";
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("a_hebrew_paragraph");
  let Ok(font_size) = draw_test_cfg.font_size(36., console_core_text_layout::LineHeight(40.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text(paragraph);
  let Ok(canvas) = text.canvas(Size {
    width: 400,
    height: 110,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_english_mixed_with_hebrew_paragraph_rendering() -> Result<(), Box<dyn std::error::Error>>
{
  let paragraph = "Many computer programs fail to display bidirectional text correctly. For example, this page is mostly LTR English script, and here is the RTL Hebrew name Sarah: שרה, spelled sin (ש) on the right, resh (ר) in the middle, and heh (ה) on the left.";
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("some_english_mixed_with_hebrew");
  let Ok(font_size) = draw_test_cfg.font_size(16., console_core_text_layout::LineHeight(20.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text(paragraph);
  let Ok(canvas) = text.canvas(Size {
    width: 400,
    height: 120,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_arabic_word_rendering() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("an_arabic_word");
  let Ok(font_size) = draw_test_cfg.font_size(36., console_core_text_layout::LineHeight(40.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("خالصة");
  let Ok(canvas) = text.canvas(Size {
    width: 120,
    height: 60,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_arabic_paragraph_rendering() -> Result<(), Box<dyn std::error::Error>> {
  let paragraph = "الثعلب البني السريع يقفز فوق الكلب الكسول";
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("an_arabic_paragraph");
  let Ok(font_size) = draw_test_cfg.font_size(36., console_core_text_layout::LineHeight(40.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text(paragraph);
  let Ok(canvas) = text.canvas(Size {
    width: 400,
    height: 110,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_english_mixed_with_arabic_paragraph_rendering() -> Result<(), Box<dyn std::error::Error>>
{
  let paragraph = "I like to render اللغة العربية in Rust!";
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("some_english_mixed_with_arabic");
  let Ok(font_size) = draw_test_cfg.font_size(36., console_core_text_layout::LineHeight(40.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text(paragraph);
  let Ok(canvas) = text.canvas(Size {
    width: 400,
    height: 110,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ligature_segmentation() -> Result<(), Box<dyn std::error::Error>> {
  use console_core_text_layout::{Buffer, FontSystem, LineHeight, Metrics, Shaping};

  let Ok(mut database) = fontdb::Database::new();
  let font = std::fs::read("fonts/Inter-Regular.ttf")?;
  let Ok(()) = database.load_font_data(font);
  let Ok(mut fonts) = FontSystem::new_with_locale_and_db("en-US".to_string(), database);
  let Ok(metrics) = Metrics::new(14.0, LineHeight(20.0));
  let Ok(mut buffer) = Buffer::new(&mut fonts, metrics);
  let Ok(mut drawing) = buffer.borrow_with(&mut fonts);
  let Ok(attrs) = Attrs::new();

  for (text, expected_words) in [("|>", 1), ("->", 1), ("!=", 1), ("++", 2)] {
    let Ok(()) = drawing.set_text(text, &attrs, Shaping::Advanced, None);
    let Ok(_runs) = drawing.layout_runs();
    let line = drawing.lines.first().ok_or("Missing fixture line")?;
    let Ok(shaped) = line.shape_opt();
    let shape = shaped.ok_or("Missing line shaping")?;
    let span = shape.spans.first().ok_or("Missing shape span")?;
    assert_eq!(
      span.words.len(),
      expected_words,
      "{text:?} has the wrong number of words"
    );
  }

  Ok(())
}
