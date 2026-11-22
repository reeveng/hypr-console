use common::DrawTestCfg;
use console_core_text_layout::{Align, Attrs, Ellipsize, EllipsizeHeightLimit, Family, Wrap};

mod common;

use console_core_geometry::Size;

#[test]
fn test_ellipsize_ltr_end_single_line() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_ltr_end_single_line");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("The quick brown fox jumps over the lazy dog.");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 180,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_ltr_end_single_line_aligned_right() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_ltr_end_single_line_aligned_right");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("The quick brown fox jumps over the lazy dog.");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
  let Ok(alignment) = ellipsize.alignment(Some(Align::Right));
  let Ok(canvas) = alignment.canvas(Size {
    width: 180,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_rtl_end_single_line() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_rtl_end_single_line");
  let Ok(font_size) = draw_test_cfg.font_size(22., console_core_text_layout::LineHeight(28.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("توانا بود هرکه دانا بود.");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 180,
    height: 55,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_mixed_end_single_line() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Noto Sans"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_mixed_end_single_line");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("Hello سلام mixed RTL/LTR world with extra words");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 190,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_ltr_start_single_line() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_ltr_start_single_line");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("The quick brown fox jumps over the lazy dog.");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Start(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 180,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_ltr_middle_single_line() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_ltr_middle_single_line");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("The quick brown fox jumps over the lazy dog.");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 180,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_ltr_end_two_lines() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_ltr_end_two_lines");
  let Ok(font_size) = draw_test_cfg.font_size(18., console_core_text_layout::LineHeight(24.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs
    .text("Pack my box with five dozen liquor jugs. Sphinx of black quartz, judge my vow.");
  let Ok(wrap) = text.wrap(Wrap::Word);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(2)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 200,
    height: 80,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_mixed_middle_single_line() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_mixed_middle_single_line");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("Hello سلام mixed RTL/LTR world with extra words");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 180,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_mixed_ltr_rtl_middle_two_lines() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_mixed_ltr_rtl_middle_two_lines");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("First line is LTR خط دوم از راست به چپ");
  let Ok(wrap) = text.wrap(Wrap::WordOrGlyph);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(2)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 180,
    height: 80,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_mixed_rtl_ltr_middle_two_lines() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_mixed_rtl_ltr_middle_two_lines");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("خط اول از راست به چپ Second line is LTR and has more words");
  let Ok(wrap) = text.wrap(Wrap::WordOrGlyph);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(2)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 210,
    height: 80,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_ltr_single_word_middle_two_lines() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_ltr_single_word_middle_two_lines");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("AVeryLongWordThatExceedsTheWidth");
  let Ok(wrap) = text.wrap(Wrap::WordOrGlyph);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(2)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 180,
    height: 80,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_mixed_ltr_rtl_ltr_middle_three_lines() -> Result<(), Box<dyn std::error::Error>>
{
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_mixed_ltr_rtl_ltr_middle_three_lines");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("This is some LTR text that keeps و یه مشت متن فارسیی.zippy");
  let Ok(wrap) = text.wrap(Wrap::WordOrGlyph);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(3)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 200,
    height: 100,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_bidi_isolates_middle_bug() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_bidi_isolates_middle_bug");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(family) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(rich_text) = font_attrs.rich_text([(
    "\u{2068}Workspace\u{2069}\u{2068} \u{2069}\u{2068}2\u{2069}",
    family,
  )]);
  let Ok(wrap) = rich_text.wrap(Wrap::WordOrGlyph);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 220,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_ellipsize_ltr_middle_small_buffer() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(attrs) = match Attrs::new() {
    Ok(value) => value,
  }
  .family(Family::Name("Inter"));
  let Ok(draw_test_cfg) = DrawTestCfg::new("ellipsize_ltr_middle_small_buffer");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(attrs);
  let Ok(text) = font_attrs.text("A/B Tester x8 Mono");
  let Ok(wrap) = text.wrap(Wrap::None);
  let Ok(ellipsize) = wrap.ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(1)));
  let Ok(canvas) = ellipsize.canvas(Size {
    width: 30,
    height: 100,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}
