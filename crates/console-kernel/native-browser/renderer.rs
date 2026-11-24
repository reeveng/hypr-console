//! Freestanding user program for the bundled HTML preview.

#![no_std]
#![no_main]

mod wire;

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

use wire::{COLUMNS, Document, FRAME_ADDRESS, Font, GLYPH_SIDE, Pixels, ROWS};

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "sysv64" fn efi_main() -> ! {
  naked_asm!("call {}", sym start);
}

extern "sysv64" fn start(scroll: u64) -> ! {
  if scroll == wire::FAULT_REQUEST {
    // SAFETY: The preview's restart command deliberately attempts a store to the
    // read-only document page. Console catches the user page fault.
    unsafe {
      asm!("mov byte ptr [{document}], 0", document = in(reg) wire::DOCUMENT_ADDRESS);
    }
    exit(0);
  }
  // SAFETY: Console maps an initialized, immutable Document here for this call.
  let document = unsafe { &*(wire::DOCUMENT_ADDRESS as *const Document) };
  // SAFETY: Console lends this page to the renderer for the duration of the call.
  let pixels = unsafe { &mut *(FRAME_ADDRESS as *mut Pixels) };
  let rows = render(document, pixels, scroll as usize);
  exit(rows as u64)
}

fn render(document: &Document, pixels: &mut Pixels, scroll: usize) -> usize {
  pixels.fill(0);
  let mut page = Page {
    rows: 0,
    column: 0,
    space: false,
    scroll,
    pixels,
    font: &document.font,
  };
  let mut visible = false;
  let mut html = document.html.as_slice();

  while let Some((&byte, rest)) = html.split_first() {
    html = rest;
    match byte {
      0 => break,
      b'<' => {
        let Some(end) = html.iter().position(|byte| *byte == b'>') else {
          break;
        };
        let Some((tag, tail)) = html.split_at_checked(end) else {
          break;
        };
        let mut words = tag.split(|byte| byte.is_ascii_whitespace());
        match words.next() {
          Some(b"body") => visible = true,
          Some(b"/body") => break,
          Some(b"br") if visible => page.break_line(),
          Some(b"/p" | b"/h1" | b"/h2") if visible => {
            page.finish_line();
            page.break_line();
          }
          _ => {}
        }
        let Some((_, rest)) = tail.split_first() else {
          break;
        };
        html = rest;
      }
      _ if !visible => {}
      byte if byte.is_ascii_whitespace() => page.space = page.column != 0,
      byte => page.character(byte),
    }
  }
  page.finish_line();
  page.rows
}

struct Page<'a> {
  rows: usize,
  column: usize,
  space: bool,
  scroll: usize,
  pixels: &'a mut Pixels,
  font: &'a Font,
}

impl Page<'_> {
  fn break_line(&mut self) {
    self.rows += 1;
    self.column = 0;
    self.space = false;
  }

  fn finish_line(&mut self) {
    if self.column != 0 {
      self.break_line();
    }
  }

  fn character(&mut self, byte: u8) {
    if self.column + usize::from(self.space) >= usize::from(COLUMNS) {
      self.break_line();
    }
    self.column += usize::from(self.space);
    self.space = false;
    if let (Some(row), Some(glyph)) = (
      self
        .rows
        .checked_sub(self.scroll)
        .filter(|row| *row < usize::from(ROWS)),
      self.font.get(usize::from(byte & 0x7f)),
    ) {
      let start = row * usize::from(GLYPH_SIDE) * usize::from(COLUMNS) + self.column;
      for (scanline, bits) in glyph.iter().enumerate() {
        if let Some(pixel) = self.pixels.get_mut(start + scanline * usize::from(COLUMNS)) {
          *pixel = *bits;
        }
      }
    }
    self.column += 1;
  }
}

#[panic_handler]
fn panic(_error: &PanicInfo<'_>) -> ! {
  exit(0)
}

fn exit(result: u64) -> ! {
  // SAFETY: This is Console's userspace return trap. It restores the kernel continuation.
  unsafe {
    asm!("int {vector}", vector = const wire::EXIT_INTERRUPT, in("rax") 0_u64, in("rdi") result, options(noreturn));
  }
}
