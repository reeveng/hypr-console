//! What the bit readers answer, held to what they answered before the crate
//! was brought under the workspace's rules.
//!
//! The slice readers and the stream reader are asked the same fields at every
//! width, so the stream, which holds a byte at a time, is held to the reader
//! that holds eight. The vectors with a picture in them -- the runs of ones
//! and zeros, the edges of a cache, the sentence a codebook spells -- are the
//! ones Symphonia's own tests carried, and the transcripts are what the
//! readers said as they were carried out of it.

use std::fmt::{Debug, Write};

use console_core_byte_streams::vlc::{
  BitOrder, Codebook, CodebookBuilder, CodebookEntry, Entry8x16, Entry16x16, Entry32x32,
};
use console_core_byte_streams::{
  BitReaderLtr, BitReaderRtl, BitStreamLtr, BufReader, FiniteBitStream, FlagBit, ReadBitsLtr, ReadBitsRtl,
  ReadBytes,
};
use console_core_never::Never;

mod noise;

type Failure = Box<dyn std::error::Error>;

fn noise(count: u32) -> Result<Vec<u8>, Never> {
  noise::seeded::<0x2545_f491>(count)
}

fn ltr(bytes: &[u8]) -> Result<BitReaderLtr<'_>, Never> {
  BitReaderLtr::new(bytes)
}

fn rtl(bytes: &[u8]) -> Result<BitReaderRtl<'_>, Never> {
  BitReaderRtl::new(bytes)
}

fn slice(bytes: &[u8]) -> Result<BufReader<'_>, Never> {
  BufReader::new(bytes)
}

fn streamed<B: ReadBytes>(source: &mut B) -> Result<BitStreamLtr<'_, B>, Never> {
  BitStreamLtr::new(source)
}

fn note<Answer: Debug>(
  out: &mut String,
  read: &str,
  answer: std::io::Result<Answer>,
) -> Result<(), Failure> {
  let line = match answer {
    Ok(value) => format!("{read}: {value:?}"),
    Err(fault) => format!("{read}: {:?} {fault}", fault.kind()),
  };

  writeln!(out, "{line}")?;

  Ok(())
}

fn ltr_fields<B: ReadBitsLtr>(bits: &mut B) -> Result<String, Failure> {
  let mut out = String::new();

  for width in 0..=32 {
    note(
      &mut out,
      &format!("leq32 {width}"),
      bits.read_bits_leq32(width),
    )?;
  }

  for width in 0..=64 {
    note(
      &mut out,
      &format!("leq64 {width}"),
      bits.read_bits_leq64(width),
    )?;
  }

  for width in 1..=32 {
    note(
      &mut out,
      &format!("signed {width}"),
      bits.read_bits_leq32_signed(width),
    )?;
  }

  Ok(out)
}

fn ltr_flags<B: ReadBitsLtr>(bits: &mut B) -> Result<String, Failure> {
  let mut out = String::new();

  note(&mut out, "flag", bits.read_flag())?;
  note(&mut out, "flag", bits.read_flag())?;
  note(&mut out, "bit", bits.read_bit())?;
  note(&mut out, "bit", bits.read_bit())?;
  note(&mut out, "ignore 13", bits.ignore_bits(13))?;
  note(&mut out, "ignore 1", bits.ignore_bit())?;

  let Ok(()) = bits.realign();

  note(&mut out, "after realign", bits.read_bits_leq32(8))?;
  note(&mut out, "bit", bits.read_bit())?;

  let Ok(()) = bits.realign();

  note(&mut out, "after realign", bits.read_bits_leq32(3))?;
  note(&mut out, "zeros", bits.read_unary_zeros())?;
  note(&mut out, "zeros", bits.read_unary_zeros())?;
  note(&mut out, "ones", bits.read_unary_ones())?;
  note(&mut out, "ones", bits.read_unary_ones())?;
  note(&mut out, "ones to 3", bits.read_unary_ones_capped(3))?;
  note(&mut out, "ones to 3", bits.read_unary_ones_capped(3))?;
  note(&mut out, "ones to 40", bits.read_unary_ones_capped(40))?;
  note(&mut out, "ignore 200", bits.ignore_bits(200))?;
  note(&mut out, "leq32 17", bits.read_bits_leq32(17))?;

  Ok(out)
}

