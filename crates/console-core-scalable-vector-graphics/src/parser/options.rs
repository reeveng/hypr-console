#[cfg(feature = "text")]
use std::sync::Arc;

#[cfg(feature = "text")]
use crate::FontResolver;
use crate::{ImageHrefResolver, ImageRendering, ShapeRendering, Size, TextRendering};

#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The public dpi and font_size fields use dots per inch and CSS pixels respectively"
  )
)]
pub struct Options<'a> {
  pub resources_dir: Option<std::path::PathBuf>,

  pub dpi: f32,

  pub font_family: String,

  pub font_size: f32,

  pub languages: Vec<String>,

  pub shape_rendering: ShapeRendering,

  pub text_rendering: TextRendering,

  pub image_rendering: ImageRendering,

  pub default_size: Size,

  pub image_href_resolver: ImageHrefResolver<'a>,

  #[cfg(feature = "text")]
  pub font_resolver: FontResolver<'a>,

  #[cfg(feature = "text")]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Nested SVG trees read the same immutable font database and fontdb_mut copies the database before editing it"
    )
  )]
  pub console_core_font_database: Arc<console_core_font_database::Database>,

  pub style_sheet: Option<String>,
}

impl Default for Options<'_> {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Default constructs deferred image resolvers. They enter the document depth limit when actually called"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "Both constant viewport dimensions are finite and strictly positive"
    )
  )]
  #[cfg_attr(
    all(feature = "text", dylint_lib = "explicit081_no_shared_ownership"),
    expect(
      explicit081_no_shared_ownership,
      reason = "The frozen font catalogue can be reused by multiple nested document readers"
    )
  )]
  fn default() -> Options<'static> {
    Options {
      resources_dir: None,
      dpi: 96.0,

      font_family: "Times New Roman".to_owned(),
      font_size: 12.0,
      languages: vec!["en".to_string()],
      shape_rendering: ShapeRendering::default(),
      text_rendering: TextRendering::default(),
      image_rendering: ImageRendering::default(),
      default_size: ({
        let Ok(explicit_value) = Size::from_wh(100.0, 100.0);
        explicit_value
      })
      .unwrap(),
      image_href_resolver: ImageHrefResolver::default(),
      #[cfg(feature = "text")]
      font_resolver: FontResolver::default(),
      #[cfg(feature = "text")]
      console_core_font_database: Arc::new({
        let Ok(explicit_value) = console_core_font_database::Database::new();
        explicit_value
      }),
      style_sheet: None,
    }
  }
}

impl Options<'_> {
  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "resources_dir is the resolution base for SVG file references. Relative references are permitted to name a parent directory"
    )
  )]
  pub fn get_abs_path(
    &self,
    rel_path: &std::path::Path,
  ) -> core::result::Result<std::path::PathBuf, console_core_never::Never> {
    Ok(match (&self.resources_dir, rel_path.is_absolute()) {
      (_, true) | (None, false) => rel_path.to_path_buf(),
      (Some(directory), false) => directory.join(rel_path),
    })
  }

  #[cfg(feature = "text")]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The established public accessor lends a mutable font database directly"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Copy-on-write keeps each nested parser’s shared immutable font database isolated from later edits"
    )
  )]
  pub fn fontdb_mut(&mut self) -> &mut console_core_font_database::Database {
    Arc::make_mut(&mut self.console_core_font_database)
  }
}
