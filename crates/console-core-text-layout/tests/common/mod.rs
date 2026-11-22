use std::path::{Component, Path, PathBuf};

use console_core_geometry::Size;
use console_core_path_rasterizer::{Paint, Pixmap, Rect, Transform};
use console_core_text_layout::{
  Align, Attrs, AttrsOwned, Buffer, Color, Ellipsize, Family, FontSize, FontSystem, GlyphCache,
  LineHeight, Metrics, Shaping, Wrap, fontdb::Database,
};

#[derive(Debug)]
pub struct DrawTestCfg {
  name: String,
  text: String,
  rich_spans: Option<Vec<(String, AttrsOwned)>>,
  font: AttrsOwned,

  font_size: FontSize,
  line_height: LineHeight,
  canvas_width: u32,
  canvas_height: u32,
  wrap: Wrap,
  ellipsize: Ellipsize,
  alignment: Option<Align>,
}

impl Default for DrawTestCfg {
  fn default() -> Self {
    let Ok(attrs) = Attrs::new();
    let Ok(font) = attrs.family(Family::Serif);
    Self {
      name: String::from("default"),
      font: match AttrsOwned::new(&font) {
        Ok(value) => value,
      },
      text: String::new(),
      rich_spans: None,
      font_size: FontSize(16.0),
      line_height: LineHeight(20.0),
      canvas_width: 300,
      canvas_height: 300,
      wrap: Wrap::WordOrGlyph,
      ellipsize: Ellipsize::None,
      alignment: None,
    }
  }
}

struct GoldenImage(String);

impl GoldenImage {
  fn new(name: &str) -> Result<Self, std::io::Error> {
    let mut parts = Path::new(name).components();

    match (parts.next(), parts.next()) {
      (Some(Component::Normal(component)), None) => match component.to_str() {
        Some(stem) => Ok(Self(format!("{stem}.png"))),
        None => Err(std::io::Error::other("invalid golden image name")),
      },
      _ => Err(std::io::Error::other("invalid golden image name")),
    }
  }
}

impl AsRef<Path> for GoldenImage {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

impl DrawTestCfg {
  pub fn new(name: impl Into<String>) -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        name: Into::<String>::into(name),
        ..Default::default()
      })
    }
  }

  pub fn text(mut self, text: impl Into<String>) -> Result<Self, console_core_never::Never> {
    {
      self.text = Into::<String>::into(text);
      Ok(self)
    }
  }

  pub fn rich_text(
    mut self,
    spans: impl IntoIterator<Item = (&'static str, Attrs<'static>)>,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.rich_spans = Some(
        spans
          .into_iter()
          .map(|(text, attrs)| {
            (
              text.to_string(),
              match AttrsOwned::new(&attrs) {
                Ok(value) => value,
              },
            )
          })
          .collect(),
      );
      Ok(self)
    }
  }

  pub fn font_attrs(mut self, attrs: Attrs) -> Result<Self, console_core_never::Never> {
    {
      let Ok(attrs_owned) = AttrsOwned::new(&attrs);
      self.font = attrs_owned;
      Ok(self)
    }
  }

  pub fn font_size(
    mut self,
    font_size: impl Into<FontSize>,
    line_height: LineHeight,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.font_size = Into::<FontSize>::into(font_size);
      self.line_height = line_height;
      Ok(self)
    }
  }

  pub fn canvas(mut self, size: Size<u32>) -> Result<Self, console_core_never::Never> {
    {
      self.canvas_width = size.width;
      self.canvas_height = size.height;
      Ok(self)
    }
  }

  pub fn wrap(mut self, wrap: Wrap) -> Result<Self, console_core_never::Never> {
    {
      self.wrap = wrap;
      Ok(self)
    }
  }

  pub fn ellipsize(mut self, ellipsize: Ellipsize) -> Result<Self, console_core_never::Never> {
    {
      self.ellipsize = ellipsize;
      Ok(self)
    }
  }

  pub fn alignment(
    mut self,
    alignment: Option<Align>,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.alignment = alignment;
      Ok(self)
    }
  }

  pub fn validate_text_rendering(self) -> Result<(), Box<dyn std::error::Error>> {
    let rendered = self.rendered()?;
    let name = GoldenImage::new(&self.name)?;
    let reference = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("tests/images")
      .join(name);
    let bytes = std::fs::read(reference)?;
    let expected = Pixmap::decode_png(&bytes)?;

    assert_eq!(expected, rendered, "rendering failed for {self:?}");

    Ok(())
  }

  fn rendered(&self) -> Result<Pixmap, Box<dyn std::error::Error>> {
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fonts");
    let Ok(mut database) = Database::new();

    let Ok(()) = database.load_fonts_dir(fonts);

    let Ok(mut fonts_2) = FontSystem::new_with_locale_and_db(String::from("En-US"), database);
    let Ok(metrics) = Metrics::new(self.font_size.0, self.line_height);
    let Ok(mut buffer) = Buffer::new(&mut fonts_2, metrics);
    let Ok(mut buffer_2) = buffer.borrow_with(&mut fonts_2);
    let Ok(width) =
      console_core_number_conversion::narrow_f32(f64::from(self.canvas_width.saturating_sub(10)));
    let Ok(height) = console_core_number_conversion::narrow_f32(f64::from(
      self.canvas_height.saturating_sub(10),
    ));
    let Ok(attrs) = self.font.as_attrs();
    let Ok(()) = buffer_2.set_wrap(self.wrap);
    let Ok(()) = buffer_2.set_ellipsize(self.ellipsize);
    let Ok(()) = buffer_2.set_size(Some(width), Some(height));

    match &self.rich_spans {
      Some(spans) => {
        let spans_2 = spans.iter().map(|(text, attrs_2)| {
          let Ok(attrs_3) = attrs_2.as_attrs();

          (text.as_str(), attrs_3)
        });
        let Ok(()) = buffer_2.set_rich_text(spans_2, &attrs, Shaping::Advanced, self.alignment);
      }
      None => {
        let Ok(()) = buffer_2.set_text(&self.text, &attrs, Shaping::Advanced, self.alignment);
      }
    }

    rasterized(
      &mut buffer_2,
      Size {
        width: self.canvas_width,
        height: self.canvas_height,
      },
    )
  }
}