struct Reads<B> {
  field: fn(&mut B, u32) -> std::io::Result<u32>,
  wide: fn(&mut B, u32) -> std::io::Result<u64>,
  flag: fn(&mut B) -> std::io::Result<FlagBit>,
  skip: fn(&mut B, u32) -> std::io::Result<()>,
}

fn drained<B>(bits: &mut B, reads: &Reads<B>) -> Result<String, Failure> {
  let mut out = String::new();
  let mut read = 0_u32;

  for _ in 0..256 {
    match (reads.field)(bits, 29) {
      Ok(_field) => read = read.saturating_add(1),
      Err(fault) => {
        writeln!(out, "after {read}: {:?} {fault}", fault.kind())?;

        break;
      }
    }
  }

  note(&mut out, "flag", (reads.flag)(bits))?;
  note(&mut out, "ignore 0", (reads.skip)(bits, 0))?;
  note(&mut out, "ignore 1", (reads.skip)(bits, 1))?;
  note(&mut out, "leq32 0", (reads.field)(bits, 0))?;
  note(&mut out, "leq64 0", (reads.wide)(bits, 0))?;

  Ok(out)
}

fn ltr_drained<B: ReadBitsLtr>(bits: &mut B) -> Result<String, Failure> {
  let reads = Reads {
    field: B::read_bits_leq32,
    wide: B::read_bits_leq64,
    flag: B::read_flag,
    skip: B::ignore_bits,
  };
  let mut out = drained(bits, &reads)?;

  note(&mut out, "zeros", bits.read_unary_zeros())?;

  Ok(out)
}

const LTR_FIELDS: &str = include_str!("transcripts/left_to_right_fields.txt");
const LTR_FLAGS: &str = concat!(
  "flag: Zero\n",
  "flag: Zero\n",
  "bit: 1\n",
  "bit: 0\n",
  "ignore 13: ()\n",
  "ignore 1: ()\n",
  "after realign: 108\n",
  "bit: 1\n",
  "after realign: 7\n",
  "zeros: 0\n",
  "zeros: 0\n",
  "ones: 1\n",
  "ones: 1\n",
  "ones to 3: 1\n",
  "ones to 3: 1\n",
  "ones to 40: 1\n",
  "ignore 200: ()\n",
  "leq32 17: 38505\n",
);
const LTR_DRAINED: &str = concat!(
  "after 23: Other unexpected end of bitstream\n",
  "flag: Zero\n",
  "ignore 0: ()\n",
  "ignore 1: ()\n",
  "leq32 0: 0\n",
  "leq64 0: 0\n",
  "zeros: 2\n",
);
const STREAM_DRAINED: &str = concat!(
  "after 23: UnexpectedEof buffer underrun\n",
  "flag: Zero\n",
  "ignore 0: ()\n",
  "ignore 1: ()\n",
  "leq32 0: 0\n",
  "leq64 0: 0\n",
  "zeros: 2\n",
);

#[test]
fn both_left_to_right_readers_read_every_width_alike() -> Result<(), Failure> {
  let bytes = noise(512)?;
  let Ok(mut held) = ltr(&bytes);
  let Ok(mut source) = slice(&bytes);
  let Ok(mut streaming) = streamed(&mut source);

  let held_fields = ltr_fields(&mut held)?;
  let streamed_fields = ltr_fields(&mut streaming)?;
  let held_flags = ltr_flags(&mut held)?;
  let streamed_flags = ltr_flags(&mut streaming)?;
  let held_left = held.bits_left();
  let held_drained = ltr_drained(&mut held)?;
  let streamed_drained = ltr_drained(&mut streaming)?;

  assert_eq!(held_fields, LTR_FIELDS);
  assert_eq!(streamed_fields, LTR_FIELDS);
  assert_eq!(held_flags, LTR_FLAGS);
  assert_eq!(streamed_flags, LTR_FLAGS);
  assert_eq!(held_left, 688);
  assert_eq!(held_drained, LTR_DRAINED);
  assert_eq!(streamed_drained, STREAM_DRAINED);

  Ok(())
}

