//! What the byte readers answer, held to what they answered before the crate
//! was brought under the workspace's rules.
//!
//! Every reader is handed the same bytes and asked the same reads, so the slice
//! reader is the reference the buffered stream is held to: the ring a
//! `MediaSourceStream` refills, wraps and grows is the part a rewrite breaks,
//! and a slice has none of it. The transcripts are the ones the readers gave
//! as they were carried out of Symphonia.

use std::fmt::{Debug, Write};
use std::io::{Cursor, ErrorKind, Read, Seek, SeekFrom};

use console_core_byte_streams::{
  BufReader, FiniteStream, MediaSource, MediaSourceStream, MediaSourceStreamOptions,
  MonitorStream, ReadBytes, ReadOnlySource, ScopedStream, SeekBuffered, Seeking,
};
use console_core_checksums::Checksum;
use console_core_checksums::ogg::Crc32;
use console_core_never::Never;
use console_core_number_conversion::index;

mod noise;

type Failure = Box<dyn std::error::Error>;

fn noise(count: u32) -> Result<Vec<u8>, Never> {
  noise::seeded::<0xec57_c4bf>(count)
}

fn slice(bytes: &[u8]) -> Result<BufReader<'_>, Never> {
  BufReader::new(bytes)
}

fn seekable(bytes: Vec<u8>) -> Result<MediaSourceStream, Never> {
  MediaSourceStream::new(
    Box::new(Cursor::new(bytes)),
    MediaSourceStreamOptions::default(),
  )
}

fn forward_only(bytes: Vec<u8>) -> Result<MediaSourceStream, Never> {
  let Ok(source) = ReadOnlySource::new(Cursor::new(bytes));

  MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default())
}

fn scoped<B: ReadBytes>(inner: B, length: u64) -> Result<ScopedStream<B>, Never> {
  ScopedStream::new(inner, length)
}

fn monitored<B: ReadBytes>(inner: B) -> Result<MonitorStream<B, Crc32>, Never> {
  let Ok(crc) = Crc32::new(0);

  MonitorStream::new(inner, crc)
}

fn note<Answer: Debug>(
  out: &mut String,
  read: &str,
  answer: std::io::Result<Answer>,
  at: u64,
) -> Result<(), Failure> {
  let line = match answer {
    Ok(value) => format!("{read} @{at}: {value:?}"),
    Err(fault) => format!("{read} @{at}: {:?} {fault}", fault.kind()),
  };

  writeln!(out, "{line}")?;

  Ok(())
}

fn words<B: ReadBytes>(reader: &mut B, out: &mut String) -> Result<(), Failure> {
  note(out, "byte", reader.read_byte(), reader.pos())?;
  note(out, "u8", reader.read_u8(), reader.pos())?;
  note(out, "i8", reader.read_i8(), reader.pos())?;
  note(out, "double", reader.read_double_bytes(), reader.pos())?;
  note(out, "u16", reader.read_u16(), reader.pos())?;
  note(out, "i16", reader.read_i16(), reader.pos())?;
  note(out, "be u16", reader.read_be_u16(), reader.pos())?;
  note(out, "be i16", reader.read_be_i16(), reader.pos())?;
  note(out, "triple", reader.read_triple_bytes(), reader.pos())?;
  note(out, "u24", reader.read_u24(), reader.pos())?;
  note(out, "i24", reader.read_i24(), reader.pos())?;
  note(out, "be u24", reader.read_be_u24(), reader.pos())?;
  note(out, "be i24", reader.read_be_i24(), reader.pos())?;
  note(out, "quad", reader.read_quad_bytes(), reader.pos())?;
  note(out, "u32", reader.read_u32(), reader.pos())?;
  note(out, "i32", reader.read_i32(), reader.pos())?;
  note(out, "be u32", reader.read_be_u32(), reader.pos())?;
  note(out, "be i32", reader.read_be_i32(), reader.pos())?;
  note(out, "be u64", reader.read_be_u64(), reader.pos())?;
  note(
    out,
    "f32",
    reader.read_f32().map(f32::to_bits),
    reader.pos(),
  )?;
  note(
    out,
    "be f32",
    reader.read_be_f32().map(f32::to_bits),
    reader.pos(),
  )?;
  note(
    out,
    "f64",
    reader.read_f64().map(f64::to_bits),
    reader.pos(),
  )?;
  note(
    out,
    "be f64",
    reader.read_be_f64().map(f64::to_bits),
    reader.pos(),
  )?;

  Ok(())
}