#[derive(Debug)]
struct Rasterization {
  pixmap: Pixmap,
  error: Option<std::io::Error>,
}

fn rasterized(
  buffer: &mut console_core_text_layout::BorrowedWithFontSystem<'_, Buffer>,
  size: Size<u32>,
) -> Result<Pixmap, Box<dyn std::error::Error>> {
  let Ok(pixmap) = Pixmap::new(size.width, size.height);
  let mut pixmap_2 =
    pixmap.ok_or_else(|| std::io::Error::other("Invalid fixture canvas dimensions"))?;

  let ::core::result::Result::Ok(()) = pixmap_2.fill(console_core_path_rasterizer::Color::WHITE);

  let mut raster = Rasterization {
    pixmap: pixmap_2,
    error: None,
  };
  let Ok(mut cache) = GlyphCache::new();
  let Ok(color) = Color::rgb([0, 0, 0]);
  let Ok(()) = buffer.draw(
    &mut cache,
    color,
    &mut raster,
    |raster_2, x, y, width, height, color_2| {
      let Ok(x_2) = console_core_number_conversion::narrow_f32(f64::from(x.saturating_add(5)));
      let Ok(y_2) = console_core_number_conversion::narrow_f32(f64::from(y.saturating_add(5)));
      let Ok(width_2) = console_core_number_conversion::narrow_f32(f64::from(width));
      let Ok(height_2) = console_core_number_conversion::narrow_f32(f64::from(height));
      let rect = match {
        let Ok(explicit_value) = Rect::from_xywh(x_2, y_2, width_2, height_2);
        explicit_value
      } {
        Some(rect) => rect,
        None => {
          raster_2.error = Some(std::io::Error::other("Invalid fixture drawing rectangle"));

          return;
        }
      };
      let Ok(red) = color_2.r();
      let Ok(green) = color_2.g();
      let Ok(blue) = color_2.b();
      let Ok(alpha) = color_2.a();
      let mut paint = Paint {
        coverage: console_core_path_rasterizer::CoverageMode::Antialiased,
        ..Paint::default()
      };

      let ::core::result::Result::Ok(()) = paint.set_color_rgba8(red, green, blue, alpha);
      let Ok(identity) = Transform::identity();
      let ::core::result::Result::Ok(()) =
        raster_2.pixmap.fill_rect(rect, &paint, identity, None);
    },
  );

  match raster.error {
    Some(error) => Err(Box::new(error)),
    None => Ok(raster.pixmap),
  }
}
