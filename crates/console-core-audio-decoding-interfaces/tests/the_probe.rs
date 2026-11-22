use std::io::Cursor;

use console_core_audio_decoding_interfaces::errors::{Error, unsupported_error};
use console_core_audio_decoding_interfaces::formats::{FormatOptions, FormatReader};
use console_core_audio_decoding_interfaces::io::{
  MediaSourceStream, MediaSourceStreamOptions, ReadBytes,
};
use console_core_audio_decoding_interfaces::probe::{Descriptor, Instantiate, Probe};
use console_core_never::Never;

fn nothing(
  _source: MediaSourceStream,
  _options: &FormatOptions,
) -> Result<Box<dyn FormatReader>, Error> {
  unsupported_error("a test reader")
}

#[cfg_attr(
  dylint_lib = "explicit034_no_unclaimed_answer",
  allow(
    explicit034_no_unclaimed_answer,
    reason = "a scorer has the signature a descriptor holds it by"
  )
)]
fn unscored(_context: &[u8]) -> Result<u8, Never> {
  Ok(0)
}

fn descriptor(markers: &'static [&'static [u8]]) -> Result<Descriptor, Never> {
  Ok(Descriptor {
    short_name: "test",
    long_name: "A test format",
    extensions: &["tst"],
    mime_types: &[],
    markers,
    score: unscored,
    inst: Instantiate::Format(nothing),
  })
}

fn stream(bytes: Vec<u8>) -> Result<MediaSourceStream, Never> {
  MediaSourceStream::new(
    Box::new(Cursor::new(bytes)),
    MediaSourceStreamOptions::default(),
  )
}

#[test]
fn a_marker_is_found_behind_other_bytes_and_the_stream_is_left_at_it() -> Result<(), Error> {
  let mut probe = Probe::default();
  let Ok(declared) = descriptor(&[b"fLaC", b"OggS\x00\x02"]);
  let Ok(()) = probe.register(&declared);

  let mut bytes = vec![0x11, 0x22, 0x33, 0x66];
  bytes.extend_from_slice(b"OggS\x00\x02rest of the stream");
  let Ok(mut source) = stream(bytes);

  let found = probe.next(&mut source)?;

  assert!(matches!(found, Instantiate::Format(_)));
  assert_eq!(source.pos(), 4);

  Ok(())
}

#[test]
fn a_stream_with_no_marker_is_unsupported() {
  let mut probe = Probe::default();
  let Ok(declared) = descriptor(&[b"fLaC"]);
  let Ok(()) = probe.register(&declared);

  let Ok(mut source) = stream(vec![0x42; 64]);

  assert!(matches!(
    probe.next(&mut source),
    Err(Error::Unsupported(_))
  ));
}

#[test]
fn a_marker_too_long_to_be_read_out_is_never_found() {
  let mut probe = Probe::default();
  let Ok(declared) = descriptor(&[b"01234567890123456"]);
  let Ok(()) = probe.register(&declared);

  let Ok(mut source) = stream(b"01234567890123456 and more bytes after it".to_vec());

  assert!(matches!(
    probe.next(&mut source),
    Err(Error::Unsupported(_))
  ));
}