fn blocks<B: ReadBytes>(reader: &mut B, out: &mut String) -> Result<(), Failure> {
  let mut some = [0_u8; 7];
  let counted = reader.read_buf(&mut some);

  note(out, "buf", counted.map(|count| (count, some)), reader.pos())?;

  let mut exact = [0_u8; 3];
  let filled = reader.read_buf_exact(&mut exact);

  note(out, "exact", filled.map(|()| exact), reader.pos())?;
  note(out, "boxed", reader.read_boxed_slice_exact(5), reader.pos())?;
  note(out, "ignore", reader.ignore_bytes(9), reader.pos())?;
  note(out, "byte", reader.read_byte(), reader.pos())?;

  Ok(())
}

fn every_width<B: ReadBytes>(reader: &mut B) -> Result<String, Failure> {
  let mut out = String::new();

  words(reader, &mut out)?;
  blocks(reader, &mut out)?;

  Ok(out)
}

fn the_end<B: ReadBytes>(reader: &mut B) -> Result<String, Failure> {
  let mut out = String::new();

  note(&mut out, "quad", reader.read_quad_bytes(), reader.pos())?;
  note(&mut out, "quad", reader.read_quad_bytes(), reader.pos())?;
  note(&mut out, "quad", reader.read_quad_bytes(), reader.pos())?;
  note(&mut out, "triple", reader.read_triple_bytes(), reader.pos())?;
  note(&mut out, "double", reader.read_double_bytes(), reader.pos())?;
  note(&mut out, "byte", reader.read_byte(), reader.pos())?;

  let mut four = [0_u8; 4];
  let counted = reader.read_buf(&mut four);

  note(
    &mut out,
    "buf",
    counted.map(|count| (count, four)),
    reader.pos(),
  )?;

  let mut one = [0_u8; 1];
  let filled = reader.read_buf_exact(&mut one);

  note(&mut out, "exact", filled.map(|()| one), reader.pos())?;
  note(&mut out, "ignore", reader.ignore_bytes(1), reader.pos())?;

  let mut none = [0_u8; 0];
  let nothing = reader.read_buf(&mut none);

  note(&mut out, "empty buf", nothing, reader.pos())?;
  note(
    &mut out,
    "boxed",
    reader.read_boxed_slice_exact(2),
    reader.pos(),
  )?;
  note(&mut out, "be u64", reader.read_be_u64(), reader.pos())?;
  note(&mut out, "byte", reader.read_byte(), reader.pos())?;

  Ok(out)
}

const EVERY_WIDTH: &str = concat!(
  "byte @1: 18\n",
  "u8 @2: 11\n",
  "i8 @3: -17\n",
  "double @5: [107, 73]\n",
  "u16 @7: 48559\n",
  "i16 @9: 5130\n",
  "be u16 @11: 61448\n",
  "be i16 @13: 31331\n",
  "triple @16: [28, 127, 112]\n",
  "u24 @19: 14538342\n",
  "i24 @22: 8031581\n",
  "be u24 @25: 1397896\n",
  "be i24 @28: 5906184\n",
  "quad @32: [71, 188, 96, 251]\n",
  "u32 @36: 3997945082\n",
  "i32 @40: -1906444271\n",
  "be u32 @44: 1012053025\n",
  "be i32 @48: 1795700691\n",
  "be u64 @56: 14844436903120016370\n",
  "f32 @60: 1923877680\n",
  "be f32 @64: 3483652275\n",
  "f64 @72: 5264525280401738722\n",
  "be f64 @80: 7275903669146825773\n",
  "buf @87: (7, [54, 188, 31, 231, 29, 6, 79])\n",
  "exact @90: [255, 216, 208]\n",
  "boxed @95: [182, 72, 87, 158, 5]\n",
  "ignore @104: ()\n",
  "byte @105: 140\n",
);