fn rtl_fields(bits: &mut BitReaderRtl<'_>) -> Result<String, Failure> {
  let mut out = String::new();

  for width in 0..=32 {
    note(
      &mut out,
      &format!("leq32 {width}"),
      bits.read_bits_leq32(width),
    )?;
  }

  for width in 0..=64 {
    note(
      &mut out,
      &format!("leq64 {width}"),
      bits.read_bits_leq64(width),
    )?;
  }

  note(&mut out, "flag", bits.read_flag())?;
  note(&mut out, "flag", bits.read_flag())?;
  note(&mut out, "ignore 13", bits.ignore_bits(13))?;
  note(&mut out, "leq32 5", bits.read_bits_leq32(5))?;
  note(&mut out, "ignore 200", bits.ignore_bits(200))?;
  note(&mut out, "leq32 17", bits.read_bits_leq32(17))?;
  writeln!(out, "left: {}", bits.bits_left())?;

  Ok(out)
}

fn rtl_drained(bits: &mut BitReaderRtl<'_>) -> Result<String, Failure> {
  let reads = Reads {
    field: <BitReaderRtl<'_> as ReadBitsRtl>::read_bits_leq32,
    wide: <BitReaderRtl<'_> as ReadBitsRtl>::read_bits_leq64,
    flag: <BitReaderRtl<'_> as ReadBitsRtl>::read_flag,
    skip: <BitReaderRtl<'_> as ReadBitsRtl>::ignore_bits,
  };
  let mut out = drained(bits, &reads)?;

  writeln!(out, "left: {}", bits.bits_left())?;

  Ok(out)
}

const RTL_FIELDS: &str = include_str!("transcripts/right_to_left_fields.txt");
const RTL_DRAINED: &str = concat!(
  "after 43: Other unexpected end of bitstream\n",
  "flag: Zero\n",
  "ignore 0: ()\n",
  "ignore 1: ()\n",
  "leq32 0: 0\n",
  "leq64 0: 0\n",
  "left: 2\n",
);

#[test]
fn the_right_to_left_reader_reads_every_width() -> Result<(), Failure> {
  let bytes = noise(512)?;
  let Ok(mut held) = rtl(&bytes);

  let left = held.bits_left();
  let fields = rtl_fields(&mut held)?;
  let drained = rtl_drained(&mut held)?;

  assert_eq!(left, 4096);
  assert_eq!(fields, RTL_FIELDS);
  assert_eq!(drained, RTL_DRAINED);

  Ok(())
}

const STEPS: [(u32, u32); 5] = [(1, 128), (3, 7), (1, 19), (5, 24), (4, 0)];

#[test]
fn a_left_to_right_reader_skips_and_reads_single_bits() -> Result<(), Failure> {
  let Ok(mut bits) = BitReaderLtr::new(&[
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xc0, 0x10, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0a,
  ]);
  let mut seen = Vec::new();

  for (read, skipped) in STEPS {
    for _ in 0..read {
      let bit = bits.read_flag()?;

      seen.push(bit);
    }

    bits.ignore_bits(skipped)?;
  }

  assert_eq!(
    seen,
    [
      FlagBit::One, FlagBit::One, FlagBit::Zero, FlagBit::Zero, FlagBit::One, FlagBit::One, FlagBit::Zero, FlagBit::Zero, FlagBit::Zero, FlagBit::Zero, FlagBit::One, FlagBit::Zero, FlagBit::One, FlagBit::Zero
    ]
  );

  let Ok(mut full) = BitReaderLtr::new(&[0xff; 32]);

  for width in [64, 64, 32, 32, 64] {
    full.ignore_bits(width)?;
  }

  assert_eq!(full.bits_left(), 0);

  Ok(())
}

#[test]
fn a_right_to_left_reader_skips_and_reads_single_bits() -> Result<(), Failure> {
  let Ok(mut bits) = BitReaderRtl::new(&[
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0x02, 0x08, 0x00, 0x80, 0x00, 0x00, 0x00, 0x50,
  ]);
  let mut seen = Vec::new();

  for (read, skipped) in STEPS {
    for _ in 0..read {
      let bit = bits.read_flag()?;

      seen.push(bit);
    }

    bits.ignore_bits(skipped)?;
  }

  assert_eq!(
    seen,
    [
      FlagBit::One, FlagBit::One, FlagBit::Zero, FlagBit::Zero, FlagBit::One, FlagBit::One, FlagBit::Zero, FlagBit::Zero, FlagBit::Zero, FlagBit::Zero, FlagBit::One, FlagBit::Zero, FlagBit::One, FlagBit::Zero
    ]
  );

  let Ok(mut full) = BitReaderRtl::new(&[0xff; 32]);

  for width in [64, 64, 32, 32, 64] {
    full.ignore_bits(width)?;
  }

  assert_eq!(full.bits_left(), 0);

  Ok(())
}

