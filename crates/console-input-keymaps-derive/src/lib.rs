//! The macros `console-input-keymaps` is written with.
//!
//! `CloneWithDelta` clones a value that carries spans with every span moved
//! by one offset, `ad_hoc_display!` is a diagnostic's message written as a
//! format string, and `diagnostic_kind` reads the severity off every kind of
//! diagnostic. They came into the tree with the keymaps as kbvm-proc, by
//! mahkoh. Input one of them cannot expand is answered with a compile error
//! at the tokens it could not read, which is where rustc shows the fault,
//! rather than with a panic inside the compiler, which kbvm-proc gave.

mod ad_hoc_display;
mod clone_with_delta;
mod diagnostic_kind;

extern crate proc_macro;

use proc_macro::TokenStream;

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  allow(
    explicit002_infallible_result,
    reason = "a macro answers the compiler in token streams, and a `Result` does not cross that boundary any more than it crosses an `extern` one"
  )
)]
#[proc_macro_derive(CloneWithDelta)]
pub fn clone_with_delta(input: TokenStream) -> TokenStream {
  let written = clone_with_delta::derive(input);

  match written {
    Ok(spelled) => TokenStream::from(spelled),
    Err(fault) => TokenStream::from(fault.to_compile_error()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  allow(
    explicit002_infallible_result,
    reason = "a macro answers the compiler in token streams, and a `Result` does not cross that boundary any more than it crosses an `extern` one"
  )
)]
#[proc_macro]
pub fn ad_hoc_display(input: TokenStream) -> TokenStream {
  let written = ad_hoc_display::expand(input);

  match written {
    Ok(spelled) => TokenStream::from(spelled),
    Err(fault) => TokenStream::from(fault.to_compile_error()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  allow(
    explicit002_infallible_result,
    reason = "a macro answers the compiler in token streams, and a `Result` does not cross that boundary any more than it crosses an `extern` one"
  )
)]
#[proc_macro_attribute]
pub fn diagnostic_kind(_args: TokenStream, input: TokenStream) -> TokenStream {
  let written = diagnostic_kind::expand(input);

  match written {
    Ok(spelled) => TokenStream::from(spelled),
    Err(fault) => TokenStream::from(fault.to_compile_error()),
  }
}
