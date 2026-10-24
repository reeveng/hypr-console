//! Macros for [Lofty](https://crates.io/crates/lofty)

#![allow(
	unknown_lints,
	clippy::too_many_lines,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::cast_possible_wrap,
	clippy::cast_possible_truncation,
	clippy::module_name_repetitions,
	clippy::must_use_candidate,
	clippy::doc_markdown,
	let_underscore_drop,
	clippy::match_wildcard_for_single_variants,
	clippy::semicolon_if_nothing_returned,
	clippy::new_without_default,
	clippy::from_over_into,
	clippy::upper_case_acronyms,
	clippy::single_match_else,
	clippy::similar_names,
	clippy::tabs_in_doc_comments,
	clippy::len_without_is_empty,
	clippy::needless_late_init,
	clippy::type_complexity,
	clippy::type_repetition_in_bounds,
	unused_qualifications,
	clippy::return_self_not_must_use,
	clippy::bool_to_int_with_if,
	clippy::uninlined_format_args, /* This should be changed for any normal "{}", but I'm not a fan of it for any debug or width specific formatting */
	clippy::manual_let_else,
	clippy::struct_excessive_bools,
	clippy::match_bool,
	clippy::needless_pass_by_value
)]

mod attribute;
mod internal;
mod metadata_error;
mod metadata_file;
mod metadata_tag;
mod util;

use crate::metadata_error::MetadataError;
use crate::metadata_file::MetadataFile;
use crate::metadata_tag::{MetadataTag, MetadataTagAttribute};

use proc_macro::TokenStream;
use syn::{ItemStruct, parse_macro_input};

/// Creates a file usable by Lofty
///
/// See [here](https://github.com/Serial-ATA/lofty-rs/tree/main/examples/custom_resolver) for an example of how to use it.
#[proc_macro_derive(MetadataFile, attributes(metadata))]
pub fn metadata_file(input: TokenStream) -> TokenStream {
	let metadata_file = parse_macro_input!(input as MetadataFile);
	match metadata_file.emit() {
		Ok(ret) => ret,
		Err(e) => e.to_compile_error().into(),
	}
}

// TODO: Actually document this
/// Handles the boilerplate for custom error types
#[proc_macro_derive(MetadataError, attributes(error))]
pub fn metadata_error(input: TokenStream) -> TokenStream {
	let metadata_error = parse_macro_input!(input as MetadataError);
	metadata_error.emit()
}

#[proc_macro_attribute]
#[doc(hidden)]
pub fn tag(args_input: TokenStream, input: TokenStream) -> TokenStream {
	let attribute = parse_macro_input!(args_input as MetadataTagAttribute);
	let input = parse_macro_input!(input as ItemStruct);

	let metadata_tag = MetadataTag::new(attribute, input);
	metadata_tag.emit()
}
