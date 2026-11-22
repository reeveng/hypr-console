use common::DrawTestCfg;
use console_core_text_layout::{Attrs, Color, Family, UnderlineStyle};

mod common;

use console_core_geometry::Size;

fn base_attrs() -> Result<Attrs<'static>, console_core_never::Never> {
  {
    let Ok(attrs) = Attrs::new();
    let Ok(family) = attrs.family(Family::Name("Noto Sans"));

    Ok(family)
  }
}

#[test]
fn test_text_decorations() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(base) = base_attrs();
  let Ok(red) = Color::rgb([0xFF, 0x00, 0x00]);
  let Ok(cyan) = Color::rgb([0x00, 0xFF, 0xFF]);

  let Ok(draw_test_cfg) = DrawTestCfg::new("text_decorations");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(base.clone());
  let Ok(underline) = base.clone().underline(UnderlineStyle::Single);
  let Ok(base_underline) = base.clone().underline(UnderlineStyle::Double);
  let Ok(strikethrough) = base.clone().strikethrough();
  let Ok(overline) = base.clone().overline();
  let Ok(underline_color) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .underline_color(red);
  let Ok(base_strikethrough) = base.clone().strikethrough();
  let Ok(strikethrough_color) = base_strikethrough.strikethrough_color(cyan);
  let Ok(underline_strikethrough) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .strikethrough();
  let Ok(strikethrough_overline) = underline_strikethrough.overline();
  let Ok(rich_text) = font_attrs.rich_text(vec![
    ("Under ", underline),
    ("Double ", base_underline),
    ("Strike ", strikethrough),
    ("Over ", overline),
    ("RedUl ", underline_color),
    ("CyanSt ", strikethrough_color),
    ("All", strikethrough_overline),
    (" Plain", base),
  ]);
  let Ok(canvas) = rich_text.canvas(Size {
    width: 600,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_text_decorations_rtl() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(base) = base_attrs();
  let Ok(red) = Color::rgb([0xFF, 0x00, 0x00]);
  let Ok(cyan) = Color::rgb([0x00, 0xFF, 0xFF]);

  let Ok(draw_test_cfg) = DrawTestCfg::new("text_decoration_rtl");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(base.clone());
  let Ok(underline) = base.clone().underline(UnderlineStyle::Single);
  let Ok(base_underline) = base.clone().underline(UnderlineStyle::Double);
  let Ok(strikethrough) = base.clone().strikethrough();
  let Ok(overline) = base.clone().overline();
  let Ok(underline_color) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .underline_color(red);
  let Ok(base_strikethrough) = base.clone().strikethrough();
  let Ok(strikethrough_color) = base_strikethrough.strikethrough_color(cyan);
  let Ok(underline_strikethrough) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .strikethrough();
  let Ok(strikethrough_overline) = underline_strikethrough.overline();
  let Ok(rich_text) = font_attrs.rich_text(vec![
    ("زیر خط ", underline),
    ("دوتایی ", base_underline),
    ("خط ", strikethrough),
    ("رو ", overline),
    ("زیر خط قرمز ", underline_color),
    ("فیروزه ای ", strikethrough_color),
    ("همگی", strikethrough_overline),
    (" هیچ", base),
  ]);
  let Ok(canvas) = rich_text.canvas(Size {
    width: 600,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_text_decorations_bidi() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(base) = base_attrs();
  let Ok(red) = Color::rgb([0xFF, 0x00, 0x00]);
  let Ok(cyan) = Color::rgb([0x00, 0xFF, 0xFF]);

  let Ok(draw_test_cfg) = DrawTestCfg::new("text_decoration_bidi");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(base.clone());
  let Ok(underline) = base.clone().underline(UnderlineStyle::Single);
  let Ok(base_underline) = base.clone().underline(UnderlineStyle::Double);
  let Ok(strikethrough) = base.clone().strikethrough();
  let Ok(overline) = base.clone().overline();
  let Ok(underline_color) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .underline_color(red);
  let Ok(base_strikethrough) = base.clone().strikethrough();
  let Ok(strikethrough_color) = base_strikethrough.strikethrough_color(cyan);
  let Ok(underline_strikethrough) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .strikethrough();
  let Ok(strikethrough_overline) = underline_strikethrough.overline();
  let Ok(rich_text) = font_attrs.rich_text(vec![
    ("زیرخط ", underline),
    ("Double ", base_underline),
    ("خط ", strikethrough),
    ("Over ", overline),
    ("Red زیر خط ", underline_color),
    ("CyanSt ", strikethrough_color),
    ("All", strikethrough_overline),
    (" Plain", base),
  ]);
  let Ok(canvas) = rich_text.canvas(Size {
    width: 600,
    height: 50,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}

#[test]
fn test_text_decorations_multiline_bidi() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(base) = base_attrs();
  let Ok(red) = Color::rgb([0xFF, 0x00, 0x00]);
  let Ok(cyan) = Color::rgb([0x00, 0xFF, 0xFF]);

  let Ok(draw_test_cfg) = DrawTestCfg::new("text_decoration_multiline_bidi");
  let Ok(font_size) = draw_test_cfg.font_size(20., console_core_text_layout::LineHeight(26.));
  let Ok(font_attrs) = font_size.font_attrs(base.clone());
  let Ok(underline) = base.clone().underline(UnderlineStyle::Single);
  let Ok(base_underline) = base.clone().underline(UnderlineStyle::Double);
  let Ok(strikethrough) = base.clone().strikethrough();
  let Ok(overline) = base.clone().overline();
  let Ok(underline_color) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .underline_color(red);
  let Ok(base_strikethrough) = base.clone().strikethrough();
  let Ok(strikethrough_color) = base_strikethrough.strikethrough_color(cyan);
  let Ok(underline_strikethrough) = match base.clone().underline(UnderlineStyle::Single) {
    Ok(value) => value,
  }
  .strikethrough();
  let Ok(strikethrough_overline) = underline_strikethrough.overline();
  let Ok(rich_text) = font_attrs.rich_text(vec![
    ("زیرخط ", underline),
    ("Double ", base_underline),
    ("خط ", strikethrough),
    ("Over \n", overline),
    ("Red زیر خط ", underline_color),
    ("CyanSt ", strikethrough_color),
    ("All", strikethrough_overline),
    (" Plain", base),
  ]);
  let Ok(canvas) = rich_text.canvas(Size {
    width: 400,
    height: 80,
  });
  canvas.validate_text_rendering()?;
  Ok(())
}
