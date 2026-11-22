//! The readers stacked the way the formats stack them: a checksum over a slice, a bound over
//! a checksum, bits over a bound, and a checksum over a stream that cannot seek.

use console_core_byte_streams::{
  BitStreamLtr, BufReader, MediaSourceStream, MediaSourceStreamOptions, MonitorStream,
  ReadBitsLtr, ReadBytes, ReadOnlySource, ScopedStream,
};
use console_core_checksums::{Checksum, crc8::Crc8Ccitt, md5::Md5, ogg::Crc32};
use std::io::Cursor;

#[test]
fn a_prefix_leaves_the_next_field_and_keeps_a_short_read_at_eof() -> std::io::Result<()> {
  let Ok(source) = ReadOnlySource::new(Cursor::new(b"firstnext"));
  let Ok(mut stream) =
    MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
  let empty = stream.read_prefix(0)?;
  let first = stream.read_prefix(5)?;
  let last = stream.read_prefix(9)?;

  assert!(empty.is_empty());
  assert_eq!(first, b"first");
  assert_eq!(last, b"next");

  Ok(())
}

#[test]
fn checksum_monitoring_composes_with_a_byte_reader() -> std::io::Result<()> {
  let Ok(crc) = Crc32::new(0);
  let Ok(digits) = BufReader::new(b"123456789");
  let Ok(mut stream) = MonitorStream::new(digits, crc);

  let double = stream.read_double_bytes()?;
  let triple = stream.read_triple_bytes()?;
  let quad = stream.read_quad_bytes()?;

  assert_eq!(double, *b"12");
  assert_eq!(triple, *b"345");
  assert_eq!(quad, *b"6789");

  let Ok(monitor) = stream.monitor();

  assert_eq!(monitor.crc(), Ok(0x89a1_897f));

  Ok(())
}

#[test]
fn a_bounded_bit_reader_keeps_checksum_and_source_position() -> std::io::Result<()> {
  let Ok(crc) = Crc8Ccitt::new(0);
  let Ok(text) = BufReader::new(b"123tail");
  let Ok(checked) = MonitorStream::new(text, crc);
  let Ok(mut bounded) = ScopedStream::new(checked, 3);
  let Ok(mut bits) = BitStreamLtr::new(&mut bounded);

  let field = bits.read_bits_leq32(24)?;

  assert_eq!(field, 0x31_3233);
  assert_eq!(
    bits.read_bits_leq32(1).map_err(|fault| fault.kind()),
    Err(std::io::ErrorKind::UnexpectedEof)
  );

  let Ok(mut expected) = Crc8Ccitt::new(0);
  let Ok(()) = expected.process_buf_bytes(b"123");
  let Ok(watched) = bounded.into_inner();
  let Ok(monitor) = watched.monitor();

  assert_eq!(monitor.crc(), expected.crc());

  let Ok(mut rest) = watched.into_inner();

  let tail = rest.read_quad_bytes()?;

  assert_eq!(tail, *b"tail");

  Ok(())
}

#[test]
fn a_forward_only_source_can_be_read_without_a_media_container() -> std::io::Result<()> {
  let Ok(source) = ReadOnlySource::new(Cursor::new(b"123456789"));
  let Ok(buffered) =
    MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
  let Ok(mut stream) = MonitorStream::new(buffered, Md5::default());
  let mut bytes = [0; 9];

  stream.read_buf_exact(&mut bytes)?;

  assert_eq!(&bytes, b"123456789");

  let Ok(monitor) = stream.monitor();
  let Ok(hash) = monitor.md5();

  assert_eq!(
    hash,
    [
      0x25, 0xf9, 0xe7, 0x94, 0x32, 0x3b, 0x45, 0x38, 0x85, 0xf5, 0x18, 0x1f, 0x1b, 0x62, 0x4d,
      0x0b
    ]
  );

  Ok(())
}