#[test]
fn every_reader_reads_every_width_in_both_orders_alike() -> Result<(), Failure> {
  let bytes = noise(128)?;
  let Ok(mut by_slice) = slice(&bytes);
  let Ok(mut lent) = slice(&bytes);
  let Ok(mut by_seeking) = seekable(bytes.clone());
  let Ok(mut by_reading) = forward_only(bytes.clone());
  let Ok(within) = slice(&bytes);
  let Ok(mut by_scope) = scoped(within, 128);
  let Ok(watched) = slice(&bytes);
  let Ok(mut by_monitor) = monitored(watched);

  let from_slice = every_width(&mut by_slice)?;
  let from_borrow = every_width(&mut &mut lent)?;
  let from_seeking = every_width(&mut by_seeking)?;
  let from_reading = every_width(&mut by_reading)?;
  let from_scope = every_width(&mut by_scope)?;
  let from_monitor = every_width(&mut by_monitor)?;

  assert_eq!(from_slice, EVERY_WIDTH);
  assert_eq!(from_borrow, EVERY_WIDTH);
  assert_eq!(from_seeking, EVERY_WIDTH);
  assert_eq!(from_reading, EVERY_WIDTH);
  assert_eq!(from_scope, EVERY_WIDTH);
  assert_eq!(from_monitor, EVERY_WIDTH);

  Ok(())
}

const SLICE_END: &str = concat!(
  "quad @4: [18, 11, 239, 107]\n",
  "quad @8: [73, 175, 189, 10]\n",
  "quad @8: UnexpectedEof buffer underrun\n",
  "triple @8: UnexpectedEof buffer underrun\n",
  "double @8: UnexpectedEof buffer underrun\n",
  "byte @9: 20\n",
  "buf @9: (0, [0, 0, 0, 0])\n",
  "exact @9: UnexpectedEof buffer underrun\n",
  "ignore @9: UnexpectedEof buffer underrun\n",
  "empty buf @9: 0\n",
  "boxed @9: UnexpectedEof buffer underrun\n",
  "be u64 @9: UnexpectedEof buffer underrun\n",
  "byte @9: UnexpectedEof buffer underrun\n",
);
const STREAM_END: &str = concat!(
  "quad @4: [18, 11, 239, 107]\n",
  "quad @8: [73, 175, 189, 10]\n",
  "quad @9: UnexpectedEof end of stream\n",
  "triple @9: UnexpectedEof end of stream\n",
  "double @9: UnexpectedEof end of stream\n",
  "byte @9: UnexpectedEof end of stream\n",
  "buf @9: UnexpectedEof end of stream\n",
  "exact @9: UnexpectedEof end of stream\n",
  "ignore @9: UnexpectedEof end of stream\n",
  "empty buf @9: 0\n",
  "boxed @9: UnexpectedEof end of stream\n",
  "be u64 @9: UnexpectedEof end of stream\n",
  "byte @9: UnexpectedEof end of stream\n",
);
const SCOPE_END: &str = concat!(
  "quad @4: [18, 11, 239, 107]\n",
  "quad @8: [73, 175, 189, 10]\n",
  "quad @8: UnexpectedEof out of bounds\n",
  "triple @8: UnexpectedEof out of bounds\n",
  "double @8: UnexpectedEof out of bounds\n",
  "byte @9: 20\n",
  "buf @9: (0, [0, 0, 0, 0])\n",
  "exact @9: UnexpectedEof out of bounds\n",
  "ignore @9: UnexpectedEof out of bounds\n",
  "empty buf @9: 0\n",
  "boxed @9: UnexpectedEof out of bounds\n",
  "be u64 @9: UnexpectedEof out of bounds\n",
  "byte @9: UnexpectedEof out of bounds\n",
);

#[test]
fn each_reader_says_where_its_bytes_ran_out() -> Result<(), Failure> {
  let bytes = noise(9)?;
  let longer = noise(20)?;
  let Ok(mut by_slice) = slice(&bytes);
  let Ok(mut by_seeking) = seekable(bytes.clone());
  let Ok(mut by_reading) = forward_only(bytes.clone());
  let Ok(within) = slice(&longer);
  let Ok(mut by_scope) = scoped(within, 9);
  let Ok(watched) = slice(&bytes);
  let Ok(mut by_monitor) = monitored(watched);

  let from_slice = the_end(&mut by_slice)?;
  let from_seeking = the_end(&mut by_seeking)?;
  let from_reading = the_end(&mut by_reading)?;
  let from_scope = the_end(&mut by_scope)?;
  let from_monitor = the_end(&mut by_monitor)?;

  assert_eq!(from_slice, SLICE_END);
  assert_eq!(from_seeking, STREAM_END);
  assert_eq!(from_reading, STREAM_END);
  assert_eq!(from_scope, SCOPE_END);
  assert_eq!(from_monitor, SLICE_END);

  Ok(())
}

#[derive(Clone, Copy)]
enum Step {
  Byte,
  Double,
  Triple,
  Quad,
  Long,
  Buf(u16),
  Exact(u16),
  Boxed(u16),
  Skip(u32),
}

