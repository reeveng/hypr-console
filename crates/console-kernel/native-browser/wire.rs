//! The one-page document input and one-page monochrome viewport of the native preview.

pub const DOCUMENT_ADDRESS: u64 = 0x4000_0000_6000;
pub const FRAME_ADDRESS: u64 = 0x4000_0000_4000;
pub const COLUMNS: u16 = 32;
pub const ROWS: u16 = 16;
pub const GLYPH_SIDE: u16 = 8;
pub const HEIGHT: u16 = ROWS * GLYPH_SIDE;
pub const HTML_BYTES: u16 = 3072;
pub const EXIT_INTERRUPT: u64 = 0x80;
pub const FAULT_REQUEST: u64 = u64::MAX;

pub type Scanline = [u8; 32];
pub type Pixels = [u8; 4096];
pub type Font = [[u8; 8]; 128];
pub type Html = [u8; 3072];

#[repr(C)]
pub struct Document {
  pub font: Font,
  pub html: Html,
}

const _: () = assert!(
  core::mem::size_of::<Document>() == 4096
    && core::mem::size_of::<Pixels>() == 4096
    && COLUMNS * HEIGHT == 4096
    && core::mem::size_of::<Scanline>() == 32
    && HTML_BYTES == 3072
);
