use console_core_geometry::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matrix {
  Bt601,
  Bt709,
  Bt2020,
  Rgb,
  Unsaid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
  Studio,
  Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour {
  pub matrix: Matrix,
  pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Planes {
  Eight([Vec<u8>; 3]),
  Deep([Vec<u16>; 3], u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strides {
  pub luma: u32,
  pub chroma: u32,
}

pub struct Decoded {
  pub pts: i64,
  pub stored: Size<u32>,
  pub chroma: Size<u32>,
  pub planes: Planes,
  pub strides: Strides,
  pub colour: Colour,
}
