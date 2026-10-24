//! AIFF specific items

pub mod error;
mod properties;
mod read;
pub(crate) mod tag;

use crate::id3::v2::tag::Id3v2Tag;

use console_core_audio_metadata_derive::MetadataFile;

// Exports

pub use properties::{AiffCompressionType, AiffProperties};
pub use tag::{AiffTextChunks, Comment};

/// An AIFF file
#[derive(MetadataFile)]
#[metadata(read_fn = "read::read_from")]
#[metadata(internal_write_module_do_not_use_anywhere_else)]
pub struct AiffFile {
	/// Any text chunks included in the file
	#[metadata(tag_type = "AiffText")]
	pub(crate) text_chunks_tag: Option<AiffTextChunks>,
	/// An ID3v2 tag
	#[metadata(tag_type = "Id3v2")]
	pub(crate) id3v2_tag: Option<Id3v2Tag>,
	/// The file's audio properties
	pub(crate) properties: AiffProperties,
}