#[test]
fn both_orders_read_the_vectors_symphonia_wrote_for_them() -> Result<(), Failure> {
  let Ok(mut left) = BitReaderLtr::new(&[0b1010_0101, 0b0111_1110, 0b1101_0011]);
  let Ok(mut right) = BitReaderRtl::new(&[0b1010_0101, 0b0111_1110, 0b1101_0011]);
  let mut read_left = Vec::new();
  let mut read_right = Vec::new();

  for width in [4, 4, 13, 3] {
    let left_field = left.read_bits_leq32(width)?;
    let right_field = right.read_bits_leq32(width)?;

    read_left.push(left_field);
    read_right.push(right_field);
  }

  assert_eq!(read_left, [0b1010, 0b0101, 0b0_1111_1101_1010, 0b011]);
  assert_eq!(read_right, [0b0101, 0b1010, 0b1_0011_0111_1110, 0b110]);

  let long = [
    0x99, 0xaa, 0x55, 0xff, 0xff, 0x55, 0xaa, 0x99, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
    0x88, 0x00, 0x11, 0x22, 0x33, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
  ];
  let Ok(mut wide_left) = BitReaderLtr::new(&long);
  let Ok(mut wide_right) = BitReaderRtl::new(&long);
  let mut left_fields = Vec::new();
  let mut right_fields = Vec::new();

  for width in [40, 4, 4, 16, 64] {
    let field = wide_left.read_bits_leq64(width)?;

    left_fields.push(field);
  }

  for width in [40, 4, 4, 16, 64, 32, 64] {
    let field = wide_right.read_bits_leq64(width)?;

    right_fields.push(field);
  }

  assert_eq!(
    left_fields,
    [0x99_aa55_ffff, 0x05, 0x05, 0xaa99, 0x1122_3344_5566_7788]
  );
  assert_eq!(
    right_fields,
    [
      0xff_ff55_aa99,
      0x05,
      0x05,
      0x99aa,
      0x8877_6655_4433_2211,
      0x3322_1100,
      0x7766_5544_3322_1100
    ]
  );

  Ok(())
}

#[test]
fn a_left_to_right_reader_counts_runs_of_ones_and_zeros() -> Result<(), Failure> {
  let Ok(mut zeros) = BitReaderLtr::new(&[
    0b0000_0001,
    0b0001_0000,
    0b0000_0000,
    0b1000_0000,
    0b1111_1011,
  ]);
  let Ok(mut ones) = BitReaderLtr::new(&[
    0b1111_1110,
    0b1110_1111,
    0b1111_1111,
    0b0111_1111,
    0b0000_0100,
  ]);
  let Ok(mut capped) = BitReaderLtr::new(&[
    0b1111_1110,
    0b1110_1111,
    0b1111_1111,
    0b0111_1111,
    0b0000_0100,
  ]);
  let mut runs = Vec::new();

  for _ in 0..10 {
    let zero_run = zeros.read_unary_zeros()?;
    let one_run = ones.read_unary_ones()?;
    let capped_run = capped.read_unary_ones_capped(9)?;

    runs.push((zero_run, one_run, capped_run));
  }

  assert_eq!(
    runs,
    [
      (7, 7, 7),
      (3, 3, 3),
      (12, 12, 9),
      (7, 7, 3),
      (0, 0, 7),
      (0, 0, 0),
      (0, 0, 0),
      (0, 0, 0),
      (1, 1, 0),
      (0, 0, 1)
    ]
  );

  Ok(())
}

