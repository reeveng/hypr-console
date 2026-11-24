//! Extracts a flat, position-independent user program from the Rust-linked image.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, ErrorKind};
use std::ops::Range;
use std::path::Path;
use std::process::Command;

const PAGE_BYTES: u32 = 4096;

pub fn build(output: &Path, rustc: &OsStr) -> io::Result<Vec<u8>> {
  println!("cargo:rerun-if-changed=native-browser");
  let image_path = output.join("native-renderer.efi");
  let status = Command::new(rustc)
    .args([
      "--target=x86_64-unknown-uefi",
      "--edition=2024",
      "-Copt-level=s",
      "-Cpanic=abort",
      "-Cno-redzone=yes",
      "-Cforce-unwind-tables=no",
      "-Clink-arg=/merge:.rdata=.text",
      "-Clink-arg=/Brepro",
      "native-browser/renderer.rs",
      "-o",
    ])
    .arg(&image_path)
    .status()?;

  match status.code() {
    Some(0) => fs::read(image_path),
    Some(1.. | ..=-1) | None => Err(io::Error::other(
      "could not compile the native browser renderer",
    )),
  }
}

pub struct InvalidProgram;

impl From<InvalidProgram> for io::Error {
  fn from(_error: InvalidProgram) -> Self {
    Self::new(
      ErrorKind::InvalidData,
      "native renderer must be a single position-independent code page",
    )
  }
}

fn split(bytes: &[u8], position: u32) -> Result<(&[u8], &[u8]), InvalidProgram> {
  match position.try_into() {
    Ok(index) => bytes.split_at_checked(index).ok_or(InvalidProgram),
    Err(_) => Err(InvalidProgram),
  }
}

fn word(bytes: &[u8], offset: u32) -> Result<u16, InvalidProgram> {
  let (_, tail) = split(bytes, offset)?;
  let field = tail.first_chunk().ok_or(InvalidProgram)?;
  Ok(u16::from_le_bytes(*field))
}

fn dword(bytes: &[u8], offset: u32) -> Result<u32, InvalidProgram> {
  let (_, tail) = split(bytes, offset)?;
  let field = tail.first_chunk().ok_or(InvalidProgram)?;
  Ok(u32::from_le_bytes(*field))
}

pub fn program(image: &[u8]) -> Result<&[u8], InvalidProgram> {
  let pe_offset = dword(image, 0x3c)?;
  let (_, pe) = split(image, pe_offset)?;
  let (signature, _) = split(pe, 6)?;

  match signature {
    b"PE\0\0\x64\x86" => {}
    _ => return Err(InvalidProgram),
  }

  let sections = word(pe, 6)?;
  let optional_len = word(pe, 20)?;
  let (_, headers) = split(pe, 24)?;
  let (optional, table) = split(headers, u32::from(optional_len))?;
  let magic = word(optional, 0)?;
  let import_size = dword(optional, 120)?;

  match (magic, import_size) {
    (0x20b, 0) => {}
    _ => return Err(InvalidProgram),
  }

  let entry = dword(optional, 16)?;
  let mut code = None;
  let mut unwind = None;
  let mut relocations = None;
  let table_size = u32::from(sections).saturating_mul(40);
  let (section_table, _) = split(table, table_size)?;

  for header in section_table.chunks_exact(40) {
    let address = dword(header, 12)?;
    let size = dword(header, 8)?;
    let start = dword(header, 20)?;
    let declared_size = dword(header, 16)?;
    let flags = dword(header, 36)?;
    let (_, data) = split(image, start)?;
    let (contents, _) = split(data, size)?;

    let readable = size <= declared_size && flags & 0x8000_0000 == 0;

    match bool::then_some(readable, ()) {
      Some(()) => {}
      None => return Err(InvalidProgram),
    }

    let (name, _) = split(header, 8)?;

    match name {
      b".text\0\0\0" => {
        let matches_entry = address == entry && (1..=PAGE_BYTES).contains(&size);

        match (code, bool::then_some(matches_entry, ())) {
          (None, Some(())) => code = Some(contents),
          _ => return Err(InvalidProgram),
        }
      }
      b".eh_fram" => match unwind {
        None => {
          let end = address.checked_add(size).ok_or(InvalidProgram)?;
          unwind = Some(address..end);
        }
        Some(_) => return Err(InvalidProgram),
      },
      b".reloc\0\0" => match relocations {
        None => relocations = Some(contents),
        Some(_) => return Err(InvalidProgram),
      },
      _ => return Err(InvalidProgram),
    }
  }

  match (relocations, unwind) {
    (Some(blocks), Some(range)) => check_relocations(blocks, range)?,
    (Some(_), None) => return Err(InvalidProgram),
    (None, _) => {}
  }

  code.ok_or(InvalidProgram)
}

fn check_relocations(mut blocks: &[u8], unwind: Range<u32>) -> Result<(), InvalidProgram> {
  for _header in blocks.chunks(8) {
    match blocks {
      [] => return Ok(()),
      [_, ..] => {}
    }

    let page = dword(blocks, 0)?;
    let length = dword(blocks, 4)?;
    let (block, remaining) = split(blocks, length)?;
    let (_, payload) = split(block, 8)?;
    let entries = payload.chunks_exact(2);

    match entries.remainder() {
      [] => {}
      [_, ..] => return Err(InvalidProgram),
    }

    for entry in entries {
      let value = word(entry, 0)?;

      match value.wrapping_shr(12) {
        0 => {}
        _ => check_relocation(page, value, &unwind)?,
      }
    }

    blocks = remaining;
  }

  Ok(())
}

fn check_relocation(page: u32, entry: u16, unwind: &Range<u32>) -> Result<(), InvalidProgram> {
  let address = page
    .checked_add(u32::from(entry & 0xfff))
    .ok_or(InvalidProgram)?;

  match bool::then_some(unwind.contains(&address), ()) {
    Some(()) => Ok(()),
    None => Err(InvalidProgram),
  }
}