const PLAN: [Step; 16] = [
  Step::Byte,
  Step::Triple,
  Step::Buf(4099),
  Step::Double,
  Step::Quad,
  Step::Skip(311),
  Step::Long,
  Step::Exact(1031),
  Step::Triple,
  Step::Boxed(77),
  Step::Skip(140_001),
  Step::Quad,
  Step::Buf(65_535),
  Step::Double,
  Step::Exact(32_769),
  Step::Byte,
];

fn heard<Answer: AsRef<[u8]>>(
  transcript: &mut Vec<u8>,
  answer: std::io::Result<Answer>,
) -> Result<(), Failure> {
  let read = answer?;

  transcript.extend_from_slice(read.as_ref());

  Ok(())
}

fn buffered<B: ReadBytes>(reader: &mut B, length: u16) -> std::io::Result<Vec<u8>> {
  let mut buffer: Vec<u8> = (0..length).map(|_| 0).collect();
  let count = reader.read_buf(&mut buffer)?;
  let Ok(kept) = index(count);

  buffer.truncate(kept);

  Ok(buffer)
}

fn exactly<B: ReadBytes>(reader: &mut B, length: u16) -> std::io::Result<Vec<u8>> {
  let mut buffer: Vec<u8> = (0..length).map(|_| 0).collect();

  reader.read_buf_exact(&mut buffer)?;

  Ok(buffer)
}

fn taken<B: ReadBytes>(
  reader: &mut B,
  step: Step,
  transcript: &mut Vec<u8>,
) -> Result<(), Failure> {
  match step {
    Step::Byte => heard(transcript, reader.read_byte().map(|byte| [byte]))?,
    Step::Double => heard(transcript, reader.read_double_bytes())?,
    Step::Triple => heard(transcript, reader.read_triple_bytes())?,
    Step::Quad => heard(transcript, reader.read_quad_bytes())?,
    Step::Long => heard(transcript, reader.read_be_u64().map(u64::to_be_bytes))?,
    Step::Buf(length) => heard(transcript, buffered(reader, length))?,
    Step::Exact(length) => heard(transcript, exactly(reader, length))?,
    Step::Boxed(length) => heard(transcript, reader.read_boxed_slice_exact(u64::from(length)))?,
    Step::Skip(count) => reader.ignore_bytes(u64::from(count))?,
  }

  transcript.extend_from_slice(&reader.pos().to_be_bytes());

  Ok(())
}

struct Walked {
  steps: u32,
  ended_at: u64,
  transcript: Vec<u8>,
}

const WORDS: [Step; 10] = [
  Step::Byte,
  Step::Triple,
  Step::Double,
  Step::Quad,
  Step::Triple,
  Step::Long,
  Step::Triple,
  Step::Byte,
  Step::Double,
  Step::Skip(1),
];

fn walk<B: ReadBytes>(reader: &mut B, plan: &[Step], most: u32) -> Result<Walked, Never> {
  let mut walked = Walked {
    steps: 0,
    ended_at: 0,
    transcript: Vec::new(),
  };
  let Ok(longest) = index(most);

  for position in 0..longest {
    let step = match position.checked_rem(plan.len()).and_then(|at| plan.get(at)) {
      Some(step) => step,
      None => break,
    };

    match taken(reader, *step, &mut walked.transcript) {
      Ok(()) => walked.steps = walked.steps.saturating_add(1),
      Err(_ran_out) => break,
    }
  }

  walked.ended_at = reader.pos();

  Ok(walked)
}

#[test]
fn a_buffered_stream_reads_across_its_refills_what_a_slice_reads() -> Result<(), Failure> {
  let bytes = noise(491_520)?;
  let Ok(mut by_slice) = slice(&bytes);
  let Ok(mut by_seeking) = seekable(bytes.clone());
  let Ok(mut by_reading) = forward_only(bytes.clone());
  let Ok(expected) = walk(&mut by_slice, &PLAN, 4096);
  let Ok(sought) = walk(&mut by_seeking, &PLAN, 4096);
  let Ok(read) = walk(&mut by_reading, &PLAN, 4096);
  let Ok(digest) = console_core_checksums::crc32::of(&expected.transcript);

  assert_eq!(
    (expected.steps, expected.ended_at, digest),
    (35, 491_520, 0xc757_0c2d)
  );
  assert_eq!(
    (sought.steps, sought.ended_at),
    (expected.steps, expected.ended_at)
  );
  assert!(
    sought.transcript == expected.transcript,
    "the seekable stream read something else"
  );
  assert_eq!(
    (read.steps, read.ended_at),
    (expected.steps, expected.ended_at)
  );
  assert!(
    read.transcript == expected.transcript,
    "the forward-only stream read something else"
  );

  Ok(())
}