#[test]
fn left_to_right_unary_reads_distinguish_the_cap_from_end_of_input() -> Result<(), Failure> {
  let Ok(mut longest_zeros) = BitReaderLtr::new(&[0, 0, 0, 0, 0, 0, 0, 1]);
  let Ok(mut longest_ones) = BitReaderLtr::new(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe]);
  let Ok(mut endless_zeros) = BitReaderLtr::new(&[0; 8]);
  let Ok(mut endless_ones) = BitReaderLtr::new(&[0xff; 8]);
  let Ok(mut long_capped) = BitReaderLtr::new(&[0xff; 56]);
  let Ok(mut at_cap) = BitReaderLtr::new(&[0b1111_1110, 0b1111_1110]);

  let longest_zero_run = longest_zeros.read_unary_zeros()?;
  let longest_one_run = longest_ones.read_unary_ones()?;
  let first_capped = long_capped.read_unary_ones_capped(144)?;
  let second_capped = long_capped.read_unary_ones_capped(256)?;
  let under_cap = at_cap.read_unary_ones_capped(8)?;
  let at_the_cap = at_cap.read_unary_ones_capped(4)?;

  assert_eq!((longest_zero_run, longest_one_run), (63, 63));
  assert_eq!(
    endless_zeros
      .read_unary_zeros()
      .map_err(|fault| fault.kind()),
    Err(std::io::ErrorKind::Other)
  );
  assert_eq!(
    endless_ones.read_unary_ones().map_err(|fault| fault.kind()),
    Err(std::io::ErrorKind::Other)
  );
  assert_eq!((first_capped, second_capped), (144, 256));
  assert_eq!((under_cap, at_the_cap), (7, 4));

  Ok(())
}

const CODE_WORDS: [u32; 25] = [
  0b001, 0b111, 0b010, 0b1001, 0b1101, 0b0001, 0b0111, 0b1000, 0b10110, 0b10111, 0b10101,
  0b10100, 0b01100, 0b000010, 0b110000, 0b000011, 0b110001, 0b000001, 0b011010, 0b000000,
  0b011011, 0b1100111, 0b1100110, 0b1100101, 0b1100100,
];

const CODE_LENS: [u8; 25] = [
  3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7,
];

const VALUES: [u8; 25] = *b"i etslno.rghupw,fymvadbcT";

const SPOKEN: [u8; 57] = [
  0xc9, 0x43, 0xbf, 0x48, 0xa7, 0xca, 0xbe, 0x64, 0x30, 0xf5, 0xdf, 0x31, 0xd9, 0xb6, 0xb5, 0xbb,
  0x6f, 0x9f, 0xa0, 0x15, 0xc1, 0xfa, 0x5e, 0xa2, 0xb8, 0x4a, 0xfb, 0x0f, 0xe1, 0x93, 0xe6, 0x8a,
  0xe8, 0x3e, 0x77, 0xe0, 0xd9, 0x92, 0xf5, 0xf8, 0xc5, 0xfb, 0x37, 0xcc, 0x7c, 0x48, 0x8f, 0x33,
  0xf0, 0x33, 0x4f, 0xb0, 0xd2, 0x9a, 0x17, 0xad, 0x80,
];

const TEXT: &str = "This silence belongs to us... and every single person out there, is waiting for us to fill it with something.";

fn built<E: CodebookEntry>(
  order: BitOrder,
  per_read: u8,
  values: &[E::ValueType],
) -> std::io::Result<Codebook<E>> {
  let Ok(mut builder) = CodebookBuilder::new(order);
  let Ok(sized) = builder.bits_per_read(per_read);

  sized.make::<E>(&CODE_WORDS, &CODE_LENS, values)
}

fn spoken_left<E: CodebookEntry<ValueType: Debug>>(
  codebook: &Codebook<E>,
) -> Result<String, Failure> {
  let Ok(mut bits) = BitReaderLtr::new(&SPOKEN);
  let mut out = String::new();

  for _ in TEXT.chars() {
    let (value, consumed) = bits.read_codebook(codebook)?;

    write!(out, "{value:?}/{consumed} ")?;
  }

  note(&mut out, "past the end", bits.read_codebook(codebook))?;

  Ok(out)
}

fn spoken_right<E: CodebookEntry<ValueType: Debug>>(
  codebook: &Codebook<E>,
) -> Result<String, Failure> {
  let reversed: Vec<u8> = SPOKEN.iter().map(|byte| byte.reverse_bits()).collect();
  let Ok(mut bits) = BitReaderRtl::new(&reversed);
  let mut out = String::new();

  for _ in TEXT.chars() {
    let (value, consumed) = bits.read_codebook(codebook)?;

    write!(out, "{value:?}/{consumed} ")?;
  }

  note(&mut out, "past the end", bits.read_codebook(codebook))?;

  Ok(out)
}

