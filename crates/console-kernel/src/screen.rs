//! The lines the kernel says, drawn where somebody holding the machine can read them.
//!
//! The serial port is where a line goes under QEMU and on a desk with a cable.
//! A handheld has no such port, and a kernel that says everything there has,
//! on the one machine it is written for, booted without a word. The firmware
//! lends the other place: the graphics output protocol names a run of memory
//! that is the panel, and unlike every other thing in boot services that
//! memory is still the panel after they have gone.
//!
//! What is here is the arithmetic and none of the machine: a run of pixels
//! somebody lends, how the panel lays them out, and where the dots of a
//! character land in it. The test lends a vector, the firmware edge lends the
//! panel, and the same lines are drawn in both.
//!
//! The characters are `console-core-bitmap-font`'s, eight dots square and
//! taken whole from `font8x8` rather than drawn again. The text engine the
//! desktop shapes with wants a heap, a font file and floating point, and this
//! has to say a fault's line with none of the three.
//!
//! The Legion Go's panel is taller than it is wide and is mounted on its
//! side: Linux's table of such panels calls it left side up, meaning the
//! panel's own left edge is the top of what a person sees. So a panel that
//! reports itself taller than wide is drawn turned, which is a decision made
//! from the shape because there is no table here yet to make it from, and a
//! monitor stood on end will show its lines sideways until there is.
//!
//! A dot is lit or dark and nothing else, because the firmware offers two
//! orders for the colours in a pixel and white is the same number in both.
//! When the lines reach the bottom the page is turned rather than scrolled: a
//! scroll moves every pixel on the panel for each line, and what this draws is
//! a boot's worth of lines and then the one that says why it stopped. Both the
//! turn and the wrap at the right edge wait for the character that needs them,
//! so a line that exactly fills the panel is not followed by an empty one and
//! the last line on a page is still there to be read.
//!
//! The framebuffer constructor borrows an aligned, writable region the caller
//! keeps mapped and exclusively owned for the screen's lifetime.

use core::fmt;
use core::num::NonZeroUsize;
use core::ptr;

use console_core_bitmap_font::legacy::BASIC_LEGACY;
use console_core_geometry::{Point, Size};
use console_core_never::Never;

pub const LIT: u32 = 0x00ff_ffff;

pub const DARK: u32 = 0;

const GLYPH: u64 = 8;

const PIXEL_BYTES: u64 = 4;

const COLUMNS_WANTED: u64 = 120;

pub(crate) const DOTS: [u8; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];

const NOT_IN_THE_FONT: [u8; 8] = [0xff; 8];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixels(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panel {
  pub width: Pixels,
  pub height: Pixels,
  pub scan_line: Pixels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
  Upright,
  LeftSideUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenError {
  TooSmall(Panel),
  ScanLineShorterThanWidth(Panel),
  LentTooFewPixels(Panel),
  InvalidFramebuffer,
  Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cell {
  column: u64,
  row: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
  Presented,
  Skipped,
}

pub struct FramebufferRegion {
  pub base: u64,
  pub pixels: Pixels,
}

#[derive(Debug)]
pub struct Screen<'a> {
  pixels: PixelMemory<'a>,
  pixel_count: u64,
  panel: Panel,
  orientation: Orientation,
  scale: u64,
  columns: u64,
  rows: u64,
  next: Cell,
}

#[derive(Debug)]
enum PixelMemory<'a> {
  Buffer(&'a mut [u32]),
  Framebuffer(u64),
}

impl Screen<'static> {
  #[allow(
    clippy::missing_safety_doc,
    reason = "the module head states the framebuffer ownership and mapping contract"
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the last framebuffer byte must fit in the Rust pointer address range"
    )
  )]
  pub unsafe fn from_framebuffer(
    region: FramebufferRegion,
    panel: Panel,
  ) -> Result<Self, ScreenError> {
    let FramebufferRegion {
      base,
      pixels: Pixels(count),
    } = region;

    match ::core::primitive::bool::then_some(base == 0 || !base.is_multiple_of(PIXEL_BYTES), ()) {
      ::core::option::Option::Some(()) => return Err(ScreenError::InvalidFramebuffer),
      ::core::option::Option::None => {}
    }

    let covered = count
      .checked_mul(PIXEL_BYTES)
      .and_then(|bytes| base.checked_add(bytes))
      .ok_or(ScreenError::Overflow)?;
    let _representable = usize::try_from(covered).map_err(|_wide| ScreenError::Overflow)?;

    Self::open(PixelMemory::Framebuffer(base), count, panel)
  }
}

