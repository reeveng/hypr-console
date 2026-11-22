//! Every kind of Opus packet an encoder makes, decoded and held to what this crate answered
//! the day it was brought under the workspace's rules.
//!
//! The fixtures are packets cut out of streams ffmpeg's libopus wrote for a synthetic signal --
//! SILK, hybrid and CELT, every bandwidth, every frame duration, mono, stereo and six-channel
//! surround, streams that hand over between CELT and hybrid with the redundant frames an
//! encoder writes at the seam, and two whose header asks for a gain. Each file is the
//! OpusHead, then packets, each behind its length as a little-endian `u16`; an empty one is a
//! packet that was lost. The checksum covers every sample the decoder returned, bit for bit,
//! so a rewrite that changes one rounding is red.

use console_core_audio_decoding_opus::{Decoder, Error};
use console_core_checksums::crc32;
use console_core_number_conversion::{fitted, index};

enum Fault {
  RunsPastTheEnd,
  Headless,
  Refused(Error),
  Wrong(Vec<Mismatch>),
}

impl From<Error> for Fault {
  fn from(refused: Error) -> Fault {
    Fault::Refused(refused)
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Heard {
  checksum: u32,
  samples: u32,
}

struct Mismatch {
  name: &'static str,
  heard: Heard,
}

impl std::fmt::Debug for Fault {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Fault::RunsPastTheEnd => write!(to, "a packet runs past the end of its file"),
      Fault::Headless => write!(to, "a fixture has no header"),
      Fault::Refused(refused) => write!(to, "the decoder refused a fixture: {refused}"),
      Fault::Wrong(wrong) => {
        for one in wrong {
          writeln!(
            to,
            "{}: checksum 0x{:08x}, samples {}",
            one.name, one.heard.checksum, one.heard.samples
          )?;
        }

        Ok(())
      }
    }
  }
}

#[derive(Clone, Copy)]
enum LostAs {
  Nothing,
  EmptyPacket,
}

#[derive(Clone, Copy)]
enum Start {
  AtTheBeginning,
  AfterThePreSkip,
}

#[derive(Clone, Copy)]
struct Listening {
  lost: LostAs,
  start: Start,
}

fn packets(file: &[u8]) -> Result<Vec<&[u8]>, Fault> {
  let mut found = Vec::new();
  let mut rest = file;

  for _two_bytes_at_least_per_packet in file.chunks(2) {
    let (length, tail) = match rest.split_first_chunk::<2>() {
      Some(split) => split,
      None => return Ok(found),
    };
    let Ok(size) = index(u16::from_le_bytes(*length));

    match tail.split_at_checked(size) {
      Some((packet, after)) => {
        found.push(packet);
        rest = after;
      }
      None => return Err(Fault::RunsPastTheEnd),
    }
  }

  Ok(found)
}

fn heard(file: &[u8], listening: Listening) -> Result<Heard, Fault> {
  let all = packets(file)?;

  let (head, rest) = match all.split_first() {
    Some(split) => split,
    None => return Err(Fault::Headless),
  };
  let mut decoder = Decoder::new(head)?;

  match listening.start {
    Start::AtTheBeginning => {}
    Start::AfterThePreSkip => {
      let Ok(()) = decoder.trim_pre_skip();
    }
  }

  let mut bytes = Vec::new();
  let mut samples = 0u32;

  for packet in rest {
    let asked = match (*packet, listening.lost) {
      ([], LostAs::Nothing) => None,
      ([], LostAs::EmptyPacket) | ([_, ..], LostAs::Nothing | LostAs::EmptyPacket) => {
        Some(*packet)
      }
    };
    let planar = decoder.decode(asked)?;

    for channel in &planar {
      let Ok(length) = fitted::<_, u32>(channel.len());

      samples = samples.saturating_add(length);

      for sample in channel {
        bytes.extend_from_slice(&sample.to_bits().to_le_bytes());
      }
    }
  }

  let Ok(checksum) = crc32::of(&bytes);

  Ok(Heard { checksum, samples })
}

struct Case {
  name: &'static str,
  file: &'static [u8],
  heard: Heard,
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "a fixture's name is the name of its file, and one definition keeps the two from being spelled apart"
  )
)]
macro_rules! fixture {
  ($name:literal, $checksum:literal, $samples:literal) => {
    Case {
      name: $name,
      file: include_bytes!(concat!("fixtures/", $name, ".packets")),
      heard: Heard {
        checksum: $checksum,
        samples: $samples,
      },
    }
  };
}