#[test]
fn a_buffered_stream_reads_words_across_its_refills_what_a_slice_reads() -> Result<(), Failure> {
  let bytes = noise(491_520)?;
  let Ok(mut by_slice) = slice(&bytes);
  let Ok(mut by_seeking) = seekable(bytes.clone());
  let Ok(mut by_reading) = forward_only(bytes.clone());
  let Ok(expected) = walk(&mut by_slice, &WORDS, 200_000);
  let Ok(sought) = walk(&mut by_seeking, &WORDS, 200_000);
  let Ok(read) = walk(&mut by_reading, &WORDS, 200_000);
  let Ok(digest) = console_core_checksums::crc32::of(&expected.transcript);

  assert_eq!(
    (expected.steps, expected.ended_at, digest),
    (175_543, 491_518, 0x5334_597e)
  );
  assert_eq!((sought.steps, sought.ended_at), (175_543, 491_520));
  assert!(
    sought.transcript == expected.transcript,
    "the seekable stream read something else"
  );
  assert_eq!((read.steps, read.ended_at), (175_543, 491_520));
  assert!(
    read.transcript == expected.transcript,
    "the forward-only stream read something else"
  );

  Ok(())
}

fn at<const WIDTH: usize>(bytes: &[u8], from: u32) -> Result<Option<[u8; WIDTH]>, Never> {
  let Ok(start) = index(from);

  Ok(
    bytes
      .get(start..)
      .and_then(|rest| rest.first_chunk::<WIDTH>())
      .copied(),
  )
}

#[test]
fn a_seekable_stream_goes_where_it_is_sent() -> Result<(), Failure> {
  let bytes = noise(300_000)?;
  let Ok(mut stream) = seekable(bytes.clone());
  let started = stream.seek(SeekFrom::Start(100_000))?;
  let quad = stream.read_quad_bytes()?;
  let back = stream.seek(SeekFrom::Current(-50))?;
  let byte = stream.read_byte()?;
  let after_byte = stream.stream_position()?;
  let ahead = stream.seek(SeekFrom::Current(1000))?;
  let ahead_pos = stream.pos();
  let double = stream.read_double_bytes()?;
  let near_end = stream.seek(SeekFrom::End(-10))?;

  assert_eq!(started, 100_000);
  assert_eq!(Ok(Some(quad)), at::<4>(&bytes, 100_000));
  assert_eq!(back, 99_954);
  assert_eq!(Ok(Some([byte])), at::<1>(&bytes, 99_954));
  assert_eq!(after_byte, 99_955);
  assert_eq!(ahead, 100_955);
  assert_eq!(ahead_pos, 100_955);
  assert_eq!(Ok(Some(double)), at::<2>(&bytes, 100_955));
  assert_eq!(near_end, 299_990);

  let mut rest = [0_u8; 16];
  let count = ReadBytes::read_buf(&mut stream, &mut rest)?;

  assert_eq!(
    Ok(rest.first_chunk::<10>().copied()),
    at::<10>(&bytes, 299_990)
  );
  assert_eq!(count, 10);

  let ended = ReadBytes::read_buf(&mut stream, &mut rest).map_err(|fault| fault.kind());
  let rewound = stream.seek(SeekFrom::Start(0))?;
  let first = stream.read_byte()?;
  let skipped = stream.ignore_bytes(200_000).map_err(|fault| fault.kind());
  let skipped_to = stream.pos();
  let far = stream.read_quad_bytes()?;

  assert_eq!(ended, Err(ErrorKind::UnexpectedEof));
  assert_eq!(rewound, 0);
  assert_eq!(Ok(Some([first])), at::<1>(&bytes, 0));
  assert_eq!(skipped, Ok(()));
  assert_eq!(skipped_to, 200_001);
  assert_eq!(Ok(Some(far)), at::<4>(&bytes, 200_001));

  Ok(())
}

