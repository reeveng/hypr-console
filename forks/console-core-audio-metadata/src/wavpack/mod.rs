//! WavPack specific items
pub mod error;
mod properties;
mod read;

use crate::ape::tag::ApeTag;
use crate::id3::v1::tag::Id3v1Tag;

use console_core_audio_metadata_derive::MetadataFile;

// Exports
pub use properties::WavPackProperties;

/// A WavPack file
#[derive(MetadataFile, Default)]
#[metadata(read_fn = "read::read_from")]
#[metadata(internal_write_module_do_not_use_anywhere_else)]
pub struct WavPackFile {
	/// An ID3v1 tag
	#[metadata(tag_type = "Id3v1")]
	pub(crate) id3v1_tag: Option<Id3v1Tag>,
	/// An APEv1/v2 tag
	#[metadata(tag_type = "Ape")]
	pub(crate) ape_tag: Option<ApeTag>,
	/// The file's audio properties
	pub(crate) properties: WavPackProperties,
}