impl Panel {
  #[must_use]
  pub fn orientation(&self) -> Result<Orientation, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.height.0 > self.width.0, ()) {
        ::core::option::Option::Some(()) => Orientation::LeftSideUp,
        ::core::option::Option::None => Orientation::Upright,
      },
    )
  }

  fn pixels(&self) -> Result<u64, ScreenError> {
    self
      .scan_line
      .0
      .checked_mul(self.height.0)
      .ok_or(ScreenError::Overflow)
  }
}

impl<'a> Screen<'a> {
  pub fn new(pixels: &'a mut [u32], panel: Panel) -> Result<Self, ScreenError> {
    let count = u64::try_from(pixels.len()).map_err(|_wide| ScreenError::Overflow)?;

    Self::open(PixelMemory::Buffer(pixels), count, panel)
  }

  fn open(pixels: PixelMemory<'a>, pixel_count: u64, panel: Panel) -> Result<Self, ScreenError> {
    let Ok(orientation) = panel.orientation();

    let (across, down) = match orientation {
      Orientation::Upright => (panel.width.0, panel.height.0),
      Orientation::LeftSideUp => (panel.height.0, panel.width.0),
    };

    let wanted = GLYPH
      .checked_mul(COLUMNS_WANTED)
      .ok_or(ScreenError::Overflow)?;
    let scale = across.div_euclid(wanted).max(1);
    let cell = GLYPH.checked_mul(scale).ok_or(ScreenError::Overflow)?;
    let columns = across.div_euclid(cell);
    let rows = down.div_euclid(cell);
    let needed = panel.pixels()?;

    match (
      columns == 0 || rows == 0,
      panel.scan_line.0 < panel.width.0,
      pixel_count < needed,
    ) {
      (true, _, _) => Err(ScreenError::TooSmall(panel)),
      (false, true, _) => Err(ScreenError::ScanLineShorterThanWidth(panel)),
      (false, false, true) => Err(ScreenError::LentTooFewPixels(panel)),
      (false, false, false) => {
        let mut screen = Screen {
          pixels,
          pixel_count: needed,
          panel,
          orientation,
          scale,
          columns,
          rows,
          next: Cell { column: 0, row: 0 },
        };
        let Ok(()) = screen.clear();

        Ok(screen)
      }
    }
  }

  pub fn print(&mut self, line: &str) -> Result<(), Never> {
    for character in line.chars() {
      let Ok(()) = self.put(character);
    }

    self.new_line()
  }

