//! WAV specific items

pub mod error;
mod properties;
pub(crate) mod read;
pub(crate) mod tag;

use crate::id3::v2::tag::Id3v2Tag;

use console_core_audio_metadata_derive::MetadataFile;

// Exports
pub use crate::iff::wav::properties::{WavFormat, WavProperties};
pub use tag::RiffInfoList;

/// A WAV file
#[derive(MetadataFile)]
#[metadata(read_fn = "read::read_from")]
#[metadata(internal_write_module_do_not_use_anywhere_else)]
pub struct WavFile {
	/// A RIFF INFO LIST
	#[metadata(tag_type = "RiffInfo")]
	pub(crate) riff_info_tag: Option<RiffInfoList>,
	/// An ID3v2 tag
	#[metadata(tag_type = "Id3v2")]
	pub(crate) id3v2_tag: Option<Id3v2Tag>,
	/// The file's audio properties
	pub(crate) properties: WavProperties,
}
