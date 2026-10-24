//! [`Picture`] error types
//!
//! [`Picture`]: crate::picture::Picture

use std::borrow::Cow;

use console_core_audio_metadata_derive::MetadataError;

/// Unable to parse a [`Picture`]
///
/// [`Picture`]: crate::picture::Picture
#[derive(MetadataError)]
#[error(message = "failed to parse picture")]
pub struct PictureParseError {
	#[error(from(
		std::io::Error,
		crate::error::NotEnoughDataError,
		crate::error::SizeMismatchError,
		crate::error::AllocationError,
		crate::util::text::TextDecodingError,
		UnknownImageFormatError,
		data_encoding::DecodeError,
	))]
	source: Box<dyn core::error::Error + Send + Sync + 'static>,
}

impl PictureParseError {
	pub(super) fn message(message: impl Into<Cow<'static, str>>) -> Self {
		let message = message.into();
		Self {
			source: message.into(),
		}
	}
}

/// Unable to determine the image format
#[derive(MetadataError)]
#[error(message = "unable to determine image format")]
pub struct UnknownImageFormatError;