  #[cfg_attr(
    dylint_lib = "explicit001_fallible_result",
    expect(
      explicit001_fallible_result,
      reason = "an already resident frame with an unrepresentable dimension cannot be placed on this panel"
    )
  )]
  pub fn draw_frame<const SIDE: usize>(
    &mut self,
    rows: &[[u32; SIDE]],
  ) -> Result<Presentation, Never> {
    let size = match (u64::try_from(SIDE), u64::try_from(rows.len())) {
      (Ok(width), Ok(height)) => Size { width, height },
      (Err(_), _) | (_, Err(_)) => return Ok(Presentation::Skipped),
    };

    self.draw_rows(size, rows.iter().map(|row| row.iter().copied()))
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "validated frame dimensions are converted at the Rust slice boundary"
    )
  )]
  pub fn draw_monochrome_frame(
    &mut self,
    size: Size<u32>,
    pixels: &[[u8; 4]],
  ) -> Result<Presentation, Never> {
    let length = u64::from(size.width).saturating_mul(u64::from(size.height));

    match ::core::primitive::bool::then_some(u64::try_from(pixels.len()) == Ok(length), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(Presentation::Skipped),
    }

    let raw_width = match usize::try_from(size.width) {
      Ok(width) => width,
      Err(_wide) => return Ok(Presentation::Skipped),
    };
    let width = match NonZeroUsize::new(raw_width) {
      Some(width) => width,
      None => return Ok(Presentation::Skipped),
    };
    let rows = pixels.chunks_exact(width.get()).map(|row| {
      row
        .iter()
        .map(|[grey, _, _, _]| u32::from_le_bytes([*grey, *grey, *grey, 0]))
    });

    self.draw_rows(
      Size {
        width: u64::from(size.width),
        height: u64::from(size.height),
      },
      rows,
    )
  }

  fn draw_rows(
    &mut self,
    size: Size<u64>,
    rows: impl IntoIterator<Item = impl IntoIterator<Item = u32>>,
  ) -> Result<Presentation, Never> {
    let (wide, tall) = match self.orientation {
      Orientation::Upright => (self.panel.width.0, self.panel.height.0),
      Orientation::LeftSideUp => (self.panel.height.0, self.panel.width.0),
    };
    let Size { width, height } = size;

    match ::core::primitive::bool::then_some(
      width == 0 || height == 0 || width > wide || height > tall,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(Presentation::Skipped);
      }
      ::core::option::Option::None => {}
    }

    let scale = wide
      .div_euclid(width)
      .min(tall.div_euclid(height))
      .div_euclid(4)
      .max(1);
    let left = wide.saturating_sub(width.saturating_mul(scale));
    let top = tall.saturating_sub(height.saturating_mul(scale));

    for (row, y) in rows.into_iter().zip(0_u64..) {
      for (colour, x) in row.into_iter().zip(0_u64..) {
        let Ok(()) = self.fill(
          Point {
            x: left.saturating_add(x.saturating_mul(scale)),
            y: top.saturating_add(y.saturating_mul(scale)),
          },
          colour,
          scale,
        );
      }
    }

    Ok(Presentation::Presented)
  }

  fn put(&mut self, character: char) -> Result<(), Never> {
    match character {
      '\n' => self.new_line(),
      '\r' => Ok(()),
      drawn => {
        let Ok(()) = self.draw(drawn);

        self.advance()
      }
    }
  }

  fn advance(&mut self) -> Result<(), Never> {
    self.next.column = self.next.column.saturating_add(1);

    Ok(())
  }

  fn new_line(&mut self) -> Result<(), Never> {
    self.next = Cell {
      column: 0,
      row: self.next.row.saturating_add(1),
    };

    Ok(())
  }

  fn clear(&mut self) -> Result<(), Never> {
    for index in 0..self.pixel_count {
      let Ok(()) = self.write_pixel(index, DARK);
    }

    Ok(())
  }

  fn write_pixel(&mut self, index: u64, colour: u32) -> Result<(), Never> {
    match ::core::primitive::bool::then_some(index >= self.pixel_count, ()) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {}
    }

    match &mut self.pixels {
      PixelMemory::Buffer(pixels) => {
        let Ok(()) = paint_buffer(pixels, index, colour);
      }
      PixelMemory::Framebuffer(base) => {
        let at = base.saturating_add(index.saturating_mul(PIXEL_BYTES));
        let Ok(()) = paint_framebuffer(at, colour);
      }
    }

    Ok(())
  }

  fn draw(&mut self, character: char) -> Result<(), Never> {
    let Ok(()) = match ::core::primitive::bool::then_some(self.next.column >= self.columns, ()) {
      ::core::option::Option::Some(()) => self.new_line(),
      ::core::option::Option::None => Ok(()),
    };

    match ::core::primitive::bool::then_some(self.next.row >= self.rows, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.clear();
        self.next = Cell { column: 0, row: 0 };
      }
      ::core::option::Option::None => {}
    }

    let Ok(glyph) = glyph(character);
    let cell = GLYPH.saturating_mul(self.scale);
    let left = self.next.column.saturating_mul(cell);
    let top = self.next.row.saturating_mul(cell);

    for (line, down) in glyph.iter().zip(0_u64..) {
      for (dot, across) in DOTS.iter().zip(0_u64..) {
        let colour = match ::core::primitive::bool::then_some(line & dot == 0, ()) {
          ::core::option::Option::Some(()) => DARK,
          ::core::option::Option::None => LIT,
        };

        let Ok(()) = self.fill(
          Point {
            x: left.saturating_add(across.saturating_mul(self.scale)),
            y: top.saturating_add(down.saturating_mul(self.scale)),
          },
          colour,
          self.scale,
        );
      }
    }

    Ok(())
  }

  fn fill(&mut self, origin: Point<u64>, colour: u32, scale: u64) -> Result<(), Never> {
    for down in 0..scale {
      for across in 0..scale {
        let Ok(()) = self.set(
          Point {
            x: origin.x.saturating_add(across),
            y: origin.y.saturating_add(down),
          },
          colour,
        );
      }
    }

    Ok(())
  }

  fn set(&mut self, at: Point<u64>, colour: u32) -> Result<(), Never> {
    let place = match self.orientation {
      Orientation::Upright => Some((at.x, at.y)),
      Orientation::LeftSideUp => self
        .panel
        .height
        .0
        .checked_sub(1)
        .and_then(|last| last.checked_sub(at.x))
        .map(|from_the_top| (at.y, from_the_top)),
    };
    let index = place.and_then(|(x, y)| {
      y.checked_mul(self.panel.scan_line.0)
        .and_then(|before| before.checked_add(x))
    });

    match index {
      Some(offset) => self.write_pixel(offset, colour),
      None => Ok(()),
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the pixel coordinate is converted only at the Rust slice lookup"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit063_fault_has_recovery",
  expect(
    explicit063_fault_has_recovery,
    reason = "an index too wide for this machine cannot address the lent slice and is clipped"
  )
)]
fn paint_buffer(pixels: &mut [u32], index: u64, colour: u32) -> Result<(), Never> {
  match usize::try_from(index) {
    Ok(at) => match pixels.get_mut(at) {
      Some(pixel) => *pixel = colour,
      None => {}
    },
    Err(_wide) => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the mapped address must match pointer width at the volatile store"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the only volatile store into the checked UEFI framebuffer region"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit063_fault_has_recovery",
  expect(
    explicit063_fault_has_recovery,
    reason = "the checked framebuffer range already fits pointer width; an out-of-range address is never dereferenced"
  )
)]
fn paint_framebuffer(address: u64, colour: u32) -> Result<(), Never> {
  match usize::try_from(address) {
    Ok(at) => {
      // SAFETY: the constructor checked range and alignment, and write_pixel only supplies an in-range address in the exclusively held region.
      unsafe { ptr::with_exposed_provenance_mut::<u32>(at).write_volatile(colour) };
    }
    Err(_wide) => {}
  }

  Ok(())
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the byte key becomes a Rust font-table index"
  )
)]
pub(crate) fn glyph(character: char) -> Result<[u8; 8], Never> {
  let found = match u8::try_from(character) {
    Ok(code) => BASIC_LEGACY.get(usize::from(code)),
    Err(_past_one_byte) => None,
  };

  Ok(match found {
    Some(dots) => *dots,
    None => NOT_IN_THE_FONT,
  })
}