const CASES: &[Case] = &[
  fixture!("a-mono-listener", 0x6fdc229c, 34560),
  fixture!("celt-full-mono-short", 0xcc05c230, 24120),
  fixture!("celt-full-mono-ten", 0x8d053a29, 38880),
  fixture!("celt-full-mono-ten-quieter", 0x2eacfb67, 38880),
  fixture!("celt-full-stereo-five", 0x8c3ae8d7, 58080),
  fixture!("celt-full-stereo-forty", 0x6246e210, 80640),
  fixture!("celt-full-stereo", 0x5d8acc8e, 117120),
  fixture!("celt-full-stereo-sixty-fixed-rate", 0x1a0374c1, 80640),
  fixture!("celt-full-stereo-thin", 0x7d37ee3c, 117120),
  fixture!("celt-handing-over-to-hybrid-stereo", 0x8e4b3644, 385920),
  fixture!("celt-narrow-stereo", 0x6d6683d1, 78720),
  fixture!("celt-superwide-stereo-fixed-rate", 0xc80de55e, 78720),
  fixture!("celt-wide-mono", 0x0eceb454, 39360),
  fixture!("hybrid-and-celt-handing-over", 0x0e0b01f7, 192960),
  fixture!("hybrid-and-celt-handing-over-lost", 0x47441968, 192960),
  fixture!("hybrid-full-mono-ten-recovering", 0xee887828, 58080),
  fixture!("hybrid-full-stereo", 0xa8f7b747, 117120),
  fixture!("hybrid-full-stereo-louder", 0xb13048d7, 117120),
  fixture!("hybrid-superwide-mono", 0x10cbeaa6, 58560),
  fixture!("hybrid-superwide-stereo-forty", 0x21c0d083, 80640),
  fixture!("mono-and-stereo-taking-turns", 0x66388b74, 214320),
  fixture!("silk-medium-mono-forty", 0x1b5ac927, 59520),
  fixture!("silk-medium-stereo", 0x6102ccfb, 78720),
  fixture!("silk-narrow-mono", 0x80125c18, 58560),
  fixture!("silk-narrow-stereo-sixty", 0x0493d0a6, 92160),
  fixture!("silk-wide-mono-sixty-recovering", 0x6f448058, 60480),
  fixture!("silk-wide-mono-ten", 0xd1deff57, 58080),
  fixture!("silk-wide-stereo", 0xb91312fb, 117120),
  fixture!("silk-wide-stereo-alike-recovering", 0xa670664f, 291840),
  fixture!("silk-wide-stereo-quiet-spells", 0xf48a201b, 232320),
  fixture!("stereo-changing-its-mind", 0xe55e996a, 65280),
  fixture!("surround", 0x344517cb, 236160),
];

const TRIMMED: &[Case] = &[
  fixture!("hybrid-full-stereo", 0x8200c6e4, 116496),
  fixture!("silk-narrow-mono", 0x3fdf23e8, 58248),
  fixture!("surround", 0x58c8de25, 234288),
];

fn mismatches(cases: &[Case], listening: Listening) -> Result<Vec<Mismatch>, Fault> {
  let mut wrong = Vec::new();

  for case in cases {
    let said = heard(case.file, listening)?;

    match (
      said.checksum.cmp(&case.heard.checksum),
      said.samples.cmp(&case.heard.samples),
    ) {
      (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => {}
      _mismatch => wrong.push(Mismatch {
        name: case.name,
        heard: said,
      }),
    }
  }

  Ok(wrong)
}

fn none_of(wrong: Vec<Mismatch>) -> Result<(), Fault> {
  match wrong.as_slice() {
    [] => Ok(()),
    [_, ..] => Err(Fault::Wrong(wrong)),
  }
}

#[test]
fn every_kind_of_stream_decodes_to_what_it_always_has() -> Result<(), Fault> {
  let wrong = mismatches(
    CASES,
    Listening {
      lost: LostAs::Nothing,
      start: Start::AtTheBeginning,
    },
  )?;

  none_of(wrong)
}

#[test]
fn a_stream_heard_from_its_start_drops_what_its_header_says_to() -> Result<(), Fault> {
  let wrong = mismatches(
    TRIMMED,
    Listening {
      lost: LostAs::Nothing,
      start: Start::AfterThePreSkip,
    },
  )?;

  none_of(wrong)
}

#[test]
fn a_lost_packet_is_concealed_the_same_whether_it_is_none_or_empty() -> Result<(), Fault> {
  let wrong = mismatches(
    CASES,
    Listening {
      lost: LostAs::EmptyPacket,
      start: Start::AtTheBeginning,
    },
  )?;

  none_of(wrong)
}

#[test]
fn a_stereo_packet_at_the_duration_limit_keeps_every_sample() -> Result<(), Fault> {
  const HEAD: &[u8] = b"OpusHead\x01\x02\x00\x00\x80\xbb\x00\x00\x00\x00\x00";
  const COUNTED_SILK_STEREO_60_MS: u8 = 31;
  let mut decoder = Decoder::new(HEAD)?;
  let output = decoder.decode(Some(&[COUNTED_SILK_STEREO_60_MS, 2]))?;

  assert_eq!(output.len(), 2);
  assert!(output.iter().all(|lane| lane.len() == 5760));
  assert!(matches!(
    decoder.decode(Some(&[COUNTED_SILK_STEREO_60_MS, 3])),
    Err(Error::MalformedFraming)
  ));

  Ok(())
}

#[test]
fn a_header_that_is_not_one_is_refused() {
  assert!(matches!(
    Decoder::new(b"OpusTags"),
    Err(Error::MissingMagic)
  ));
  assert!(matches!(Decoder::new(&[]), Err(Error::MissingMagic)));
}