#[test]
fn a_forward_only_stream_reads_where_it_cannot_seek() -> Result<(), Failure> {
  let bytes = noise(300_000)?;
  let Ok(mut stream) = forward_only(bytes.clone());
  let refused = stream
    .seek(SeekFrom::Start(0))
    .map_err(|fault| format!("{:?} {fault}", fault.kind()));
  let first = stream.read_byte()?;
  let position = stream.stream_position()?;

  assert_eq!(
    refused,
    Err(String::from("Other source does not support seeking"))
  );
  assert_eq!(Ok(Some([first])), at::<1>(&bytes, 0));
  assert_eq!(position, 1);

  let forward = stream
    .seek(SeekFrom::Current(5))
    .map_err(|fault| fault.kind());
  let skipped = stream.ignore_bytes(200_000).map_err(|fault| fault.kind());
  let skipped_to = stream.pos();
  let far = stream.read_quad_bytes()?;

  assert_eq!(forward, Err(ErrorKind::Other));
  assert_eq!(skipped, Ok(()));
  assert_eq!(skipped_to, 200_001);
  assert_eq!(Ok(Some(far)), at::<4>(&bytes, 200_001));

  Ok(())
}

fn buffered_lengths(stream: &MediaSourceStream) -> Result<(u64, String), Never> {
  Ok((
    stream.pos(),
    format!(
      "{} {}",
      stream.read_buffer_len(),
      stream.unread_buffer_len()
    ),
  ))
}

#[test]
fn a_stream_seeks_back_and_forth_within_what_it_holds() -> Result<(), Failure> {
  let bytes = noise(1_048_576)?;
  let Ok(mut stream) = seekable(bytes.clone());
  let Ok(fresh) = buffered_lengths(&stream);

  assert_eq!(fresh, (0, String::from("0 0")));

  stream.ignore_bytes(5122)?;

  let Ok(skipped) = buffered_lengths(&stream);

  assert_eq!(skipped, (5122, String::from("5122 2046")));

  let upper = stream.read_byte()?;
  let back = stream.seek_buffered_rel(-1000);
  let Ok(behind) = buffered_lengths(&stream);

  assert_eq!(back, 4123);
  assert_eq!(behind, (4123, String::from("4123 3045")));

  let returned = stream.seek_buffered_rel(999);
  let again = stream.read_byte()?;
  let start = stream.seek_buffered_rel(-1_000_000);
  let end = stream.seek_buffered_rel(1_000_000);
  let middle = stream.seek_buffered(3000);
  let byte = stream.read_byte()?;
  let past = stream.seek_buffered(100_000);

  assert_eq!(returned, 5122);
  assert_eq!(again, upper);
  assert_eq!(start, 0);
  assert_eq!(end, 7168);
  assert_eq!(middle, 3000);
  assert_eq!(Ok(Some([byte])), at::<1>(&bytes, 3000));
  assert_eq!(past, 7168);

  let Ok(()) = stream.seek_buffered_rev(100);
  let Ok(reversed) = buffered_lengths(&stream);

  assert_eq!(reversed, (7068, String::from("7068 100")));

  Ok(())
}

#[test]
fn a_stream_keeps_what_it_was_asked_to_keep_for_seeking_back() -> Result<(), Failure> {
  let bytes = noise(1_048_576)?;
  let Ok(mut kept) = seekable(bytes.clone());
  let Ok(mut plain) = seekable(bytes.clone());

  kept.ensure_seekback_buffer(100_000);
  kept.ignore_bytes(150_000)?;
  plain.ignore_bytes(150_000)?;

  let Ok(kept_lengths) = buffered_lengths(&kept);
  let Ok(plain_lengths) = buffered_lengths(&plain);

  assert_eq!(kept_lengths, (150_000, String::from("150000 12816")));
  assert_eq!(plain_lengths, (150_000, String::from("33792 31744")));
  assert_eq!(kept.seek_buffered_rel(-100_000), 50_000);
  assert_eq!(plain.seek_buffered_rel(-100_000), 181_744);

  let quad = kept.read_quad_bytes()?;

  assert_eq!(Ok(Some(quad)), at::<4>(&bytes, 50_000));

  kept.read_byte()?;
  kept.ensure_seekback_buffer(10);

  let Ok(regrown) = buffered_lengths(&kept);

  assert_eq!(regrown, (50_005, String::from("50005 112811")));

  Ok(())
}