impl fmt::Write for Screen<'_> {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    for character in text.chars() {
      let Ok(()) = self.put(character);
    }

    Ok(())
  }
}

impl fmt::Display for ScreenError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ScreenError::TooSmall(panel) => write!(
        to,
        "a panel {} by {} has no room for one character",
        panel.width.0, panel.height.0
      ),
      ScreenError::ScanLineShorterThanWidth(panel) => write!(
        to,
        "the panel says a line of it is {} pixels and is stored in {}",
        panel.width.0, panel.scan_line.0
      ),
      ScreenError::LentTooFewPixels(panel) => write!(
        to,
        "a panel {} by {} was lent fewer pixels than it has",
        panel.width.0, panel.height.0
      ),
      ScreenError::InvalidFramebuffer => {
        to.write_str("the framebuffer address is missing or unaligned")
      }
      ScreenError::Overflow => {
        to.write_str("the panel's size does not fit in the numbers it is counted in")
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use alloc::string::String;
  use alloc::vec::Vec;

  use super::*;

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the hosted fixture turns its validated stride into a row size for slice iteration"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit056_no_hidden_panic_api",
    expect(
      explicit056_no_hidden_panic_api,
      reason = "only the fixture row strides set by the panel helper reach chunks_exact"
    )
  )]
  fn drawn(pixels: &[u32], panel: Panel) -> Result<Vec<String>, ScreenError> {
    let width = usize::try_from(panel.scan_line.0).map_err(|_too_wide| ScreenError::Overflow)?;

    Ok(
      pixels
        .chunks_exact(width)
        .map(|line| {
          line
            .iter()
            .map(
              |pixel| match ::core::primitive::bool::then_some(*pixel == DARK, ()) {
                ::core::option::Option::Some(()) => '.',
                ::core::option::Option::None => '#',
              },
            )
            .collect()
        })
        .collect(),
    )
  }

  fn panel(size: Size<u64>) -> Result<Panel, Never> {
    Ok(Panel {
      width: Pixels(size.width),
      height: Pixels(size.height),
      scan_line: Pixels(size.width),
    })
  }

  #[test]
  fn a_letter_is_drawn_in_the_dots_the_font_gives_it() -> Result<(), ScreenError> {
    let Ok(panel) = panel(Size {
      width: 8,
      height: 8,
    });
    let mut pixels = [LIT; 64];
    let mut screen = Screen::new(&mut pixels, panel)?;
    let Ok(()) = screen.print("A");

    let rendered = drawn(&pixels, panel)?;
    assert_eq!(
      rendered,
      [
        "..##....", ".####...", "##..##..", "##..##..", "######..", "##..##..", "##..##..",
        "........"
      ]
    );

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the test lends its exclusive live and aligned local pixel array as the mapped framebuffer"
    )
  )]
  fn the_framebuffer_writes_pixels_at_the_firmware_stride() -> Result<(), ScreenError> {
    let panel = Panel {
      width: Pixels(8),
      height: Pixels(8),
      scan_line: Pixels(10),
    };
    let mut pixels = [LIT; 81];
    let base = u64::try_from(pixels.as_mut_ptr().expose_provenance())
      .map_err(|_wide| ScreenError::Overflow)?;
    // SAFETY: this array owns the aligned range and stays alive until the
    // screen is finished. The screen has sole access while it draws.
    let mut screen = unsafe {
      Screen::from_framebuffer(
        FramebufferRegion {
          base,
          pixels: Pixels(81),
        },
        panel,
      )
    }?;
    let Ok(()) = screen.print("A");

    assert_eq!(
      pixels.get(10..20),
      Some([DARK, LIT, LIT, LIT, LIT, DARK, DARK, DARK, DARK, DARK].as_slice())
    );
    assert_eq!(pixels.last(), Some(&LIT));

    Ok(())
  }

  #[test]
  fn the_second_letter_is_beside_the_first_and_the_second_line_under_it()
  -> Result<(), ScreenError> {
    let Ok(panel) = panel(Size {
      width: 16,
      height: 16,
    });
    let mut pixels = [DARK; 256];
    let mut screen = Screen::new(&mut pixels, panel)?;
    let Ok(()) = screen.print("AA");
    let Ok(()) = screen.print("A");
    let lines = drawn(&pixels, panel)?;

    assert_eq!(lines.first().map(String::as_str), Some("..##......##...."));
    assert_eq!(lines.get(8).map(String::as_str), Some("..##............"));

    Ok(())
  }

  #[test]
  fn a_panel_taller_than_wide_is_drawn_with_its_left_side_up() -> Result<(), ScreenError> {
    let Ok(panel) = panel(Size {
      width: 8,
      height: 16,
    });
    let mut pixels = [DARK; 128];
    let mut screen = Screen::new(&mut pixels, panel)?;
    let Ok(()) = screen.print("A");

    let rendered = drawn(&pixels, panel)?;
    assert_eq!(
      rendered,
      [
        "........", "........", "........", "........", "........", "........", "........",
        "........", "........", "........", "..#####.", ".######.", "##..#...", "##..#...",
        ".######.", "..#####.",
      ]
    );

    Ok(())
  }

  #[test]
  fn a_wide_panel_draws_every_dot_as_a_square_of_pixels() -> Result<(), ScreenError> {
    let Ok(panel) = panel(Size {
      width: 1920,
      height: 16,
    });
    let mut pixels = [DARK; 30720];
    let mut screen = Screen::new(&mut pixels, panel)?;
    let Ok(()) = screen.print("A");
    let lines = drawn(&pixels, panel)?;

    assert_eq!(
      lines.first().and_then(|line| line.get(..16)),
      Some("....####........")
    );
    assert_eq!(
      lines.get(1).and_then(|line| line.get(..16)),
      Some("....####........")
    );
    assert_eq!(
      lines.get(2).and_then(|line| line.get(..16)),
      Some("..########......")
    );

    Ok(())
  }

  #[test]
  fn a_line_past_the_last_row_turns_the_page() -> Result<(), ScreenError> {
    let Ok(panel) = panel(Size {
      width: 8,
      height: 8,
    });
    let mut turned = [DARK; 64];
    let mut screen = Screen::new(&mut turned, panel)?;
    let Ok(()) = screen.print("A");
    let Ok(()) = screen.print("B");
    let mut fresh = [DARK; 64];
    let mut screen_2 = Screen::new(&mut fresh, panel)?;
    let Ok(()) = screen_2.print("B");

    assert_eq!(turned, fresh);
    let rendered = drawn(&turned, panel)?;
    assert_ne!(rendered.first().map(String::as_str), Some("..##...."));

    Ok(())
  }

  #[test]
  fn a_character_the_font_does_not_hold_is_a_block_rather_than_a_gap() -> Result<(), ScreenError>
  {
    let Ok(panel) = panel(Size {
      width: 8,
      height: 8,
    });
    let mut pixels = [DARK; 64];
    let mut screen = Screen::new(&mut pixels, panel)?;
    let Ok(()) = screen.print("\u{4e16}");

    assert_eq!(pixels, [LIT; 64]);

    Ok(())
  }

  #[test]
  fn a_panel_lent_fewer_pixels_than_it_has_is_refused() -> Result<(), Never> {
    let Ok(panel) = panel(Size {
      width: 8,
      height: 8,
    });
    let mut pixels = [DARK; 63];

    assert_eq!(
      Screen::new(&mut pixels, panel).map(|_screen| ()),
      Err(ScreenError::LentTooFewPixels(panel))
    );

    Ok(())
  }
}
