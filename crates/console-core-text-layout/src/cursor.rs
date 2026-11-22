#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Cursor {
  pub line: u32,

  pub index: u32,

  pub affinity: Affinity,
}

impl Cursor {
  pub const fn new(
    line: u32,
    index: crate::ByteIndex,
  ) -> Result<Self, console_core_never::Never> {
    let crate::ByteIndex(index_2) = index;

    {
      let Ok(cursor) = Self::new_with_affinity(line, crate::ByteIndex(index_2), Affinity::Before);

      Ok(cursor)
    }
  }

  pub const fn new_with_affinity(
    line: u32,
    index: crate::ByteIndex,
    affinity: Affinity,
  ) -> Result<Self, console_core_never::Never> {
    let crate::ByteIndex(index_2) = index;

    {
      Ok(Self {
        line,
        index: index_2,
        affinity,
      })
    }
  }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum Affinity {
  #[default]
  Before,
  After,
}

impl Affinity {
  pub const fn from_before(
    boundary: crate::BoundarySide,
  ) -> Result<Self, console_core_never::Never> {
    Ok(match boundary {
      crate::BoundarySide::Before => Self::Before,
      crate::BoundarySide::After => Self::After,
    })
  }

  pub const fn from_after(
    boundary: crate::BoundarySide,
  ) -> Result<Self, console_core_never::Never> {
    Self::from_before(boundary)
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LayoutCursor {
  pub line: u32,

  pub layout: u32,

  pub glyph: u32,
}

impl LayoutCursor {
  pub const fn new(
    line: u32,
    layout: crate::LayoutIndex,
    glyph: crate::GlyphIndex,
  ) -> Result<Self, console_core_never::Never> {
    let crate::LayoutIndex(layout_2) = layout;

    let crate::GlyphIndex(glyph_2) = glyph;

    {
      Ok(Self {
        line,
        layout: layout_2,
        glyph: glyph_2,
      })
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "public scroll offsets preserve their signed fractional layout pixel representation"
  )
)]
pub struct Scroll {
  pub line: u32,

  pub vertical: f32,

  pub horizontal: f32,
}

impl Scroll {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the existing const constructor takes the vertical offset in fractional layout pixels"
    )
  )]
  pub const fn new(
    line: u32,
    vertical: f32,
    horizontal: crate::HorizontalOffset,
  ) -> Result<Self, console_core_never::Never> {
    let crate::HorizontalOffset(horizontal_2) = horizontal;

    {
      Ok(Self {
        line,
        vertical,
        horizontal: horizontal_2,
      })
    }
  }
}