#[test]
fn a_monitor_sees_every_byte_read_and_none_skipped() -> Result<(), Failure> {
  let bytes = noise(64)?;
  let Ok(inner) = slice(&bytes);
  let Ok(mut watched) = monitored(inner);
  let mut block = [0_u8; 5];
  let mut tail = [0_u8; 32];

  watched.read_byte()?;
  watched.read_double_bytes()?;
  watched.read_triple_bytes()?;
  watched.read_quad_bytes()?;
  watched.read_be_u64()?;
  watched.read_buf_exact(&mut block)?;
  watched.ignore_bytes(7)?;
  watched.read_boxed_slice_exact(3)?;

  let count = watched.read_buf(&mut tail)?;
  let Ok(expected_first) = Crc32::new(0);
  let mut expected = expected_first;
  let seen = bytes.get(..23).ok_or("short")?;
  let after = bytes.get(30..).ok_or("short")?;
  let Ok(()) = expected.process_buf_bytes(seen);
  let Ok(()) = expected.process_buf_bytes(after);

  let Ok(monitor) = watched.monitor();

  assert_eq!(count, 31);
  assert_eq!(monitor.crc(), expected.crc());
  assert_eq!(monitor.crc(), Ok(0x57aa_594f));
  assert_eq!(watched.pos(), 64);

  let Ok(kept) = watched.inner();

  assert_eq!(kept.bytes_available(), 0);

  let Ok(lent) = watched.inner_mut();

  assert_eq!(
    lent.read_byte().map_err(|fault| fault.kind()),
    Err(ErrorKind::UnexpectedEof)
  );

  let Ok(taken_back) = watched.into_inner();

  assert_eq!(taken_back.pos(), 64);

  Ok(())
}

#[test]
fn a_scope_ends_where_it_was_told_to() -> Result<(), Failure> {
  let bytes = noise(64)?;
  let Ok(inner) = slice(&bytes);
  let Ok(mut bounded) = scoped(inner, 20);
  let mut wide = [0_u8; 32];

  assert_eq!((bounded.byte_len(), bounded.bytes_available()), (20, 20));

  let quad = bounded.read_quad_bytes()?;
  let count = bounded.read_buf(&mut wide)?;

  assert_eq!(Ok(Some(quad)), at::<4>(&bytes, 0));
  assert_eq!(count, 16);
  assert_eq!(Ok(wide.first_chunk::<16>().copied()), at::<16>(&bytes, 4));
  assert_eq!((bounded.bytes_available(), bounded.pos()), (0, 20));
  let refused = bounded
    .read_byte()
    .map_err(|fault| format!("{:?} {fault}", fault.kind()));

  assert_eq!(refused, Err(String::from("UnexpectedEof out of bounds")));
  assert_eq!(
    bounded.ignore_bytes(1).map_err(|fault| fault.kind()),
    Err(ErrorKind::UnexpectedEof)
  );

  let Ok(mut beyond) = bounded.into_inner();
  let after = beyond.read_byte()?;

  assert_eq!(Ok(Some([after])), at::<1>(&bytes, 20));

  Ok(())
}

#[test]
fn a_scope_refuses_a_read_that_would_leave_it() -> Result<(), Failure> {
  let bytes = noise(64)?;
  let Ok(inner) = slice(&bytes);
  let Ok(mut bounded) = scoped(inner, 6);
  let mut seven = [0_u8; 7];

  assert_eq!(
    bounded
      .read_buf_exact(&mut seven)
      .map_err(|fault| fault.kind()),
    Err(ErrorKind::UnexpectedEof)
  );
  assert_eq!(
    bounded.ignore_bytes(7).map_err(|fault| fault.kind()),
    Err(ErrorKind::UnexpectedEof)
  );
  assert_eq!(
    bounded
      .read_boxed_slice_exact(7)
      .map_err(|fault| fault.kind()),
    Err(ErrorKind::UnexpectedEof)
  );
  assert_eq!(bounded.pos(), 0);

  let triple = bounded.read_triple_bytes()?;

  assert_eq!(Ok(Some(triple)), at::<3>(&bytes, 0));
  assert_eq!(bounded.ignore().map_err(|fault| fault.kind()), Ok(()));
  assert_eq!((bounded.bytes_available(), bounded.pos()), (3, 6));

  Ok(())
}

#[test]
fn a_scope_over_a_stream_seeks_only_within_itself() -> Result<(), Failure> {
  let bytes = noise(4096)?;
  let Ok(mut stream) = seekable(bytes.clone());

  stream.ignore_bytes(100)?;

  let Ok(mut bounded) = scoped(&mut stream, 200);

  bounded.ignore_bytes(150)?;

  let lengths = format!(
    "{} {}",
    bounded.read_buffer_len(),
    bounded.unread_buffer_len()
  );

  assert_eq!(lengths, "150 50");
  assert_eq!(bounded.seek_buffered_rel(-1000), 100);
  assert_eq!(bounded.seek_buffered_rel(1000), 150);
  assert_eq!(bounded.seek_buffered(10), 100);
  assert_eq!(bounded.seek_buffered(1000), 300);

  Ok(())
}

