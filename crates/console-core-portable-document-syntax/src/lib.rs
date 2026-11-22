/*!
A low-level library for reading PDF files.

This crate implements the `Syntax` chapter of the PDF reference, and therefore
serves as a very good basis for building various abstractions on top of it, without having to reimplement
the PDF parsing logic.

This crate does not provide more high-level functionality, such as parsing fonts or color spaces.
Such functionality is out-of-scope for `console_core_portable_document_rasterizer-syntax`, since this crate is supposed to be
as *light-weight* and *application-agnostic* as possible.

Functionality-wise, this crate is therefore close to feature-complete. The main missing feature
is support for password-protected documents. In addition to that, more low-level APIs might be
added in the future.

The crate is `no_std` compatible but requires an allocator to be available.

# Example
This short example shows you how to load a PDF file and iterate over the content streams of all
pages. Reading the file and loading the document each say how they failed, and the example hands
both faults back from `main` rather than unwrapping them.
```rust
use console_core_portable_document_syntax::{LoadPdfError, Pdf};
use std::path::PathBuf;

#[derive(Debug)]
enum Failure {
    Unreadable(std::io::Error),
    Invalid(LoadPdfError),
}

fn main() -> Result<(), Failure> {
    let data = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/version-header.pdf"),
    )
    .map_err(Failure::Unreadable)?;

    let pdf = Pdf::new(data).map_err(Failure::Invalid)?;

    let Ok(pages) = pdf.pages();

    for page in pages.iter() {
        let Ok(mut ops) = page.typed_operations();

        while let Ok(Some(op)) = ops.next() {
            println!("{op:?}");
        }
    }

    Ok(())
}
```

# Safety
When the `unsafe` feature is disabled, there is only use of `unsafe` in `console_core_portable_document_rasterizer-syntax`.


For better performance, it is strongly recommended to enable the `unsafe` feature,
which does result in slightly more unsafe code being, but does give better performance:
- We will use the `flate2` crate for decoding flate streams.
- SIMD will be enabled to accelerate decoding of images.
- `memchr` will be used to accelerate some of the fallback parsing code.

# Features
The supported features include:
- Parsing xref tables in all its possible formats, including xref streams.
- Best-effort attempt at repairing PDF files with broken xref tables.
- Parsing of all objects types (also in object streams).
- Parsing and decoding PDF streams.
- Iterating over pages as well as their content streams in a typed or untyped fashion.
- The crate is very lightweight, especially in comparison to other PDF crates.

# Limitations
- There are still a few features missing, for example, support for
  password-protected PDFs. In addition to that, many properties (like page annotations) are
  currently not exposed.
- This crate is for read-only processing, you cannot directly use it to manipulate PDF files.
  If you need to do that, there are other crates in the Rust ecosystem that are suitable for this.
*/

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[macro_use]
mod log;

pub(crate) mod math;
pub(crate) mod sync;

mod data;
pub(crate) mod filter;
pub(crate) mod pdf;
pub(crate) mod trivia;
pub(crate) mod util;

pub mod content;
mod crypto;
pub mod metadata;
pub mod object;
pub mod page;
pub mod transform;
pub mod xref;

pub mod bit_reader;

pub mod byte_reader;

pub mod reader;

pub use data::PdfData;
pub use filter::*;
pub use pdf::*;

mod answers;
pub use answers::*;

#[cfg(test)]
mod test_support;