const SPOKEN_TEXT: &str = include_str!("transcripts/spoken.txt");

#[test]
fn a_codebook_spells_the_sentence_in_either_order_at_any_block_width() -> Result<(), Failure> {
  let wide: Vec<u16> = VALUES.iter().map(|value| u16::from(*value)).collect();
  let widest: Vec<u32> = VALUES.iter().map(|value| u32::from(*value)).collect();

  for per_read in 1..=16 {
    let narrow_left = built::<Entry8x16>(BitOrder::Verbatim, per_read, &VALUES)?;
    let narrow_right = built::<Entry8x16>(BitOrder::Reverse, per_read, &VALUES)?;
    let wide_left = built::<Entry16x16>(BitOrder::Verbatim, per_read, &wide)?;
    let widest_right = built::<Entry32x32>(BitOrder::Reverse, per_read, &widest)?;
    let spelled = spoken_left(&narrow_left)?;

    let reversed = spoken_right(&narrow_right)?;
    let widened = spoken_left(&wide_left)?;
    let widest_spelled = spoken_right(&widest_right)?;

    assert_eq!(spelled, SPOKEN_TEXT, "{per_read} bits a read");
    assert_eq!(reversed, spelled, "{per_read} bits a read");
    assert_eq!(
      widened.replace(' ', ""),
      spelled.replace(' ', ""),
      "{per_read} bits a read"
    );
    assert_eq!(
      widest_spelled.replace(' ', ""),
      spelled.replace(' ', ""),
      "{per_read} bits a read"
    );
  }

  Ok(())
}

#[test]
fn a_codebook_refuses_words_that_do_not_make_one() -> Result<(), Failure> {
  let said = |made: std::io::Result<Codebook<Entry8x16>>| match made {
    Ok(codebook) => format!("made {}", codebook.table.len()),
    Err(fault) => format!("{:?} {fault}", fault.kind()),
  };
  let Ok(mut complete) = CodebookBuilder::new(BitOrder::Verbatim);
  let Ok(mut holes) = CodebookBuilder::new_sparse(BitOrder::Verbatim);
  let Ok(mut wide) = CodebookBuilder::new(BitOrder::Verbatim);
  let Ok(widest) = wide.bits_per_read(16);
  let incomplete = complete.make::<Entry8x16>(&[0b0, 0b10], &[1, 2], &[1, 2]);
  let empty_word = complete.make::<Entry8x16>(&[0b0, 0b1, 0b0], &[1, 1, 0], &[1, 2, 3]);
  let sparse = holes.make::<Entry8x16>(&[0b0, 0b10, 0], &[1, 2, 0], &[1, 2, 3]);
  let overflowing = widest.make::<Entry8x16>(
    &[0, 1, 0b1_1111_1111_1111_1110, 0b1_1111_1111_1111_1111],
    &[17, 17, 17, 17],
    &[1, 2, 3, 4],
  );
  let nothing = complete.make::<Entry8x16>(&[], &[], &[]);

  assert_eq!(said(incomplete), "Other core (io): codebook is incomplete");
  assert_eq!(said(empty_word), "Other core (io): zero length codeword");
  assert_eq!(said(overflowing), "Other core (io): codebook overflow");
  assert_eq!(said(nothing), "made 0");

  let holed = holes.make::<Entry8x16>(&[0b0, 0b10, 0], &[1, 2, 0], &[1, 2, 3])?;
  let Ok(mut bits) = BitReaderLtr::new(&[0b0101_1100, 0b1100_0000]);
  let mut out = String::new();

  for _ in 0..6 {
    note(&mut out, "word", bits.read_codebook(&holed))?;
  }

  assert_eq!(said(sparse), "made 5");
  assert_eq!(
    out,
    concat!(
      "word: (1, 1)\n",
      "word: (2, 2)\n",
      "word: (0, 0)\n",
      "word: (0, 0)\n",
      "word: (0, 0)\n",
      "word: (0, 0)\n",
    )
  );

  Ok(())
}