#[test]
fn a_slice_reader_scans_and_lends_what_it_holds() -> Result<(), Failure> {
  let text = b"abc\0def\0";
  let wide = [b'a', 0, 0, b'b', 0, 0, b'c', 0];
  let Ok(mut narrow) = slice(text);
  let Ok(mut aligned) = slice(&wide);
  let Ok(mut unaligned) = slice(&wide);
  let Ok(mut short) = slice(text);

  let first = narrow.scan_bytes_aligned_ref(&[0], 1, 100)?;
  let cut = narrow.scan_bytes_aligned_ref(&[0], 1, 2)?;
  let missed = narrow.scan_bytes_aligned_ref(b"xy", 1, 1)?;
  let rest = narrow.scan_bytes_aligned_ref(b"x", 1, 100)?;
  let paired = aligned.scan_bytes_aligned_ref(&[0, 0], 2, 100)?;
  let early = unaligned.scan_bytes_aligned_ref(&[0, 0], 1, 100)?;
  let lent = short.read_buf_bytes_ref(3)?;

  assert_eq!(first, b"abc\0");
  assert_eq!(cut, b"de");
  assert_eq!(missed, b"f");
  assert_eq!(rest, b"\0");
  assert_eq!(paired, [b'a', 0, 0, b'b', 0, 0]);
  assert_eq!(early, [b'a', 0, 0]);
  assert_eq!(lent, b"abc");

  let underrun = short
    .read_buf_bytes_ref(6)
    .map_err(|fault| format!("{:?} {fault}", fault.kind()));

  assert_eq!(underrun, Err(String::from("UnexpectedEof buffer underrun")));
  assert_eq!(
    (short.byte_len(), short.bytes_available(), short.pos()),
    (8, 5, 3)
  );
  assert_eq!(
    short.read_buf_bytes_available_ref(),
    Ok(b"\0def\0".as_slice())
  );
  assert_eq!((short.bytes_available(), short.pos()), (0, 8));
  assert_eq!(short.read_buf_bytes_available_ref(), Ok(b"".as_slice()));

  Ok(())
}

#[test]
fn each_source_says_whether_it_can_seek_and_how_long_it_is() -> Result<(), Failure> {
  let bytes = noise(1000)?;
  let licence = std::fs::File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/LICENSE"))?;
  let metadata = licence.metadata()?;
  let measured = metadata.len();
  let Ok(sought) = seekable(bytes.clone());
  let Ok(read) = forward_only(bytes.clone());
  let cursor = Cursor::new(bytes.clone());
  let Ok(forward) = ReadOnlySource::new(Cursor::new(bytes.clone()));

  assert_eq!(
    (cursor.seeking(), cursor.byte_len()),
    (Seeking::Anywhere, Some(1000))
  );
  assert_eq!(
    (forward.seeking(), forward.byte_len()),
    (Seeking::ForwardOnly, None)
  );
  assert_eq!(
    (licence.seeking(), licence.byte_len()),
    (Seeking::Anywhere, Some(measured))
  );
  assert_eq!(
    (sought.seeking(), sought.byte_len()),
    (Seeking::Anywhere, Some(1000))
  );
  assert_eq!(
    (read.seeking(), read.byte_len()),
    (Seeking::ForwardOnly, None)
  );

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "both fixture readers wrap an in-memory Cursor and cannot wait on a device"
  )
)]
fn a_stream_reads_to_its_end_through_the_standard_reader() -> Result<(), Failure> {
  const FIXTURE_LENGTH: u32 = 491_520;

  let bytes = noise(FIXTURE_LENGTH)?;
  let Ok(mut sought) = seekable(bytes.clone());
  let Ok(read) = forward_only(bytes.clone());
  let mut from_seeking = Vec::new();
  let mut from_reading = Vec::new();
  let mut nothing = [0_u8; 0];

  assert_eq!(
    sought.read(&mut nothing).map_err(|fault| fault.kind()),
    Ok(0)
  );

  let limit = u64::from(FIXTURE_LENGTH).saturating_add(1);

  sought.take(limit).read_to_end(&mut from_seeking)?;
  read.take(limit).read_to_end(&mut from_reading)?;

  assert!(
    from_seeking == bytes,
    "the seekable stream read something else"
  );
  assert!(
    from_reading == bytes,
    "the forward-only stream read something else"
  );

  Ok(())
}
